//! Item handlers on the list page (MB-0004, MB-0005, MB-0021, MB-0036, MB-0052): add free text or a favorite from one field, or a recipe; save an item's category and quantity with one Save; remove and restore.

use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, post, web};
use kauppa_core::{Entity, Invalid};
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
struct RecipeForm {
    // Missing and empty both mean "nothing chosen", which becomes a message.
    #[serde(default)]
    recipe_id: String,
    #[serde(default)]
    multiplier: String,
}

/// An item's edit section (MB-0052): one form, one Save.
#[derive(Debug, Deserialize)]
struct ItemEditForm {
    // Missing and empty both leave the category as it is (RR-0033).
    #[serde(default)]
    category_id: Option<String>,
    // Missing and empty both clear the quantity.
    #[serde(default)]
    quantity: String,
}

fn parse_recipe(raw: &str) -> CoreResult<Uuid> {
    match raw.trim() {
        "" => Err(CoreError::Invalid(Invalid::ChooseRecipe)),
        raw => {
            Uuid::parse_str(raw).map_err(|_| CoreError::Invalid(Invalid::Unknown(Entity::Recipe)))
        }
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
/// list owner's favorites adds that favorite (its name and category copied
/// now); any other text is a free-text item,
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

/// Save an item's edit section (MB-0052, RR-0033): its quantity (set, or
/// cleared when blank, MB-0021) and, when one is chosen, its category, in
/// one transaction. No or a blank `category_id` leaves the category as it
/// is; a malformed one is the "unknown category" rejection. The repo
/// decides access: `edit` for the quantity alone, the owner's with a
/// category (a grantee who sends one gets 403, and nothing changes).
#[post("/{id}/items/{item}")]
pub async fn update_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<ItemEditForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let category = match form.category_id.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(raw) => parse_category(raw).map(Some),
    };
    let outcome = match category {
        Ok(category) => {
            ShoppingListRepo::update_item(
                &state.pool,
                user,
                list_id,
                item_id,
                category,
                Some(&form.quantity),
            )
            .await
        }
        Err(e) => Err(e),
    };
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
