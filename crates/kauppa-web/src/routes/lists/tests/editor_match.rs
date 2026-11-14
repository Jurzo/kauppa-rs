//! MB-0036 follow-up (RR-0024): an `edit` grantee's typed text matching the
//! list owner's favorites, seen in the store view; there is no id-based add
//! (removed by MB-0048, RR-0036 sub-answer 2); a revoked editor is denied at
//! once.

use actix_web::http::StatusCode;

use kauppa_core::repo::ShoppingListRepo;

use super::{list_snapshot, shared_list};

/// RR-0024: the editor's "maito" lands in the store view under the owner's
/// Kylmä, for the editor and the owner alike. The old
/// `POST /lists/{id}/favorites` (by product id) is gone: 404 for the editor,
/// nothing changes. After a revoke, the editor's next add is a 404 (RR-0002
/// constraint 4).
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn editor_match_store_view_and_id_add() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let login = |email: &'static str| {
        let app = &app;
        async move { session_cookie(&call_service(app, login_request(email).to_request()).await) }
    };
    let owner = login("owner@example.com").await;
    let editor = login("editor@example.com").await;
    let base = format!("/lists/{}", s.list);
    let add = |text: &'static str| {
        TestRequest::post()
            .uri(&format!("{base}/items"))
            .cookie(editor.clone())
            .set_form([("text", text)])
            .to_request()
    };

    let resp = call_service(&app, add("MAITO")).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // The store view groups it under the owner's Kylmä.
    for (who, session) in [("editor", &editor), ("owner", &owner)] {
        for htmx in [false, true] {
            let mut req = TestRequest::get()
                .uri(&format!("{base}/shop"))
                .cookie(session.clone());
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::OK, "{who} htmx={htmx}");
            let body = body_string(resp).await;
            let start = body.find("<h2>Kylmä</h2>").expect("the owner's Kylmä");
            let end = start + body[start..].find("</section>").expect("group end");
            assert!(
                body[start..end].contains(r#"<span class="product-name">Maito</span>"#),
                "{who} htmx={htmx}: Maito under Kylmä"
            );
        }
    }

    // Adding by product id no longer exists (RR-0024 sub-answer 3, amended
    // by RR-0036).
    let before = list_snapshot(&state, &s).await;
    for htmx in [false, true] {
        let mut req = TestRequest::post()
            .uri(&format!("{base}/favorites"))
            .cookie(editor.clone())
            .set_form([("product_id", s.product.to_string())]);
        if htmx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "htmx={htmx}");
    }
    assert_eq!(list_snapshot(&state, &s).await, before, "nothing changed");

    // A revoked editor's next add is denied, as for any stranger.
    ShoppingListRepo::revoke(&state.pool, s.owner, s.list, s.editor_share)
        .await
        .expect("revoke");
    let before = list_snapshot(&state, &s).await;
    let resp = call_service(&app, add("maito")).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(list_snapshot(&state, &s).await, before, "nothing changed");
}
