//! MB-0007 (P3, P5): opening and resuming lists, owner isolation, htmx and auth.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;

use crate::state::AppState;

/// List names in index order.
#[allow(clippy::expect_used)]
async fn list_names(state: &AppState, user: Uuid) -> Vec<String> {
    ShoppingListRepo::list_for_owner(&state.pool, user)
        .await
        .expect("list lists")
        .into_iter()
        .map(|l| l.name)
        .collect()
}

/// P3: opening a list by id loads it to continue — as a page, after boosted
/// navigation, and as a fragment — while another user's list, or no session,
/// is rejected.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn open_resume_owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let list = ShoppingListRepo::create(&state.pool, user, "Viikonloppu")
        .await
        .expect("create list");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let open = || TestRequest::get().uri(&format!("/lists/{}", list.id));

    // The index links to it, and opening it shows it.
    let my_index = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/lists")
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(my_index.contains(&format!(r#"href="/lists/{}""#, list.id)));
    let resp = call_service(&app, open().cookie(session.clone()).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("Viikonloppu"));
    assert!(body.contains(&format!(r#"data-list-id="{}""#, list.id)));

    // Boosted navigation gets the page; a plain htmx request the fragment.
    let boosted = body_string(
        call_service(
            &app,
            open()
                .cookie(session.clone())
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
            open()
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(!fragment.contains("<html"));
    assert!(fragment.contains(&format!(r#"id="list-{}""#, list.id)));

    // Another user: 404, for the page and the fragment; never on their index.
    for hx in [false, true] {
        let mut req = open().cookie(intruder.clone());
        if hx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "htmx: {hx}");
    }
    let their_index = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/lists")
                .cookie(intruder.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(!their_index.contains(&list.id.to_string()));

    // A missing list is a 404 too; no session is a 401.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/lists/{}", Uuid::new_v4()))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = call_service(&app, open().to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// P5: create, rename, and delete answer htmx with the `#lists` fragment and
/// plain requests with a 303 (or the full page when the input was rejected);
/// another user's list is a 404 and stays unchanged; every endpoint rejects a
/// request without a session.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn lifecycle_htmx_and_auth() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let post = |uri: String, name: &str| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form([("name", name.to_owned())])
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true"));

    // Create over htmx: the fragment with the new list.
    let resp = call_service(&app, hx(post("/lists".to_owned(), "Arki")).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(r#"id="lists""#));
    assert!(body.contains("Arki"));

    // Create without htmx: a 303 into the new list.
    let resp = call_service(&app, post("/lists".to_owned(), "Juhlat").to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .expect("location")
        .to_str()
        .expect("ascii")
        .to_owned();
    let juhlat: Uuid = location
        .strip_prefix("/lists/")
        .expect("list url")
        .parse()
        .expect("list id");
    assert_eq!(list_names(&state, user).await, ["Juhlat", "Arki"]);

    // A rejected name: in the fragment over htmx, the full page with 400
    // otherwise; nothing is created.
    let resp = call_service(&app, hx(post("/lists".to_owned(), "  ")).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("must not be empty"));
    let resp = call_service(&app, post("/lists".to_owned(), "").to_request()).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("must not be empty"));
    assert_eq!(list_names(&state, user).await.len(), 2);

    // Rename: fragment over htmx, 303 otherwise; it moves to the top.
    let arki = ShoppingListRepo::list_for_owner(&state.pool, user)
        .await
        .expect("lists")
        .into_iter()
        .find(|l| l.name == "Arki")
        .expect("Arki")
        .id;
    let resp = call_service(
        &app,
        hx(post(format!("/lists/{arki}/rename"), "Arkiruoka")).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Arkiruoka"));
    let resp = call_service(
        &app,
        post(format!("/lists/{juhlat}/rename"), "Synttärit").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(list_names(&state, user).await, ["Synttärit", "Arkiruoka"]);

    // Another user can't rename or delete; nothing changes.
    for uri in [
        format!("/lists/{arki}/rename"),
        format!("/lists/{arki}/delete"),
    ] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&uri)
                .cookie(intruder.clone())
                .set_form([("name", "Kaapattu")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    assert_eq!(list_names(&state, user).await, ["Synttärit", "Arkiruoka"]);

    // Delete: fragment over htmx, 303 otherwise.
    let resp = call_service(
        &app,
        hx(TestRequest::post()
            .uri(&format!("/lists/{arki}/delete"))
            .cookie(session.clone()))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(r#"id="lists""#) && !body.contains("Arkiruoka"));
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{juhlat}/delete"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(list_names(&state, user).await, Vec::<String>::new());

    // No session: every endpoint is rejected.
    let id = Uuid::new_v4();
    for req in [
        TestRequest::get().uri("/lists"),
        TestRequest::post().uri("/lists").set_form([("name", "x")]),
        TestRequest::get().uri(&format!("/lists/{id}")),
        TestRequest::post()
            .uri(&format!("/lists/{id}/rename"))
            .set_form([("name", "x")]),
        TestRequest::post().uri(&format!("/lists/{id}/delete")),
    ] {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
