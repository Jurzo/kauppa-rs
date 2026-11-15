//! MB-0061 P2 (RR-0048): the recipient sees only what is shared, through
//! the predicate; anything else is 404.

use actix_web::http::StatusCode;
use uuid::Uuid;

use kauppa_core::repo::{CatalogShareRepo, CategoryRepo, ProductRepo};

/// P2: shared favorites are listed by the sharer's categories, without the
/// sharer's products from recipes; a stranger, a never-made share, a share
/// with the favorites switch off, a revoked share and an unknown user all
/// answer 404 on every favorites route, over htmx or not.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn only_shared_content() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_recipe_with, make_user, session_cookie, test_app,
        test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let me = make_user(&state, "me@example.com").await;
    let friend_id = make_user(&state, "friend@example.com").await;
    make_user(&state, "stranger@example.com").await;
    let hevi = CategoryRepo::create(&state.pool, me, "Hevi")
        .await
        .expect("category")
        .id;
    let kylma = CategoryRepo::create(&state.pool, me, "Kylmä")
        .await
        .expect("category")
        .id;
    for (name, category) in [("Omena", hevi), ("Maito", kylma)] {
        ProductRepo::create(&state.pool, me, name, category)
            .await
            .expect("favorite");
    }
    make_recipe_with(&state, me, "Salaatti", hevi, &[("Salaattikurkku", None)]).await;
    let login = |email: &'static str| {
        let app = &app;
        async move { session_cookie(&call_service(app, login_request(email).to_request()).await) }
    };
    let friend = login("friend@example.com").await;
    let stranger = login("stranger@example.com").await;
    let browse = format!("/sharing/from/{me}/favorites");
    // (kind, method, uri): every favorites and recipes route.
    let routes = |owner: Uuid| {
        let base = format!("/sharing/from/{owner}/favorites");
        let recipes = format!("/sharing/from/{owner}/recipes");
        let one = format!("{recipes}/{}", Uuid::new_v4());
        [
            ("favorites", "GET", base.clone()),
            ("favorites", "POST", format!("{base}/import")),
            ("favorites", "POST", format!("{base}/import/confirm")),
            ("favorites", "GET", format!("{base}/import/done")),
            ("recipes", "GET", recipes),
            ("recipes", "GET", one.clone()),
            ("recipes", "POST", format!("{one}/import")),
            ("recipes", "POST", format!("{one}/import/confirm")),
        ]
    };
    let all_404 = |cookie: actix_web::cookie::Cookie<'static>,
                   owner: Uuid,
                   kinds: &'static [&'static str],
                   why: &'static str| {
        let app = &app;
        async move {
            for (kind, method, uri) in routes(owner) {
                if !kinds.contains(&kind) {
                    continue;
                }
                for htmx in [false, true] {
                    let mut req = if method == "GET" {
                        TestRequest::get()
                    } else {
                        TestRequest::post().set_form([("all", "1")])
                    }
                    .uri(&uri)
                    .cookie(cookie.clone());
                    if htmx {
                        req = req.insert_header(("HX-Request", "true"));
                    }
                    let resp = call_service(app, req.to_request()).await;
                    assert_eq!(
                        resp.status(),
                        StatusCode::NOT_FOUND,
                        "{why}: {method} {uri} htmx={htmx}"
                    );
                }
            }
        }
    };

    all_404(
        friend.clone(),
        me,
        &["favorites", "recipes"],
        "never shared",
    )
    .await;
    let share = CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", false, true)
        .await
        .expect("share")
        .expect("matched");
    all_404(friend.clone(), me, &["favorites"], "favorites switch off").await;
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/sharing/from/{me}/recipes"))
            .cookie(friend.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "recipes switch on");
    // A recipe that isn't the sharer's (unknown, or the viewer's own) is
    // 404 even with the recipes switch on.
    let own = crate::routes::test_support::make_recipe(&state, friend_id, "Oma").await;
    for recipe in [Uuid::new_v4(), own] {
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/sharing/from/{me}/recipes/{recipe}"))
                .cookie(friend.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{recipe}");
    }
    CatalogShareRepo::set_catalog_share(&state.pool, me, share.id, true, false)
        .await
        .expect("change");
    all_404(friend.clone(), me, &["recipes"], "recipes switch off").await;

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&browse)
            .cookie(friend.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("Favorites from Test User"));
    let hevi_at = body.find("<legend>Hevi</legend>").expect("Hevi");
    let kylma_at = body.find("<legend>Kylmä</legend>").expect("Kylmä");
    assert!(hevi_at < kylma_at, "the sharer's category order");
    assert!(body.contains("Omena") && body.contains("Maito"));
    assert!(
        !body.contains("Salaattikurkku"),
        "products from recipes aren't shared"
    );
    assert!(!body.contains("me@example.com"), "never the sharer's email");
    let form = &body[body.find(r#"class="export-review""#).expect("form")..];
    assert!(!form.contains("checked"), "nothing ticked by default");

    all_404(stranger, me, &["favorites", "recipes"], "a stranger").await;
    all_404(
        friend.clone(),
        Uuid::new_v4(),
        &["favorites", "recipes"],
        "an unknown user",
    )
    .await;
    CatalogShareRepo::revoke_catalog_share(&state.pool, me, share.id)
        .await
        .expect("revoke");
    all_404(friend, me, &["favorites", "recipes"], "revoked").await;
}
