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
            .uri(&format!("/favorites/{any}"))
            .set_form([("name", "Maito"), ("category_id", any_id.as_str())]),
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
    assert!(body.contains(r#"<span class="group-name">Hevi</span>"#) && body.contains("Omena"));

    // Create without htmx: 303 back to the page, its group open (MB-0059).
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
        Some(format!("/favorites?open=group-{kylma}").as_str())
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

    // Rename, reposition, and recategorize over htmx (one Save, MB-0058).
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{omena}"),
            &[("name", "Vihreä omena"), ("category_id", hevi)],
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
            &format!("/favorites/{banaani}"),
            &[("name", "Banaani"), ("category_id", kylma)],
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

/// MB-0059 P1: each favorites group (and Uncategorized) is a `<details>` with
/// a stable id, its summary the name and product count, and every group is
/// closed on a full page load. `?open=` (the redirect after a plain action)
/// opens the named groups only; unknown names are ignored.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn groups_collapsed_by_default() {
    use actix_web::test::{TestRequest, call_service};

    use kauppa_core::repo::CategoryRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    use super::units;

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let gone = make_category(&state, user, "Poistuva").await;
    for (name, category) in [
        ("Omena", hevi),
        ("Banaani", hevi),
        ("Maito", kylma),
        ("Mehu", gone),
    ] {
        ProductRepo::create(&state.pool, user, name, category)
            .await
            .expect("create product");
    }
    // Deleting a category leaves its favorites Uncategorized (RR-0001).
    CategoryRepo::delete(&state.pool, user, gone)
        .await
        .expect("delete category");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let page = |uri: &str| {
        TestRequest::get()
            .uri(uri)
            .cookie(session.clone())
            .to_request()
    };

    let body = body_string(call_service(&app, page("/favorites")).await).await;
    let (hevi_id, kylma_id) = (format!("group-{hevi}"), format!("group-{kylma}"));
    assert_eq!(
        units(&body),
        [
            (hevi_id.clone(), false),
            (kylma_id.clone(), false),
            ("group-uncategorized".to_owned(), false),
        ],
        "every group is a closed <details>, in category order"
    );
    for summary in [
        r#"<summary><span class="group-name">Hevi</span> <span class="group-count">2 products</span></summary>"#,
        r#"<summary><span class="group-name">Kylmä</span> <span class="group-count">1 product</span></summary>"#,
        r#"<summary><span class="group-name">Uncategorized</span> <span class="group-count">1 product</span></summary>"#,
    ] {
        assert!(body.contains(summary), "{summary}");
    }

    // A rename keeps the group's id (RR-0044 constraint 2).
    CategoryRepo::rename(&state.pool, user, hevi, "Hedelmät")
        .await
        .expect("rename category");
    let body = body_string(
        call_service(
            &app,
            page(&format!(
                "/favorites?open={kylma_id},nonsense,group-{}",
                Uuid::new_v4()
            )),
        )
        .await,
    )
    .await;
    assert_eq!(
        units(&body),
        [
            (hevi_id, false),
            (kylma_id, true),
            ("group-uncategorized".to_owned(), false),
        ]
    );
    assert!(body.contains(r#"<span class="group-name">Hedelmät</span>"#));
}

/// MB-0059 P2 (RR-0044 item 1): an action that stays inside one group swaps
/// only that group, rendered open; one that moves a product between groups,
/// adds one, or adds or removes a group swaps the whole catalog with the
/// touched groups open (`HX-Retarget: #favorites`); an error swaps the
/// catalog with the touched group open. Without htmx: a 303 naming the
/// touched groups, or the full page with them open.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn group_swaps() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    use super::{open_units, retarget};

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let leipa = make_category(&state, user, "Leipä").await;
    for (name, category) in [
        ("Omena", hevi),
        ("Banaani", hevi),
        ("Päärynä", hevi),
        ("Maito", kylma),
        ("Ruisleipä", leipa),
    ] {
        ProductRepo::create(&state.pool, user, name, category)
            .await
            .expect("create product");
    }
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true")).to_request();
    let (g_hevi, g_kylma) = (format!("group-{hevi}"), format!("group-{kylma}"));
    let (hevi_s, kylma_s) = (hevi.to_string(), kylma.to_string());
    let omena = product_id(&state, user, "Omena").await;
    let banaani = product_id(&state, user, "Banaani").await;
    let paaryna = product_id(&state, user, "Päärynä").await;
    let ruis = product_id(&state, user, "Ruisleipä").await;

    // The forms inside a group target the group.
    let page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/favorites")
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(page.contains(&format!(
        r#"hx-post="/favorites/{omena}/move" hx-target="closest .catalog-unit""#
    )));
    assert!(page.contains(&format!(
        r#"hx-post="/favorites/{omena}" hx-target="closest .catalog-unit""#
    )));

    // Within one group: ↑/↓, a Save that keeps the category, a delete that
    // leaves products. Each answers that group alone, open, not retargeted.
    let within = [
        post(&format!("/favorites/{banaani}/move"), &[("dir", "up")]),
        post(
            &format!("/favorites/{omena}"),
            &[("name", "Vihreä omena"), ("category_id", &hevi_s)],
        ),
        post(&format!("/favorites/{paaryna}/delete"), &[]),
    ];
    for req in within {
        let resp = call_service(&app, hx(req)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(retarget(&resp), None);
        let body = body_string(resp).await;
        assert!(
            body.trim_start().starts_with(&format!(
                r#"<details id="{g_hevi}" class="product-group catalog-unit" open>"#
            )),
            "{body}"
        );
        assert!(!body.contains(r#"id="favorites""#) && !body.contains("Maito"));
    }
    assert_eq!(
        super::catalog_shape(&state, user).await[0],
        (
            Some("Hevi".to_owned()),
            vec!["Banaani".to_owned(), "Vihreä omena".to_owned()]
        )
    );

    // Across groups: the whole catalog, retargeted, with the touched groups
    // open and the others closed.
    let cases: [(TestRequest, Vec<&str>); 4] = [
        // A Save that changes the category: the old and the new group.
        (
            post(
                &format!("/favorites/{banaani}"),
                &[("name", "Banaani"), ("category_id", &kylma_s)],
            ),
            vec![&g_hevi, &g_kylma],
        ),
        // "No category": the old group and Uncategorized (which appears).
        (
            post(
                &format!("/favorites/{omena}"),
                &[("name", "Vihreä omena"), ("category_id", "none")],
            ),
            vec!["group-uncategorized"],
        ),
        // Adding a favorite: its group.
        (
            post(
                "/favorites",
                &[("name", "Kerma"), ("category_id", &kylma_s)],
            ),
            vec![&g_kylma],
        ),
        // Deleting a group's last product: the group is gone, none open.
        (post(&format!("/favorites/{ruis}/delete"), &[]), vec![]),
    ];
    for (req, open) in cases {
        let resp = call_service(&app, hx(req)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(retarget(&resp).as_deref(), Some("#favorites"));
        let body = body_string(resp).await;
        assert!(body.contains(r#"<div id="favorites""#) && !body.contains("<html"));
        assert_eq!(open_units(&body), open, "{body}");
    }
    // Hevi was emptied by the "No category" move and Leipä by the delete,
    // so neither is open: only groups that exist are.
    assert_eq!(
        super::catalog_shape(&state, user).await,
        [
            (
                Some("Kylmä".to_owned()),
                vec!["Maito".to_owned(), "Banaani".to_owned(), "Kerma".to_owned()]
            ),
            (None, vec!["Vihreä omena".to_owned()]),
        ]
    );

    // An error: the catalog with the error and the touched group open.
    let maito = product_id(&state, user, "Maito").await;
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{maito}"),
            &[("name", "Kerma"), ("category_id", &kylma_s)],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(retarget(&resp).as_deref(), Some("#favorites"));
    let body = body_string(resp).await;
    assert!(body.contains(r#"role="alert""#) && body.contains("product already exists"));
    assert_eq!(open_units(&body), std::slice::from_ref(&g_kylma));

    // Without htmx: a 303 naming the touched groups; following it opens
    // them. A rejection is the full page with the group open.
    let resp = call_service(
        &app,
        post(&format!("/favorites/{maito}/move"), &[("dir", "down")]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .expect("location");
    assert_eq!(location, format!("/favorites?open={g_kylma}"));
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&location)
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(body.contains("<html"));
    assert_eq!(open_units(&body), std::slice::from_ref(&g_kylma));

    let resp = call_service(
        &app,
        post(
            &format!("/favorites/{maito}"),
            &[("name", "Kerma"), ("category_id", &kylma_s)],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("product already exists"));
    assert_eq!(open_units(&body), [g_kylma]);
}
