//! MB-0061 P1 and P6 (RR-0048): sharing, changing and revoking; no new
//! account disclosure.

use actix_web::http::{StatusCode, header};

use kauppa_core::repo::{CatalogShareRepo, CategoryRepo, ProductRepo};

/// P1: the sharer shares by email with switches, sees who they share with,
/// changes the switches and revokes; revoking stops further imports and
/// never removes copies already imported.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn share_and_revoke() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let me = make_user(&state, "me@example.com").await;
    let friend = make_user(&state, "friend@example.com").await;
    let hevi = CategoryRepo::create(&state.pool, me, "Hevi")
        .await
        .expect("category")
        .id;
    ProductRepo::create(&state.pool, me, "Omena", hevi)
        .await
        .expect("favorite");
    let mine =
        session_cookie(&call_service(&app, login_request("me@example.com").to_request()).await);
    let theirs =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);
    let page = |cookie: &actix_web::cookie::Cookie<'static>| {
        TestRequest::get()
            .uri("/sharing")
            .cookie(cookie.clone())
            .to_request()
    };

    // Empty to start; the form has both switches ticked.
    let body = body_string(call_service(&app, page(&mine)).await).await;
    assert!(
        body.contains("You don&#x27;t share with anyone.")
            || body.contains("You don't share with anyone.")
    );
    assert!(
        body.contains(r#"name="favorites" value="1" checked"#)
            && body.contains(r#"name="recipes" value="1" checked"#)
    );

    // Share favorites only.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/sharing")
            .cookie(mine.clone())
            .set_form([("email", "FRIEND@example.com"), ("favorites", "1")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/sharing?shared=1")
    );
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/sharing?shared=1")
                .cookie(mine.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(body.contains("If an account uses that email, you now share with it."));
    assert!(body.contains("People you share with (1)") && body.contains("friend@example.com"));
    let share = CatalogShareRepo::catalog_shares(&state.pool, me)
        .await
        .expect("shares")
        .pop()
        .expect("one share");
    assert_eq!((share.favorites, share.recipes), (true, false));

    // The friend sees it, and can browse.
    let body = body_string(call_service(&app, page(&theirs)).await).await;
    let browse = format!("/sharing/from/{me}/favorites");
    assert!(body.contains(&format!(r#"href="{browse}""#)), "{body}");
    let get = |cookie: &actix_web::cookie::Cookie<'static>, uri: &str| {
        TestRequest::get()
            .uri(uri)
            .cookie(cookie.clone())
            .to_request()
    };
    assert_eq!(
        call_service(&app, get(&theirs, &browse)).await.status(),
        StatusCode::OK
    );

    // Change the switches: recipes only; favorites are then 404.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/sharing/{}/update", share.id))
            .cookie(mine.clone())
            .set_form([("recipes", "1")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        call_service(&app, get(&theirs, &browse)).await.status(),
        StatusCode::NOT_FOUND
    );

    // Favorites again; the friend imports Omena; then revoke.
    call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/sharing/{}/update", share.id))
            .cookie(mine.clone())
            .set_form([("favorites", "1")])
            .to_request(),
    )
    .await;
    CategoryRepo::create(&state.pool, friend, "hevi")
        .await
        .expect("category");
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{browse}/import"))
            .cookie(theirs.clone())
            .set_form([("all", "1")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/sharing/{}/revoke", share.id))
            .cookie(mine.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        call_service(&app, get(&theirs, &browse)).await.status(),
        StatusCode::NOT_FOUND
    );
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{browse}/import"))
            .cookie(theirs.clone())
            .set_form([("all", "1")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "no further imports");
    let kept: Vec<String> = ProductRepo::list_for_owner(&state.pool, friend)
        .await
        .expect("favorites")
        .into_iter()
        .flat_map(|g| g.products)
        .map(|p| p.name)
        .collect();
    assert_eq!(kept, ["Omena"], "the copy stays");

    // Both switches off deletes a share; only the owner can change or
    // revoke one.
    let share = CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", true, true)
        .await
        .expect("share")
        .expect("matched");
    for uri in [
        format!("/sharing/{}/update", share.id),
        format!("/sharing/{}/revoke", share.id),
    ] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&uri)
                .cookie(theirs.clone())
                .set_form([("favorites", "1")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/sharing/{}/update", share.id))
            .cookie(mine.clone())
            .set_form(Vec::<(&str, &str)>::new())
            .to_request(),
    )
    .await;
    assert_eq!(
        CatalogShareRepo::catalog_shares(&state.pool, me)
            .await
            .expect("shares"),
        []
    );
}

/// P6: the share form answers the same for a matching, a non-matching and
/// the sharer's own email (status, location, then the same notice page); the
/// recipient sees the sharer's display name, never their email; an empty
/// email is the form's own error.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn no_new_account_disclosure() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "me@example.com").await;
    make_user(&state, "friend@example.com").await;
    let mine =
        session_cookie(&call_service(&app, login_request("me@example.com").to_request()).await);
    let theirs =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);

    let mut answers = Vec::new();
    for email in ["nobody@example.com", "friend@example.com", "me@example.com"] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri("/sharing")
                .cookie(mine.clone())
                .set_form([("email", email), ("favorites", "1"), ("recipes", "1")])
                .to_request(),
        )
        .await;
        let location = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        answers.push((resp.status(), location, body_string(resp).await));
    }
    assert!(answers.windows(2).all(|w| w[0] == w[1]), "{answers:?}");

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/sharing")
            .cookie(mine.clone())
            .set_form([("email", " "), ("favorites", "1")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(body_string(resp).await.contains(r#"role="alert""#));

    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/sharing")
                .cookie(theirs.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    let with_you = &body[body.find("Shared with you").expect("section")..];
    assert!(with_you.contains("Test User"), "the sharer's display name");
    assert!(
        !with_you.contains("me@example.com"),
        "never the sharer's email"
    );
}
