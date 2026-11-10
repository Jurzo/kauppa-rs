//! P4's walk: every page and fragment, and the main rejections, rendered
//! for a Finnish user (MB-0035).

use actix_web::cookie::Cookie;
use actix_web::http::StatusCode;
use actix_web::test::{TestRequest, call_service};
use chrono::Utc;

use kauppa_core::domain::{Language, ShareRole};
use kauppa_core::repo::{AdminRepo, ProductRepo, ShoppingListRepo, UserRepo};

use crate::routes::auth::tests::{invitation, register_request};
use crate::routes::test_support::{
    body_string, login_request, make_category, make_list, make_recipe_with, make_user,
    session_cookie, test_app, test_state,
};

/// One response of the walk.
pub struct Rendered {
    /// What was requested, for messages.
    pub what: String,
    pub status: StatusCode,
    /// Whether it is a full page (has `<html`).
    pub full: bool,
    pub body: String,
}

/// Every page and fragment the app renders, and its main rejections, as
/// a user (and a signed-out visitor) who chose `lang`.
#[allow(clippy::too_many_lines)]
pub async fn walk(lang: Language) -> Vec<Rendered> {
    let state = test_state().await;
    let app = test_app!(state);

    // An admin with a catalog, a list with every kind of item, a recipe,
    // a recipe-only product, an invitation, and a list shared with a viewer.
    let owner = make_user(&state, "owner@example.com").await;
    AdminRepo::grant(&state.pool, owner, None, Utc::now())
        .await
        .unwrap();
    let viewer = make_user(&state, "viewer@example.com").await;
    for user in [owner, viewer] {
        UserRepo::set_language(&state.pool, user, Some(lang))
            .await
            .unwrap();
    }
    let hevi = make_category(&state, owner, "Hevi").await;
    let _empty = make_category(&state, owner, "Pakasteet").await;
    ProductRepo::create(&state.pool, owner, "Maito", hevi)
        .await
        .unwrap();
    let list = make_list(&state, owner, "Viikko").await;
    ShoppingListRepo::add_owner_favorite_by_name(&state.pool, owner, list, "Maito", Some("1 l"))
        .await
        .unwrap();
    let candles = ShoppingListRepo::add_item(&state.pool, owner, list, "Kynttilät", Some("2"))
        .await
        .unwrap();
    let bread = ShoppingListRepo::add_item(&state.pool, owner, list, "Leipä", None)
        .await
        .unwrap();
    ShoppingListRepo::remove_item(&state.pool, owner, list, bread.id)
        .await
        .unwrap();
    ShoppingListRepo::set_checked(&state.pool, owner, list, candles.id, true)
        .await
        .unwrap();
    ShoppingListRepo::share(
        &state.pool,
        owner,
        list,
        "viewer@example.com",
        ShareRole::View,
    )
    .await
    .unwrap();
    let recipe = make_recipe_with(
        &state,
        owner,
        "Pannukakku",
        hevi,
        &[("Jauhot", Some("3 dl"))],
    )
    .await;
    let token = invitation(&state).await;

    let owner_session =
        session_cookie(&call_service(&app, login_request("owner@example.com").to_request()).await);
    let viewer_session =
        session_cookie(&call_service(&app, login_request("viewer@example.com").to_request()).await);
    let visitor = Cookie::new("kauppa_lang", lang.tag());

    let mut out = Vec::new();
    let mut fetch = async |what: &str, req: TestRequest| {
        let resp = call_service(&app, req.to_request()).await;
        let status = resp.status();
        let body = body_string(resp).await;
        out.push(Rendered {
            what: what.to_owned(),
            status,
            full: body.contains("<html"),
            body,
        });
    };
    let get = |uri: &str| TestRequest::get().uri(uri);
    let post = |uri: &str| TestRequest::post().uri(uri);
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true"));

    // Signed out.
    for uri in ["/", "/auth/login", "/auth/register"] {
        fetch(uri, get(uri).cookie(visitor.clone())).await;
    }
    let uri = format!("/auth/register?invite={token}");
    fetch(
        "register with invitation",
        get(&uri).cookie(visitor.clone()),
    )
    .await;
    fetch(
        "login rejected",
        post("/auth/login").cookie(visitor.clone()).set_form([
            ("email", "owner@example.com"),
            ("password", "wrong password here"),
        ]),
    )
    .await;
    fetch(
        "register: bad invitation",
        register_request(Some("not-a-token"), "new@example.com").cookie(visitor.clone()),
    )
    .await;
    fetch(
        "register: email taken",
        register_request(Some(&token), "owner@example.com").cookie(visitor.clone()),
    )
    .await;
    fetch(
        "register: short password",
        post("/auth/register").cookie(visitor.clone()).set_form([
            ("invite", token.as_str()),
            ("display_name", "Pat"),
            ("email", "pat@example.com"),
            ("password", "short"),
        ]),
    )
    .await;

    // The owner's pages.
    let o = |req: TestRequest| req.cookie(owner_session.clone());
    for uri in [
        "/".to_owned(),
        "/lists".to_owned(),
        format!("/lists/{list}"),
        format!("/lists/{list}/shop"),
        format!("/lists/{list}/sharing"),
        "/recipes".to_owned(),
        format!("/recipes/{recipe}"),
        "/favorites".to_owned(),
        "/categories".to_owned(),
        "/admin/invitations".to_owned(),
    ] {
        fetch(&uri, o(get(&uri))).await;
    }

    // Fragments, and rejections in them.
    let cases: Vec<(&str, TestRequest)> = vec![
        ("new list (empty)", post("/lists").set_form([("name", "")])),
        ("new list", post("/lists").set_form([("name", "Juhlat")])),
        (
            "item added",
            post(&format!("/lists/{list}/items")).set_form([("text", "Kahvi"), ("quantity", "")]),
        ),
        (
            "item (empty)",
            post(&format!("/lists/{list}/items")).set_form([("text", " "), ("quantity", "")]),
        ),
        (
            "recipe to list (none chosen)",
            post(&format!("/lists/{list}/recipes"))
                .set_form([("recipe_id", ""), ("multiplier", "1")]),
        ),
        (
            "recipe to list (bad times)",
            post(&format!("/lists/{list}/recipes")).set_form([
                ("recipe_id", recipe.to_string().as_str()),
                ("multiplier", "x"),
            ]),
        ),
        (
            "shop check",
            post(&format!("/lists/{list}/shop/items/{}/check", candles.id))
                .set_form([("checked", "false")]),
        ),
        (
            "share (empty)",
            post(&format!("/lists/{list}/shares")).set_form([("email", ""), ("role", "view")]),
        ),
        (
            "share",
            post(&format!("/lists/{list}/shares"))
                .set_form([("email", "nobody@example.com"), ("role", "edit")]),
        ),
        (
            "new category (empty)",
            post("/categories").set_form([("name", "")]),
        ),
        (
            "new category (taken)",
            post("/categories").set_form([("name", "Hevi")]),
        ),
        (
            "new favorite (no category)",
            post("/favorites").set_form([("name", "Kahvi"), ("category_id", "")]),
        ),
        (
            "new favorite (taken)",
            post("/favorites").set_form([
                ("name", "Maito"),
                ("category_id", hevi.to_string().as_str()),
            ]),
        ),
        (
            "new recipe (empty)",
            post("/recipes").set_form([("title", "")]),
        ),
        (
            "recipe details (bad servings)",
            post(&format!("/recipes/{recipe}")).set_form([
                ("title", "Pannukakku"),
                ("servings", "0"),
                ("instructions", ""),
            ]),
        ),
        (
            "ingredient match: existing",
            get(&format!("/recipes/{recipe}/ingredients/match?name=Maito")),
        ),
        (
            "ingredient match: new",
            get(&format!("/recipes/{recipe}/ingredients/match?name=Uusi")),
        ),
        (
            "ingredient: new product prompt",
            post(&format!("/recipes/{recipe}/ingredients"))
                .set_form([("name", "Uusi"), ("quantity", "")]),
        ),
        (
            "ingredient (empty)",
            post(&format!("/recipes/{recipe}/ingredients"))
                .set_form([("name", " "), ("quantity", "")]),
        ),
        (
            "recipe to a list",
            post(&format!("/recipes/{recipe}/add-to-list"))
                .set_form([("list_id", list.to_string().as_str()), ("multiplier", "2")]),
        ),
        (
            "recipe to a list (none chosen)",
            post(&format!("/recipes/{recipe}/add-to-list"))
                .set_form([("list_id", ""), ("multiplier", "1")]),
        ),
    ];
    for (what, req) in cases {
        fetch(&format!("{what} (htmx)"), o(hx(req))).await;
    }
    fetch(
        "invitation created",
        o(post("/admin/invitations").set_form([("note", "Mummo")])),
    )
    .await;
    fetch(
        "invitation (note too long)",
        o(post("/admin/invitations").set_form([("note", "x".repeat(300).as_str())])),
    )
    .await;

    // The viewer: a list shared to view, in their index and store view.
    let v = |req: TestRequest| req.cookie(viewer_session.clone());
    for uri in [
        "/lists".to_owned(),
        format!("/lists/{list}"),
        format!("/lists/{list}/shop"),
    ] {
        fetch(&format!("viewer {uri}"), v(get(&uri))).await;
    }
    out
}
