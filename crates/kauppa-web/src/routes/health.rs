use actix_web::{HttpResponse, get};

#[get("/healthz")]
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain").body("ok")
}
