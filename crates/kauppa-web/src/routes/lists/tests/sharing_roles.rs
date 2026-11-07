//! MB-0008 (P2–P4): what view and edit grantees can do, and owner-only administration.

use actix_web::http::{StatusCode, header};

use kauppa_core::domain::{ListAccess, ShareRole};
use kauppa_core::repo::ShoppingListRepo;

use super::{list_posts, list_snapshot, shared_list};

/// MB-0008 P2: a `view` grantee can read the list — the list page and the
/// store view, full page and fragment, and the index — without any edit
/// controls, and every mutation (add, check, remove, restore, categorize,
/// rename, delete, share, revoke) is a 403 over htmx or not, changing
/// nothing.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn view_role_is_read_only() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let viewer =
        session_cookie(&call_service(&app, login_request("viewer@example.com").to_request()).await);
    let base = format!("/lists/{}", s.list);

    for (uri, marker) in [(base.clone(), "Leipä"), (format!("{base}/shop"), "Leipä")] {
        for htmx in [false, true] {
            let mut req = TestRequest::get().uri(&uri).cookie(viewer.clone());
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::OK, "{uri} htmx={htmx}");
            let body = body_string(resp).await;
            assert_eq!(body.contains("<html"), !htmx, "{uri} htmx={htmx}");
            if !htmx {
                assert!(body.contains("<title>Kauppa — kauppa</title>"), "{uri}");
            }
            assert!(body.contains(marker), "{uri} htmx={htmx}: shows the items");
            // No form posts to the list (the layout's sign-out form is the
            // page's only form), and no link to its sharing page.
            for control in [
                format!(r#"action="{base}"#),
                "hx-post".to_owned(),
                format!("{base}/sharing"),
            ] {
                assert!(
                    !body.contains(&control),
                    "{uri} htmx={htmx}: a viewer sees no `{control}` control"
                );
            }
            let forms = body.matches(r#"method="post""#).count();
            assert_eq!(
                forms,
                usize::from(!htmx),
                "{uri} htmx={htmx}: sign-out only"
            );
        }
    }
    // The index lists the list as shared.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/lists")
            .cookie(viewer.clone())
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert!(body.contains("Shared with you"));
    assert!(body.contains(&format!(r#"href="/lists/{}""#, s.list)));
    assert!(body.contains("view only"));

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("{base}/sharing"))
            .cookie(viewer.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "sharing page");

    let before = list_snapshot(&state, &s).await;
    for (name, uri, form) in list_posts(&s) {
        for htmx in [false, true] {
            let mut req = TestRequest::post()
                .uri(&uri)
                .cookie(viewer.clone())
                .set_form(form.clone());
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{name} htmx={htmx}");
        }
    }
    assert_eq!(list_snapshot(&state, &s).await, before, "nothing changed");
    assert_eq!(
        ShoppingListRepo::access(&state.pool, s.viewer, s.list)
            .await
            .expect("still shared"),
        ListAccess::View
    );
}

/// MB-0008 P3: an `edit` grantee can add, check, remove and restore items,
/// but can't re-share, revoke, rename or delete the list, open its sharing
/// page, or use the owner's catalog on it (403s, nothing changes).
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn edit_role_cannot_administer() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let editor =
        session_cookie(&call_service(&app, login_request("editor@example.com").to_request()).await);
    let base = format!("/lists/{}", s.list);

    // The full page and the fragment offer the item controls only.
    for htmx in [false, true] {
        let mut req = TestRequest::get().uri(&base).cookie(editor.clone());
        if htmx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert_eq!(body.contains("<html"), !htmx);
        assert!(
            body.contains(&format!(r#"action="{base}/items""#)),
            "htmx={htmx}"
        );
        assert!(body.contains(&format!("{base}/items/{}/remove", s.bread)));
        assert!(body.contains(&format!("{base}/items/{}/restore", s.sugar)));
        for control in [
            format!(r#"action="{base}/favorites""#),
            format!(r#"action="{base}/recipes""#),
            r#"/category""#.to_owned(),
            format!("{base}/sharing"),
            format!("{base}/rename"),
        ] {
            assert!(!body.contains(&control), "htmx={htmx}: no `{control}`");
        }
    }

    // Item writes go through, as plain posts (303) and over htmx (200).
    let posts = list_posts(&s);
    let allowed = ["add_item", "check", "remove", "restore"];
    for (name, uri, form) in posts.iter().filter(|(n, ..)| allowed.contains(n)) {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(uri)
                .cookie(editor.clone())
                .set_form(form.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{name}");
    }
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/shop/items/{}/check", s.sugar))
            .cookie(editor.clone())
            .insert_header(("HX-Request", "true"))
            .set_form([("checked", "true")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "htmx check");
    let body = body_string(resp).await;
    assert!(body.contains(&format!(r#"id="shop-{}""#, s.list)));
    let active: Vec<(String, bool)> = ShoppingListRepo::items(&state.pool, s.owner, s.list, false)
        .await
        .expect("items")
        .into_iter()
        .map(|i| (i.text, i.checked))
        .collect();
    assert_eq!(
        active,
        [("Sokeri".to_owned(), true), ("Munat".to_owned(), false)],
        "bread checked then removed, sugar restored and checked, eggs added"
    );

    // Administration and the owner's catalog are refused.
    let before = list_snapshot(&state, &s).await;
    for (name, uri, form) in posts.iter().filter(|(n, ..)| !allowed.contains(n)) {
        if *name == "shop_remove" {
            continue; // an item write, covered by "remove"
        }
        for htmx in [false, true] {
            let mut req = TestRequest::post()
                .uri(uri)
                .cookie(editor.clone())
                .set_form(form.clone());
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{name} htmx={htmx}");
        }
    }
    // Revoking their own grant is administration too.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/shares/{}/revoke", s.editor_share))
            .cookie(editor.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "revoke own grant");
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("{base}/sharing"))
            .cookie(editor.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "sharing page");
    assert_eq!(list_snapshot(&state, &s).await, before, "nothing changed");
    assert_eq!(
        ShoppingListRepo::access(&state.pool, s.editor, s.list)
            .await
            .expect("still shared"),
        ListAccess::Edit
    );
}

/// MB-0008 P4: only the owner shares, revokes, renames and deletes. The owner
/// can do all four (and a revoke takes the grantee's access away); a stranger
/// gets 404 for each and a grantee 403.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn owner_only_administration() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    make_user(&state, "stranger@example.com").await;
    let invitee = make_user(&state, "invitee@example.com").await;
    let login = |email: &'static str| login_request(email).to_request();
    let owner = session_cookie(&call_service(&app, login("owner@example.com")).await);
    let stranger = session_cookie(&call_service(&app, login("stranger@example.com")).await);
    let viewer = session_cookie(&call_service(&app, login("viewer@example.com")).await);
    let editor = session_cookie(&call_service(&app, login("editor@example.com")).await);
    let base = format!("/lists/{}", s.list);
    let admin: Vec<(String, Vec<(&str, String)>)> = vec![
        (
            format!("{base}/shares"),
            vec![
                ("email", "invitee@example.com".to_owned()),
                ("role", "view".to_owned()),
            ],
        ),
        (format!("{base}/shares/{}/revoke", s.editor_share), vec![]),
        (
            format!("{base}/rename"),
            vec![("name", "Hijacked".to_owned())],
        ),
        (format!("{base}/delete"), vec![]),
    ];

    // Non-owners: a stranger can't tell the list exists; grantees are refused.
    let before = list_snapshot(&state, &s).await;
    for (who, cookie, status) in [
        ("stranger", &stranger, StatusCode::NOT_FOUND),
        ("viewer", &viewer, StatusCode::FORBIDDEN),
        ("editor", &editor, StatusCode::FORBIDDEN),
    ] {
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri(&format!("{base}/sharing"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), status, "{who} sharing page");
        for (uri, form) in &admin {
            for htmx in [false, true] {
                let mut req = TestRequest::post()
                    .uri(uri)
                    .cookie(cookie.clone())
                    .set_form(form.clone());
                if htmx {
                    req = req.insert_header(("HX-Request", "true"));
                }
                let resp = call_service(&app, req.to_request()).await;
                assert_eq!(resp.status(), status, "{who} {uri} htmx={htmx}");
            }
        }
    }
    assert_eq!(list_snapshot(&state, &s).await, before, "nothing changed");

    // The owner's sharing page lists who has access, as a full page.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("{base}/sharing"))
            .cookie(owner.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Kauppa — kauppa</title>"));
    assert!(body.contains("People with access (2)"));
    assert!(body.contains("viewer@example.com"));
    assert!(body.contains(&format!("{base}/shares/{}/revoke", s.editor_share)));
    // ...and the list page links to it for the owner only.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(owner.clone())
            .to_request(),
    )
    .await;
    assert!(body_string(resp).await.contains(&format!("{base}/sharing")));

    // Share: a 303 back to the sharing page; the grant exists.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&admin[0].0)
            .cookie(owner.clone())
            .set_form(admin[0].1.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        &format!("{base}/sharing?shared=1")
    );
    assert_eq!(
        ShoppingListRepo::access(&state.pool, invitee, s.list)
            .await
            .expect("invitee granted"),
        ListAccess::View
    );

    // Revoke over htmx: the grantee list fragment, without the editor; the
    // editor is denied on their next request.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&admin[1].0)
            .cookie(owner.clone())
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(&format!(r#"id="grantees-{}""#, s.list)));
    assert!(!body.contains("editor@example.com"));
    assert!(
        body.contains("People with access (2)"),
        "viewer and invitee"
    );
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(editor.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "revoked editor");

    // A grant id from another of the owner's lists isn't on this one.
    let other = ShoppingListRepo::create(&state.pool, s.owner, "Toinen")
        .await
        .expect("list")
        .id;
    let other_share = ShoppingListRepo::share(
        &state.pool,
        s.owner,
        other,
        "editor@example.com",
        ShareRole::Edit,
    )
    .await
    .expect("share")
    .expect("matched")
    .id;
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/shares/{other_share}/revoke"))
            .cookie(owner.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "foreign grant id");

    // Rename and delete: 303s; deleting takes every grant with it.
    for (uri, form) in &admin[2..] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(uri)
                .cookie(owner.clone())
                .set_form(form.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{uri}");
    }
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(viewer.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "deleted");
    assert!(
        ShoppingListRepo::shared_with(&state.pool, s.viewer)
            .await
            .expect("shared")
            .is_empty()
    );
}

/// MB-0021 P3: changing an item's quantity follows the list's roles through
/// the central access predicate. The owner and an `edit` grantee can (plain
/// 303, htmx fragment) and see the form; a `view` grantee gets 403 and a
/// stranger 404, over htmx or not, and the quantity is unchanged.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn quantity_follows_roles() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    make_user(&state, "stranger@example.com").await;
    let login = |email: &'static str| {
        let app = &app;
        async move { session_cookie(&call_service(app, login_request(email).to_request()).await) }
    };
    let owner = login("owner@example.com").await;
    let editor = login("editor@example.com").await;
    let viewer = login("viewer@example.com").await;
    let stranger = login("stranger@example.com").await;
    let base = format!("/lists/{}", s.list);
    let uri = format!("{base}/items/{}/quantity", s.bread);
    let quantity = || async {
        ShoppingListRepo::items(&state.pool, s.owner, s.list, false)
            .await
            .expect("items")
            .into_iter()
            .find(|i| i.id == s.bread)
            .expect("bread")
            .quantity
    };

    // The owner and the editor see the form; the viewer doesn't.
    for (who, session, offered) in [
        ("owner", &owner, true),
        ("editor", &editor, true),
        ("viewer", &viewer, false),
    ] {
        for htmx in [false, true] {
            let mut req = TestRequest::get().uri(&base).cookie(session.clone());
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let body = body_string(call_service(&app, req.to_request()).await).await;
            assert_eq!(
                body.contains(&format!(r#"action="{uri}""#)),
                offered,
                "{who} htmx={htmx}"
            );
        }
    }

    // The owner and the editor can change it, plain and over htmx.
    for (who, session, plain, hx) in [
        ("owner", &owner, "2", "3 kpl"),
        ("editor", &editor, "4", "5 kpl"),
    ] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&uri)
                .cookie(session.clone())
                .set_form([("quantity", plain)])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{who}");
        assert_eq!(quantity().await.as_deref(), Some(plain), "{who}");
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&uri)
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .set_form([("quantity", hx)])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{who} htmx");
        let body = body_string(resp).await;
        assert!(body.contains(&format!(r#"id="list-{}""#, s.list)), "{who}");
        assert!(body.contains(hx), "{who}");
        assert_eq!(quantity().await.as_deref(), Some(hx), "{who} htmx");
    }

    // A viewer is refused (403) and a stranger doesn't see the list (404);
    // nothing changes.
    let before = (quantity().await, list_snapshot(&state, &s).await);
    for (who, session, status) in [
        ("viewer", &viewer, StatusCode::FORBIDDEN),
        ("stranger", &stranger, StatusCode::NOT_FOUND),
    ] {
        for htmx in [false, true] {
            for value in ["9", ""] {
                let mut req = TestRequest::post()
                    .uri(&uri)
                    .cookie(session.clone())
                    .set_form([("quantity", value)]);
                if htmx {
                    req = req.insert_header(("HX-Request", "true"));
                }
                let resp = call_service(&app, req.to_request()).await;
                assert_eq!(resp.status(), status, "{who} htmx={htmx} {value:?}");
            }
        }
    }
    assert_eq!(
        (quantity().await, list_snapshot(&state, &s).await),
        before,
        "nothing changed"
    );
    assert_eq!(before.0.as_deref(), Some("5 kpl"));
}
