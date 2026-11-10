//! Favorite products — the per-user product catalog (MB-0002) — and, below
//! them, the products created from recipe ingredients (MB-0014).
//!
//! Thin handlers: parse → call `ProductRepo` → render. htmx requests get the
//! re-rendered catalog fragment; plain requests get a 303 back to the page
//! (PRG), or the full page when the input was rejected. All endpoints require
//! an authenticated user.
//!
//! RR-0001: a favorite must have a category. The column is nullable (deleting
//! a category un-categorizes its products), so the rule lives here, in
//! `parse_category`; the repo additionally refuses a category the user doesn't
//! own.
//!
//! The rename, category, and delete handlers work on any product the user
//! owns, favorite or recipe-created; `make_favorite` promotes a recipe-created
//! one in place.
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

#[derive(Debug, Deserialize)]
struct NameForm {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CategoryForm {
    #[serde(default)]
    category_id: String,
}

#[derive(Debug, Deserialize)]
struct MoveForm {
    dir: Direction,
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
    /// Recipe-created products (MB-0014), shown below the favorites.
    has_hidden: bool,
    hidden: Vec<HiddenGroupView>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddView {
    name: String,
    picker: Picker,
}

#[derive(Debug, Serialize)]
struct GroupView {
    /// `None` for the uncategorized group.
    name: Option<String>,
    products: Vec<ProductView>,
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
    /// delete confirmation names it (MB-0014 P5).
    fn new(categories: &[Category], product: Product, recipe_count: i64) -> Self {
        Self {
            picker: Picker::new(categories, product.category_id),
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
/// instead. Always required. Also used by the recipe editor's category
/// prompt (MB-0039).
#[derive(Debug, Serialize)]
pub struct Picker {
    options: Vec<PickerOption>,
    has_selection: bool,
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
        }
    }
}

impl CatalogView {
    fn new(
        categories: &[Category],
        groups: Vec<ProductGroup>,
        hidden: Vec<HiddenProductGroup>,
        draft: Draft,
        error: Option<String>,
    ) -> Self {
        // Favorites have no usage count here; their delete confirmation stays
        // as it was (MB-0002).
        let groups = groups
            .into_iter()
            .map(|group| GroupView {
                name: group.category.map(|c| c.name),
                products: group
                    .products
                    .into_iter()
                    .map(|p| ProductView::new(categories, p, 0))
                    .collect(),
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
            has_hidden: !hidden.is_empty(),
            hidden,
            error,
        }
    }
}

/// Render the catalog as the full page or, for htmx, the `#favorites` fragment.
async fn render(
    state: &AppState,
    ui: &Ui,
    user: Uuid,
    fragment: bool,
    status: StatusCode,
    draft: Draft,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let categories = CategoryRepo::list_for_owner(&state.pool, user).await?;
    let groups = ProductRepo::list_for_owner(&state.pool, user).await?;
    let hidden = ProductRepo::list_hidden_with_usage(&state.pool, user).await?;
    let view = CatalogView::new(&categories, groups, hidden, draft, error);
    let template = if fragment {
        "products/_catalog"
    } else {
        "products/index"
    };
    let body = ui.render(state, template, &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}

/// Answer a mutation. Rejected input (`Invalid` / `Conflict`) is shown to the
/// user: in the fragment for htmx — with 200, since htmx doesn't swap 4xx
/// responses by default — or on the full page with the 4xx status. Success is
/// the fragment for htmx, else a 303 back to the page.
async fn respond<T>(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    user: Uuid,
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

    if is_hx(req) {
        render(state, ui, user, true, StatusCode::OK, draft, error).await
    } else if error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, "/favorites"))
            .finish())
    } else {
        render(state, ui, user, false, status, draft, error).await
    }
}

#[get("")]
pub async fn index(state: web::Data<AppState>, ui: Ui, id: Identity) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    render(
        &state,
        &ui,
        user,
        false,
        StatusCode::OK,
        Draft::default(),
        None,
    )
    .await
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
    respond(&state, &ui, &req, user, outcome, draft).await
}

#[post("/{id}/rename")]
pub async fn rename(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ProductRepo::rename(&state.pool, user, path.into_inner(), &form.name).await;
    respond(&state, &ui, &req, user, outcome, Draft::default()).await
}

#[post("/{id}/category")]
pub async fn recategorize(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<CategoryForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = match parse_category(&form.category_id) {
        Ok(category) => {
            ProductRepo::recategorize(&state.pool, user, path.into_inner(), category).await
        }
        Err(e) => Err(e),
    };
    respond(&state, &ui, &req, user, outcome, Draft::default()).await
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
    let outcome =
        ProductRepo::move_within_category(&state.pool, user, path.into_inner(), form.dir).await;
    respond(&state, &ui, &req, user, outcome, Draft::default()).await
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
    let outcome = ProductRepo::make_favorite(&state.pool, user, path.into_inner()).await;
    respond(&state, &ui, &req, user, outcome, Draft::default()).await
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
    let outcome = ProductRepo::delete(&state.pool, user, path.into_inner()).await;
    respond(&state, &ui, &req, user, outcome, Draft::default()).await
}
