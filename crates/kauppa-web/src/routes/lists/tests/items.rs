//! MB-0004 (P5, P6): list items over HTTP, owner isolation and htmx fragments; MB-0021 (P2): editing a quantity; MB-0036 (P1–P3): one add field with favorite suggestions.

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
            .uri(&format!("{base}/items/{item}/quantity"))
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
    assert!(page.contains(&format!(r#"action="{base}/items/{oats}/quantity""#)));
    assert!(page.contains(&format!(r#"action="{base}/items/{candles}/quantity""#)));
    assert!(!page.contains(&format!("{base}/items/{sugar}/quantity")));
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
    // fragments.
    for uri in [base.clone(), format!("{base}/shop")] {
        for htmx in [false, true] {
            let body = get(uri.clone(), htmx).await;
            assert_eq!(body.contains("<html"), !htmx, "{uri} htmx={htmx}");
            if !htmx {
                assert!(body.contains("<title>Viikko — kauppa</title>"), "{uri}");
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
    for uri in [base.clone(), format!("{base}/shop")] {
        let body = get(uri.clone(), false).await;
        assert!(!body.contains(r#"class="ingredient-quantity""#), "{uri}");
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

/// MB-0036 fixture: an owner with favorites Maito (Kylmä) and Omena (Hevi),
/// a recipe-created (non-favorite) product Kaura, and a list. Another user
/// has a favorite "Juusto".
struct AddFixture {
    user: uuid::Uuid,
    list: uuid::Uuid,
    maito: uuid::Uuid,
    hevi: uuid::Uuid,
}

#[allow(clippy::expect_used)]
async fn add_fixture(state: &crate::state::AppState) -> AddFixture {
    use crate::routes::test_support::{make_category, make_list, make_recipe_with, make_user};

    let user = make_user(state, "u@example.com").await;
    let other = make_user(state, "other@example.com").await;
    let kylma = make_category(state, user, "Kylmä").await;
    let hevi = make_category(state, user, "Hevi").await;
    let maito = ProductRepo::create(&state.pool, user, "Maito", kylma)
        .await
        .expect("favorite")
        .id;
    ProductRepo::create(&state.pool, user, "Omena", hevi)
        .await
        .expect("favorite");
    make_recipe_with(state, user, "Puuro", kylma, &[("Kaura", Some("2 dl"))]).await;
    let theirs = make_category(state, other, "Maitotuotteet").await;
    ProductRepo::create(&state.pool, other, "Juusto", theirs)
        .await
        .expect("their favorite");
    let list = make_list(state, user, "Viikko").await;
    AddFixture {
        user,
        list,
        maito,
        hevi,
    }
}

/// MB-0036 P1: the list page has one add form — item text and an optional
/// quantity, posting to `/items` — and no separate favorite `<select>` form,
/// on the full page and in the fragment.
#[actix_web::test]
async fn single_add_form() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let f = add_fixture(&state).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{}", f.list);

    for htmx in [false, true] {
        let mut req = TestRequest::get().uri(&base).cookie(session.clone());
        if htmx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert_eq!(body.contains("<html"), !htmx);
        assert_eq!(
            body.matches(&format!(r#"action="{base}/items""#)).count(),
            1,
            "htmx={htmx}: one add form"
        );
        assert_eq!(body.matches(r#"name="text""#).count(), 1, "htmx={htmx}");
        assert!(!body.contains(&format!("{base}/favorites")), "htmx={htmx}");
        assert!(!body.contains(r#"name="product_id""#), "htmx={htmx}");
        assert!(!body.contains("Choose a favorite"), "htmx={htmx}");
    }
}

/// MB-0036 P2: the item field offers the user's favorites (not their
/// recipe-created products, not another user's favorites) as a `<datalist>`
/// that sits outside `#list-{id}`, so htmx fragments don't resend it.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn favorite_suggestions() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let f = add_fixture(&state).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{}", f.list);
    let input = r#"list="favorite-suggestions""#;
    let option = |name: &str| format!(r#"<option value="{name}"></option>"#);

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
    assert!(page.contains(input), "the item field uses the suggestions");
    let start = page
        .find(r#"<datalist id="favorite-suggestions">"#)
        .expect("a datalist of favorites");
    let end = start + page[start..].find("</datalist>").expect("closed");
    let datalist = &page[start..end];
    assert!(datalist.contains(&option("Maito")) && datalist.contains(&option("Omena")));
    assert!(!datalist.contains("Kaura"), "only favorites");
    assert!(!datalist.contains("Juusto"), "only the user's own");
    // Outside the swapped fragment, which comes first.
    let fragment = page
        .find(&format!(r#"id="list-{}""#, f.list))
        .expect("fragment");
    assert!(fragment < start);
    assert_eq!(page.matches("<datalist").count(), 1);

    // Fragments (a GET and a mutation) keep the field's link but don't
    // resend the datalist.
    let get = TestRequest::get().uri(&base);
    let post = TestRequest::post()
        .uri(&format!("{base}/items"))
        .set_form([("text", "Leipä")]);
    for req in [get, post] {
        let resp = call_service(
            &app,
            req.cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert!(body.contains(input) && !body.contains("<datalist"));
    }
}

/// MB-0036 P3: text whose `name_key` equals a favorite's adds that favorite
/// as `add_product` does (product link, its name and category, the quantity
/// kept); any other text — a recipe-created product's name included — adds a
/// free-form, uncategorized item. A favorite whose category was deleted
/// follows `add_product`: linked, uncategorized. htmx gets the fragment, a
/// plain post a 303.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_resolves_favorite() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::repo::CategoryRepo;

    use crate::routes::test_support::{
        body_string, login_request, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let f = add_fixture(&state).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{}", f.list);
    let post = |text: &str, quantity: &str, htmx: bool| {
        let mut req = TestRequest::post()
            .uri(&format!("{base}/items"))
            .cookie(session.clone())
            .set_form([("text", text.to_owned()), ("quantity", quantity.to_owned())]);
        if htmx {
            req = req.insert_header(("HX-Request", "true"));
        }
        call_service(&app, req.to_request())
    };

    // A favorite, typed in another case with spaces, over htmx.
    let resp = post("  mAITO ", "2 l", true).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(&format!(r#"id="list-{}""#, f.list)));
    assert!(body.contains("Maito") && body.contains("· Kylmä"));

    // Free text, as a plain post.
    let resp = post("Kynttilät", "6", false).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        base.as_str()
    );
    // A recipe-created product isn't a favorite: free text.
    assert_eq!(
        post("kaura", "", false).await.status(),
        StatusCode::SEE_OTHER
    );

    // A favorite whose category was deleted: linked, uncategorized.
    CategoryRepo::delete(&state.pool, f.user, f.hevi)
        .await
        .expect("delete category");
    assert_eq!(
        post("omena", "", false).await.status(),
        StatusCode::SEE_OTHER
    );

    let items: Vec<_> = ShoppingListRepo::items(&state.pool, f.user, f.list, false)
        .await
        .expect("items")
        .into_iter()
        .map(|i| (i.text, i.quantity, i.category.map(|c| c.name), i.product_id))
        .collect();
    assert_eq!(items.len(), 4, "{items:?}");
    let omena = items[3].3;
    assert!(omena.is_some(), "omena is linked to its favorite");
    assert_eq!(
        items,
        [
            (
                "Maito".to_owned(),
                Some("2 l".to_owned()),
                Some("Kylmä".to_owned()),
                Some(f.maito)
            ),
            ("Kynttilät".to_owned(), Some("6".to_owned()), None, None),
            ("kaura".to_owned(), None, None, None),
            ("Omena".to_owned(), None, None, omena),
        ]
    );

    // An empty field is still rejected as before.
    let resp = post("  ", "2", false).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(body_string(resp).await.contains("must not be empty"));
}
