//! MB-0006 (P1, P3–P6): the store view.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::{CategoryRepo, ProductRepo, ShoppingListRepo};

use crate::state::AppState;

use super::ShopFixture;

#[allow(clippy::expect_used)]
async fn shop_fixture(state: &AppState, user: Uuid) -> ShopFixture {
    use crate::routes::test_support::make_category;

    let kylma = make_category(state, user, "Kylmä").await;
    let hevi = make_category(state, user, "Hevi").await;
    make_category(state, user, "Pakasteet").await;
    let list = ShoppingListRepo::create(&state.pool, user, "Viikko")
        .await
        .expect("list")
        .id;
    let candles = ShoppingListRepo::add_item(&state.pool, user, list, "Kynttilät", None)
        .await
        .expect("item")
        .id;
    let mut milk = Uuid::nil();
    for (name, category) in [("Maito", kylma), ("Omena", hevi), ("Jogurtti", kylma)] {
        let product = ProductRepo::create(&state.pool, user, name, category)
            .await
            .expect("favorite");
        let item = ShoppingListRepo::add_product(&state.pool, user, list, product.id, Some("1"))
            .await
            .expect("item");
        if name == "Maito" {
            milk = item.id;
        }
    }
    let sugar = ShoppingListRepo::add_item(&state.pool, user, list, "Sokeri", None)
        .await
        .expect("item")
        .id;
    ShoppingListRepo::remove_item(&state.pool, user, list, sugar)
        .await
        .expect("remove");
    ShopFixture {
        list,
        milk,
        candles,
        sugar,
    }
}

/// `(group heading, [item texts])` in the order the page shows them,
/// read from the rendered HTML.
fn rendered_groups(body: &str) -> Vec<(String, Vec<String>)> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for part in body.split("<h2>").skip(1) {
        let heading = part.split("</h2>").next().unwrap_or_default().to_owned();
        let items = part
            .split(r#"class="product-name">"#)
            .skip(1)
            .map(|chunk| {
                let name = chunk.split("</span>\n").next().unwrap_or_default();
                // Drop the quantity span, if any.
                name.rsplit("</span> ").next().unwrap_or(name).to_owned()
            })
            .collect();
        groups.push((heading, items));
    }
    groups
}

/// MB-0006 P1: the store view groups active items by their own category in
/// the user's category order, with uncategorized items in one trailing group;
/// empty categories and removed items don't show.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn view_groups_by_category() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let fixture = shop_fixture(&state, user).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/lists/{}/shop", fixture.list))
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    let owned = |items: &[&str]| items.iter().map(|&s| s.to_owned()).collect::<Vec<_>>();
    assert_eq!(
        rendered_groups(&body),
        [
            ("Kylmä".to_owned(), owned(&["Maito", "Jogurtti"])),
            ("Hevi".to_owned(), owned(&["Omena"])),
            ("Uncategorized".to_owned(), owned(&["Kynttilät"])),
        ]
    );
    assert!(!body.contains("Sokeri") && !body.contains("Pakasteet"));
    assert!(body.contains("0 of 4 found"));

    // Recategorizing the item (not the product) moves it between groups.
    let hevi = CategoryRepo::list_for_owner(&state.pool, user)
        .await
        .expect("categories")
        .into_iter()
        .find(|c| c.name == "Hevi")
        .expect("Hevi")
        .id;
    ShoppingListRepo::assign_category(&state.pool, user, fixture.list, fixture.candles, hevi)
        .await
        .expect("assign");
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/lists/{}/shop", fixture.list))
                .cookie(session)
                .to_request(),
        )
        .await,
    )
    .await;
    assert_eq!(
        rendered_groups(&body),
        [
            ("Kylmä".to_owned(), owned(&["Maito", "Jogurtti"])),
            ("Hevi".to_owned(), owned(&["Omena", "Kynttilät"])),
        ]
    );
}

/// MB-0006 P3: marking an item found persists it and returns the fragment;
/// the form sends the state to set, so repeating it is harmless.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn toggle_found() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let fixture = shop_fixture(&state, user).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let uri = format!("/lists/{}/shop/items/{}/check", fixture.list, fixture.milk);
    let checked = || async {
        ShoppingListRepo::items(&state.pool, user, fixture.list, false)
            .await
            .expect("items")
            .into_iter()
            .find(|i| i.id == fixture.milk)
            .expect("milk")
            .checked
    };

    for _ in 0..2 {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&uri)
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .set_form([("checked", "true")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert!(!body.contains("<html"));
        assert!(body.contains(&format!(r#"id="shop-{}""#, fixture.list)));
        assert!(
            body.contains("1 of 4 found"),
            "a repeat doesn't flip it back"
        );
        assert!(body.contains("Mark Maito not found"));
        assert!(checked().await);
    }

    // Unchecking without htmx: a 303 back to the store view.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form([("checked", "false")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        format!("/lists/{}/shop", fixture.list).as_str()
    );
    assert!(!checked().await);

    // A garbled state is rejected; a removed item can't be checked.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form([("checked", "maybe")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!(
                "/lists/{}/shop/items/{}/check",
                fixture.list, fixture.sugar
            ))
            .cookie(session)
            .set_form([("checked", "true")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// MB-0006 P4: removing an item in the store view soft-deletes it — it
/// leaves the view but can still be restored.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn remove_from_view() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let fixture = shop_fixture(&state, user).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!(
                "/lists/{}/shop/items/{}/remove",
                fixture.list, fixture.candles
            ))
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("Kynttilät") && !body.contains(">Uncategorized<"));
    assert!(body.contains("0 of 3 found"));

    let removed = ShoppingListRepo::items(&state.pool, user, fixture.list, true)
        .await
        .expect("removed");
    assert!(
        removed
            .iter()
            .any(|i| i.id == fixture.candles && i.removed_at.is_some())
    );

    // Recoverable: restoring brings it back into the view.
    ShoppingListRepo::restore_item(&state.pool, user, fixture.list, fixture.candles)
        .await
        .expect("restore");
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/lists/{}/shop", fixture.list))
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(body.contains("Kynttilät"));

    // Plain request: 303; removing twice is a 404.
    let uri = format!("/lists/{}/shop/items/{}/remove", fixture.list, fixture.milk);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = call_service(
        &app,
        TestRequest::post().uri(&uri).cookie(session).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// MB-0006 P5: another user can't see the store view, check items off, or
/// remove them; nothing changes.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn view_owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let fixture = shop_fixture(&state, user).await;
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let base = format!("/lists/{}/shop", fixture.list);

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(intruder.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    for hx in [false, true] {
        for (uri, form) in [
            (
                format!("{base}/items/{}/check", fixture.milk),
                vec![("checked", "true")],
            ),
            (format!("{base}/items/{}/remove", fixture.milk), vec![]),
        ] {
            let mut req = TestRequest::post()
                .uri(&uri)
                .cookie(intruder.clone())
                .set_form(form);
            if hx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri} (htmx: {hx})");
        }
    }
    let milk = ShoppingListRepo::items(&state.pool, user, fixture.list, false)
        .await
        .expect("items")
        .into_iter()
        .find(|i| i.id == fixture.milk)
        .expect("milk still active");
    assert!(!milk.checked);
}

/// MB-0006 P6: the store view is a full page on a normal (or boosted)
/// request and a fragment for a plain htmx request; check and remove answer
/// htmx with the fragment; every endpoint rejects a request without a
/// session.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn view_htmx_and_auth() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let fixture = shop_fixture(&state, user).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{}/shop", fixture.list);
    let fragment_id = format!(r#"id="shop-{}""#, fixture.list);
    let get = || TestRequest::get().uri(&base).cookie(session.clone());

    let page = body_string(call_service(&app, get().to_request()).await).await;
    assert!(page.contains("<html") && page.contains(&fragment_id));
    assert!(
        page.contains(&format!(r#"href="/lists/{}""#, fixture.list)),
        "links back to editing"
    );
    let boosted = body_string(
        call_service(
            &app,
            get()
                .insert_header(("HX-Request", "true"))
                .insert_header(("HX-Boosted", "true"))
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(boosted.contains("<html"));
    let fragment = body_string(
        call_service(
            &app,
            get().insert_header(("HX-Request", "true")).to_request(),
        )
        .await,
    )
    .await;
    assert!(!fragment.contains("<html") && fragment.contains(&fragment_id));

    // The list page links to the store view.
    let list_page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/lists/{}", fixture.list))
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(list_page.contains(&format!(r#"href="{base}""#)));

    for (uri, form) in [
        (
            format!("{base}/items/{}/check", fixture.milk),
            vec![("checked", "true")],
        ),
        (format!("{base}/items/{}/remove", fixture.candles), vec![]),
    ] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&uri)
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .set_form(form)
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{uri}");
        let body = body_string(resp).await;
        assert!(
            !body.contains("<html") && body.contains(&fragment_id),
            "{uri}"
        );
    }

    // No session: rejected everywhere.
    for req in [
        TestRequest::get().uri(&base),
        TestRequest::post()
            .uri(&format!("{base}/items/{}/check", fixture.milk))
            .set_form([("checked", "true")]),
        TestRequest::post().uri(&format!("{base}/items/{}/remove", fixture.milk)),
    ] {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
