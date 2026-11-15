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
