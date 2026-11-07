//! MB-0003 (P2, P6): the editor — the category rule for ingredients, create and edit over htmx and not, owner isolation.

use actix_web::http::StatusCode;
use uuid::Uuid;

use kauppa_core::domain::NewIngredient;
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo};

#[cfg(test)]
use crate::routes::test_support::make_recipe;

use super::ingredient_names;

/// P2: the server refuses an ingredient whose product has no category — an
/// existing product left uncategorized by a category deletion, or a new name
/// without a chosen category — even when a client skips the form, and adds
/// nothing. A categorized product is accepted.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn ingredient_requires_categorized_product() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let juomat = make_category(&state, user, "Juomat").await;
    let theirs = make_category(&state, other, "Pakasteet").await;
    let recipe = make_recipe(&state, user, "Aamiainen").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    // Uncategorized products: a favorite and a recipe-created (hidden) one
    // whose category was deleted.
    ProductRepo::create(&state.pool, user, "Kahvi", juomat)
        .await
        .expect("create favorite");
    let scratch = make_recipe(&state, user, "Luonnos").await;
    RecipeRepo::add_ingredient(
        &state.pool,
        user,
        scratch,
        &NewIngredient {
            product_name: "Mehu".to_owned(),
            quantity: None,
            category_id: Some(juomat),
        },
    )
    .await
    .expect("create hidden product");
    CategoryRepo::delete(&state.pool, user, juomat)
        .await
        .expect("delete category");

    let submit = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/ingredients"))
            .cookie(session.clone())
            .set_form(form)
    };
    let field = |k: &'static str, v: &str| (k, v.to_owned());

    let rejected = [
        (
            vec![
                field("name", "kahvi"),
                field("category_id", &hevi.to_string()),
            ],
            "has no category",
        ),
        (vec![field("name", "MEHU")], "has no category"),
        (vec![field("name", "Kaura")], "choose a category"),
        (
            vec![field("name", "Kaura"), field("category_id", "")],
            "choose a category",
        ),
        (
            vec![field("name", "Kaura"), field("category_id", "not-a-uuid")],
            "unknown category",
        ),
        (
            vec![
                field("name", "Kaura"),
                field("category_id", &theirs.to_string()),
            ],
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
    assert_eq!(
        ingredient_names(&state, user, recipe).await,
        Vec::<String>::new()
    );
    assert!(
        ProductRepo::find_by_name(&state.pool, user, "Kaura")
            .await
            .expect("lookup")
            .is_none()
    );
    let kahvi = ProductRepo::find_by_name(&state.pool, user, "Kahvi")
        .await
        .expect("lookup")
        .expect("exists");
    assert_eq!(kahvi.category_id, None, "a rejection doesn't recategorize");

    // Over htmx the rejection is the fragment, keeping the typed values.
    let resp = call_service(
        &app,
        submit(vec![field("name", "Kaura"), field("quantity", "1 dl")])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html"), "fragment only");
    assert!(body.contains(r#"id="ingredients""#));
    assert!(body.contains("choose a category for the new product Kaura"));
    assert!(body.contains(r#"value="Kaura""#) && body.contains(r#"value="1 dl""#));

    // Control: a categorized product, existing or new, is accepted.
    for form in [
        vec![
            field("name", "Kaura"),
            field("category_id", &hevi.to_string()),
        ],
        vec![field("name", "kaura"), field("quantity", "2 dl")],
    ] {
        let resp = call_service(&app, submit(form.clone()).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{form:?}");
    }
    assert_eq!(
        ingredient_names(&state, user, recipe).await,
        ["Kaura", "Kaura"]
    );
}

/// P6: `GET /recipes/{id}` renders a recipe with its ingredients, create and
/// edit work over htmx (fragments and `HX-Redirect`) and without it (303),
/// and another user can neither read nor change the recipe.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let post = |uri: String, form: Vec<(&'static str, String)>| {
        TestRequest::post().uri(&uri).set_form(form)
    };

    // Create over htmx: an HX-Redirect into the editor.
    let resp = call_service(
        &app,
        post(
            "/recipes".to_owned(),
            vec![("title", "Pannukakku".to_owned())],
        )
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let location = resp
        .headers()
        .get("HX-Redirect")
        .expect("HX-Redirect")
        .to_str()
        .expect("ascii")
        .to_owned();
    let recipe: Uuid = location
        .strip_prefix("/recipes/")
        .expect("editor url")
        .parse()
        .expect("recipe id");

    // Without htmx: a 303 to the editor. An empty title re-renders the form.
    let resp = call_service(
        &app,
        post("/recipes".to_owned(), vec![("title", "Puuro".to_owned())])
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = call_service(
        &app,
        post("/recipes".to_owned(), vec![("title", " ".to_owned())])
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert!(body.contains(r#"id="new-recipe""#) && body.contains("must not be empty"));

    // Edit over htmx: each section answers with its own fragment.
    let resp = call_service(
        &app,
        post(
            format!("/recipes/{recipe}"),
            vec![
                ("title", "Pannukakku".to_owned()),
                ("servings", "4".to_owned()),
                ("instructions", "Paista uunissa.".to_owned()),
            ],
        )
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(r#"id="recipe-details""#));
    assert!(body.contains("Saved."));
    let resp = call_service(
        &app,
        post(
            format!("/recipes/{recipe}/ingredients"),
            vec![
                ("name", "Maito".to_owned()),
                ("quantity", "5 dl".to_owned()),
                ("category_id", kylma.to_string()),
            ],
        )
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(r#"id="ingredients""#));
    assert!(body.contains("Maito"));

    // The page renders the recipe with its ingredients and their categories.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{recipe}"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    for expected in [
        "<html",
        "Pannukakku",
        "Paista uunissa.",
        "5 dl",
        "Maito",
        "Kylmä",
    ] {
        assert!(body.contains(expected), "page should show {expected:?}");
    }
    let milk = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe")
        .ingredients[0]
        .id;

    // Another user: every read and write is a 404, and nothing changes.
    let base = format!("/recipes/{recipe}");
    let item = format!("{base}/ingredients/{milk}");
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(intruder.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let writes = [
        (base.clone(), vec![("title", "Kaapattu".to_owned())]),
        (
            format!("{base}/ingredients"),
            vec![
                ("name", "Omena".to_owned()),
                ("category_id", kylma.to_string()),
            ],
        ),
        (
            format!("{item}/quantity"),
            vec![("quantity", "1 l".to_owned())],
        ),
        (format!("{item}/move"), vec![("dir", "down".to_owned())]),
        (format!("{item}/delete"), vec![]),
    ];
    for (uri, form) in writes {
        for hx in [false, true] {
            let mut req = post(uri.clone(), form.clone()).cookie(intruder.clone());
            if hx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri} (htmx: {hx})");
        }
    }
    let their_index = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/recipes")
                .cookie(intruder.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(!their_index.contains(&base), "not listed for another user");

    let detail = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe");
    assert_eq!(detail.recipe.title, "Pannukakku");
    assert_eq!(detail.ingredients.len(), 1);
    assert_eq!(detail.ingredients[0].quantity.as_deref(), Some("5 dl"));

    // Signed out: no access at all.
    let resp = call_service(&app, TestRequest::get().uri(&base).to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
