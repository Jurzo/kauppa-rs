//! Sharing favorites and recipes for import (MB-0061, RR-0048).
//!
//! `/sharing` is the user's own page: the share form (an email and the
//! favorites and recipes switches), the people they share with (change the
//! switches, revoke), and who shares with them. The share form answers with
//! the same notice whether or not the email matched (RR-0002 constraint 3).
//!
//! `/sharing/from/{owner}/favorites` lists `owner`'s favorites if they share
//! them with the viewer, else 404; every read goes through
//! `CatalogShareRepo` and its predicate (RR-0048 constraint 1). Importing
//! ticked favorites, or all, goes through MB-0060's plan and review
//! (`routes::review`); the chosen ids ride along in the review form.
//!
//! `/sharing/from/{owner}/recipes` lists `owner`'s recipes if they share
//! them, and `…/recipes/{recipe}` shows one read-only with "Import". The
//! import copies it with `imported_from` (a repeat is refused with a link
//! to the copy) and links its ingredients to the importer's products,
//! asking only for missing categories through the same review (RR-0048
//! sub-answers 6 and 7).
//!
//! Plain forms (boosted by the layout): every write answers 303. Tests are
//! in `tests/<area>.rs`.

use actix_identity::Identity;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::domain::{SharedFavorites, SharedRecipe};
use kauppa_core::export::{Candidate, ExportOutcome, RecipeImport};
use kauppa_core::repo::{CatalogShareRepo, ProductRepo, RecipeRepo};

use crate::error::WebResult;
use crate::i18n::{Ui, VARY};
use crate::routes::request::current_user;
use crate::routes::review::{self, DoneQuery, Page, Source};
use crate::state::AppState;

#[cfg(test)]
mod tests;

/// The import form's field for a ticked favorite, and for "Import all".
const PICK: &str = "pick";
const ALL: &str = "all";

#[derive(Debug, Deserialize)]
struct ShareForm {
    #[serde(default)]
    email: String,
    favorites: Option<String>,
    recipes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SwitchForm {
    favorites: Option<String>,
    recipes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IndexQuery {
    /// Set by the 303 after sharing, to show the generic notice.
    shared: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BrowseQuery {
    /// Set by the 303 after an import with nothing ticked.
    none: Option<String>,
}

/// `/sharing` (`sharing/index`).
#[derive(Debug, Serialize)]
struct IndexView {
    notice: bool,
    error: Option<String>,
    email: String,
    shares: Vec<ShareView>,
    shares_count: usize,
    shared: Vec<SharedView>,
}

#[derive(Debug, Serialize)]
struct ShareView {
    id: Uuid,
    name: String,
    email: String,
    favorites: bool,
    recipes: bool,
}

#[derive(Debug, Serialize)]
struct SharedView {
    owner_id: Uuid,
    owner_name: String,
    favorites: bool,
    recipes: bool,
}

/// `/sharing/from/{owner}/recipes` (`sharing/recipes`).
#[derive(Debug, Serialize)]
struct RecipesView {
    owner_id: Uuid,
    owner_name: String,
    recipes: Vec<RecipeLink>,
}

#[derive(Debug, Serialize)]
struct RecipeLink {
    id: Uuid,
    title: String,
}

/// `/sharing/from/{owner}/recipes/{recipe}` (`sharing/recipe`).
#[derive(Debug, Serialize)]
struct RecipeView {
    owner_id: Uuid,
    owner_name: String,
    id: Uuid,
    title: String,
    instructions: String,
    servings: Option<i64>,
    ingredients: Vec<IngredientLine>,
    /// The viewer's copy, if they already imported it (RR-0048 sub-answer 7).
    copy: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct IngredientLine {
    text: String,
    quantity: String,
}

/// `/sharing/from/{owner}/favorites` (`sharing/favorites`).
#[derive(Debug, Serialize)]
struct BrowseView {
    owner_id: Uuid,
    owner_name: String,
    none_chosen: bool,
    groups: Vec<BrowseGroup>,
}

#[derive(Debug, Serialize)]
struct BrowseGroup {
    /// The sharer's category name; `None` for uncategorized.
    name: Option<String>,
    products: Vec<BrowseProduct>,
}

#[derive(Debug, Serialize)]
struct BrowseProduct {
    id: Uuid,
    name: String,
}

fn html(status: actix_web::http::StatusCode, body: String) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("text/html")
        .insert_header(VARY)
        .body(body)
}

fn see_other(location: String) -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header((header::LOCATION, location))
        .finish()
}

async fn render_index(
    state: &AppState,
    ui: &Ui,
    user: Uuid,
    status: actix_web::http::StatusCode,
    notice: bool,
    email: String,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let shares: Vec<ShareView> = CatalogShareRepo::catalog_shares(&state.pool, user)
        .await?
        .into_iter()
        .map(|s| ShareView {
            id: s.id,
            name: s.grantee_name,
            email: s.grantee_email,
            favorites: s.favorites,
            recipes: s.recipes,
        })
        .collect();
    let with_me = CatalogShareRepo::shared_with(&state.pool, user)
        .await?
        .into_iter()
        .map(|s| SharedView {
            owner_id: s.owner_id,
            owner_name: s.owner_name,
            favorites: s.favorites,
            recipes: s.recipes,
        })
        .collect();
    let view = IndexView {
        notice,
        error,
        email,
        shares_count: shares.len(),
        shares,
        shared: with_me,
    };
    Ok(html(status, ui.render(state, "sharing/index", &view)?))
}

#[get("")]
pub async fn index(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    query: web::Query<IndexQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let notice = query.shared.is_some();
    let ok = actix_web::http::StatusCode::OK;
    render_index(&state, &ui, user, ok, notice, String::new(), None).await
}

/// Share with an email. Matched or not, the answer is the same 303 and
/// notice; only the owner's own list of shares shows who matched, as a
/// list's sharing page does (RR-0048 sub-answer 3).
#[post("")]
pub async fn share(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    form: web::Form<ShareForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (favorites, recipes) = (form.favorites.is_some(), form.recipes.is_some());
    match CatalogShareRepo::share_catalog(&state.pool, user, &form.email, favorites, recipes).await
    {
        Ok(share) => {
            if let Some(share) = share {
                tracing::info!(user_id = %user, share_id = %share.id, favorites, recipes, "catalog shared");
            }
            Ok(see_other("/sharing?shared=1".to_owned()))
        }
        Err(CoreError::Invalid(reason)) => {
            let status = actix_web::http::StatusCode::BAD_REQUEST;
            let error = Some(ui.invalid(&reason)?);
            render_index(&state, &ui, user, status, false, form.email.clone(), error).await
        }
        Err(e) => Err(e.into()),
    }
}

/// Change a share's switches; both off deletes it.
#[post("/{share}/update")]
pub async fn update(
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<SwitchForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let share_id = path.into_inner();
    let (favorites, recipes) = (form.favorites.is_some(), form.recipes.is_some());
    CatalogShareRepo::set_catalog_share(&state.pool, user, share_id, favorites, recipes).await?;
    tracing::info!(user_id = %user, share_id = %share_id, favorites, recipes, "catalog share changed");
    Ok(see_other("/sharing".to_owned()))
}

/// Revoke a share. Copies already imported stay.
#[post("/{share}/revoke")]
pub async fn revoke(
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let share_id = path.into_inner();
    CatalogShareRepo::revoke_catalog_share(&state.pool, user, share_id).await?;
    tracing::info!(user_id = %user, share_id = %share_id, "catalog share revoked");
    Ok(see_other("/sharing".to_owned()))
}

/// Browse `owner`'s favorites, if shared with the viewer (else 404).
#[get("/from/{owner}/favorites")]
pub async fn browse(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    query: web::Query<BrowseQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let shared = CatalogShareRepo::shared_favorites(&state.pool, user, path.into_inner()).await?;
    let view = BrowseView {
        owner_id: shared.owner_id,
        owner_name: shared.owner_name,
        none_chosen: query.none.is_some(),
        groups: shared
            .groups
            .into_iter()
            .map(|g| BrowseGroup {
                name: g.category.map(|c| c.name),
                products: g
                    .products
                    .into_iter()
                    .map(|p| BrowseProduct {
                        id: p.id,
                        name: p.name,
                    })
                    .collect(),
            })
            .collect(),
    };
    Ok(html(
        actix_web::http::StatusCode::OK,
        ui.render(&state, "sharing/favorites", &view)?,
    ))
}

/// The review and summary pages for importing from `owner`, carrying the
/// choice (`all`, or the ticked ids) through the review.
fn import_page(owner: Uuid, owner_name: &str, choice: &Choice) -> Page {
    let base = format!("/sharing/from/{owner}/favorites/import");
    let hidden = match choice {
        Choice::All => vec![(ALL.to_owned(), "1".to_owned())],
        Choice::Picked(ids) => ids
            .iter()
            .map(|id| (PICK.to_owned(), id.to_string()))
            .collect(),
    };
    Page {
        action: format!("{base}/confirm"),
        base,
        back: format!("/sharing/from/{owner}/favorites"),
        source: Source::Shared(owner_name.to_owned()),
        hidden,
    }
}

/// What the importer chose.
#[derive(Debug)]
enum Choice {
    All,
    Picked(Vec<Uuid>),
}

impl Choice {
    /// From the form's fields; ids that don't parse are ignored (they match
    /// nothing anyway).
    fn from_fields(fields: &[(String, String)]) -> Self {
        if fields.iter().any(|(name, _)| name == ALL) {
            return Self::All;
        }
        Self::Picked(
            fields
                .iter()
                .filter(|(name, _)| name == PICK)
                .filter_map(|(_, value)| Uuid::parse_str(value).ok())
                .collect(),
        )
    }

    const fn is_empty(&self) -> bool {
        matches!(self, Self::Picked(ids) if ids.is_empty())
    }
}

/// The chosen shared favorites as candidates: their names and the sharer's
/// categories, which the plan matches by name (RR-0048 sub-answer 5).
fn candidates(shared: SharedFavorites, choice: &Choice) -> Vec<Candidate> {
    shared
        .groups
        .into_iter()
        .flat_map(|g| {
            let category = g.category;
            g.products.into_iter().filter_map(move |p| {
                let chosen = match choice {
                    Choice::All => true,
                    Choice::Picked(ids) => ids.contains(&p.id),
                };
                chosen.then(|| Candidate {
                    name: p.name,
                    category: category.clone(),
                })
            })
        })
        .collect()
}

async fn import_from(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    user: Uuid,
    owner: Uuid,
    fields: Vec<(String, String)>,
) -> WebResult<HttpResponse> {
    let (answers, others) = review::answers(fields);
    let choice = Choice::from_fields(&others);
    if choice.is_empty() {
        // Nothing ticked: back to the list with a hint (after the share is
        // checked, so a stranger still gets 404).
        CatalogShareRepo::shared_favorites(&state.pool, user, owner).await?;
        return Ok(see_other(format!("/sharing/from/{owner}/favorites?none=1")));
    }
    let shared = CatalogShareRepo::shared_favorites(&state.pool, user, owner).await?;
    let page = import_page(owner, &shared.owner_name, &choice);
    let items = candidates(shared, &choice);
    let review = (!answers.fingerprint.is_empty()).then_some(&answers);
    let outcome = ProductRepo::apply_export(&state.pool, user, &items, review).await?;
    if let ExportOutcome::Saved(s) = &outcome {
        tracing::info!(user_id = %user, owner_id = %owner, added = s.added, "favorites imported");
    }
    review::respond(state, ui, req, &page, outcome)
}

/// Import ticked favorites, or all: save, or show the review.
#[post("/from/{owner}/favorites/import")]
pub async fn import(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<Vec<(String, String)>>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let mut fields = form.into_inner();
    // A first import carries no answers; drop any, so a review is planned.
    fields.retain(|(name, _)| name == PICK || name == ALL);
    import_from(&state, &ui, &req, user, path.into_inner(), fields).await
}

/// Confirm an import's review.
#[post("/from/{owner}/favorites/import/confirm")]
pub async fn import_confirm(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<Vec<(String, String)>>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let mut fields = form.into_inner();
    if !fields.iter().any(|(name, _)| name == "fingerprint") {
        // A confirmation always carries one; an empty one never matches.
        fields.push(("fingerprint".to_owned(), "-".to_owned()));
    }
    import_from(&state, &ui, &req, user, path.into_inner(), fields).await
}

/// The summary after an import.
#[get("/from/{owner}/favorites/import/done")]
pub async fn import_done(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    query: web::Query<DoneQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let owner = path.into_inner();
    // Shown only while the share lasts, like the rest of the import.
    let shared = CatalogShareRepo::shared_favorites(&state.pool, user, owner).await?;
    let source = Source::Shared(shared.owner_name);
    review::done(
        &state,
        &ui,
        format!("/sharing/from/{owner}/favorites"),
        &source,
        query.into_inner(),
    )
}

/// Browse `owner`'s recipes, if shared with the viewer (else 404).
#[get("/from/{owner}/recipes")]
pub async fn browse_recipes(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let shared = CatalogShareRepo::shared_recipes(&state.pool, user, path.into_inner()).await?;
    let view = RecipesView {
        owner_id: shared.owner_id,
        owner_name: shared.owner_name,
        recipes: shared
            .recipes
            .into_iter()
            .map(|r| RecipeLink {
                id: r.id,
                title: r.title,
            })
            .collect(),
    };
    Ok(html(
        actix_web::http::StatusCode::OK,
        ui.render(&state, "sharing/recipes", &view)?,
    ))
}

/// One shared recipe, read-only, with "Import" or a link to the copy.
#[get("/from/{owner}/recipes/{recipe}")]
pub async fn show_recipe(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (owner, recipe_id) = path.into_inner();
    let shared = CatalogShareRepo::shared_recipe(&state.pool, user, owner, recipe_id).await?;
    let copy = RecipeRepo::imported_copy(&state.pool, user, recipe_id).await?;
    let view = RecipeView {
        owner_id: shared.owner_id,
        owner_name: shared.owner_name,
        id: shared.recipe.id,
        title: shared.recipe.title,
        instructions: shared.recipe.instructions,
        servings: shared.recipe.servings,
        ingredients: shared
            .ingredients
            .into_iter()
            .map(|i| IngredientLine {
                text: i.text,
                quantity: i.quantity.unwrap_or_default(),
            })
            .collect(),
        copy,
    };
    Ok(html(
        actix_web::http::StatusCode::OK,
        ui.render(&state, "sharing/recipe", &view)?,
    ))
}

fn recipe_page(shared: &SharedRecipe) -> Page {
    let back = format!(
        "/sharing/from/{}/recipes/{}",
        shared.owner_id, shared.recipe.id
    );
    Page {
        action: format!("{back}/import/confirm"),
        base: format!("{back}/import"),
        back,
        source: Source::SharedRecipe {
            name: shared.owner_name.clone(),
            title: shared.recipe.title.clone(),
        },
        hidden: Vec::new(),
    }
}

async fn import_recipe_from(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    user: Uuid,
    (owner, recipe_id): (Uuid, Uuid),
    answers: Option<&kauppa_core::export::ReviewAnswers>,
) -> WebResult<HttpResponse> {
    let shared = CatalogShareRepo::shared_recipe(&state.pool, user, owner, recipe_id).await?;
    let page = recipe_page(&shared);
    match RecipeRepo::import_recipe(&state.pool, user, &shared, answers).await? {
        RecipeImport::Saved {
            recipe: copy,
            categories_created,
        } => {
            tracing::info!(user_id = %user, owner_id = %owner, recipe_id = %copy, categories_created, "recipe imported");
            Ok(see_other(if categories_created == 0 {
                format!("/recipes/{copy}")
            } else {
                format!("/recipes/{copy}?created={categories_created}")
            }))
        }
        // Refused (RR-0048 sub-answer 7): the recipe's page names the copy.
        RecipeImport::AlreadyImported(_) => Ok(see_other(page.back)),
        RecipeImport::Review {
            plan,
            categories,
            reason,
        } => review::review_page(state, ui, req, &page, &plan, &categories, reason),
    }
}

/// Import a shared recipe: saved (to the copy), refused (back to the
/// recipe, which links the copy), or the review.
#[post("/from/{owner}/recipes/{recipe}/import")]
pub async fn import_recipe(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    import_recipe_from(&state, &ui, &req, user, path.into_inner(), None).await
}

/// Confirm a recipe import's review.
#[post("/from/{owner}/recipes/{recipe}/import/confirm")]
pub async fn import_recipe_confirm(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<Vec<(String, String)>>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (mut answers, _) = review::answers(form.into_inner());
    if answers.fingerprint.is_empty() {
        // A confirmation always carries one; a missing one never matches.
        "-".clone_into(&mut answers.fingerprint);
    }
    import_recipe_from(&state, &ui, &req, user, path.into_inner(), Some(&answers)).await
}
