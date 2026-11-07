//! MB-0014 (P3–P5): the "From recipes" section.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::{CategoryRepo, ProductRepo};

#[cfg(test)]
use crate::routes::test_support::make_recipe_with;
use crate::state::AppState;

use super::catalog_shape;

/// `(group name, [product names])` of the user's recipe-created products;
/// `None` is uncategorized.
#[allow(clippy::expect_used)]
async fn hidden_shape(state: &AppState, user: Uuid) -> Vec<(Option<String>, Vec<String>)> {
    ProductRepo::list_hidden_with_usage(&state.pool, user)
        .await
        .expect("list hidden products")
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.products.into_iter().map(|u| u.product.name).collect(),
            )
        })
        .collect()
}

#[allow(clippy::expect_used)]
async fn any_product_id(state: &AppState, user: Uuid, name: &str) -> Uuid {
    ProductRepo::find_by_name(&state.pool, user, name)
        .await
        .expect("find product")
        .expect("product exists")
        .id
}

/// The part of a `/favorites` body from the recipe-created section on.
fn recipe_section(body: &str) -> &str {
    body.find(r#"id="recipe-products""#)
        .map_or("", |start| &body[start..])
}

/// P3: `/favorites` lists the user's recipe-created products below the
/// favorites, grouped by category with their usage counts, and make favorite,
/// rename, change category, and delete work on them over htmx (the fragment)
/// and without it (303). Another user's products never appear and can't be
/// changed.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn hidden_products_section() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);

    // Unauthenticated: the new route is rejected like the others.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/favorites/{}/favorite", Uuid::new_v4()))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "Salaiset").await;
    ProductRepo::create(&state.pool, user, "Omena", hevi)
        .await
        .expect("create favorite");
    make_recipe_with(
        &state,
        user,
        "Salaatti",
        hevi,
        &[("Kurkku", None), ("Tomaatti", None)],
    )
    .await;
    make_recipe_with(&state, user, "Tzatziki", hevi, &[("Kurkku", None)]).await;
    make_recipe_with(
        &state,
        user,
        "Pannukakku",
        kylma,
        &[("Maito", Some("5 dl")), ("Kerma", None), ("Voi", None)],
    )
    .await;
    make_recipe_with(&state, other, "Heidän", theirs, &[("Salainen", None)]).await;
    let secret = any_product_id(&state, other, "Salainen").await;

    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let get = || TestRequest::get().uri("/favorites").cookie(session.clone());
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true")).to_request();

    // The full page: favorites first, then the section with its groups,
    // usage counts, and a "make favorite" form per product.
    let resp = call_service(&app, get().to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let page = body_string(resp).await;
    assert!(page.contains("<html") && page.contains("From recipes"));
    let favorites_at = page.find("Omena").unwrap_or(usize::MAX);
    let section_at = page.find(r#"id="recipe-products""#);
    assert!(
        section_at.is_some_and(|s| favorites_at < s),
        "the section comes after the favorites"
    );
    let section = recipe_section(&page);
    assert!(!section.contains("Omena"), "favorites aren't repeated");
    let hevi_at = section.find("<h3>Hevi</h3>");
    let kylma_at = section.find("<h3>Kylmä</h3>");
    assert!(hevi_at.is_some() && hevi_at < kylma_at, "{section}");
    for name in ["Kurkku", "Tomaatti", "Maito", "Kerma", "Voi"] {
        let id = any_product_id(&state, user, name).await;
        assert!(section.contains(&format!(r#"id="recipe-product-{id}""#)));
        assert!(section.contains(&format!(r#"action="/favorites/{id}/favorite""#)));
    }
    assert!(section.contains("Used in 2 recipes") && section.contains("Used in 1 recipe"));
    assert!(!page.contains("Salainen"), "another user's product");
    assert!(!page.contains(&secret.to_string()));

    let kurkku = any_product_id(&state, user, "Kurkku").await;
    let tomaatti = any_product_id(&state, user, "Tomaatti").await;
    let maito = any_product_id(&state, user, "Maito").await;
    let kerma = any_product_id(&state, user, "Kerma").await;
    let voi = any_product_id(&state, user, "Voi").await;
    let (hevi_s, kylma_s) = (hevi.to_string(), kylma.to_string());

    // Rename: over htmx, then without it. They stay recipe-created.
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{kurkku}/rename"),
            &[("name", "Kurkut")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    // The htmx fragment carries the section, too.
    let fragment = body_string(resp).await;
    assert!(!fragment.contains("<html"));
    let section = recipe_section(&fragment);
    assert!(section.contains("Kurkut") && section.contains("Used in 2 recipes"));
    let resp = call_service(
        &app,
        post(
            &format!("/favorites/{voi}/rename"),
            &[("name", "Voi (suolaton)")],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Change category: over htmx, then without it.
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{maito}/category"),
            &[("category_id", hevi_s.as_str())],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = call_service(
        &app,
        post(
            &format!("/favorites/{tomaatti}/category"),
            &[("category_id", kylma_s.as_str())],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let s = |v: &[&str]| v.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        hidden_shape(&state, user).await,
        [
            (Some("Hevi".to_owned()), s(&["Kurkut", "Maito"])),
            (
                Some("Kylmä".to_owned()),
                s(&["Kerma", "Tomaatti", "Voi (suolaton)"])
            ),
        ]
    );

    // Make favorite over htmx: it moves from the section to the favorites.
    let resp = call_service(
        &app,
        hx(post(&format!("/favorites/{kurkku}/favorite"), &[])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html"));
    let at = body.find("Kurkut");
    assert!(
        at.is_some() && at < body.find(r#"id="recipe-products""#),
        "listed with the favorites"
    );
    assert!(!recipe_section(&body).contains("Kurkut"));
    // And without htmx: 303 back to the page.
    let resp = call_service(
        &app,
        post(&format!("/favorites/{maito}/favorite"), &[]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/favorites")
    );
    assert_eq!(
        catalog_shape(&state, user).await,
        [(Some("Hevi".to_owned()), s(&["Omena", "Kurkut", "Maito"]))]
    );

    // Delete: over htmx, then without it.
    let resp = call_service(&app, hx(post(&format!("/favorites/{kerma}/delete"), &[]))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_string(resp).await.contains("Kerma"));
    let resp = call_service(
        &app,
        post(&format!("/favorites/{voi}/delete"), &[]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        hidden_shape(&state, user).await,
        [(Some("Kylmä".to_owned()), s(&["Tomaatti"]))]
    );

    // The last one becomes a favorite: the section goes away.
    call_service(
        &app,
        hx(post(&format!("/favorites/{tomaatti}/favorite"), &[])),
    )
    .await;
    let page = body_string(call_service(&app, get().to_request()).await).await;
    assert!(!page.contains(r#"id="recipe-products""#));

    // Another user's product: every action is `NotFound`, nothing changes.
    let secret_s = secret.to_string();
    let attempts = [
        post(&format!("/favorites/{secret}/favorite"), &[]),
        post(&format!("/favorites/{secret}/rename"), &[("name", "Minun")]),
        post(
            &format!("/favorites/{secret}/category"),
            &[("category_id", hevi_s.as_str())],
        ),
        post(&format!("/favorites/{secret}/delete"), &[]),
    ];
    for req in attempts {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
    assert_eq!(
        hidden_shape(&state, other).await,
        [(Some("Salaiset".to_owned()), s(&["Salainen"]))]
    );
    assert_eq!(
        any_product_id(&state, other, "Salainen").await.to_string(),
        secret_s
    );
}

/// P4: a recipe-created product left uncategorized by a category deletion is
/// shown as uncategorized, can't be made a favorite yet, and can be given a
/// category here — after which it is accepted as a recipe ingredient again.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn hidden_uncategorized_fixable() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let juomat = make_category(&state, user, "Juomat").await;
    make_recipe_with(&state, user, "Aamiainen", juomat, &[("Mehu", None)]).await;
    CategoryRepo::delete(&state.pool, user, juomat)
        .await
        .expect("delete category");
    let mehu = any_product_id(&state, user, "Mehu").await;
    let recipe = make_recipe_with(&state, user, "Smoothie", hevi, &[]).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let add_mehu = || {
        post(
            &format!("/recipes/{recipe}/ingredients"),
            &[("name", "mehu")],
        )
    };

    // Shown as uncategorized, with no "make favorite" form.
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
    let section = recipe_section(&page);
    assert!(section.contains("<h3>Uncategorized</h3>") && section.contains("Mehu"));
    assert!(section.contains("No category: set one under Edit"));
    assert!(!section.contains(&format!("/favorites/{mehu}/favorite")));

    // The dead end MB-0003 P2 leaves: refused as an ingredient…
    let resp = call_service(&app, add_mehu().to_request()).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(body_string(resp).await.contains("has no category"));
    // …and refused as a favorite, without htmx and with it.
    let resp = call_service(
        &app,
        post(&format!("/favorites/{mehu}/favorite"), &[]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("choose a category for Mehu"));
    let resp = call_service(
        &app,
        post(&format!("/favorites/{mehu}/favorite"), &[])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(r#"role="alert""#) && body.contains("choose a category for Mehu"));

    // Fix it from the section, over htmx.
    let resp = call_service(
        &app,
        post(
            &format!("/favorites/{mehu}/category"),
            &[("category_id", hevi.to_string().as_str())],
        )
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let section = recipe_section(&body);
    assert!(section.contains("<h3>Hevi</h3>") && !section.contains("Uncategorized"));
    assert!(section.contains(&format!("/favorites/{mehu}/favorite")));
    assert!(
        !ProductRepo::find_by_name(&state.pool, user, "Mehu")
            .await
            .expect("find")
            .expect("exists")
            .is_favorite,
        "still recipe-created"
    );

    // Now it is accepted as an ingredient, linked to the same product.
    let resp = call_service(&app, add_mehu().to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let ingredients = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe")
        .ingredients;
    assert_eq!(ingredients.len(), 1);
    assert_eq!(ingredients[0].product_id, Some(mehu));
    assert_eq!(ingredients[0].text, "Mehu");
}

/// P5: deleting a product that recipes use asks for confirmation naming the
/// usage count (and says so next to the button for clients without htmx);
/// afterwards those ingredients keep their name — the current one, after a
/// rename — and quantity, unlinked (RR-0001 constraint 4).
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn delete_used_product_warns() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let salaatti = make_recipe_with(
        &state,
        user,
        "Salaatti",
        hevi,
        &[("Tomaatti", Some("2")), ("Kurkku", Some("1 kpl"))],
    )
    .await;
    let tzatziki = make_recipe_with(&state, user, "Tzatziki", hevi, &[("Kurkku", Some("½"))]).await;
    let scratch = make_recipe_with(&state, user, "Luonnos", hevi, &[("Kerma", None)]).await;
    let kerma_ingredient = RecipeRepo::get(&state.pool, user, scratch)
        .await
        .expect("get recipe")
        .ingredients[0]
        .id;
    RecipeRepo::remove_ingredient(&state.pool, user, scratch, kerma_ingredient)
        .await
        .expect("remove ingredient");
    let kurkku = any_product_id(&state, user, "Kurkku").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let hx_post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .set_form(form)
            .to_request()
    };

    // Renamed first, so the saved ingredient text must follow the new name.
    call_service(
        &app,
        hx_post(
            &format!("/favorites/{kurkku}/rename"),
            &[("name", "Kurkut")],
        ),
    )
    .await;

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
    let section = recipe_section(&page);
    let warning = "Used in 2 recipes. Deleting it keeps the ingredient in those recipes, \
                   by name and quantity, no longer linked to a product.";
    assert!(
        section.contains(&format!(r#"hx-confirm="Delete “Kurkut”? {warning}""#)),
        "{section}"
    );
    assert!(section.contains(&format!("<p class=\"muted\">{warning}</p>")));
    assert!(section.contains("hx-confirm=\"Delete “Tomaatti”? Used in 1 recipe. "));
    // An unused product gets the plain confirmation.
    assert!(section.contains(r#"hx-confirm="Delete “Kerma”?""#));

    let resp = call_service(&app, hx_post(&format!("/favorites/{kurkku}/delete"), &[])).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_string(resp).await.contains("Kurkut"));
    assert!(
        ProductRepo::find_by_name(&state.pool, user, "Kurkut")
            .await
            .expect("find")
            .is_none()
    );

    // The ingredients stay, by name and quantity, unlinked.
    let lines = |recipe: Uuid| {
        let state = state.clone();
        async move {
            RecipeRepo::get(&state.pool, user, recipe)
                .await
                .expect("get recipe")
                .ingredients
                .into_iter()
                .map(|i| (i.text, i.quantity, i.product_id.is_some()))
                .collect::<Vec<_>>()
        }
    };
    let line = |name: &str, quantity: &str, linked: bool| {
        (name.to_owned(), Some(quantity.to_owned()), linked)
    };
    assert_eq!(
        lines(salaatti).await,
        [line("Tomaatti", "2", true), line("Kurkut", "1 kpl", false)]
    );
    assert_eq!(lines(tzatziki).await, [line("Kurkut", "½", false)]);

    // The recipe page still lists it by name.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{tzatziki}"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Kurkut"));
}
