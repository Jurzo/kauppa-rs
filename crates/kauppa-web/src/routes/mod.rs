use actix_web::middleware::from_fn;
use actix_web::web;

pub mod admin;
pub mod assets;
pub mod auth;
pub mod categories;
pub mod health;
pub mod language;
pub mod lists;
pub mod pages;
pub mod products;
pub mod recipes;
pub mod request;

#[cfg(test)]
pub mod test_support;

/// Every route, behind the app-wide cross-site check (MB-0018, RR-0010): one
/// catch-all scope wrapped in [`crate::csrf::guard`], so no handler is
/// registered outside it (`csrf::tests::coverage` checks).
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(from_fn(crate::csrf::guard))
            .configure(protected),
    );
}

/// The routes. Register new ones here, never beside the scope above.
fn protected(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health)
        .service(pages::index)
        .service(language::switch)
        .service(
            web::scope("/auth")
                .service(auth::login_form)
                .service(auth::login_submit)
                .service(auth::register_form)
                .service(auth::register_submit)
                .service(auth::logout),
        )
        .service(
            web::scope("/admin")
                .service(admin::index)
                .service(admin::invitations)
                .service(admin::create_invitation)
                .service(admin::revoke_invitation),
        )
        .service(
            web::scope("/lists")
                .service(lists::index)
                .service(lists::create)
                .service(lists::show)
                .service(lists::rename)
                .service(lists::delete)
                .service(lists::add_item)
                .service(lists::add_favorite)
                .service(lists::add_recipe)
                .service(lists::shop)
                .service(lists::check_item)
                .service(lists::remove_from_shop)
                .service(lists::assign_category)
                .service(lists::set_item_quantity)
                .service(lists::remove_item)
                .service(lists::restore_item)
                .service(lists::sharing)
                .service(lists::share)
                .service(lists::revoke),
        )
        .service(
            web::scope("/recipes")
                .service(recipes::index)
                .service(recipes::create)
                .service(recipes::show)
                .service(recipes::update)
                .service(recipes::delete)
                .service(recipes::add_ingredient)
                .service(recipes::ingredient_match)
                .service(recipes::set_quantity)
                .service(recipes::move_ingredient)
                .service(recipes::remove_ingredient)
                .service(recipes::add_recipe_to_list),
        )
        .service(
            web::scope("/categories")
                .service(categories::index)
                .service(categories::create)
                .service(categories::rename)
                .service(categories::move_category)
                .service(categories::delete),
        )
        .service(
            web::scope("/favorites")
                .service(products::index)
                .service(products::create)
                .service(products::rename)
                .service(products::recategorize)
                .service(products::move_product)
                .service(products::make_favorite)
                .service(products::delete),
        );
}
