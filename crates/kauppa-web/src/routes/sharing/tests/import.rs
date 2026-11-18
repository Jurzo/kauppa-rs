//! MB-0061 P3 (RR-0048): importing shared favorites through MB-0060's plan
//! and review.

use actix_web::dev::ServiceResponse;
use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::{CatalogShareRepo, CategoryRepo, ProductRepo};

use crate::state::AppState;

#[allow(clippy::expect_used)]
async fn favorites(state: &AppState, user: Uuid) -> Vec<(Option<String>, Vec<String>)> {
    ProductRepo::list_for_owner(&state.pool, user)
        .await
        .expect("favorites")
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.products.into_iter().map(|p| p.name).collect(),
            )
        })
        .collect()
}

fn location<B>(resp: &ServiceResponse<B>) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

fn fingerprint(body: &str) -> String {
    let marker = r#"name="fingerprint" value=""#;
    body.find(marker)
        .map(|at| &body[at + marker.len()..])
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_owned()
}

/// P3: ticked favorites (or all) are imported with MB-0060's rules: a new
/// name becomes a favorite in the recipient's category of the same name; a
/// conflict and an unmatched category go to the same review, none
/// preselected, with the chosen ids carried through it; nothing ticked
/// shows a hint; the sharer's catalog is untouched and none of its ids
/// enters the recipient's.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn favorites_use_export_review() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let me = make_user(&state, "me@example.com").await;
    let friend = make_user(&state, "friend@example.com").await;
    let (hevi, kylma, leipa) = (
        CategoryRepo::create(&state.pool, me, "Hevi")
            .await
            .expect("c")
            .id,
        CategoryRepo::create(&state.pool, me, "Kylmä")
            .await
            .expect("c")
            .id,
        CategoryRepo::create(&state.pool, me, "Leipä")
            .await
            .expect("c")
            .id,
    );
    let mut ids = Vec::new();
    for (name, category) in [
        ("Omena", hevi),
        ("Maito", kylma),
        ("Ruisleipä", leipa),
        ("Päärynä", hevi),
    ] {
        ids.push(
            ProductRepo::create(&state.pool, me, name, category)
                .await
                .expect("favorite")
                .id,
        );
    }
    let (omena, maito, ruis, _paaryna) = (ids[0], ids[1], ids[2], ids[3]);
    let my_hevi = CategoryRepo::create(&state.pool, friend, "HEVI")
        .await
        .expect("c")
        .id;
    let my_kylma = CategoryRepo::create(&state.pool, friend, "Maitotuotteet")
        .await
        .expect("c")
        .id;
    let their_kylma = CategoryRepo::create(&state.pool, friend, "kylmä")
        .await
        .expect("c")
        .id;
    ProductRepo::create(&state.pool, friend, "maito", my_kylma)
        .await
        .expect("their own Maito");
    CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", true, false)
        .await
        .expect("share")
        .expect("matched");
    let sharer_before = format!("{:?}", favorites(&state, me).await);
    let cookie =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);
    let base = format!("/sharing/from/{me}/favorites");
    let post = |uri: String, form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(cookie.clone())
            .set_form(form)
            .to_request()
    };

    // Nothing ticked: back with a hint, nothing imported.
    let resp = call_service(&app, post(format!("{base}/import"), vec![])).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp), format!("{base}?none=1"));
    let hint = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&location(&resp))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(hint.contains("Tick at least one favorite, or import them all."));

    // Omena only: no question (Hevi ↔ HEVI), saved at once.
    let resp = call_service(
        &app,
        post(format!("{base}/import"), vec![("pick", omena.to_string())]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let done = location(&resp);
    assert_eq!(
        done,
        format!("{base}/import/done?added=1&already=0&favorite=0&moved=0&skipped=0")
    );
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&done)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(
        body.contains("1 new favorite") && body.contains("Back to the favorites from Test User")
    );

    // Maito (a conflict: theirs is in Maitotuotteet) and Ruisleipä (Leipä
    // isn't one of theirs): the review, with the picks carried.
    let resp = call_service(
        &app,
        post(
            format!("{base}/import"),
            vec![("pick", maito.to_string()), ("pick", ruis.to_string())],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("These favorites from Test User need your choice"));
    assert!(body.contains(&format!(r#"action="{base}/import/confirm""#)));
    for id in [maito, ruis] {
        assert!(body.contains(&format!(
            r#"<input type="hidden" name="pick" value="{id}" />"#
        )));
    }
    let form = &body[body.find(r#"class="export-review""#).expect("form")..];
    assert!(!form.contains("checked"), "nothing preselected");
    assert!(form.contains("Your favorite: Maitotuotteet."));
    assert!(form.contains("In their favorites: kylmä.") && form.contains("Move to kylmä"));
    assert!(form.contains("In their favorites: Leipä, which isn't one of your categories."));
    for id in [hevi, kylma, leipa] {
        assert!(
            !body.contains(&id.to_string()),
            "no sharer category id offered"
        );
    }
    assert_eq!(
        favorites(&state, friend).await.len(),
        2,
        "nothing saved yet"
    );

    // Confirm: keep their Maito; Ruisleipä into HEVI.
    let resp = call_service(
        &app,
        post(
            format!("{base}/import/confirm"),
            vec![
                ("fingerprint", fingerprint(&body)),
                ("pick", maito.to_string()),
                ("pick", ruis.to_string()),
                ("r0", my_kylma.to_string()),
                ("r1", my_hevi.to_string()),
            ],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        location(&resp),
        format!("{base}/import/done?added=1&already=1&favorite=0&moved=0&skipped=0")
    );
    assert_eq!(
        favorites(&state, friend).await,
        [
            (
                Some("HEVI".to_owned()),
                vec!["Omena".to_owned(), "Ruisleipä".to_owned()]
            ),
            (Some("Maitotuotteet".to_owned()), vec!["maito".to_owned()]),
        ]
    );
    assert_eq!(
        format!("{:?}", favorites(&state, me).await),
        sharer_before,
        "the sharer's catalog is untouched"
    );

    // "Import all": everything already there or reviewed; a stale review
    // (the sharer added a favorite meanwhile) saves nothing.
    let resp = call_service(
        &app,
        post(format!("{base}/import"), vec![("all", "1".to_owned())]),
    )
    .await;
    let body = body_string(resp).await;
    assert!(body.contains(r#"<input type="hidden" name="all" value="1" />"#));
    ProductRepo::create(&state.pool, me, "Kaura", leipa)
        .await
        .expect("new favorite");
    let resp = call_service(
        &app,
        post(
            format!("{base}/import/confirm"),
            vec![
                ("fingerprint", fingerprint(&body)),
                ("all", "1".to_owned()),
                ("r0", my_kylma.to_string()),
            ],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("changed since"));
    let mine: Vec<Uuid> = ProductRepo::list_for_owner(&state.pool, friend)
        .await
        .expect("favorites")
        .into_iter()
        .flat_map(|g| g.products)
        .filter_map(|p| p.category_id)
        .collect();
    assert!(
        mine.iter()
            .all(|c| [my_hevi, my_kylma, their_kylma].contains(c)),
        "only their own categories"
    );
}

/// A recipe of `owner`'s with ingredients in different categories:
/// Kurkkusalaatti (serves 2): 1 Kurkku (Hevi), 1 tl Suola (Mausteet),
/// 1 dl Maito (Kylmä).
#[allow(clippy::expect_used)]
async fn salad(state: &AppState, owner: Uuid) -> Uuid {
    use kauppa_core::domain::{NewIngredient, RecipeInput};
    use kauppa_core::repo::RecipeRepo;

    let recipe = RecipeRepo::create(
        &state.pool,
        owner,
        &RecipeInput {
            title: "Kurkkusalaatti".to_owned(),
            instructions: "Sekoita.".to_owned(),
            servings: Some(2),
        },
    )
    .await
    .expect("recipe")
    .id;
    for (name, quantity, category) in [
        ("Kurkku", "1", "Hevi"),
        ("Suola", "1 tl", "Mausteet"),
        ("Maito", "1 dl", "Kylmä"),
    ] {
        let category = match CategoryRepo::create(&state.pool, owner, category).await {
            Ok(c) => c.id,
            Err(_) => {
                CategoryRepo::list_for_owner(&state.pool, owner)
                    .await
                    .expect("categories")
                    .into_iter()
                    .find(|c| c.name == category)
                    .expect("category")
                    .id
            }
        };
        RecipeRepo::add_ingredient(
            &state.pool,
            owner,
            recipe,
            &NewIngredient {
                product_name: name.to_owned(),
                quantity: Some(quantity.to_owned()),
                category_id: Some(category),
            },
        )
        .await
        .expect("ingredient");
    }
    recipe
}

/// P4: importing a shared recipe creates the recipient's copy (title,
/// instructions, servings, ingredients with quantities and order,
/// `imported_from`) in one transaction with its products: an existing
/// product is linked as it is; a new one whose category maps is created
/// hidden there; a new one whose category doesn't map is asked on the
/// review, with no skip. The sharer's catalog is untouched.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn recipe_copy_with_products() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let me = make_user(&state, "me@example.com").await;
    let friend = make_user(&state, "friend@example.com").await;
    let recipe = salad(&state, me).await;
    let my_hevi = CategoryRepo::create(&state.pool, friend, "HEVI")
        .await
        .expect("c")
        .id;
    let dairy = CategoryRepo::create(&state.pool, friend, "Maitotuotteet")
        .await
        .expect("c")
        .id;
    let their_maito = ProductRepo::create(&state.pool, friend, "maito", dairy)
        .await
        .expect("their Maito")
        .id;
    CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", false, true)
        .await
        .expect("share")
        .expect("matched");
    let sharer_before = format!(
        "{:?}",
        RecipeRepo::get(&state.pool, me, recipe)
            .await
            .expect("recipe")
    );
    let cookie =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);
    let page = format!("/sharing/from/{me}/recipes/{recipe}");

    // The list and the recipe, read-only, with "Import recipe".
    let list = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/sharing/from/{me}/recipes"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(list.contains(&format!(r#"href="{page}">Kurkkusalaatti</a>"#)));
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&page)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(
        body.contains("Kurkkusalaatti") && body.contains("Serves 2") && body.contains("Sekoita.")
    );
    assert!(body.contains(&format!(r#"action="{page}/import""#)));
    let (kurkku_at, suola_at) = (
        body.find("Kurkku<").expect("k"),
        body.find("Suola<").expect("s"),
    );
    assert!(kurkku_at < suola_at, "in order");

    // Import: Suola's Mausteet isn't one of theirs, so the review asks.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{page}/import"))
            .cookie(cookie.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let form = &body[body.find(r#"class="export-review""#).expect("form")..];
    assert!(body.contains("Some ingredients of Kurkkusalaatti from Test User need a category"));
    assert!(form.contains("<legend>Suola</legend>"));
    assert!(form.contains("In their recipe: Mausteet, which isn't one of your categories."));
    assert!(form.contains(r#"<select name="r1" required>"#));
    assert!(
        !form.contains(r#"value="skip""#),
        "an ingredient can't be skipped"
    );
    assert!(!form.contains("<legend>Kurkku</legend>") && !form.contains("<legend>Maito</legend>"));
    assert_eq!(
        RecipeRepo::list_for_owner(&state.pool, friend)
            .await
            .expect("r")
            .len(),
        0
    );

    // A skip is refused (not offered); nothing saved.
    let confirm = |answer: String, fp: String| {
        TestRequest::post()
            .uri(&format!("{page}/import/confirm"))
            .cookie(cookie.clone())
            .set_form([("fingerprint", fp), ("r1", answer)])
            .to_request()
    };
    let resp = call_service(&app, confirm("skip".to_owned(), fingerprint(&body))).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        RecipeRepo::list_for_owner(&state.pool, friend)
            .await
            .expect("r")
            .len(),
        0
    );

    // Suola into HEVI: saved, to the copy.
    let resp = call_service(&app, confirm(my_hevi.to_string(), fingerprint(&body))).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let copy = RecipeRepo::imported_copy(&state.pool, friend, recipe)
        .await
        .expect("copy")
        .expect("imported");
    assert_eq!(location(&resp), format!("/recipes/{copy}"));
    let detail = RecipeRepo::get(&state.pool, friend, copy)
        .await
        .expect("copy");
    assert_eq!(
        (
            detail.recipe.title.as_str(),
            detail.recipe.instructions.as_str(),
            detail.recipe.servings
        ),
        ("Kurkkusalaatti", "Sekoita.", Some(2))
    );
    let lines: Vec<(String, Option<String>, Option<String>)> = detail
        .ingredients
        .iter()
        .map(|i| {
            (
                i.text.clone(),
                i.quantity.clone(),
                i.category.as_ref().map(|c| c.name.clone()),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            (
                "Kurkku".to_owned(),
                Some("1".to_owned()),
                Some("HEVI".to_owned())
            ),
            (
                "Suola".to_owned(),
                Some("1 tl".to_owned()),
                Some("HEVI".to_owned())
            ),
            (
                "maito".to_owned(),
                Some("1 dl".to_owned()),
                Some("Maitotuotteet".to_owned())
            ),
        ]
    );
    assert_eq!(
        detail.ingredients[2].product_id,
        Some(their_maito),
        "linked as it is"
    );
    // Their Maito stays a favorite in Maitotuotteet; Kurkku and Suola are
    // new products from recipes.
    assert_eq!(
        favorites(&state, friend).await,
        [(Some("Maitotuotteet".to_owned()), vec!["maito".to_owned()])]
    );
    let hidden: Vec<String> = ProductRepo::list_hidden_with_usage(&state.pool, friend)
        .await
        .expect("hidden")
        .into_iter()
        .flat_map(|g| g.products)
        .map(|u| u.product.name)
        .collect();
    assert_eq!(hidden, ["Kurkku", "Suola"]);
    for ingredient in &detail.ingredients {
        let category = ingredient.category.as_ref().expect("categorized");
        assert_eq!(category.owner_id, friend, "only their own categories");
    }
    assert_eq!(
        format!(
            "{:?}",
            RecipeRepo::get(&state.pool, me, recipe)
                .await
                .expect("recipe")
        ),
        sharer_before,
        "the sharer's recipe is untouched"
    );
}

/// P5: a repeat import is refused, with a link to the copy, and saves
/// nothing; once the copy is deleted, importing works again; a review made
/// stale by a change to the shared recipe saves nothing and is shown again;
/// a missing answer too.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn repeat_and_stale() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::domain::NewIngredient;
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let me = make_user(&state, "me@example.com").await;
    let friend = make_user(&state, "friend@example.com").await;
    let recipe = salad(&state, me).await;
    for name in ["Hevi", "Kylmä"] {
        CategoryRepo::create(&state.pool, friend, name)
            .await
            .expect("c");
    }
    let mausteet = CategoryRepo::create(&state.pool, friend, "Mausteet")
        .await
        .expect("c")
        .id;
    CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", false, true)
        .await
        .expect("share")
        .expect("matched");
    let cookie =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);
    let page = format!("/sharing/from/{me}/recipes/{recipe}");
    let import = || {
        TestRequest::post()
            .uri(&format!("{page}/import"))
            .cookie(cookie.clone())
            .to_request()
    };

    // Every category maps: saved at once.
    let resp = call_service(&app, import()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let copy = RecipeRepo::imported_copy(&state.pool, friend, recipe)
        .await
        .expect("copy")
        .expect("imported");

    // Again: refused, back to the recipe, which links the copy.
    let resp = call_service(&app, import()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp), page);
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&page)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(
        body.contains("You already have this recipe.")
            && body.contains(&format!(r#"href="/recipes/{copy}""#))
    );
    assert!(!body.contains(&format!(r#"action="{page}/import""#)));
    assert_eq!(
        RecipeRepo::list_for_owner(&state.pool, friend)
            .await
            .expect("r")
            .len(),
        1
    );

    // Deleted copy: importing works again. The sharer adds an ingredient in
    // a category the friend lacks, so there is a review; the sharer then
    // changes the recipe again, and the stale review saves nothing.
    RecipeRepo::delete(&state.pool, friend, copy)
        .await
        .expect("delete");
    let leivonta = CategoryRepo::create(&state.pool, me, "Leivonta")
        .await
        .expect("c")
        .id;
    RecipeRepo::add_ingredient(
        &state.pool,
        me,
        recipe,
        &NewIngredient {
            product_name: "Sokeri".to_owned(),
            quantity: None,
            category_id: Some(leivonta),
        },
    )
    .await
    .expect("ingredient");
    let resp = call_service(&app, import()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let review = body_string(resp).await;
    RecipeRepo::add_ingredient(
        &state.pool,
        me,
        recipe,
        &NewIngredient {
            product_name: "Pippuri".to_owned(),
            quantity: None,
            category_id: Some(leivonta),
        },
    )
    .await
    .expect("ingredient");
    let confirm = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&format!("{page}/import/confirm"))
            .cookie(cookie.clone())
            .set_form(form)
            .to_request()
    };
    let resp = call_service(
        &app,
        confirm(vec![
            ("fingerprint", fingerprint(&review)),
            ("r3", mausteet.to_string()),
        ]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("changed since"));
    assert_eq!(
        RecipeRepo::list_for_owner(&state.pool, friend)
            .await
            .expect("r")
            .len(),
        0
    );

    // A missing answer: shown again, nothing saved.
    let fresh = fingerprint(&body);
    let resp = call_service(
        &app,
        confirm(vec![
            ("fingerprint", fresh.clone()),
            ("r3", mausteet.to_string()),
        ]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(
        body_string(resp)
            .await
            .contains("Choose an option for every item.")
    );
    assert_eq!(
        RecipeRepo::list_for_owner(&state.pool, friend)
            .await
            .expect("r")
            .len(),
        0
    );

    // Answered: saved, with all five ingredients.
    let resp = call_service(
        &app,
        confirm(vec![
            ("fingerprint", fresh),
            ("r3", mausteet.to_string()),
            ("r4", mausteet.to_string()),
        ]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let copy = RecipeRepo::imported_copy(&state.pool, friend, recipe)
        .await
        .expect("copy")
        .expect("imported again");
    assert_eq!(
        RecipeRepo::get(&state.pool, friend, copy)
            .await
            .expect("copy")
            .ingredients
            .len(),
        5
    );
}

/// The review's answer fields (`r<index>`), one per row, in order.
fn answer_fields(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in body.split(r#"name="r"#).skip(1) {
        let field = format!("r{}", part.split('"').next().unwrap_or_default());
        if !out.contains(&field) {
            out.push(field);
        }
    }
    out
}

/// The review page's rows, split at each fieldset.
fn rows(body: &str) -> Vec<&str> {
    body.split(r#"<fieldset class="export-row">"#)
        .skip(1)
        .collect()
}

/// MB-0065 P1 (RR-0052): a shared favorite, and a shared recipe's
/// ingredient, whose category the recipient lacks offer "Create ‹name›" in a
/// "New category" group, sent as an index and never preselected, beside the
/// recipient's categories (and "Skip" for a favorite, none for an
/// ingredient). Choosing it creates the category; a recipe import's page
/// says so.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn create_missing_category_offered() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let me = make_user(&state, "me@example.com").await;
    let friend = make_user(&state, "friend@example.com").await;
    let hevi = CategoryRepo::create(&state.pool, me, "Hevi")
        .await
        .expect("c")
        .id;
    let leipa = CategoryRepo::create(&state.pool, me, "Leipä")
        .await
        .expect("c")
        .id;
    for (name, category) in [("Omena", hevi), ("Ruisleipä", leipa)] {
        ProductRepo::create(&state.pool, me, name, category)
            .await
            .expect("favorite");
    }
    CategoryRepo::create(&state.pool, friend, "HEVI")
        .await
        .expect("c");
    let dairy = CategoryRepo::create(&state.pool, friend, "Maitotuotteet")
        .await
        .expect("c")
        .id;
    ProductRepo::create(&state.pool, friend, "maito", dairy)
        .await
        .expect("their Maito");
    CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", true, true)
        .await
        .expect("share")
        .expect("matched");
    let cookie =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);
    let post = |uri: String, form: Vec<(String, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(cookie.clone())
            .set_form(form)
            .to_request()
    };

    // Favorites: Omena maps to HEVI; Ruisleipä's Leipä is offered.
    let base = format!("/sharing/from/{me}/favorites");
    let body = body_string(
        call_service(
            &app,
            post(
                format!("{base}/import"),
                vec![("all".to_owned(), "1".to_owned())],
            ),
        )
        .await,
    )
    .await;
    let found = rows(&body);
    assert_eq!(found.len(), 1);
    let row = found[0];
    assert!(row.contains("<legend>Ruisleipä</legend>"));
    assert!(row.contains(r#"<optgroup label="New category">"#));
    assert!(row.contains(r#"<option value="new:0">Create Leipä</option>"#));
    assert!(row.contains(r#"<option value="skip">Skip</option>"#));
    let (create_at, skip_at) = (
        row.find("Create Leipä").expect("create"),
        row.find(">Skip<").expect("skip"),
    );
    assert!(
        row.find(">HEVI<").expect("theirs") < create_at && create_at < skip_at,
        "below their categories, above Skip"
    );
    let form = &body[body.find(r#"class="export-review""#).expect("form")..];
    assert!(!form.contains("checked"), "nothing preselected");
    assert!(!body.contains(&leipa.to_string()), "no sharer category id");
    let mut answers = vec![
        ("fingerprint".to_owned(), fingerprint(&body)),
        ("all".to_owned(), "1".to_owned()),
    ];
    answers.extend(
        answer_fields(&body)
            .into_iter()
            .map(|f| (f, "new:0".to_owned())),
    );
    let resp = call_service(&app, post(format!("{base}/import/confirm"), answers)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        location(&resp),
        format!("{base}/import/done?added=2&already=0&favorite=0&moved=0&skipped=0&categories=1")
    );

    // A recipe: Suola's Mausteet is offered, with no skip.
    let recipe = salad(&state, me).await;
    let page = format!("/sharing/from/{me}/recipes/{recipe}");
    let body = body_string(call_service(&app, post(format!("{page}/import"), vec![])).await).await;
    let found = rows(&body);
    assert_eq!(found.len(), 1);
    let row = found[0];
    assert!(row.contains("<legend>Suola</legend>"));
    assert!(row.contains(r#"<option value="new:0">Create Mausteet</option>"#));
    assert!(
        !row.contains(r#"value="skip""#),
        "an ingredient has no skip"
    );
    let mut answers = vec![("fingerprint".to_owned(), fingerprint(&body))];
    answers.extend(
        answer_fields(&body)
            .into_iter()
            .map(|f| (f, "new:0".to_owned())),
    );
    let resp = call_service(&app, post(format!("{page}/import/confirm"), answers)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let to = location(&resp);
    let copy: Uuid = to
        .strip_prefix("/recipes/")
        .and_then(|rest| rest.strip_suffix("?created=1"))
        .and_then(|id| id.parse().ok())
        .expect("the copy, with the count");
    let shown = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&to)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(shown.contains("1 new category"));
    let names: Vec<String> = CategoryRepo::list_for_owner(&state.pool, friend)
        .await
        .expect("categories")
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, ["HEVI", "Maitotuotteet", "Leipä", "Mausteet"]);
    let ingredients = RecipeRepo::get(&state.pool, friend, copy)
        .await
        .expect("copy")
        .ingredients;
    let suola = ingredients
        .iter()
        .find(|i| i.text == "Suola")
        .expect("Suola");
    assert_eq!(
        suola.category.as_ref().map(|c| c.name.as_str()),
        Some("Mausteet")
    );
}

/// MB-0065 P2 (RR-0052): each chosen name is created once, last in the
/// recipient's order and in the order the review shows it; rows that chose
/// the same name share it; the summary counts it; none of the sharer's
/// ids is used.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn created_category_is_shared_by_rows() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let me = make_user(&state, "me@example.com").await;
    let friend = make_user(&state, "friend@example.com").await;
    let leipa = CategoryRepo::create(&state.pool, me, "Leipä")
        .await
        .expect("c")
        .id;
    let mausteet = CategoryRepo::create(&state.pool, me, "Mausteet")
        .await
        .expect("c")
        .id;
    for (name, category) in [("Ruisleipä", leipa), ("Kaura", leipa), ("Suola", mausteet)] {
        ProductRepo::create(&state.pool, me, name, category)
            .await
            .expect("favorite");
    }
    let my_hevi = CategoryRepo::create(&state.pool, friend, "HEVI")
        .await
        .expect("c")
        .id;
    CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", true, false)
        .await
        .expect("share")
        .expect("matched");
    let cookie =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);
    let base = format!("/sharing/from/{me}/favorites");
    let post = |uri: String, form: Vec<(String, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(cookie.clone())
            .set_form(form)
            .to_request()
    };

    let body = body_string(
        call_service(
            &app,
            post(
                format!("{base}/import"),
                vec![("all".to_owned(), "1".to_owned())],
            ),
        )
        .await,
    )
    .await;
    let legends: Vec<&str> = rows(&body)
        .iter()
        .filter_map(|r| r.split("<legend>").nth(1)?.split("</legend>").next())
        .collect();
    assert_eq!(legends, ["Ruisleipä", "Kaura", "Suola"]);
    let mut answers = vec![
        ("fingerprint".to_owned(), fingerprint(&body)),
        ("all".to_owned(), "1".to_owned()),
    ];
    answers.extend(
        answer_fields(&body)
            .into_iter()
            .map(|f| (f, "new:0".to_owned())),
    );
    let resp = call_service(&app, post(format!("{base}/import/confirm"), answers)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let done = location(&resp);
    assert_eq!(
        done,
        format!("{base}/import/done?added=3&already=0&favorite=0&moved=0&skipped=0&categories=2")
    );
    let summary = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&done)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(summary.contains("2 new categories"));

    let mine = CategoryRepo::list_for_owner(&state.pool, friend)
        .await
        .expect("categories");
    let names: Vec<&str> = mine.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        ["HEVI", "Leipä", "Mausteet"],
        "once each, last, in order"
    );
    assert_eq!(mine[0].id, my_hevi);
    for c in &mine[1..] {
        assert!(c.id != leipa && c.id != mausteet && c.owner_id == friend);
    }
    assert_eq!(
        favorites(&state, friend).await,
        [
            (
                Some("Leipä".to_owned()),
                vec!["Ruisleipä".to_owned(), "Kaura".to_owned()]
            ),
            (Some("Mausteet".to_owned()), vec!["Suola".to_owned()]),
        ]
    );
}

/// MB-0065 P3 (RR-0052): a "Create" answer for a name that isn't in the
/// plan (an index out of range, not a number, or on a row with nothing to
/// create) is refused and saves nothing; a category of that name made
/// between the review and the confirmation shows the review again
/// ("changed"), and no second category is created.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn create_race_and_unknown_name() {
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
        .expect("c")
        .id;
    let leipa = CategoryRepo::create(&state.pool, me, "Leipä")
        .await
        .expect("c")
        .id;
    for (name, category) in [("Omena", hevi), ("Ruisleipä", leipa)] {
        ProductRepo::create(&state.pool, me, name, category)
            .await
            .expect("favorite");
    }
    let my_hevi = CategoryRepo::create(&state.pool, friend, "HEVI")
        .await
        .expect("c")
        .id;
    let fruit = CategoryRepo::create(&state.pool, friend, "Hedelmät")
        .await
        .expect("c")
        .id;
    ProductRepo::create(&state.pool, friend, "omena", fruit)
        .await
        .expect("their Omena");
    CatalogShareRepo::share_catalog(&state.pool, me, "friend@example.com", true, false)
        .await
        .expect("share")
        .expect("matched");
    let cookie =
        session_cookie(&call_service(&app, login_request("friend@example.com").to_request()).await);
    let base = format!("/sharing/from/{me}/favorites");
    let post = |uri: String, form: Vec<(String, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(cookie.clone())
            .set_form(form)
            .to_request()
    };
    let before = favorites(&state, friend).await;
    let categories = || async {
        CategoryRepo::list_for_owner(&state.pool, friend)
            .await
            .expect("categories")
            .into_iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
    };

    // Omena: a conflict, nothing to create; Ruisleipä: "Create Leipä".
    let body = body_string(
        call_service(
            &app,
            post(
                format!("{base}/import"),
                vec![("all".to_owned(), "1".to_owned())],
            ),
        )
        .await,
    )
    .await;
    let found = rows(&body);
    assert!(found[0].contains("<legend>Omena</legend>") && !found[0].contains("new:"));
    assert!(found[1].contains(r#"value="new:0""#));
    let fields = answer_fields(&body);
    let confirm = |omena: String, ruis: &str| {
        vec![
            ("fingerprint".to_owned(), fingerprint(&body)),
            ("all".to_owned(), "1".to_owned()),
            (fields[0].clone(), omena),
            (fields[1].clone(), ruis.to_owned()),
        ]
    };

    for (omena, ruis) in [
        (my_hevi.to_string(), "new:1"),
        (my_hevi.to_string(), "new:x"),
        (my_hevi.to_string(), "new:"),
        (my_hevi.to_string(), "new:-1"),
        ("new:0".to_owned(), "new:0"),
    ] {
        let resp = call_service(
            &app,
            post(
                format!("{base}/import/confirm"),
                confirm(omena.clone(), ruis),
            ),
        )
        .await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "{omena} / {ruis} refused"
        );
        assert_eq!(favorites(&state, friend).await, before, "nothing saved");
        assert_eq!(categories().await, ["HEVI", "Hedelmät"], "nothing created");
    }

    // The race: "LEIPÄ" made meanwhile; the review is shown again.
    CategoryRepo::create(&state.pool, friend, "LEIPÄ")
        .await
        .expect("made meanwhile");
    let resp = call_service(
        &app,
        post(
            format!("{base}/import/confirm"),
            confirm(my_hevi.to_string(), "new:0"),
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let again = body_string(resp).await;
    assert!(again.contains("changed since"));
    assert!(!again.contains("Create Leipä"), "the name now matches");
    assert_eq!(favorites(&state, friend).await, before, "nothing saved");
    assert_eq!(
        categories().await,
        ["HEVI", "Hedelmät", "LEIPÄ"],
        "no second one"
    );
}
