//! Item handlers on the list page (MB-0004, MB-0005, MB-0021, MB-0036): add free text or a favorite from one field, or a recipe; assign a category; edit a quantity; remove and restore.

use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::domain::ListAccess;
use kauppa_core::repo::{ProductRepo, ShoppingListRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::i18n::Ui;
use crate::routes::products::parse_category;
use crate::routes::request::current_user;
use crate::state::AppState;

use super::{ItemDraft, parse_multiplier, respond_item};

#[derive(Debug, Deserialize)]
struct ItemForm {
    text: String,
    #[serde(default)]
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct FavoriteForm {
    // Missing and empty both mean "nothing chosen", which becomes a message.
    #[serde(default)]
    product_id: String,
    #[serde(default)]
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct RecipeForm {
    // Missing and empty both mean "nothing chosen", which becomes a message.
    #[serde(default)]
    recipe_id: String,
    #[serde(default)]
    multiplier: String,
}

#[derive(Debug, Deserialize)]
struct QuantityForm {
    // Missing and empty both clear the quantity.
    #[serde(default)]
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct CategoryForm {
    #[serde(default)]
    category_id: String,
}

fn parse_recipe(raw: &str) -> CoreResult<Uuid> {
    match raw.trim() {
        "" => Err(CoreError::Invalid("choose a recipe to add".to_owned())),
        raw => Uuid::parse_str(raw).map_err(|_| CoreError::Invalid("unknown recipe".to_owned())),
    }
}

fn parse_product(raw: &str) -> CoreResult<Uuid> {
    match raw.trim() {
        "" => Err(CoreError::Invalid("choose a favorite to add".to_owned())),
        raw => Uuid::parse_str(raw).map_err(|_| CoreError::Invalid("unknown product".to_owned())),
    }
}

/// The owner's favorite that `text` names by `name_key` (RR-0003), if any
/// (MB-0036). Only favorites match, not recipe-created products. Called
/// only for the list's owner, so `user`'s catalog is the list owner's.
async fn owner_favorite(state: &AppState, user: Uuid, text: &str) -> CoreResult<Option<Uuid>> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let product = ProductRepo::find_by_name(&state.pool, user, text).await?;
    Ok(product.filter(|p| p.is_favorite).map(|p| p.id))
}

/// Add an item from the one add field (MB-0036): text naming one of the
/// list owner's favorites adds that favorite as `add_favorite` does (its
/// name and category copied now); any other text is a free-text item,
/// uncategorized until a category is chosen. The owner's text resolves
/// through their catalog and `add_product`; anyone else's goes to
/// `add_owner_favorite_by_name`, which matches the owner's favorites
/// silently at `edit` and refuses a `view` grantee (RR-0024). The repo
/// methods authorize.
#[post("/{id}/items")]
pub async fn add_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<ItemForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let quantity = Some(form.quantity.as_str());
    let pool = &state.pool;
    let outcome = match ShoppingListRepo::access(pool, user, list_id).await {
        Ok(ListAccess::Owner) => match owner_favorite(&state, user, &form.text).await {
            Ok(Some(product)) => {
                ShoppingListRepo::add_product(pool, user, list_id, product, quantity).await
            }
            Ok(None) => ShoppingListRepo::add_item(pool, user, list_id, &form.text, quantity).await,
            Err(e) => Err(e),
        },
        Ok(_) => {
            ShoppingListRepo::add_owner_favorite_by_name(pool, user, list_id, &form.text, quantity)
                .await
        }
        Err(e) => Err(e),
    };
    let draft = if outcome.is_ok() {
        ItemDraft::default()
    } else {
        let form = form.into_inner();
        ItemDraft {
            text: form.text,
            quantity: form.quantity,
            ..ItemDraft::default()
        }
    };
    respond_item(&ui, &req, user, list_id, outcome, draft).await
}

/// Add one of the user's favorites by id, copying its name and category now.
/// The list page no longer has this form (MB-0036: the add field matches
/// favorites by name); the route stays for pages loaded before that change.
#[post("/{id}/favorites")]
pub async fn add_favorite(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<FavoriteForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let outcome = match parse_product(&form.product_id) {
        Ok(product) => {
            ShoppingListRepo::add_product(&state.pool, user, list_id, product, Some(&form.quantity))
                .await
        }
        Err(e) => Err(e),
    };
    respond_item(&ui, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Add a recipe's ingredients, optionally multiplied (MB-0005).
#[post("/{id}/recipes")]
pub async fn add_recipe(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<RecipeForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let outcome = match (
        parse_recipe(&form.recipe_id),
        parse_multiplier(&form.multiplier),
    ) {
        (Ok(recipe), Ok(multiplier)) => {
            ShoppingListRepo::add_recipe(&state.pool, user, list_id, recipe, multiplier).await
        }
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    let draft = ItemDraft {
        multiplier: if outcome.is_ok() {
            String::new()
        } else {
            form.into_inner().multiplier
        },
        ..ItemDraft::default()
    };
    respond_item(&ui, &req, user, list_id, outcome, draft).await
}

#[post("/{id}/items/{item}/category")]
pub async fn assign_category(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<CategoryForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = match parse_category(&form.category_id) {
        Ok(category) => {
            ShoppingListRepo::assign_category(&state.pool, user, list_id, item_id, category).await
        }
        Err(e) => Err(e),
    };
    respond_item(&ui, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Set or clear (blank) an item's quantity (MB-0021). The owner and `edit`
/// grantees may; the repo's access predicate decides.
#[post("/{id}/items/{item}/quantity")]
pub async fn set_item_quantity(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<QuantityForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::set_item_quantity(
        &state.pool,
        user,
        list_id,
        item_id,
        Some(&form.quantity),
    )
    .await;
    respond_item(&ui, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Remove an item (soft delete; it can be restored).
#[post("/{id}/items/{item}/remove")]
pub async fn remove_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::remove_item(&state.pool, user, list_id, item_id).await;
    respond_item(&ui, &req, user, list_id, outcome, ItemDraft::default()).await
}

#[post("/{id}/items/{item}/restore")]
pub async fn restore_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::restore_item(&state.pool, user, list_id, item_id).await;
    respond_item(&ui, &req, user, list_id, outcome, ItemDraft::default()).await
}

// Sharing (MB-0008): the owner's `/lists/{id}/sharing` page, with the share
// form (`#share-form-{id}`) and the people with access (`#grantees-{id}`).
