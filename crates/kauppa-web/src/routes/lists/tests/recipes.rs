//! MB-0005 (P5, P6) and MB-0016 (P1): adding a recipe from the list page.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;

#[cfg(test)]
use crate::routes::test_support::make_recipe_with;

use super::item_shape;

/// MB-0005 P5: a user can't add from another user's recipe, nor onto another
/// user's list — over htmx or not — and nothing is added anywhere.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_recipe_owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let mine = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "Kylmä").await;
    let my_recipe = make_recipe_with(&state, user, "Puuro", mine, &[("Maito", Some("5 dl"))]).await;
    let their_recipe = make_recipe_with(
        &state,
        other,
        "Salaatti",
        theirs,
        &[("Feta", Some("1 pkt"))],
    )
    .await;
    let my_list = ShoppingListRepo::create(&state.pool, user, "Oma")
        .await
        .expect("list")
        .id;
    let their_list = ShoppingListRepo::create(&state.pool, other, "Heidän")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    for (list, recipe) in [
        (my_list, their_recipe),
        (their_list, my_recipe),
        (their_list, their_recipe),
        (my_list, Uuid::new_v4()),
    ] {
        for hx in [false, true] {
            let mut req = TestRequest::post()
                .uri(&format!("/lists/{list}/recipes"))
                .cookie(session.clone())
                .set_form([
                    ("recipe_id", recipe.to_string()),
                    ("multiplier", "2".to_owned()),
                ]);
            if hx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(
                resp.status(),
                StatusCode::NOT_FOUND,
                "{list}/{recipe} (htmx: {hx})"
            );
        }
    }
    assert_eq!(item_shape(&state, user, my_list).await.len(), 0);
    assert_eq!(item_shape(&state, other, their_list).await.len(), 0);

    // The list page offers only the user's own recipes.
    let page = crate::routes::test_support::body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/lists/{my_list}"))
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(page.contains(&my_recipe.to_string()));
    assert!(!page.contains(&their_recipe.to_string()));
}

/// MB-0005 P6: adding a recipe answers htmx with the `#list-{id}` fragment
/// and plain requests with a 303 (or the full page with 400 for rejected
/// input); without a session it is rejected.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_recipe_htmx() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let pancakes = make_recipe_with(
        &state,
        user,
        "Pannukakku",
        kylma,
        &[("Maito", Some("5 dl")), ("Kananmuna", Some("2"))],
    )
    .await;
    let empty = make_recipe_with(&state, user, "Tyhjä", kylma, &[]).await;
    let list = ShoppingListRepo::create(&state.pool, user, "Viikko")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let uri = format!("/lists/{list}/recipes");
    let post = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let fragment_id = format!(r#"id="list-{list}""#);

    // htmx: the fragment with the scaled ingredients.
    let resp = call_service(
        &app,
        post(vec![
            ("recipe_id", pancakes.to_string()),
            ("multiplier", "1,5".to_owned()),
        ])
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(&fragment_id));
    assert!(body.contains("7.5 dl") && body.contains("Maito") && body.contains("· Kylmä"));

    // Plain: a 303 back to the list; a blank multiplier means 1.
    let resp = call_service(
        &app,
        post(vec![
            ("recipe_id", pancakes.to_string()),
            ("multiplier", String::new()),
        ])
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        format!("/lists/{list}").as_str()
    );
    assert_eq!(
        ShoppingListRepo::items(&state.pool, user, list, false)
            .await
            .expect("items")
            .into_iter()
            .map(|i| i.quantity.unwrap_or_default())
            .collect::<Vec<_>>(),
        ["7.5 dl", "3", "5 dl", "2"]
    );

    // Rejected input: in the fragment over htmx (keeping the multiplier),
    // the full page with 400 otherwise; nothing is added.
    let rejected = [
        (vec![("multiplier", "2".to_owned())], "choose a recipe"),
        (vec![("recipe_id", "nope".to_owned())], "unknown recipe"),
        (
            vec![
                ("recipe_id", pancakes.to_string()),
                ("multiplier", "kaksi".to_owned()),
            ],
            "must be a number",
        ),
        (
            vec![
                ("recipe_id", pancakes.to_string()),
                ("multiplier", "0".to_owned()),
            ],
            "more than 0",
        ),
        (vec![("recipe_id", empty.to_string())], "has no ingredients"),
    ];
    for (form, message) in rejected {
        let resp = call_service(
            &app,
            post(form.clone())
                .insert_header(("HX-Request", "true"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{form:?}");
        let body = body_string(resp).await;
        assert!(
            body.contains(&fragment_id) && body.contains(message),
            "{form:?}"
        );
        let resp = call_service(&app, post(form.clone()).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{form:?}");
        let body = body_string(resp).await;
        assert!(body.contains("<html") && body.contains(message), "{form:?}");
    }
    let body = body_string(
        call_service(
            &app,
            post(vec![
                ("recipe_id", pancakes.to_string()),
                ("multiplier", "kaksi".to_owned()),
            ])
            .insert_header(("HX-Request", "true"))
            .to_request(),
        )
        .await,
    )
    .await;
    assert!(
        body.contains(r#"value="kaksi""#),
        "keeps the typed multiplier"
    );
    assert_eq!(item_shape(&state, user, list).await.len(), 4);

    // Signed out: rejected.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&uri)
            .set_form([("recipe_id", pancakes.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// MB-0016 P1: on the full-page load of a list, the recipe picker names each
/// recipe by its own title, as the htmx fragment does, while the page title
/// stays the list's name.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn recipe_picker_shows_titles() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let pancakes = make_recipe_with(&state, user, "Pannukakku", kylma, &[]).await;
    let porridge = make_recipe_with(&state, user, "Puuro", kylma, &[]).await;
    let list = ShoppingListRepo::create(&state.pool, user, "Viikonloppu")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let options = [
        format!(r#"<option value="{pancakes}">Pannukakku</option>"#),
        format!(r#"<option value="{porridge}">Puuro</option>"#),
    ];

    for htmx in [false, true] {
        let mut req = TestRequest::get()
            .uri(&format!("/lists/{list}"))
            .cookie(session.clone());
        if htmx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert_eq!(body.contains("<html"), !htmx, "htmx={htmx}");
        if !htmx {
            assert!(body.contains("<title>Viikonloppu — kauppalista.app</title>"));
        }
        for option in &options {
            assert!(body.contains(option.as_str()), "htmx={htmx}: {option}");
        }
    }
}
