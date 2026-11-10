use actix_identity::Identity;
use actix_web::{HttpResponse, get, web};
use serde_json::json;

use crate::error::WebResult;
use crate::i18n::{Ui, VARY};
use crate::state::AppState;

#[get("/")]
pub async fn index(
    state: web::Data<AppState>,
    ui: Ui,
    id: Option<Identity>,
) -> WebResult<HttpResponse> {
    let signed_in = id.and_then(|i| i.id().ok()).is_some_and(|v| !v.is_empty());
    let body = ui.render(&state, "index", &json!({ "signed_in": signed_in }))?;
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}
