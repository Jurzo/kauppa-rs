//! MB-0058 (RR-0043, RR-0046): one Save for a product's name and category.

use actix_web::http::StatusCode;
use actix_web::test::{TestRequest, call_service};
use uuid::Uuid;

use kauppa_core::repo::{ProductRepo, RecipeRepo};

use crate::routes::products::NO_CATEGORY;
use crate::routes::test_support::{
    body_string, login_request, make_category, make_recipe_with, make_user, session_cookie,
    test_app, test_state,
};

use super::catalog_shape;

#[allow(clippy::expect_used)]
async fn id_of(state: &crate::state::AppState, user: Uuid, name: &str) -> Uuid {
    ProductRepo::find_by_name(&state.pool, user, name)
        .await
        .expect("find product")
        .expect("product exists")
        .id
}

/// The `<details class="product-edit">` that holds `id`'s Save form.
#[allow(clippy::expect_used)]
fn edit_section(body: &str, id: Uuid) -> &str {
    let action = format!(r#"action="/favorites/{id}""#);
    let at = body.find(&action).expect("the product's edit form");
    let start = body[..at].rfind("<details").expect("its edit section");
    let end = at + body[at..].find("</details>").expect("section end");
    &body[start..end]
}

fn owned(groups: &[(&str, &[&str])]) -> Vec<(Option<String>, Vec<String>)> {
    groups
        .iter()
        .map(|(g, names)| {
            (
                (!g.is_empty()).then(|| (*g).to_owned()),
                names.iter().map(|n| (*n).to_owned()).collect(),
            )
        })
        .collect()
}

/// P1: one form with the name, the picker ("No category" plus the
/// categories, the current choice selected) and one Save; the only other
/// control is Delete: in the edit section of a recipe product, on the row
/// of a favorite (MB-0068, RR-0055 sub-answer 3).
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn one_save_button() {
    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    ProductRepo::create(&state.pool, user, "Omena", hevi)
        .await
        .expect("create");
    let salaatti = make_recipe_with(&state, user, "Salaatti", hevi, &[("Kurkku", None)]).await;
    let kurkku = id_of(&state, user, "Kurkku").await;
    // An uncategorized recipe product: its category goes with Hevi.
    let other_cat = make_category(&state, user, "Poistuva").await;
    let kerma = {
        let new = kauppa_core::domain::NewIngredient {
            product_name: "Kerma".to_owned(),
            quantity: None,
            category_id: Some(other_cat),
        };
        RecipeRepo::add_ingredient(&state.pool, user, salaatti, &new)
            .await
            .expect("add ingredient");
        id_of(&state, user, "Kerma").await
    };
    kauppa_core::repo::CategoryRepo::delete(&state.pool, user, other_cat)
        .await
        .expect("delete category");
    let omena = id_of(&state, user, "Omena").await;

    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/favorites")
                .cookie(session)
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(
        !page.contains("/rename\"") && !page.contains("/category\""),
        "old routes gone"
    );

    for (id, selected, on_row) in [
        (omena, hevi.to_string(), true),
        (kurkku, hevi.to_string(), false),
    ] {
        let section = edit_section(&page, id);
        let forms = if on_row { 1 } else { 2 };
        assert_eq!(
            section.matches("<form").count(),
            forms,
            "Save, and Delete unless it is on the row: {section}"
        );
        assert_eq!(
            section.matches(r#"type="submit""#).count(),
            forms,
            "{section}"
        );
        let delete = format!(r#"action="/favorites/{id}/delete""#);
        assert_eq!(section.contains(&delete), !on_row, "{section}");
        let form = page
            .find(&format!(r#"action="/favorites/{id}""#))
            .expect("its form");
        let row_start = page[..form].rfind("<li").expect("its row");
        let row = &page[row_start..row_start + page[row_start..].find("</li>").expect("row end")];
        assert_eq!(row.matches(&delete).count(), 1, "one Delete per row: {row}");
        assert!(section.contains(r#"name="name""#) && section.contains(">Save</button>"));
        assert!(section.contains(&format!(
            r#"<option value="{NO_CATEGORY}">No category</option>"#
        )));
        assert!(section.contains(&format!(
            r#"<option value="{selected}" selected>Hevi</option>"#
        )));
        assert!(!section.contains("Choose a category"), "no placeholder");
    }
    let section = edit_section(&page, kerma);
    assert!(
        section.contains(&format!(
            r#"<option value="{NO_CATEGORY}" selected>No category</option>"#
        )),
        "an uncategorized product opens on No category: {section}"
    );
    // The add form keeps its placeholder and has no "No category".
    let add = &page[..page.find("product-group").unwrap_or(page.len())];
    assert!(add.contains("Choose a category") && !add.contains(r#"value="none""#));
}

/// P2: one Save stores both; a rejected part (a taken name, an empty name, a
/// foreign or unknown category) changes neither and shows the error, over
/// htmx (200, the fragment) and without it (the full page, 4xx).
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn saves_name_and_category_together() {
    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "Pakasteet").await;
    for (name, cat) in [("Omena", hevi), ("Maito", kylma)] {
        ProductRepo::create(&state.pool, user, name, cat)
            .await
            .expect("create");
    }
    let omena = id_of(&state, user, "Omena").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let save = |id: Uuid, form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&format!("/favorites/{id}"))
            .cookie(session.clone())
            .set_form(form)
    };
    let (hevi_s, kylma_s) = (hevi.to_string(), kylma.to_string());

    let resp = call_service(
        &app,
        save(
            omena,
            vec![
                ("name", "Vihreä omena".into()),
                ("category_id", kylma_s.clone()),
            ],
        )
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains("Vihreä omena"));
    assert_eq!(
        catalog_shape(&state, user).await,
        owned(&[("Kylmä", &["Maito", "Vihreä omena"])])
    );

    // Each rejection, over htmx: 200 with the error, nothing changed.
    let rejected = [
        (
            vec![("name", "MAITO".into()), ("category_id", hevi_s.clone())],
            "product already exists",
        ),
        (
            vec![("name", "  ".into()), ("category_id", hevi_s.clone())],
            "role=\"alert\"",
        ),
        (
            vec![
                ("name", "Omppu".into()),
                ("category_id", theirs.to_string()),
            ],
            "unknown category",
        ),
        (
            vec![
                ("name", "Omppu".into()),
                ("category_id", Uuid::new_v4().to_string()),
            ],
            "unknown category",
        ),
        (
            vec![
                ("name", "Omppu".into()),
                ("category_id", "not-a-uuid".into()),
            ],
            "unknown category",
        ),
    ];
    for (form, message) in rejected {
        let resp = call_service(
            &app,
            save(omena, form.clone())
                .insert_header(("HX-Request", "true"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{form:?}");
        let body = body_string(resp).await;
        assert!(
            body.contains("role=\"alert\"") && body.contains(message),
            "{form:?}: {body}"
        );
        assert_eq!(
            catalog_shape(&state, user).await,
            owned(&[("Kylmä", &["Maito", "Vihreä omena"])]),
            "{form:?} changed nothing"
        );
    }

    // Without htmx: success is a 303 back; a rejection is the full page.
    let resp = call_service(
        &app,
        save(
            omena,
            vec![("name", "Omena".into()), ("category_id", hevi_s.clone())],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = call_service(
        &app,
        save(
            omena,
            vec![("name", "Maito".into()), ("category_id", kylma_s)],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert!(body_string(resp).await.contains("<html"));
    assert_eq!(
        catalog_shape(&state, user).await,
        owned(&[("Hevi", &["Omena"]), ("Kylmä", &["Maito"])])
    );

    // Another user's product: NotFound, unchanged.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/favorites/{omena}"))
            .cookie(session_cookie(
                &call_service(&app, login_request("other@example.com").to_request()).await,
            ))
            .set_form([("name", "Hax"), ("category_id", NO_CATEGORY)])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        catalog_shape(&state, user).await,
        owned(&[("Hevi", &["Omena"]), ("Kylmä", &["Maito"])])
    );
}

/// P3: "No category" clears it (a favorite moves to Uncategorized; a recipe
/// product stays uncategorized and can be renamed); the current choice changes
/// only the name; a missing or empty `category_id` is rejected and changes
/// nothing (RR-0043 constraint 2); recipe ingredients keep their link.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn no_category_and_unchanged_fields() {
    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    for name in ["Omena", "Banaani"] {
        ProductRepo::create(&state.pool, user, name, hevi)
            .await
            .expect("create");
    }
    let salaatti = make_recipe_with(&state, user, "Salaatti", hevi, &[("Kurkku", Some("1"))]).await;
    let omena = id_of(&state, user, "Omena").await;
    let banaani = id_of(&state, user, "Banaani").await;
    let kurkku = id_of(&state, user, "Kurkku").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let save = |id: Uuid, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(&format!("/favorites/{id}"))
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .set_form(form)
            .to_request()
    };
    let hevi_s = hevi.to_string();

    // Missing and empty category: rejected, nothing changes.
    for form in [
        vec![("name", "Omppu")],
        vec![("name", "Omppu"), ("category_id", "")],
    ] {
        let resp = call_service(&app, save(omena, &form)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert!(
            body.contains("role=\"alert\"") && body.contains("choose a category"),
            "{body}"
        );
    }
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/favorites/{omena}"))
            .cookie(session.clone())
            .set_form([("name", "Omppu")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        catalog_shape(&state, user).await,
        owned(&[("Hevi", &["Omena", "Banaani"])])
    );

    // The current choice: only the name changes, in place.
    call_service(
        &app,
        save(omena, &[("name", "Omenat"), ("category_id", &hevi_s)]),
    )
    .await;
    assert_eq!(
        catalog_shape(&state, user).await,
        owned(&[("Hevi", &["Omenat", "Banaani"])])
    );

    // "No category": the favorite moves to Uncategorized, still a favorite.
    let resp = call_service(
        &app,
        save(
            banaani,
            &[("name", "Banaani"), ("category_id", NO_CATEGORY)],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        catalog_shape(&state, user).await,
        owned(&[("Hevi", &["Omenat"]), ("", &["Banaani"])])
    );
    let banaani_now = ProductRepo::find_by_name(&state.pool, user, "Banaani")
        .await
        .expect("find")
        .expect("exists");
    assert!(banaani_now.is_favorite && banaani_now.category_id.is_none());

    // A recipe product: cleared, then renamed while uncategorized; the
    // recipe's ingredient keeps its link and follows the name.
    call_service(
        &app,
        save(kurkku, &[("name", "Kurkku"), ("category_id", NO_CATEGORY)]),
    )
    .await;
    call_service(
        &app,
        save(kurkku, &[("name", "Kurkut"), ("category_id", NO_CATEGORY)]),
    )
    .await;
    let renamed = ProductRepo::find_by_name(&state.pool, user, "Kurkut")
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(renamed.id, kurkku);
    assert!(!renamed.is_favorite && renamed.category_id.is_none());
    let recipe = RecipeRepo::get(&state.pool, user, salaatti)
        .await
        .expect("get recipe");
    assert_eq!(recipe.ingredients[0].product_id, Some(kurkku));
}
