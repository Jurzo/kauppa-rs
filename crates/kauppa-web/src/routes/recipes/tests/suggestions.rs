//! MB-0013 (P2, P3): ingredient name suggestions and the match hint.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::domain::NewIngredient;
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo};

#[cfg(test)]
use crate::routes::test_support::make_recipe;
use crate::state::AppState;

use super::ingredient_names;

/// A recipe-created (hidden) product, through a scratch recipe.
#[allow(clippy::expect_used)]
async fn make_hidden(state: &AppState, user: Uuid, name: &str, category: Uuid) {
    let scratch = make_recipe(state, user, "Luonnos").await;
    RecipeRepo::add_ingredient(
        &state.pool,
        user,
        scratch,
        &NewIngredient {
            product_name: name.to_owned(),
            quantity: None,
            category_id: Some(category),
        },
    )
    .await
    .expect("create hidden product");
}

/// The `value`s of the ingredient-name `<datalist>`'s options, in order.
#[allow(clippy::expect_used)]
fn suggestion_options(body: &str) -> Vec<String> {
    let start = body
        .find(r#"<datalist id="ingredient-suggestions">"#)
        .expect("datalist");
    let end = start + body[start..].find("</datalist>").expect("closed");
    body[start..end]
        .split(r#"<option value=""#)
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// P2: the editor's ingredient name input offers the user's product names —
/// favorites and recipe-created ones, sorted ignoring case — through a native
/// `<datalist>` linked by `list=`. The list is rendered once per page load,
/// outside the `#ingredients` fragment, whose input still links to it; a
/// product created by an ingredient is offered from the next load. Another
/// user's products never appear.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn ingredient_suggestions() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let theirs = make_category(&state, other, "Hevi").await;
    ProductRepo::create(&state.pool, user, "Maito", kylma)
        .await
        .expect("create favorite");
    ProductRepo::create(&state.pool, user, "äyräs", hevi)
        .await
        .expect("create favorite");
    make_hidden(&state, user, "Kaura", hevi).await;
    ProductRepo::create(&state.pool, other, "Salaatti", theirs)
        .await
        .expect("create their favorite");
    make_hidden(&state, other, "Zucchini", theirs).await;
    let recipe = make_recipe(&state, user, "Puuro").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

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
    assert!(body.contains("<title>Puuro — kauppa</title>"), "full page");
    assert!(body.contains(r#"list="ingredient-suggestions""#), "linked");
    assert_eq!(suggestion_options(&body), ["Kaura", "Maito", "äyräs"]);
    for theirs in ["Salaatti", "Zucchini"] {
        assert!(!body.contains(theirs), "{theirs} is another user's");
    }

    // The htmx fragment keeps the link but doesn't re-send the list ...
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/ingredients"))
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .set_form([
                ("name", "banaani".to_owned()),
                ("category_id", hevi.to_string()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html"), "fragment only");
    assert!(body.contains(r#"list="ingredient-suggestions""#));
    assert!(!body.contains("<datalist"), "sent once per page");

    // ... and the next load offers the product the ingredient created.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{recipe}"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert_eq!(
        suggestion_options(&body),
        ["banaani", "Kaura", "Maito", "äyräs"]
    );
    assert!(!body.contains("Salaatti") && !body.contains("Zucchini"));

    // The other user's editor suggests only their own products.
    let their_recipe = make_recipe(&state, other, "Keitto").await;
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{their_recipe}"))
            .cookie(intruder)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert_eq!(suggestion_options(&body), ["Salaatti", "Zucchini"]);
    // ("Maito" is also the input placeholder, so check the others.)
    for mine in ["Kaura", "äyräs", "banaani"] {
        assert!(!body.contains(mine), "{mine} is another user's");
    }
}

/// Percent-encode a query value (tests only; every non-alphanumeric byte).
fn query_value(raw: &str) -> String {
    use std::fmt::Write;

    raw.bytes().fold(String::new(), |mut out, b| {
        if b.is_ascii_alphanumeric() {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
        out
    })
}

/// P3: the match hint names an existing product's category (any case, hidden
/// products too), says a new name's category is asked for after adding
/// (MB-0039), flags an uncategorized product, and is empty for a blank name.
/// It is an htmx fragment wired to the name input; without htmx it is a 303
/// to the editor, and another user's recipe is a 404. Without JavaScript the
/// form still submits and is validated as before, and the full page shows the
/// hint for the typed name after a rejection (and the category prompt, not
/// the hint, for a new name).
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn ingredient_match_hint() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    const EMPTY_HINT: &str =
        r#"<p id="ingredient-match" class="match-hint" aria-live="polite"></p>"#;

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let gone = make_category(&state, user, "Poistuva").await;
    let theirs = make_category(&state, other, "Pakasteet").await;
    ProductRepo::create(&state.pool, user, "Maito", kylma)
        .await
        .expect("create favorite");
    make_hidden(&state, user, "Kaura", hevi).await;
    ProductRepo::create(&state.pool, user, "Kahvi", gone)
        .await
        .expect("create favorite");
    CategoryRepo::delete(&state.pool, user, gone)
        .await
        .expect("delete category");
    ProductRepo::create(&state.pool, other, "Herne", theirs)
        .await
        .expect("create their favorite");
    let recipe = make_recipe(&state, user, "Puuro").await;
    let their_recipe = make_recipe(&state, other, "Hernekeitto").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let hint_uri = |recipe: Uuid, name: &str| {
        format!(
            "/recipes/{recipe}/ingredients/match?name={}",
            query_value(name)
        )
    };
    let hint = |uri: String| {
        TestRequest::get()
            .uri(&uri)
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
    };

    // The editor wires the name input to the hint, which starts empty.
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
    assert!(body.contains("<title>Puuro — kauppa</title>"), "full page");
    for attr in [
        format!(r#"hx-get="/recipes/{recipe}/ingredients/match""#),
        r#"hx-trigger="input changed delay:300ms""#.to_owned(),
        r##"hx-target="#ingredient-match" hx-swap="outerHTML""##.to_owned(),
        EMPTY_HINT.to_owned(),
    ] {
        assert!(body.contains(&attr), "editor should carry {attr}");
    }

    let cases = [
        ("maito", "Uses Kylmä, an existing product."),
        ("  MAITO ", "Uses Kylmä"),
        ("kAURA", "Uses Hevi"),
        ("kahvi", "Kahvi has no category"),
        (
            "Kaurajuoma",
            "New product — you'll choose its category after adding.",
        ),
        ("Herne", "New product — you'll choose its category"),
    ];
    for (name, expected) in cases {
        let resp = call_service(&app, hint(hint_uri(recipe, name)).to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK, "{name:?}");
        let body = body_string(resp).await;
        assert!(!body.contains("<html"), "fragment only");
        assert!(body.starts_with(r#"<p id="ingredient-match""#), "{body}");
        assert!(body.contains(expected), "{name:?} should say {expected:?}");
        assert!(!body.contains("Pakasteet"), "never another user's category");
    }
    for blank in ["", "   "] {
        let resp = call_service(&app, hint(hint_uri(recipe, blank)).to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_string(resp).await.trim_end(), EMPTY_HINT);
    }

    // Without htmx: back to the editor.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&hint_uri(recipe, "maito"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        format!("/recipes/{recipe}").as_str()
    );

    // Another user's recipe is a 404, with or without htmx; signed out, 401.
    for hx in [false, true] {
        let mut req = TestRequest::get()
            .uri(&hint_uri(their_recipe, "Herne"))
            .cookie(session.clone());
        if hx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "htmx: {hx}");
    }
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&hint_uri(recipe, "maito"))
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Without JavaScript the form still submits, validated as before: an
    // existing product ignores the picker and keeps its category ...
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/ingredients"))
            .cookie(session.clone())
            .set_form([
                ("name", "MAITO".to_owned()),
                ("category_id", hevi.to_string()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(ingredient_names(&state, user, recipe).await, ["Maito"]);
    let maito = ProductRepo::find_by_name(&state.pool, user, "maito")
        .await
        .expect("lookup")
        .expect("exists");
    assert_eq!(maito.category_id, Some(kylma), "the picker didn't apply");

    // ... a rejected name re-renders the full page with its hint ...
    let post = |name: &'static str| {
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/ingredients"))
            .cookie(session.clone())
            .set_form([("name", name)])
            .to_request()
    };
    let resp = call_service(&app, post("kahvi")).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Puuro — kauppa</title>"), "full page");
    assert!(body.contains(
        r#"aria-live="polite">Kahvi has no category — choose one for it on Favorites first.</p>"#
    ));

    // ... and a new name the category prompt in place of the hint (MB-0039).
    let resp = call_service(&app, post("Kaurajuoma")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Puuro — kauppa</title>"), "full page");
    assert!(body.contains("Kaurajuoma is a new product — choose its category"));
    assert!(body.contains(EMPTY_HINT), "the prompt replaces the hint");
    assert_eq!(ingredient_names(&state, user, recipe).await, ["Maito"]);
}
