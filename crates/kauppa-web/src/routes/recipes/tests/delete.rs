//! MB-0012 (P3): deleting a recipe.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::domain::NewIngredient;
use kauppa_core::repo::{ProductRepo, RecipeRepo};

#[cfg(test)]
use crate::routes::test_support::make_recipe;

use super::ingredient_names;

/// MB-0012 P3: the editor's delete form asks for confirmation; `POST
/// /recipes/{id}/delete` lands on `/recipes` without the recipe — an
/// `HX-Redirect` over htmx, a 303 otherwise — and another user gets a 404.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn delete_recipe() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let pancakes = make_recipe(&state, user, "Pannukakku").await;
    let porridge = make_recipe(&state, user, "Puuro").await;
    RecipeRepo::add_ingredient(
        &state.pool,
        user,
        pancakes,
        &NewIngredient {
            product_name: "Maito".to_owned(),
            quantity: Some("5 dl".to_owned()),
            category_id: Some(kylma),
        },
    )
    .await
    .expect("add ingredient");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let delete_req = |recipe: Uuid| TestRequest::post().uri(&format!("/recipes/{recipe}/delete"));
    let index_page = || async {
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri("/recipes")
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        body_string(resp).await
    };

    // The editor (a full page) has the delete form with its confirmation.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{pancakes}"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Pannukakku — kauppa</title>"));
    assert!(body.contains(&format!(
        r#"method="post" action="/recipes/{pancakes}/delete""#
    )));
    assert!(body.contains(&format!(r#"hx-post="/recipes/{pancakes}/delete""#)));
    assert!(body.contains(r#"hx-confirm="Delete the recipe “Pannukakku” and its ingredients?"#));

    // Another user: a 404 with and without htmx, and nothing is deleted.
    for hx in [false, true] {
        let mut req = delete_req(pancakes).cookie(intruder.clone());
        if hx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "htmx: {hx}");
    }
    assert_eq!(ingredient_names(&state, user, pancakes).await, ["Maito"]);

    // Over htmx: an HX-Redirect to the index, which no longer lists it.
    let resp = call_service(
        &app,
        delete_req(pancakes)
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("HX-Redirect").expect("HX-Redirect"),
        "/recipes"
    );
    let body = index_page().await;
    assert!(!body.contains(&format!("/recipes/{pancakes}")), "gone");
    assert!(
        body.contains(&format!("/recipes/{porridge}")),
        "others stay"
    );
    assert!(matches!(
        RecipeRepo::get(&state.pool, user, pancakes).await,
        Err(CoreError::NotFound)
    ));
    assert!(
        ProductRepo::find_by_name(&state.pool, user, "Maito")
            .await
            .expect("lookup")
            .is_some(),
        "the product stays in the catalog"
    );

    // Without htmx: a 303 to the index (PRG).
    let resp = call_service(
        &app,
        delete_req(porridge).cookie(session.clone()).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("Location"),
        "/recipes"
    );
    assert!(!index_page().await.contains(&format!("/recipes/{porridge}")));

    // Already deleted, or never existed: a 404.
    for recipe in [porridge, Uuid::new_v4()] {
        let resp = call_service(
            &app,
            delete_req(recipe).cookie(session.clone()).to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // Signed out: no access.
    let resp = call_service(&app, delete_req(pancakes).to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
