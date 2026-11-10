//! MB-0036 (P1–P3): one add field on the list page, with the owner's favorites
//! as suggestions and name matches added as that favorite.

use actix_web::http::{StatusCode, header};

use kauppa_core::repo::{ProductRepo, ShoppingListRepo};

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
