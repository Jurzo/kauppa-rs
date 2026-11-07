//! MB-0004 (P5, P6): list items over HTTP, owner isolation and htmx fragments.

use actix_web::http::{StatusCode, header};

use kauppa_core::repo::{ProductRepo, ShoppingListRepo};

use super::item_shape;

/// MB-0004 P5: items are isolated through their list — another user can't
/// read the list or add, categorize, remove, or restore its items, over htmx
/// or not, and nothing changes.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let their_category = make_category(&state, other, "Kylmä").await;
    let their_milk = ProductRepo::create(&state.pool, other, "Maito", their_category)
        .await
        .expect("their favorite");
    let list = ShoppingListRepo::create(&state.pool, user, "Oma")
        .await
        .expect("list")
        .id;
    let bread = ShoppingListRepo::add_item(&state.pool, user, list, "Leipä", None)
        .await
        .expect("item")
        .id;
    let gone = ShoppingListRepo::add_item(&state.pool, user, list, "Sokeri", None)
        .await
        .expect("item")
        .id;
    ShoppingListRepo::remove_item(&state.pool, user, list, gone)
        .await
        .expect("remove");
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);

    let base = format!("/lists/{list}");
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(intruder.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let writes: [(String, Vec<(&str, String)>); 5] = [
        (format!("{base}/items"), vec![("text", "Omena".to_owned())]),
        (
            format!("{base}/favorites"),
            vec![("product_id", their_milk.id.to_string())],
        ),
        (
            format!("{base}/items/{bread}/category"),
            vec![("category_id", kylma.to_string())],
        ),
        (format!("{base}/items/{bread}/remove"), vec![]),
        (format!("{base}/items/{gone}/restore"), vec![]),
    ];
    for (uri, form) in writes {
        for hx in [false, true] {
            let mut req = TestRequest::post()
                .uri(&uri)
                .cookie(intruder.clone())
                .set_form(form.clone());
            if hx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri} (htmx: {hx})");
        }
    }

    assert_eq!(
        item_shape(&state, user, list).await,
        [("Leipä".to_owned(), None)]
    );
    let removed = ShoppingListRepo::items(&state.pool, user, list, true)
        .await
        .expect("removed");
    assert_eq!(removed.len(), 1, "still removed");

    // And the owner can't add another user's favorite to their own list.
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/favorites"))
            .cookie(session)
            .set_form([("product_id", their_milk.id.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(item_shape(&state, user, list).await.len(), 1);
}

/// MB-0004 P6: add-item, add-favorite, assign-category, and remove (and
/// restore) answer htmx with the `#list-{id}` fragment and plain requests with
/// a 303, or the full page with 400 when the input was rejected.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn htmx_fragments() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let koti = make_category(&state, user, "Koti").await;
    let maito = ProductRepo::create(&state.pool, user, "Maito", kylma)
        .await
        .expect("favorite");
    let list = ShoppingListRepo::create(&state.pool, user, "Viikko")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{list}");
    let post = |uri: String, form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let fragment_id = format!(r#"id="list-{list}""#);

    // htmx: each mutation answers with the fragment.
    let hx_call = |req: TestRequest| {
        let app = &app;
        async move {
            let resp =
                call_service(app, req.insert_header(("HX-Request", "true")).to_request()).await;
            assert_eq!(resp.status(), StatusCode::OK);
            body_string(resp).await
        }
    };
    let body = hx_call(post(
        format!("{base}/items"),
        vec![
            ("text", "Kynttilät".to_owned()),
            ("quantity", "6".to_owned()),
        ],
    ))
    .await;
    assert!(!body.contains("<html") && body.contains(&fragment_id));
    assert!(body.contains("Kynttilät") && body.contains("Uncategorized"));
    let candles = ShoppingListRepo::items(&state.pool, user, list, false)
        .await
        .expect("items")[0]
        .id;

    let body = hx_call(post(
        format!("{base}/favorites"),
        vec![("product_id", maito.id.to_string())],
    ))
    .await;
    assert!(body.contains(&fragment_id) && body.contains("Maito"));

    let body = hx_call(post(
        format!("{base}/items/{candles}/category"),
        vec![("category_id", koti.to_string())],
    ))
    .await;
    assert!(body.contains(&fragment_id) && body.contains("· Koti"));

    let body = hx_call(post(format!("{base}/items/{candles}/remove"), vec![])).await;
    assert!(body.contains(&fragment_id) && body.contains("Removed (1)"));
    assert_eq!(
        item_shape(&state, user, list).await,
        [("Maito".to_owned(), Some("Kylmä".to_owned()))]
    );

    let body = hx_call(post(format!("{base}/items/{candles}/restore"), vec![])).await;
    assert!(body.contains(&fragment_id) && !body.contains("Removed ("));

    // htmx rejections stay in the fragment, keeping the typed values.
    let body = hx_call(post(
        format!("{base}/items"),
        vec![("text", " ".to_owned()), ("quantity", "2".to_owned())],
    ))
    .await;
    assert!(body.contains(&fragment_id) && body.contains("must not be empty"));
    assert!(body.contains(r#"value="2""#));
    let body = hx_call(post(format!("{base}/favorites"), vec![])).await;
    assert!(body.contains("choose a favorite"));

    // Without htmx: a 303 back to the list on success.
    for (uri, form) in [
        (format!("{base}/items"), vec![("text", "Leipä".to_owned())]),
        (
            format!("{base}/favorites"),
            vec![
                ("product_id", maito.id.to_string()),
                ("quantity", "1 l".to_owned()),
            ],
        ),
        (
            format!("{base}/items/{candles}/category"),
            vec![("category_id", kylma.to_string())],
        ),
        (format!("{base}/items/{candles}/remove"), vec![]),
        (format!("{base}/items/{candles}/restore"), vec![]),
    ] {
        let resp = call_service(&app, post(uri.clone(), form).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{uri}");
        assert_eq!(
            resp.headers().get(header::LOCATION).expect("location"),
            base.as_str()
        );
    }
    assert_eq!(
        item_shape(&state, user, list).await,
        [
            ("Kynttilät".to_owned(), Some("Kylmä".to_owned())),
            ("Maito".to_owned(), Some("Kylmä".to_owned())),
            ("Leipä".to_owned(), None),
            ("Maito".to_owned(), Some("Kylmä".to_owned())),
        ]
    );

    // Without htmx, rejected input is the full page with 400.
    for (uri, form, message) in [
        (
            format!("{base}/items"),
            vec![("text", String::new())],
            "must not be empty",
        ),
        (
            format!("{base}/favorites"),
            vec![("product_id", "nope".to_owned())],
            "unknown product",
        ),
        (
            format!("{base}/items/{candles}/category"),
            vec![("category_id", String::new())],
            "choose a category",
        ),
    ] {
        let resp = call_service(&app, post(uri.clone(), form).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{uri}");
        let body = body_string(resp).await;
        assert!(body.contains("<html") && body.contains(message), "{uri}");
    }

    // The page shows the builder with the favorites picker.
    let page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&base)
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(page.contains("<html") && page.contains(&fragment_id));
    assert!(page.contains(&format!(r#"<option value="{}">Maito</option>"#, maito.id)));
}
