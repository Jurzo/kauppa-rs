use actix_identity::Identity;
use actix_web::{HttpResponse, get, post, web};
use serde_json::json;
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::repo::RecipeRepo;

use crate::error::{WebError, WebResult};
use crate::state::AppState;

fn current_user(id: &Identity) -> WebResult<Uuid> {
    let raw = id
        .id()
        .map_err(|_| WebError::Core(CoreError::Unauthorized))?;
    Uuid::parse_str(&raw).map_err(|_| WebError::Core(CoreError::Unauthorized))
}

#[get("")]
pub async fn index(state: web::Data<AppState>, id: Identity) -> WebResult<HttpResponse> {
    let user_id = current_user(&id)?;
    let recipes = state.pool.list_for_owner(user_id).await?;

    let body = state
        .templates
        .render("recipes/index", &json!({ "recipes": recipes }))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

#[post("")]
pub async fn create(_id: Identity) -> HttpResponse {
    // TODO(MB-0002): wire up recipe creation + htmx fragment response.
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("recipe creation not yet implemented")
}
