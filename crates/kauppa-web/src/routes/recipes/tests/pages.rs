//! MB-0016 (P2–P4): full pages show their own titles and drafts.

use actix_web::http::StatusCode;

use kauppa_core::repo::RecipeRepo;

#[cfg(test)]
use crate::routes::test_support::make_recipe;

/// MB-0016 P2: the `/recipes` index shows each card with its own title; the
/// page title is still "Recipes".
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn index_shows_titles() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let pancakes = make_recipe(&state, user, "Pannukakku").await;
    let porridge = make_recipe(&state, user, "Puuro").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/recipes")
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Recipes — kauppa</title>"));
    for (id, title) in [(pancakes, "Pannukakku"), (porridge, "Puuro")] {
        let card = format!(r#"<a href="/recipes/{id}">{title}</a>"#);
        assert!(body.contains(&card), "{card}");
    }
}

/// MB-0016 P3: the new-recipe title field is empty on a normal load, and
/// after a rejected submission without htmx it keeps what the user typed.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn new_recipe_draft() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "u@example.com").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    // The new-recipe title input; its placeholder is unique on the page. (The
    // tag spans two lines, re-indented where the partial is included.)
    let input = |value: &str| format!(r#"placeholder="e.g. Pannukakku" value="{value}" />"#);

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/recipes")
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Recipes — kauppa</title>"));
    assert!(body.contains(&input("")), "empty on a normal load");

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/recipes")
            .cookie(session)
            .set_form([("title", " ")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Recipes — kauppa</title>"));
    assert!(body.contains("must not be empty"));
    assert!(body.contains(&input(" ")), "keeps the typed value");
}

/// MB-0016 P4: after a rejected details edit without htmx, the title field
/// holds the typed title; the page heading still shows the stored one.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn details_draft_survives_rejection() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let recipe = make_recipe(&state, user, "Pannukakku").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}"))
            .cookie(session)
            .set_form([("title", "Uusi nimi"), ("servings", "abc")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Pannukakku — kauppa</title>"));
    assert!(body.contains("<h1>Pannukakku</h1>"));
    assert!(
        body.contains(r#"<input name="title" type="text" required value="Uusi nimi" />"#),
        "keeps the typed title"
    );
    assert_eq!(
        RecipeRepo::get(&state.pool, user, recipe)
            .await
            .expect("get recipe")
            .recipe
            .title,
        "Pannukakku",
        "nothing saved"
    );
}
