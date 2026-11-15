//! Favorite products — the per-user product catalog (MB-0002) — and, below
//! them, the products created from recipe ingredients (MB-0014).
//!
//! Thin handlers: parse → call `ProductRepo` → render. All endpoints require
//! an authenticated user.
//!
//! The catalog is a set of collapsible units (MB-0059, RR-0044): a
//! `<details>` per favorites group, closed on a full page load, and one for
//! the "From recipes" section. An action answers per [`Swap`]: only the unit
//! it touched, rendered open, when its effect stays inside that unit, so the
//! groups the user opened keep their state; otherwise the whole catalog with
//! the touched units open (`HX-Retarget: #favorites`). Without htmx, success
//! is a 303 back to the page with the touched units named in `?open=` (PRG),
//! and rejected input the full page with them open.
//!
//! RR-0001 constraint 3, as amended by RR-0046: a favorite needs a category
//! when it is created or made a favorite (`parse_category`); its owner may
//! clear it later with the edit form's "No category" (RR-0043,
//! `parse_category_choice`). The column is nullable (deleting a category
//! un-categorizes its products), so the rule lives here; the repo additionally
//! refuses a category the user doesn't own.
//!
//! The save (name and category together, MB-0058) and delete handlers work on
//! any product the user owns, favorite or recipe-created; `make_favorite`
//! promotes a recipe-created one in place.
//!
//! Tests are in `tests/<area>.rs` (MB-0019, RR-0009).

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use kauppa_core::{Entity, Invalid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Category, Direction, HiddenProductGroup, Product, ProductGroup};
use kauppa_core::repo::{CategoryRepo, ProductRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::i18n::{Ui, VARY};
use crate::routes::request::{current_user, is_hx};
use crate::state::AppState;

#[cfg(test)]
mod tests;

#[derive(Debug, Deserialize)]
struct CreateForm {
    name: String,
    // A missing field and an empty one both mean "no category chosen", which
    // `parse_category` turns into a user-facing message rather than a 400
    // from the extractor.
    #[serde(default)]
    category_id: String,
}

/// The edit form (MB-0058): the name and the category choice, saved together.
/// A missing `category_id` is rejected by `parse_category_choice`, so a form
/// without the picker can't clear a category (RR-0043 constraint 2).
#[derive(Debug, Deserialize)]
struct SaveForm {
    name: String,
    category_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MoveForm {
    dir: Direction,
}

/// `GET /favorites?open=<unit>,<unit>`: the units to render open, as the
/// redirect after a plain (non-htmx) action names them (RR-0044). Unknown
/// names are ignored.
#[derive(Debug, Deserialize)]
struct PageQuery {
    open: Option<String>,
}

/// A part of the catalog that can be swapped on its own (RR-0044): a
/// favorites group, by category (`None` is Uncategorized), or the "From
/// recipes" section. Its DOM id is stable: a renamed category keeps its id
/// (constraint 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Group(Option<Uuid>),
    Recipes,
}

const UNCATEGORIZED_ID: &str = "group-uncategorized";
const RECIPES_ID: &str = "recipe-products";

impl Unit {
    fn dom_id(self) -> String {
        match self {
            Self::Group(Some(category)) => format!("group-{category}"),
            Self::Group(None) => UNCATEGORIZED_ID.to_owned(),
            Self::Recipes => RECIPES_ID.to_owned(),
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            UNCATEGORIZED_ID => Some(Self::Group(None)),
            RECIPES_ID => Some(Self::Recipes),
            _ => raw
                .strip_prefix("group-")
                .and_then(|id| Uuid::parse_str(id).ok())
                .map(|id| Self::Group(Some(id))),
        }
    }

    /// The `?open=` list for `units`.
    fn query(units: &[Self]) -> String {
        units
            .iter()
            .map(|u| u.dom_id())
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// What an action touched, for choosing its [`Swap`].
#[derive(Debug, Clone, Copy)]
enum Touch {
    /// Adding a favorite (always a cross-group action, RR-0044 item 3): the
    /// chosen category, if one parsed.
    Add(Option<Uuid>),
    /// An action on an existing product: its id and where it was before.
    Product { id: Uuid, before: Option<Unit> },
}

/// How an action is answered (RR-0044).
#[derive(Debug, PartialEq, Eq)]
enum Swap {
    /// Only this unit, rendered open.
    Unit(Unit),
    /// The whole catalog, with these units open.
    Catalog(Vec<Unit>),
}

/// The user's catalog as the page shows it.
struct Catalog {
    categories: Vec<Category>,
    groups: Vec<ProductGroup>,
    hidden: Vec<HiddenProductGroup>,
}

impl Catalog {
    async fn load(state: &AppState, user: Uuid) -> CoreResult<Self> {
        Ok(Self {
            categories: CategoryRepo::list_for_owner(&state.pool, user).await?,
            groups: ProductRepo::list_for_owner(&state.pool, user).await?,
            hidden: ProductRepo::list_hidden_with_usage(&state.pool, user).await?,
        })
    }

    /// The unit that shows product `id`, if it is the user's.
    fn locate(&self, id: Uuid) -> Option<Unit> {
        if let Some(group) = self
            .groups
            .iter()
            .find(|g| g.products.iter().any(|p| p.id == id))
        {
            return Some(Unit::Group(group.category.as_ref().map(|c| c.id)));
        }
        self.hidden
            .iter()
            .any(|g| g.products.iter().any(|u| u.product.id == id))
            .then_some(Unit::Recipes)
    }

    fn has(&self, unit: Unit) -> bool {
        match unit {
            Unit::Group(category) => self
                .groups
                .iter()
                .any(|g| g.category.as_ref().map(|c| c.id) == category),
            Unit::Recipes => !self.hidden.is_empty(),
        }
    }

    /// Where product `id` is now, before an action on it.
    async fn locate_now(state: &AppState, user: Uuid, id: Uuid) -> CoreResult<Option<Unit>> {
        Ok(Self::load(state, user).await?.locate(id))
    }
}

/// RR-0044 item 1: an action whose effect stays inside one unit (the product
/// is still there, or was deleted and the unit still has products) swaps that
/// unit; anything else — a move between units, an add, a unit appearing or
/// disappearing, an error — swaps the catalog with the touched units open.
fn choose_swap(after: &Catalog, touch: Touch, failed: bool) -> Swap {
    match touch {
        Touch::Add(category) => {
            Swap::Catalog(category.map(|c| Unit::Group(Some(c))).into_iter().collect())
        }
        Touch::Product { before, .. } if failed => Swap::Catalog(before.into_iter().collect()),
        Touch::Product { id, before } => match (before, after.locate(id)) {
            (Some(b), Some(a)) if a == b => Swap::Unit(b),
            (Some(b), None) if after.has(b) => Swap::Unit(b),
            (b, a) => {
                let mut open: Vec<Unit> = b.into_iter().collect();
                open.extend(a.filter(|a| Some(*a) != b));
                Swap::Catalog(open)
            }
        },
    }
}

/// Enforce "a favorite must have a category" (RR-0001) on the submitted id.
/// Whether the category is the user's is checked by the repo.
pub fn parse_category(raw: &str) -> CoreResult<Uuid> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CoreError::Invalid(Invalid::ChooseCategory));
    }
    Uuid::parse_str(raw).map_err(|_| CoreError::Invalid(Invalid::Unknown(Entity::Category)))
}

/// The edit picker's "No category" value (RR-0043): a fixed token, never an
/// empty or missing field.
pub const NO_CATEGORY: &str = "none";

/// The edit form's category choice (RR-0043): a category id, or
/// [`NO_CATEGORY`] to clear it. Missing or empty is rejected, so only an
/// explicit choice clears a category (constraint 2).
fn parse_category_choice(raw: Option<&str>) -> CoreResult<Option<Uuid>> {
    match raw.map(str::trim) {
        None | Some("") => Err(CoreError::Invalid(Invalid::ChooseCategory)),
        Some(NO_CATEGORY) => Ok(None),
        Some(id) => parse_category(id).map(Some),
    }
}

/// What the "add" form shows after a response: the category is kept after an
/// add (quick entry of several products), the name only after a rejected one.
#[derive(Debug, Default)]
struct Draft {
    name: String,
    category_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct CatalogView {
    has_categories: bool,
    add: AddView,
    groups: Vec<GroupView>,
    /// Recipe-created products (MB-0014), shown below the favorites; `None`
    /// when there are none.
    recipes: Option<RecipesView>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddView {
    name: String,
    picker: Picker,
}

/// A favorites group (`products/_group`): a `<details>`, open only when an
/// action touched it (RR-0044).
#[derive(Debug, Serialize)]
struct GroupView {
    dom_id: String,
    open: bool,
    /// `None` for the uncategorized group.
    name: Option<String>,
    count: usize,
    products: Vec<ProductView>,
}

/// The "From recipes" section (`products/_recipe_products`): one
/// `<details>`, its groups shown inside as headings (RR-0044 item 2).
#[derive(Debug, Serialize)]
struct RecipesView {
    dom_id: &'static str,
    open: bool,
    count: usize,
    groups: Vec<HiddenGroupView>,
}

/// A product row's edit controls (`products/_product_edit`).
#[derive(Debug, Serialize)]
struct ProductView {
    id: Uuid,
    name: String,
    picker: Picker,
    /// How many of the user's recipes use the product: the delete
    /// confirmation and warning name it (MB-0014 P5), in the template.
    recipe_count: i64,
}

impl ProductView {
    /// `recipe_count` is how many of the user's recipes use the product; the
    /// delete confirmation names it (MB-0014 P5). The picker offers "No
    /// category" and opens on the current choice (RR-0043).
    fn new(categories: &[Category], product: Product, recipe_count: i64) -> Self {
        Self {
            picker: Picker::with_no_category(categories, product.category_id),
            id: product.id,
            name: product.name,
            recipe_count,
        }
    }
}

#[derive(Debug, Serialize)]
struct HiddenGroupView {
    /// `None` for the uncategorized group.
    name: Option<String>,
    products: Vec<HiddenProductView>,
}

/// A recipe-created product: the shared edit controls plus its usage, and
/// whether it can be made a favorite (only with a category, RR-0001).
#[derive(Debug, Serialize)]
struct HiddenProductView {
    #[serde(flatten)]
    product: ProductView,
    can_favorite: bool,
}

/// A category `<select>` (`products/_category_picker`); `has_selection` is
/// false when the placeholder ("Choose a category") should be selected
/// instead. Required, except in a list item's edit section, where no
/// choice leaves the category as it is (MB-0052, RR-0033). Also used by the
/// recipe editor's category prompt (MB-0039).
#[derive(Debug, Serialize)]
pub struct Picker {
    options: Vec<PickerOption>,
    has_selection: bool,
    required: bool,
    /// Offer "No category" ([`NO_CATEGORY`]) instead of the placeholder; only
    /// a favorite's edit form does (RR-0043).
    no_category: bool,
}

#[derive(Debug, Serialize)]
struct PickerOption {
    id: Uuid,
    name: String,
    selected: bool,
}

impl Picker {
    /// A required picker over `categories`, with `selected` chosen.
    pub fn new(categories: &[Category], selected: Option<Uuid>) -> Self {
        let options: Vec<PickerOption> = categories
            .iter()
            .map(|c| PickerOption {
                id: c.id,
                name: c.name.clone(),
                selected: Some(c.id) == selected,
            })
            .collect();
        let has_selection = options.iter().any(|o| o.selected);
        Self {
            options,
            has_selection,
            required: true,
            no_category: false,
        }
    }

    /// A favorite's edit picker (RR-0043): the categories and "No category",
    /// selected when `selected` is `None` (or not one of `categories`), so a
    /// choice is always sent.
    pub fn with_no_category(categories: &[Category], selected: Option<Uuid>) -> Self {
        Self {
            no_category: true,
            ..Self::new(categories, selected)
        }
    }

    /// The same picker, not required: submitting it with the placeholder
    /// sends no `category_id` (MB-0052).
    pub fn optional(categories: &[Category], selected: Option<Uuid>) -> Self {
        Self {
            required: false,
            ..Self::new(categories, selected)
        }
    }
}

impl CatalogView {
    /// The catalog with the units in `open` rendered open.
    fn new(catalog: Catalog, open: &[Unit], draft: Draft, error: Option<String>) -> Self {
        let Catalog {
            categories,
            groups,
            hidden,
        } = catalog;
        let categories = categories.as_slice();
        // Favorites have no usage count here; their delete confirmation stays
        // as it was (MB-0002).
        let groups = groups
            .into_iter()
            .map(|group| {
                let unit = Unit::Group(group.category.as_ref().map(|c| c.id));
                GroupView {
                    dom_id: unit.dom_id(),
                    open: open.contains(&unit),
                    name: group.category.map(|c| c.name),
                    count: group.products.len(),
                    products: group
                        .products
                        .into_iter()
                        .map(|p| ProductView::new(categories, p, 0))
                        .collect(),
                }
            })
            .collect();
        let hidden: Vec<HiddenGroupView> = hidden
            .into_iter()
            .map(|group| {
                let can_favorite = group.category.is_some();
                HiddenGroupView {
                    name: group.category.map(|c| c.name),
                    products: group
                        .products
                        .into_iter()
                        .map(|u| HiddenProductView {
                            can_favorite,
                            product: ProductView::new(categories, u.product, u.recipe_count),
                        })
                        .collect(),
                }
            })
            .collect();
        Self {
            has_categories: !categories.is_empty(),
            add: AddView {
                name: draft.name,
                picker: Picker::new(categories, draft.category_id),
            },
            groups,
            recipes: (!hidden.is_empty()).then(|| RecipesView {
                dom_id: RECIPES_ID,
                open: open.contains(&Unit::Recipes),
                count: hidden.iter().map(|g| g.products.len()).sum(),
                groups: hidden,
            }),
            error,
        }
    }
}

fn html(status: StatusCode, body: String) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("text/html")
        .insert_header(VARY)
        .body(body)
}

/// Render the catalog with `open` units open, as the full page or, for htmx,
/// the `#favorites` fragment, retargeted there (an action inside a unit
/// targets the unit, RR-0044).
#[allow(clippy::too_many_arguments)]
fn render_catalog(
    state: &AppState,
    ui: &Ui,
    catalog: Catalog,
    open: &[Unit],
    fragment: bool,
    status: StatusCode,
    draft: Draft,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let view = CatalogView::new(catalog, open, draft, error);
    if fragment {
        let body = ui.render(state, "products/_catalog", &view)?;
        let mut resp = html(status, body);
        resp.headers_mut().insert(
            header::HeaderName::from_static("hx-retarget"),
            header::HeaderValue::from_static("#favorites"),
        );
        Ok(resp)
    } else {
        Ok(html(status, ui.render(state, "products/index", &view)?))
    }
}

/// Render one unit, open, as the answer to an action that stayed inside it.
fn render_unit(state: &AppState, ui: &Ui, catalog: Catalog, unit: Unit) -> WebResult<HttpResponse> {
    let view = CatalogView::new(catalog, &[unit], Draft::default(), None);
    let body = match unit {
        Unit::Recipes => match &view.recipes {
            Some(recipes) => ui.render(state, "products/_recipe_products", recipes)?,
            None => ui.render(state, "products/_catalog", &view)?,
        },
        Unit::Group(_) => {
            let id = unit.dom_id();
            match view.groups.iter().find(|g| g.dom_id == id) {
                Some(group) => ui.render(state, "products/_group", group)?,
                None => ui.render(state, "products/_catalog", &view)?,
            }
        }
    };
    Ok(html(StatusCode::OK, body))
}

/// Answer a mutation. Rejected input (`Invalid` / `Conflict`) is shown to the
/// user: over htmx in the catalog fragment — with 200, since htmx doesn't swap
/// 4xx responses by default — or on the full page with the 4xx status; the
/// touched unit is open either way. Success is answered per [`Swap`] over
/// htmx, else with a 303 back to the page naming the units to open.
async fn respond<T>(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    user: Uuid,
    touch: Touch,
    outcome: CoreResult<T>,
    draft: Draft,
) -> WebResult<HttpResponse> {
    let (status, error) = match outcome {
        Ok(_) => (StatusCode::OK, None),
        // Logged in English, shown in the request's language (MB-0035).
        Err(CoreError::Invalid(m)) => {
            tracing::debug!(user_id = %user, reason = %m, "favorites input rejected");
            (StatusCode::BAD_REQUEST, Some(ui.invalid(&m)?))
        }
        Err(CoreError::Conflict(c)) => {
            tracing::debug!(user_id = %user, reason = %c, "favorites input rejected");
            (StatusCode::CONFLICT, Some(ui.conflict(&c)?))
        }
        Err(e) => return Err(e.into()),
    };

    let catalog = Catalog::load(state, user).await?;
    let swap = choose_swap(&catalog, touch, error.is_some());
    let open = match &swap {
        Swap::Unit(unit) => vec![*unit],
        Swap::Catalog(open) => open.clone(),
    };

    if is_hx(req) {
        match swap {
            Swap::Unit(unit) => render_unit(state, ui, catalog, unit),
            Swap::Catalog(open) => render_catalog(
                state,
                ui,
                catalog,
                &open,
                true,
                StatusCode::OK,
                draft,
                error,
            ),
        }
    } else if error.is_none() {
        let open: Vec<Unit> = open.into_iter().filter(|u| catalog.has(*u)).collect();
        let location = if open.is_empty() {
            "/favorites".to_owned()
        } else {
            format!("/favorites?open={}", Unit::query(&open))
        };
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, location))
            .finish())
    } else {
        render_catalog(state, ui, catalog, &open, false, status, draft, error)
    }
}

#[get("")]
pub async fn index(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    query: web::Query<PageQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let open: Vec<Unit> = query
        .open
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .filter_map(Unit::parse)
        .collect();
    let catalog = Catalog::load(&state, user).await?;
    render_catalog(
        &state,
        &ui,
        catalog,
        &open,
        false,
        StatusCode::OK,
        Draft::default(),
        None,
    )
}

#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    form: web::Form<CreateForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let category = parse_category(&form.category_id);
    let category_id = category.as_ref().ok().copied();
    let outcome = match category {
        Ok(category) => ProductRepo::create(&state.pool, user, &form.name, category).await,
        Err(e) => Err(e),
    };
    let draft = Draft {
        name: if outcome.is_ok() {
            String::new()
        } else {
            form.name.clone()
        },
        category_id,
    };
    respond(
        &state,
        &ui,
        &req,
        user,
        Touch::Add(category_id),
        outcome,
        draft,
    )
    .await
}

/// Save a product's name and category together (MB-0058, RR-0043): one
/// transaction, so a rejected part changes neither.
#[post("/{id}")]
pub async fn save(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<SaveForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let product = path.into_inner();
    let before = Catalog::locate_now(&state, user, product).await?;
    let outcome = match parse_category_choice(form.category_id.as_deref()) {
        Ok(category) => ProductRepo::update(&state.pool, user, product, &form.name, category).await,
        Err(e) => Err(e),
    };
    let touch = Touch::Product {
        id: product,
        before,
    };
    respond(&state, &ui, &req, user, touch, outcome, Draft::default()).await
}

/// Move a product one step up or down within its category.
#[post("/{id}/move")]
pub async fn move_product(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<MoveForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let product = path.into_inner();
    let before = Catalog::locate_now(&state, user, product).await?;
    let outcome = ProductRepo::move_within_category(&state.pool, user, product, form.dir).await;
    let touch = Touch::Product {
        id: product,
        before,
    };
    respond(&state, &ui, &req, user, touch, outcome, Draft::default()).await
}

/// Make a recipe-created product a favorite, in its own category (MB-0014).
#[post("/{id}/favorite")]
pub async fn make_favorite(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let product = path.into_inner();
    let before = Catalog::locate_now(&state, user, product).await?;
    let outcome = ProductRepo::make_favorite(&state.pool, user, product).await;
    let touch = Touch::Product {
        id: product,
        before,
    };
    respond(&state, &ui, &req, user, touch, outcome, Draft::default()).await
}

#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let product = path.into_inner();
    let before = Catalog::locate_now(&state, user, product).await?;
    let outcome = ProductRepo::delete(&state.pool, user, product).await;
    let touch = Touch::Product {
        id: product,
        before,
    };
    respond(&state, &ui, &req, user, touch, outcome, Draft::default()).await
}
