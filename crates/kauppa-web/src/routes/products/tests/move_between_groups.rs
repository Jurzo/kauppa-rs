//! MB-0071 P2: `POST /favorites/order` with `moved` saves a favorite dragged
//! into another group: its new category and the group's full new order,
//! together, only if the order is exactly that group's favorites plus the
//! moved one. The catalog comes back with both groups open (RR-0044 item 1);
//! another user's favorite or category is 404.

use actix_web::http::StatusCode;
use actix_web::test::{TestRequest, call_service};
use uuid::Uuid;

use kauppa_core::domain::NewIngredient;
use kauppa_core::repo::{ProductRepo, RecipeRepo};

use super::{catalog_shape, open_units, retarget};
use crate::routes::test_support::{
    body_string, login_request, make_category, make_recipe, make_user, session_cookie, test_app,
    test_state,
};

/// The order form's fields: `category`, then `moved` if any, then the ids.
fn fields(category: &str, moved: Option<&str>, order: &[Uuid]) -> Vec<(&'static str, String)> {
    let mut fields = vec![("category", category.to_owned())];
    if let Some(moved) = moved {
        fields.push(("moved", moved.to_owned()));
    }
    fields.extend(order.iter().map(|id| ("id", id.to_string())));
    fields
}

#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn move_between_groups() {
    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "X").await;
    let favorite = |owner, name: &'static str, category| {
        let state = state.clone();
        async move {
            ProductRepo::create(&state.pool, owner, name, category)
                .await
                .expect("favorite")
                .id
        }
    };
    let omena = favorite(user, "Omena", hevi).await;
    let kurkku = favorite(user, "Kurkku", hevi).await;
    let maito = favorite(user, "Maito", kylma).await;
    let voi = favorite(user, "Voi", kylma).await;
    let leipa = favorite(other, "Leipä", theirs).await;
    // A recipe-created product in Kylmä.
    let recipe = make_recipe(&state, user, "Salaatti").await;
    let tilli = RecipeRepo::add_ingredient(
        &state.pool,
        user,
        recipe,
        &NewIngredient {
            product_name: "Tilli".to_owned(),
            quantity: None,
            category_id: Some(kylma),
        },
    )
    .await
    .expect("hidden product")
    .product_id
    .expect("linked");

    let cookie =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |fields: Vec<(&'static str, String)>, hx: bool| {
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
    let shape = || async { catalog_shape(&state, user).await };
    let names = |list: &[&str]| list.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    let (hevi_key, kylma_key) = (hevi.to_string(), kylma.to_string());
    let (group_hevi, group_kylma) = (format!("group-{hevi}"), format!("group-{kylma}"));

    // Maito into Hevi, between Omena and Kurkku: saved; the catalog comes
    // back (retargeted) with Kylmä, which it left, and Hevi open.
    let maito_id = maito.to_string();
    let resp = call_service(
        &app,
        post(
            fields(&hevi_key, Some(&maito_id), &[omena, maito, kurkku]),
            true,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(retarget(&resp).as_deref(), Some("#favorites"));
    let body = body_string(resp).await;
    assert_eq!(open_units(&body), [group_hevi.clone(), group_kylma.clone()]);
    let after_move = vec![
        (
            Some("Hevi".to_owned()),
            names(&["Omena", "Maito", "Kurkku"]),
        ),
        (Some("Kylmä".to_owned()), names(&["Voi"])),
    ];
    assert_eq!(shape().await, after_move);

    // Not exactly Kylmä's favorites plus Maito: nothing saved, the catalog
    // with both groups open.
    for order in [
        vec![voi],
        vec![maito, voi, omena],
        vec![maito, voi, voi],
        vec![maito, voi, Uuid::new_v4()],
    ] {
        let resp = call_service(
            &app,
            post(fields(&kylma_key, Some(&maito_id), &order), true),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{order:?}");
        let body = body_string(resp).await;
        assert_eq!(
            open_units(&body),
            [group_hevi.clone(), group_kylma.clone()],
            "{order:?}"
        );
        assert_eq!(shape().await, after_move, "{order:?}");
    }
    // Into its own group, or a recipe-created product: not a move.
    for (moved, order) in [
        (maito, vec![omena, maito, kurkku]),
        (tilli, vec![omena, tilli, maito, kurkku]),
    ] {
        let resp = call_service(
            &app,
            post(fields(&hevi_key, Some(&moved.to_string()), &order), true),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{moved}");
        assert_eq!(shape().await, after_move, "{moved}");
    }
    // Without `moved`, the same order is a reorder of Kylmä and refused, as
    // before (MB-0066).
    let resp = call_service(&app, post(fields(&kylma_key, None, &[omena, voi]), true)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(shape().await, after_move);

    // Another user's favorite or category, or a malformed `moved`: 404.
    for (category, moved, order) in [
        (
            hevi_key.clone(),
            leipa.to_string(),
            vec![omena, leipa, maito, kurkku],
        ),
        (theirs.to_string(), voi.to_string(), vec![voi]),
        (hevi_key.clone(), "x".to_owned(), vec![omena, maito, kurkku]),
    ] {
        let resp = call_service(&app, post(fields(&category, Some(&moved), &order), true)).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{category} {moved}");
        assert_eq!(shape().await, after_move, "{category} {moved}");
    }
    assert_eq!(
        catalog_shape(&state, other).await,
        vec![(Some("X".to_owned()), names(&["Leipä"]))]
    );

    // Into Uncategorized, without htmx: the category is cleared, and the
    // redirect opens both groups. Kylmä, now empty, isn't shown.
    let voi_id = voi.to_string();
    let resp = call_service(&app, post(fields("none", Some(&voi_id), &[voi]), false)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get("location").and_then(|v| v.to_str().ok()),
        Some("/favorites?open=group-uncategorized")
    );
    assert_eq!(
        shape().await,
        vec![
            (
                Some("Hevi".to_owned()),
                names(&["Omena", "Maito", "Kurkku"])
            ),
            (None, names(&["Voi"])),
        ]
    );
}
