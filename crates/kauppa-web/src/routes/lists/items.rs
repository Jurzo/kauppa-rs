//! Item handlers on the list page (MB-0004, MB-0005, MB-0021): add free text, a favorite or a recipe; assign a category; edit a quantity; remove and restore.

use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
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

/// Add a free-text item; it stays uncategorized until a category is chosen.
#[post("/{id}/items")]
pub async fn add_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<ItemForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let outcome =
        ShoppingListRepo::add_item(&state.pool, user, list_id, &form.text, Some(&form.quantity))
            .await;
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
    respond_item(&state, &req, user, list_id, outcome, draft).await
}

/// Add one of the user's favorites, copying its name and category now.
#[post("/{id}/favorites")]
pub async fn add_favorite(
    req: HttpRequest,
    state: web::Data<AppState>,
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
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Add a recipe's ingredients, optionally multiplied (MB-0005).
#[post("/{id}/recipes")]
pub async fn add_recipe(
    req: HttpRequest,
    state: web::Data<AppState>,
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
    respond_item(&state, &req, user, list_id, outcome, draft).await
}

#[post("/{id}/items/{item}/category")]
pub async fn assign_category(
    req: HttpRequest,
    state: web::Data<AppState>,
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
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Set or clear (blank) an item's quantity (MB-0021). The owner and `edit`
/// grantees may; the repo's access predicate decides.
#[post("/{id}/items/{item}/quantity")]
pub async fn set_item_quantity(
    req: HttpRequest,
    state: web::Data<AppState>,
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
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Remove an item (soft delete; it can be restored).
#[post("/{id}/items/{item}/remove")]
pub async fn remove_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::remove_item(&state.pool, user, list_id, item_id).await;
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

#[post("/{id}/items/{item}/restore")]
pub async fn restore_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::restore_item(&state.pool, user, list_id, item_id).await;
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

// Sharing (MB-0008): the owner's `/lists/{id}/sharing` page, with the share
// form (`#share-form-{id}`) and the people with access (`#grantees-{id}`).
