//! Tests for `routes::products`, by area (RR-0009). Fixtures shared by several areas live here.

mod catalog;
mod edit;
mod hidden;

use uuid::Uuid;

use kauppa_core::repo::ProductRepo;

use crate::state::AppState;

/// Product names per group, in display order; `None` is uncategorized.
#[allow(clippy::expect_used)]
async fn catalog_shape(state: &AppState, user: Uuid) -> Vec<(Option<String>, Vec<String>)> {
    ProductRepo::list_for_owner(&state.pool, user)
        .await
        .expect("list products")
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.products.into_iter().map(|p| p.name).collect(),
            )
        })
        .collect()
}

/// The catalog units (MB-0059) in `body`, in order, as `(dom id, open)`.
fn units(body: &str) -> Vec<(String, bool)> {
    body.match_indices("<details id=\"")
        .filter_map(|(at, start)| {
            let rest = &body[at + start.len()..];
            let id_end = rest.find('"')?;
            let tag = &rest[..rest.find('>')?];
            tag.contains("catalog-unit")
                .then(|| (rest[..id_end].to_owned(), tag.ends_with(" open")))
        })
        .collect()
}

/// The ids of the units `body` renders open.
fn open_units(body: &str) -> Vec<String> {
    units(body)
        .into_iter()
        .filter_map(|(id, open)| open.then_some(id))
        .collect()
}

/// The `HX-Retarget` header of an answer, if any.
fn retarget<B>(resp: &actix_web::dev::ServiceResponse<B>) -> Option<String> {
    resp.headers()
        .get("HX-Retarget")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// MB-0066 P3 (RR-0053 sub-answer 4): `POST /favorites/order` saves a full
/// new order of one group's favorites (a category, or `none` for
/// Uncategorized) only if it is exactly that group's favorites; hidden
/// products keep their place. Anything else saves nothing and the group
/// comes back as it is; another user's or a malformed category is 404.
/// With htmx only the group is swapped, open; without, a redirect opens it.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn order_within_category() {
    use actix_web::http::StatusCode;
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::domain::NewIngredient;
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_recipe, make_user, session_cookie,
        test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "X").await;
    let mut ids = Vec::new();
    for name in ["Omena", "Banaani", "Kurkku", "Maito"] {
        let category = if name == "Maito" { kylma } else { hevi };
        ids.push(
            ProductRepo::create(&state.pool, user, name, category)
                .await
                .expect("favorite")
                .id,
        );
    }
    let (omena, banaani, kurkku, maito) = (ids[0], ids[1], ids[2], ids[3]);
    // A hidden product in Hevi, between the favorites' slots.
    let recipe = make_recipe(&state, user, "Salaatti").await;
    RecipeRepo::add_ingredient(
        &state.pool,
        user,
        recipe,
        &NewIngredient {
            product_name: "Tilli".to_owned(),
            quantity: None,
            category_id: Some(hevi),
        },
    )
    .await
    .expect("hidden product");
    // Two uncategorized favorites.
    let mut loose = Vec::new();
    for name in ["Kynttilät", "Paristot"] {
        let id = ProductRepo::create(&state.pool, user, name, kylma)
            .await
            .expect("favorite")
            .id;
        ProductRepo::update(&state.pool, user, id, name, None)
            .await
            .expect("uncategorized");
        loose.push(id);
    }

    let cookie =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |category: Option<String>, order: &[Uuid], hx: bool| {
        let mut fields: Vec<(&'static str, String)> = Vec::new();
        if let Some(category) = category {
            fields.push(("category", category));
        }
        fields.extend(order.iter().map(|id| ("id", id.to_string())));
        let req = TestRequest::post()
            .uri("/favorites/order")
            .cookie(cookie.clone())
            .set_form(fields);
        if hx {
            req.insert_header(("HX-Request", "true")).to_request()
        } else {
            req.to_request()
        }
    };
    let hevi_key = Some(hevi.to_string());

    let resp = call_service(
        &app,
        post(hevi_key.clone(), &[kurkku, omena, banaani], true),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert_eq!(
        units(&body),
        [(format!("group-{hevi}"), true)],
        "only the group, open"
    );
    assert!(body.contains(&format!(r#"form="group-{hevi}-order""#)));
    let shape = || async { catalog_shape(&state, user).await };
    let hevi_after = vec![
        "Kurkku".to_owned(),
        "Omena".to_owned(),
        "Banaani".to_owned(),
    ];
    assert_eq!(
        shape().await[0],
        (Some("Hevi".to_owned()), hevi_after.clone())
    );
    let hidden = ProductRepo::list_hidden_with_usage(&state.pool, user)
        .await
        .expect("hidden");
    assert_eq!(format!("{hidden:?}").matches("Tilli").count(), 1, "kept");

    // Not exactly Hevi's favorites: nothing saved, the group as it is.
    for order in [
        vec![kurkku, omena],
        vec![kurkku, omena, banaani, maito],
        vec![kurkku, omena, omena],
        vec![kurkku, omena, Uuid::new_v4()],
    ] {
        let resp = call_service(&app, post(hevi_key.clone(), &order, true)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{order:?}");
        assert_eq!(shape().await[0].1, hevi_after, "{order:?}");
    }
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/favorites/order")
            .cookie(cookie.clone())
            .set_form([("category", hevi.to_string()), ("id", "x".to_owned())])
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "a malformed id saves nothing"
    );
    assert_eq!(shape().await[0].1, hevi_after);

    // Another user's, a malformed or a missing category: 404.
    for category in [Some(theirs.to_string()), Some("x".to_owned()), None] {
        let resp = call_service(&app, post(category.clone(), &[], true)).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{category:?}");
    }

    // Uncategorized, without htmx: saved, redirected with the group open.
    let resp = call_service(
        &app,
        post(Some("none".to_owned()), &[loose[1], loose[0]], false),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get("location").and_then(|v| v.to_str().ok()),
        Some("/favorites?open=group-uncategorized")
    );
    let groups = shape().await;
    assert_eq!(
        groups.last(),
        Some(&(None, vec!["Paristot".to_owned(), "Kynttilät".to_owned()]))
    );
    assert_eq!(groups[0].1, hevi_after, "other groups untouched");
}
