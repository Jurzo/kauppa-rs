use actix_identity::Identity;
use actix_web::{HttpResponse, get, web};
use serde_json::json;

use crate::error::WebResult;
use crate::state::AppState;

#[get("/")]
pub async fn index(state: web::Data<AppState>, id: Option<Identity>) -> WebResult<HttpResponse> {
    let signed_in = id.and_then(|i| i.id().ok()).is_some_and(|v| !v.is_empty());
    let body = state
        .templates
        .render("index", &json!({ "signed_in": signed_in }))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}
