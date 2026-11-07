//! MB-0002 (P1, P6) and MB-0010 (P4): favorites — categories, CRUD, auth, case-insensitive names.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::ProductRepo;

use crate::state::AppState;

use super::catalog_shape;

#[allow(clippy::expect_used)]
async fn product_id(state: &AppState, user: Uuid, name: &str) -> Uuid {
    ProductRepo::list_for_owner(&state.pool, user)
        .await
        .expect("list products")
        .into_iter()
        .flat_map(|g| g.products)
        .find(|p| p.name == name)
        .expect("product exists")
        .id
}

/// P1: the server rejects a favorite with a missing, malformed, or foreign
/// category — even when a client skips the `<select required>` — and creates
/// nothing.
#[actix_web::test]
async fn create_requires_owned_category() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let theirs = make_category(&state, other, "Pakasteet").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let submit = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri("/favorites")
            .cookie(session.clone())
            .set_form(form)
    };
    let name = || ("name", "Maito".to_owned());

    let rejected = [
        (vec![name()], "choose a category"),
        (
            vec![name(), ("category_id", String::new())],
            "choose a category",
        ),
        (
            vec![name(), ("category_id", "not-a-uuid".to_owned())],
            "unknown category",
        ),
        (
            vec![name(), ("category_id", theirs.to_string())],
            "unknown category",
        ),
        (
            vec![name(), ("category_id", Uuid::new_v4().to_string())],
            "unknown category",
        ),
    ];
    for (form, message) in rejected {
        let resp = call_service(&app, submit(form.clone()).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{form:?}");
        let body = body_string(resp).await;
        assert!(body.contains("<html"), "full page without htmx");
        assert!(body.contains(message), "{form:?} should say {message:?}");
    }

    // Over htmx the rejection comes back as the fragment, carrying the error
    // and keeping the typed name for correction.
    let resp = call_service(
        &app,
        submit(vec![name(), ("category_id", theirs.to_string())])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        !body.contains("<html"),
        "htmx response should be a fragment"
    );
    assert!(body.contains(r#"role="alert""#) && body.contains("unknown category"));
    assert!(body.contains(r#"value="Maito""#));

    assert!(
        catalog_shape(&state, user).await.is_empty(),
        "nothing created"
    );
    assert!(
        catalog_shape(&state, other).await.is_empty(),
        "nothing created"
    );

    // An owned category is accepted.
    let resp = call_service(
        &app,
        submit(vec![name(), ("category_id", hevi.to_string())]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        catalog_shape(&state, user).await,
        [(Some("Hevi".to_owned()), vec!["Maito".to_owned()])]
    );
}

/// P6: `GET /favorites` renders the grouped catalog; create / edit / delete
/// return fragments over htmx and redirect to the full page otherwise;
/// unauthenticated requests are rejected.
#[actix_web::test]
async fn crud_and_auth() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);

    // Unauthenticated: every route is rejected before touching anything.
    let any = Uuid::new_v4();
    let any_id = any.to_string();
    let anonymous = [
        TestRequest::get().uri("/favorites"),
        TestRequest::post()
            .uri("/favorites")
            .set_form([("name", "Maito"), ("category_id", any_id.as_str())]),
        TestRequest::post()
            .uri(&format!("/favorites/{any}/rename"))
            .set_form([("name", "Maito")]),
        TestRequest::post()
            .uri(&format!("/favorites/{any}/category"))
            .set_form([("category_id", any_id.as_str())]),
        TestRequest::post()
            .uri(&format!("/favorites/{any}/move"))
            .set_form([("dir", "up")]),
        TestRequest::post().uri(&format!("/favorites/{any}/delete")),
    ];
    for req in anonymous {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true")).to_request();
    let (hevi, kylma) = (hevi.to_string(), kylma.to_string());
    let (hevi, kylma) = (hevi.as_str(), kylma.as_str());

    // The page: a full document with the add form's category picker.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/favorites")
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("Favorites"));
    assert!(body.contains(&format!(r#"<option value="{kylma}""#)));

    // Create over htmx: the fragment, grouped under the category.
    let resp = call_service(
        &app,
        hx(post(
            "/favorites",
            &[("name", "Omena"), ("category_id", hevi)],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        !body.contains("<html"),
        "htmx response should be a fragment"
    );
    assert!(body.contains(r#"id="favorites""#));
    assert!(body.contains("<h2>Hevi</h2>") && body.contains("Omena"));

    // Create without htmx: 303 back to the page.
    let resp = call_service(
        &app,
        post("/favorites", &[("name", "Maito"), ("category_id", kylma)]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/favorites")
    );
    call_service(
        &app,
        hx(post(
            "/favorites",
            &[("name", "Banaani"), ("category_id", hevi)],
        )),
    )
    .await;

    // A duplicate without htmx: the full page with the error, as 409.
    let resp = call_service(
        &app,
        post("/favorites", &[("name", "Maito"), ("category_id", hevi)]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("product already exists: Maito"));

    let omena = product_id(&state, user, "Omena").await;
    let banaani = product_id(&state, user, "Banaani").await;
    let maito = product_id(&state, user, "Maito").await;

    // Rename, reposition, and recategorize over htmx.
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{omena}/rename"),
            &[("name", "Vihreä omena")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Vihreä omena"));

    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{banaani}/move"),
            &[("dir", "up")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let (moved, other) = (body.find("Banaani"), body.find("Vihreä omena"));
    assert!(
        moved.is_some() && moved < other,
        "Banaani moved above Vihreä omena"
    );

    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{banaani}/category"),
            &[("category_id", kylma)],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        catalog_shape(&state, user).await,
        [
            (Some("Hevi".to_owned()), vec!["Vihreä omena".to_owned()]),
            (
                Some("Kylmä".to_owned()),
                vec!["Maito".to_owned(), "Banaani".to_owned()]
            ),
        ]
    );

    // Edits without htmx redirect back to the page.
    let resp = call_service(
        &app,
        post(&format!("/favorites/{maito}/move"), &[("dir", "down")]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Delete over htmx; the product is gone from the fragment.
    let resp = call_service(&app, hx(post(&format!("/favorites/{maito}/delete"), &[]))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_string(resp).await.contains("Maito"));
    assert_eq!(
        catalog_shape(&state, user).await,
        [
            (Some("Hevi".to_owned()), vec!["Vihreä omena".to_owned()]),
            (Some("Kylmä".to_owned()), vec!["Banaani".to_owned()]),
        ]
    );
}

/// MB-0010 P6: adding a favorite whose name differs only by case from an
/// existing one shows the "already exists" error, over htmx and without it,
/// and creates nothing.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn case_duplicate_rejected() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    for name in ["maito", "Äyräs"] {
        ProductRepo::create(&state.pool, user, name, kylma)
            .await
            .expect("create product");
    }
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let hevi = hevi.to_string();
    let post = |name: &str| {
        TestRequest::post()
            .uri("/favorites")
            .cookie(session.clone())
            .set_form([("name", name), ("category_id", hevi.as_str())])
    };

    // Over htmx: the fragment with the alert.
    let resp = call_service(
        &app,
        post("Maito")
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        !body.contains("<html"),
        "htmx response should be a fragment"
    );
    assert!(body.contains(r#"role="alert""#) && body.contains("product already exists"));

    // Without htmx: the full page with 409.
    let resp = call_service(&app, post("ÄYRÄS").to_request()).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("product already exists"));

    assert_eq!(
        catalog_shape(&state, user).await,
        [(
            Some("Kylmä".to_owned()),
            vec!["maito".to_owned(), "Äyräs".to_owned()]
        )]
    );
}
