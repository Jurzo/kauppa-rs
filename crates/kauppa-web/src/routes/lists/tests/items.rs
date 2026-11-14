//! MB-0004 (P5, P6): list items over HTTP, owner isolation and htmx fragments; MB-0021 (P2): editing a quantity;
//! MB-0037 (P1, P2): edit sections start closed, the uncategorized cue stays.

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

    let writes: [(String, Vec<(&str, String)>); 4] = [
        (format!("{base}/items"), vec![("text", "Omena".to_owned())]),
        (
            format!("{base}/items/{bread}"),
            vec![
                ("category_id", kylma.to_string()),
                ("quantity", "2".to_owned()),
            ],
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

    // The add-by-id route is gone (MB-0048 P2, RR-0036 sub-answer 2): even
    // the owner, posting another user's favorite, gets a 404 and nothing is
    // added. The add field only matches the owner's own favorites by name.
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
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(item_shape(&state, user, list).await.len(), 1);
}

/// MB-0004 P6: add-item (free text, or a favorite by name since MB-0036),
/// assign-category, and remove (and restore) answer htmx with the `#list-{id}` fragment and plain requests with
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
    ProductRepo::create(&state.pool, user, "Maito", kylma)
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
        format!("{base}/items"),
        vec![("text", "maito".to_owned())],
    ))
    .await;
    assert!(body.contains(&fragment_id) && body.contains("Maito"));

    let body = hx_call(post(
        format!("{base}/items/{candles}"),
        vec![
            ("category_id", koti.to_string()),
            ("quantity", "6".to_owned()),
        ],
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

    // Without htmx: a 303 back to the list on success.
    for (uri, form) in [
        (format!("{base}/items"), vec![("text", "Leipä".to_owned())]),
        (
            format!("{base}/items"),
            vec![("text", "Maito".to_owned()), ("quantity", "1 l".to_owned())],
        ),
        (
            format!("{base}/items/{candles}"),
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
            // A blank category saves the quantity only (MB-0052,
            // `item_edit`); a malformed one is rejected.
            format!("{base}/items/{candles}"),
            vec![("category_id", "nope".to_owned())],
            "unknown category",
        ),
    ] {
        let resp = call_service(&app, post(uri.clone(), form).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{uri}");
        let body = body_string(resp).await;
        assert!(body.contains("<html") && body.contains(message), "{uri}");
    }

    // The page shows the builder with the favorites as suggestions (MB-0036).
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
    assert!(page.contains(r#"<option value="Maito"></option>"#));
}

/// MB-0021 P2: an active item's quantity is edited inline on the list page.
/// htmx gets the `#list-{id}` fragment, a plain request a 303 back to the
/// list; the new quantity shows on the list page and in the store view, and
/// an empty value clears it. A removed item is a 404.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn edit_quantity() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_list, make_recipe_with, make_user,
        session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let recipe = make_recipe_with(&state, user, "Puuro", kylma, &[("Kaura", Some("2 dl"))]).await;
    let list = make_list(&state, user, "Viikko").await;
    let oats = ShoppingListRepo::add_recipe(&state.pool, user, list, recipe, 1.0)
        .await
        .expect("add recipe")[0]
        .id;
    let candles = ShoppingListRepo::add_item(&state.pool, user, list, "Kynttilät", None)
        .await
        .expect("item")
        .id;
    let sugar = ShoppingListRepo::add_item(&state.pool, user, list, "Sokeri", Some("1 kg"))
        .await
        .expect("item")
        .id;
    ShoppingListRepo::remove_item(&state.pool, user, list, sugar)
        .await
        .expect("remove");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{list}");
    let fragment_id = format!(r#"id="list-{list}""#);
    let shown = |q: &str| format!(r#"<span class="ingredient-quantity">{q}</span>"#);
    let post = |item: uuid::Uuid, form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&format!("{base}/items/{item}"))
            .cookie(session.clone())
            .set_form(form)
    };
    let quantities = || async {
        ShoppingListRepo::items(&state.pool, user, list, false)
            .await
            .expect("items")
            .into_iter()
            .map(|i| (i.text, i.quantity))
            .collect::<Vec<_>>()
    };
    let get = |uri: String, htmx: bool| {
        let app = &app;
        let session = session.clone();
        async move {
            let mut req = TestRequest::get().uri(&uri).cookie(session);
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::OK, "{uri}");
            body_string(resp).await
        }
    };

    // Each active item offers the form, prefilled; the removed one doesn't.
    let page = get(base.clone(), false).await;
    assert!(page.contains(&format!(r#"action="{base}/items/{oats}""#)));
    assert!(page.contains(&format!(r#"action="{base}/items/{candles}""#)));
    assert!(!page.contains(&format!(r#"action="{base}/items/{sugar}""#)));
    assert!(page.contains(r#"value="2 dl""#));

    // htmx: the fragment, with the new (trimmed) quantity.
    let resp = call_service(
        &app,
        post(oats, vec![("quantity", "  3 dl ".to_owned())])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(&fragment_id));
    assert!(body.contains(&shown("3 dl")) && !body.contains(&shown("2 dl")));
    assert!(body.contains(r#"value="3 dl""#));

    // Plain: a 303 back to the list; an item without a quantity gets one.
    let resp = call_service(
        &app,
        post(candles, vec![("quantity", "2 kpl".to_owned())]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        base.as_str()
    );
    assert_eq!(
        quantities().await,
        [
            ("Kaura".to_owned(), Some("3 dl".to_owned())),
            ("Kynttilät".to_owned(), Some("2 kpl".to_owned())),
        ]
    );

    // The list page and the store view show both, as full pages and as
    // fragments. The store view shows "(qty)" after the name (MB-0038).
    let shop_shown = |q: &str| format!(r#"<span class="shop-quantity">({q})</span>"#);
    for (uri, shown) in [
        (base.clone(), &shown as &dyn Fn(&str) -> String),
        (format!("{base}/shop"), &shop_shown),
    ] {
        for htmx in [false, true] {
            let body = get(uri.clone(), htmx).await;
            assert_eq!(body.contains("<html"), !htmx, "{uri} htmx={htmx}");
            if !htmx {
                assert!(
                    body.contains("<title>Viikko — kauppalista.app</title>"),
                    "{uri}"
                );
            }
            assert!(body.contains(&shown("3 dl")), "{uri} htmx={htmx}");
            assert!(body.contains(&shown("2 kpl")), "{uri} htmx={htmx}");
        }
    }

    // An empty value clears it, over htmx and plain; so does a missing one.
    let resp = call_service(
        &app,
        post(candles, vec![("quantity", "   ".to_owned())])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(&fragment_id) && !body.contains(&shown("2 kpl")));
    let resp = call_service(
        &app,
        post(oats, vec![("quantity", String::new())]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        quantities().await,
        [("Kaura".to_owned(), None), ("Kynttilät".to_owned(), None)]
    );
    call_service(
        &app,
        post(oats, vec![("quantity", "1 dl".to_owned())]).to_request(),
    )
    .await;
    let resp = call_service(&app, post(oats, vec![]).to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(quantities().await[0], ("Kaura".to_owned(), None));
    for (uri, class) in [
        (base.clone(), r#"class="ingredient-quantity""#),
        (format!("{base}/shop"), r#"class="shop-quantity""#),
    ] {
        let body = get(uri.clone(), false).await;
        assert!(!body.contains(class), "{uri}");
    }

    // A removed or unknown item is a 404, and stays as it was.
    for item in [sugar, uuid::Uuid::new_v4()] {
        for htmx in [false, true] {
            let mut req = post(item, vec![("quantity", "9".to_owned())]);
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "htmx={htmx}");
        }
    }
    let removed = ShoppingListRepo::items(&state.pool, user, list, true)
        .await
        .expect("removed");
    assert_eq!(removed[0].quantity.as_deref(), Some("1 kg"));
}

/// Every `<details class="product-edit" …>` opening tag in `body`, up to and
/// including its `>`.
fn edit_section_tags(body: &str) -> Vec<&str> {
    body.match_indices(r#"<details class="product-edit""#)
        .map(|(start, _)| {
            let rest = &body[start..];
            rest.find('>').map_or(rest, |end| &rest[..=end])
        })
        .collect()
}

/// MB-0037 P1: on the list page and in the fragment htmx gets after adding an
/// item, no item's edit section is rendered `open` — uncategorized or not,
/// with or without the user having categories.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn edit_sections_start_closed() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    // One owner with categories, one without.
    let with = make_user(&state, "with@example.com").await;
    let kylma = make_category(&state, with, "Kylmä").await;
    let without = make_user(&state, "without@example.com").await;

    for (user, email, has_categories) in [
        (with, "with@example.com", true),
        (without, "without@example.com", false),
    ] {
        let list = ShoppingListRepo::create(&state.pool, user, "Viikko")
            .await
            .expect("list")
            .id;
        ShoppingListRepo::add_item(&state.pool, user, list, "Leipä", None)
            .await
            .expect("uncategorized item");
        let milk = ShoppingListRepo::add_item(&state.pool, user, list, "Maito", None)
            .await
            .expect("item")
            .id;
        if has_categories {
            ShoppingListRepo::assign_category(&state.pool, user, list, milk, kylma)
                .await
                .expect("categorized item");
        }
        let session = session_cookie(&call_service(&app, login_request(email).to_request()).await);
        let base = format!("/lists/{list}");

        let resp = call_service(
            &app,
            TestRequest::get()
                .uri(&base)
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let page = body_string(resp).await;
        assert!(page.contains("<html"), "full page");

        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&format!("{base}/items"))
                .cookie(session)
                .insert_header(("HX-Request", "true"))
                .set_form([("text", "Kynttilät")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let fragment = body_string(resp).await;
        assert!(!fragment.contains("<html"));
        assert!(fragment.contains(&format!(r#"id="list-{list}""#)));

        for (what, body, items) in [("page", &page, 2), ("fragment", &fragment, 3)] {
            let tags = edit_section_tags(body);
            assert_eq!(
                tags.len(),
                items,
                "{email} {what}: an edit section per item"
            );
            for tag in tags {
                assert_eq!(
                    tag, r#"<details class="product-edit">"#,
                    "{email} {what}: edit section starts closed"
                );
            }
        }
    }
}

/// MB-0037 P2: with the edit section closed, an uncategorized item's summary
/// still tells the owner it needs a category; a categorized one says "Edit".
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn uncategorized_summary_cue() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let list = ShoppingListRepo::create(&state.pool, user, "Viikko")
        .await
        .expect("list")
        .id;
    let milk = ShoppingListRepo::add_item(&state.pool, user, list, "Maito", None)
        .await
        .expect("item")
        .id;
    ShoppingListRepo::assign_category(&state.pool, user, list, milk, kylma)
        .await
        .expect("categorized item");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{list}");

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/items"))
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .set_form([("text", "Kynttilät")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let fragment = body_string(resp).await;
    let resp = call_service(
        &app,
        TestRequest::get().uri(&base).cookie(session).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let page = body_string(resp).await;

    for (what, body) in [("fragment", &fragment), ("page", &page)] {
        assert!(
            body.contains(
                r#"<summary>Choose category<span class="visually-hidden"> Kynttilät</span></summary>"#
            ),
            "{what}: uncategorized cue"
        );
        assert!(
            body.contains(r#"<summary>Edit<span class="visually-hidden"> Maito</span></summary>"#),
            "{what}: categorized summary"
        );
    }
}
