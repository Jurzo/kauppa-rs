use actix_web::web;

pub mod auth;
pub mod categories;
pub mod health;
pub mod lists;
pub mod pages;
pub mod recipes;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health)
        .service(pages::index)
        .service(
            web::scope("/auth")
                .service(auth::login_form)
                .service(auth::login_submit)
                .service(auth::register_form)
                .service(auth::register_submit)
                .service(auth::logout),
        )
        .service(
            web::scope("/lists")
                .service(lists::index)
                .service(lists::create),
        )
        .service(
            web::scope("/recipes")
                .service(recipes::index)
                .service(recipes::create),
        )
        .service(
            web::scope("/categories")
                .service(categories::index)
                .service(categories::create)
                .service(categories::rename)
                .service(categories::move_category)
                .service(categories::delete),
        );
}
