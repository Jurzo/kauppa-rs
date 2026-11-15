//! MB-0062 (P1–P5; RR-0045): archiving lists, the Archived section on
//! `/lists`, and an archived list's read-only pages.

use actix_web::http::{StatusCode, header};
use kauppa_core::repo::ShoppingListRepo;

use super::{list_posts, list_snapshot, shared_list};

/// The item and recipe writes in `list_posts`: refused on an archived list.
const ITEM_WRITES: [&str; 7] = [
    "add_item",
    "add_recipe",
    "check",
    "shop_remove",
    "category",
    "remove",
    "restore",
];

/// The Archived section's opening tag in `body`, if it is there.
fn archived_section(body: &str) -> Option<&str> {
    let start = body.find(r#"<details id="archived-lists""#)?;
    let rest = &body[start..];
    Some(&rest[..=rest.find('>')?])
}

/// The part of `body` from the Archived section on ("" if absent).
fn archived_part(body: &str) -> &str {
    body.find(r#"<details id="archived-lists""#)
        .map_or("", |start| &body[start..])
}

/// The part of `body` before the Archived section (the main list and
/// "Shared with you").
fn main_part(body: &str) -> &str {
    body.find(r#"<details id="archived-lists""#)
        .map_or(body, |end| &body[..end])
}

fn location<B>(resp: &actix_web::dev::ServiceResponse<B>) -> Option<String> {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// P1: the owner archives a list from its Edit section on `/lists`; it
/// leaves the main list for an "Archived" section — a `<details>`, closed on
/// a full page load — that shows its name and when it was archived. With
/// nothing archived there is no section.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn archive_moves_to_section() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let arki = ShoppingListRepo::create(&state.pool, user, "Arki")
        .await
        .expect("list")
        .id;
    let mokki = ShoppingListRepo::create(&state.pool, user, "Mökki")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let page = || async {
        body_string(
            call_service(
                &app,
                TestRequest::get()
                    .uri("/lists")
                    .cookie(session.clone())
                    .to_request(),
            )
            .await,
        )
        .await
    };

    let body = page().await;
    assert_eq!(
        archived_section(&body),
        None,
        "nothing archived, no section"
    );
    assert!(body.contains(&format!(
        r##"hx-post="/lists/{mokki}/archive" hx-target="#lists""##
    )));
    assert!(body.contains(r#"Archive<span class="visually-hidden"> Mökki</span>"#));

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{mokki}/archive"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp).as_deref(), Some("/lists"));

    let body = page().await;
    assert_eq!(
        archived_section(&body),
        Some(r#"<details id="archived-lists" class="archived-lists">"#),
        "closed on a full page load"
    );
    assert!(main_part(&body).contains(&format!(r#"href="/lists/{arki}""#)));
    assert!(!main_part(&body).contains("Mökki"));
    let archived = archived_part(&body);
    assert!(archived.contains(
        r#"<summary><span class="group-name">Archived</span> <span class="group-count">1 list</span></summary>"#
    ));
    let list = ShoppingListRepo::get(&state.pool, user, mokki)
        .await
        .expect("list");
    let day = list.archived_at.expect("archived").format("%-d.%-m.%Y");
    assert!(
        archived.contains(&format!(r#"href="/lists/{mokki}">Mökki</a>"#))
            && archived.contains(&format!("Archived on {day}")),
        "{archived}"
    );
}

/// P2: Restore returns the list to the main list in its usual order (by its
/// unchanged `updated_at`).
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn restore() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let mut ids = Vec::new();
    for name in ["Ensimmäinen", "Toinen", "Kolmas"] {
        ids.push(
            ShoppingListRepo::create(&state.pool, user, name)
                .await
                .expect("list")
                .id,
        );
    }
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let get = |uri: &str| {
        TestRequest::get()
            .uri(uri)
            .cookie(session.clone())
            .to_request()
    };
    let order = |body: &str| {
        let main = main_part(body);
        let mut found: Vec<(usize, &str)> = ["Ensimmäinen", "Toinen", "Kolmas"]
            .into_iter()
            .filter_map(|n| main.find(&format!(">{n}</a>")).map(|at| (at, n)))
            .collect();
        found.sort_unstable();
        found.into_iter().map(|(_, n)| n).collect::<Vec<_>>()
    };
    let before = order(&body_string(call_service(&app, get("/lists")).await).await);
    assert_eq!(before, ["Kolmas", "Toinen", "Ensimmäinen"]);

    let toinen = ids[1];
    ShoppingListRepo::archive(&state.pool, user, toinen)
        .await
        .expect("archive");
    let body = body_string(call_service(&app, get("/lists")).await).await;
    assert_eq!(order(&body), ["Kolmas", "Ensimmäinen"]);
    assert!(archived_part(&body).contains(&format!(
        r##"hx-post="/lists/{toinen}/unarchive" hx-target="#lists""##
    )));

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{toinen}/unarchive"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp).as_deref(), Some("/lists?open=archived"));
    let body = body_string(call_service(&app, get("/lists?open=archived")).await).await;
    assert_eq!(order(&body), before, "back in its usual place");
    assert_eq!(archived_section(&body), None, "nothing archived any more");
}

/// P3: an archived list's page and store view are read-only with an
/// "Archived on …" notice and, for the owner only, Restore (which returns to
/// the page it was on); item and recipe changes are refused with 409 and the
/// translated error page, over htmx or not, changing nothing; the owner can
/// still rename it, manage its sharing and delete it.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn archived_list_pages() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    ShoppingListRepo::archive(&state.pool, s.owner, s.list)
        .await
        .expect("archive");
    let owner =
        session_cookie(&call_service(&app, login_request("owner@example.com").to_request()).await);
    let editor =
        session_cookie(&call_service(&app, login_request("editor@example.com").to_request()).await);
    let base = format!("/lists/{}", s.list);
    let day = ShoppingListRepo::get(&state.pool, s.owner, s.list)
        .await
        .expect("list")
        .archived_at
        .expect("archived")
        .format("%-d.%-m.%Y")
        .to_string();

    // The pages, as the owner and as an editor, full and fragment.
    for (who, cookie) in [("owner", &owner), ("editor", &editor)] {
        for (uri, back) in [(base.clone(), "list"), (format!("{base}/shop"), "shop")] {
            for htmx in [false, true] {
                let mut req = TestRequest::get().uri(&uri).cookie(cookie.clone());
                if htmx {
                    req = req.insert_header(("HX-Request", "true"));
                }
                let resp = call_service(&app, req.to_request()).await;
                assert_eq!(resp.status(), StatusCode::OK, "{who} {uri}");
                let body = body_string(resp).await;
                let at = format!("{who} {uri} htmx={htmx}");
                assert!(body.contains("Leipä"), "{at}: items still shown");
                assert!(body.contains(&format!("Archived on {day}")), "{at}");
                let restore = format!(r#"<input type="hidden" name="back" value="{back}" />"#);
                assert_eq!(body.contains(&restore), who == "owner", "{at}: Restore");
                // No item or recipe controls.
                for control in [
                    format!(r#"hx-post="{base}/items""#),
                    format!(r#"hx-post="{base}/recipes""#),
                    format!(r#"hx-post="{base}/items/{}""#, s.bread),
                    format!(r#"hx-post="{base}/items/{}/restore""#, s.sugar),
                    format!(r#"hx-post="{base}/shop/items/{}/check""#, s.bread),
                    format!(r#"hx-post="{base}/shop/items/{}/remove""#, s.bread),
                ] {
                    assert!(!body.contains(&control), "{at}: {control}");
                }
                assert!(!body.contains("Shared with you to view"), "{at}");
                // Restore is the page's primary action; the store view's
                // boxes are drawn disabled.
                if who == "owner" {
                    assert!(
                        body.contains(r#"<button type="submit" class="button primary">Restore"#),
                        "{at}"
                    );
                }
                if back == "shop" {
                    assert!(
                        body.contains(r#"<span class="shop-toggle is-disabled">"#),
                        "{at}"
                    );
                }
            }
        }
    }

    // Item and recipe writes: 409 with the error page, nothing changes.
    let before = list_snapshot(&state, &s).await;
    for (name, uri, form) in list_posts(&s)
        .into_iter()
        .filter(|(n, ..)| ITEM_WRITES.contains(n))
    {
        for htmx in [false, true] {
            let mut req = TestRequest::post()
                .uri(&uri)
                .cookie(owner.clone())
                .set_form(form.clone());
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::CONFLICT, "{name} htmx={htmx}");
            let body = body_string(resp).await;
            // "That didn't work" (`error_page.other.title`), HTML-escaped.
            assert!(
                body.contains("<html") && body.contains("That didn"),
                "{name}: the 409 page"
            );
        }
    }
    // An editor's item write is refused the same way.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/items"))
            .cookie(editor.clone())
            .set_form([("text", "Munat")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(list_snapshot(&state, &s).await, before, "nothing changed");

    // Allowed for the owner: rename, the sharing page, share and revoke.
    let owner_post = |uri: String, form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(owner.clone())
            .set_form(form)
            .to_request()
    };
    let resp = call_service(
        &app,
        owner_post(
            format!("{base}/rename"),
            vec![("name", "Kesä 2026".to_owned())],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp).as_deref(), Some("/lists?open=archived"));
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("{base}/sharing"))
            .cookie(owner.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = call_service(
        &app,
        owner_post(format!("{base}/shares/{}/revoke", s.viewer_share), vec![]),
    )
    .await;
    assert!(resp.status().is_redirection() || resp.status().is_success());
    let list = ShoppingListRepo::get(&state.pool, s.owner, s.list)
        .await
        .expect("list");
    assert_eq!(list.name, "Kesä 2026");
    assert!(list.archived_at.is_some(), "still archived");

    // Restore from the store view returns there; the list is editable again.
    let resp = call_service(
        &app,
        owner_post(
            format!("{base}/unarchive"),
            vec![("back", "shop".to_owned())],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp), Some(format!("{base}/shop")));
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("{base}/shop"))
                .cookie(owner.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(body.contains(&format!(r#"hx-post="{base}/shop/items/{}/check""#, s.bread)));
    assert!(!body.contains("Archived on"));
    assert!(!body.contains("is-disabled"), "active again");

    // Delete is allowed on an archived list.
    ShoppingListRepo::archive(&state.pool, s.owner, s.list)
        .await
        .expect("archive");
    let resp = call_service(&app, owner_post(format!("{base}/delete"), vec![])).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert!(
        ShoppingListRepo::get(&state.pool, s.owner, s.list)
            .await
            .is_err()
    );
}

/// P4: an archived shared list leaves the grantee's "Shared with you" for
/// their own Archived section, read-only (no Restore or Edit); shares are
/// unchanged; only the owner archives or restores — grantees get 403 and a
/// stranger 404, over htmx or not, changing nothing.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn owner_only() {
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
    let viewer = login("viewer@example.com").await;
    let editor = login("editor@example.com").await;
    let stranger = login("stranger@example.com").await;
    let index = |cookie: &actix_web::cookie::Cookie<'static>| {
        TestRequest::get()
            .uri("/lists")
            .cookie(cookie.clone())
            .to_request()
    };

    // Before: in "Shared with you", no Archived section.
    let body = body_string(call_service(&app, index(&viewer)).await).await;
    assert!(body.contains("Shared with you") && body.contains("Kauppa"));
    assert_eq!(archived_section(&body), None);

    // Grantees and a stranger can't archive (or restore).
    let before = list_snapshot(&state, &s).await;
    for (who, cookie, status) in [
        ("viewer", &viewer, StatusCode::FORBIDDEN),
        ("editor", &editor, StatusCode::FORBIDDEN),
        ("stranger", &stranger, StatusCode::NOT_FOUND),
    ] {
        for action in ["archive", "unarchive"] {
            for htmx in [false, true] {
                let mut req = TestRequest::post()
                    .uri(&format!("/lists/{}/{action}", s.list))
                    .cookie(cookie.clone());
                if htmx {
                    req = req.insert_header(("HX-Request", "true"));
                }
                let resp = call_service(&app, req.to_request()).await;
                assert_eq!(resp.status(), status, "{who} {action} htmx={htmx}");
            }
        }
    }
    assert!(
        ShoppingListRepo::get(&state.pool, s.owner, s.list)
            .await
            .expect("list")
            .archived_at
            .is_none()
    );

    ShoppingListRepo::archive(&state.pool, s.owner, s.list)
        .await
        .expect("archive");
    assert_eq!(list_snapshot(&state, &s).await, before, "shares unchanged");

    // Each grantee: gone from "Shared with you", in their Archived section
    // with the owner's name, without the owner's controls.
    for (who, cookie) in [("viewer", &viewer), ("editor", &editor)] {
        let body = body_string(call_service(&app, index(cookie)).await).await;
        assert!(!main_part(&body).contains("Kauppa"), "{who}");
        assert!(
            !body.contains("Shared with you"),
            "{who}: nothing else shared"
        );
        let archived = archived_part(&body);
        assert!(
            archived.contains(&format!(r#"href="/lists/{}">Kauppa</a>"#, s.list)),
            "{who}"
        );
        assert!(
            archived.contains("Test User · Archived on"),
            "{who}: {archived}"
        );
        assert!(
            !archived.contains("/unarchive") && !archived.contains("/rename"),
            "{who}"
        );
        assert_eq!(
            archived_section(&body),
            Some(r#"<details id="archived-lists" class="archived-lists">"#)
        );
    }
    // Restoring is refused for grantees too, now that it's archived.
    for cookie in [&viewer, &editor] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&format!("/lists/{}/unarchive", s.list))
                .cookie(cookie.clone())
                .set_form([("back", "list")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/lists/{}", s.list))
            .cookie(stranger.clone())
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "still hidden from strangers"
    );
    assert_eq!(
        ShoppingListRepo::archived(&state.pool, s.viewer)
            .await
            .expect("archived")
            .len(),
        1
    );
}

/// P5: over htmx, archive and restore answer the `#lists` fragment; the
/// Archived section is open only after an action inside it (restore, rename,
/// delete). Without htmx, a 303 to `/lists` (`?open=archived` after an action
/// inside the section, which opens it). The cross-site check covers the new
/// routes.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn htmx_and_plain() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let mut ids = Vec::new();
    for name in ["Yksi", "Kaksi", "Kolme"] {
        ids.push(
            ShoppingListRepo::create(&state.pool, user, name)
                .await
                .expect("list")
                .id,
        );
    }
    let (yksi, kaksi, kolme) = (ids[0], ids[1], ids[2]);
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |uri: String| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
    };
    let fragment = |body: &str| body.trim_start().starts_with(r#"<div id="lists""#);
    let open = r#"<details id="archived-lists" class="archived-lists" open>"#;
    let closed = r#"<details id="archived-lists" class="archived-lists">"#;

    // Archive over htmx: the fragment; the section appears, closed.
    for id in [yksi, kaksi, kolme] {
        let resp = call_service(&app, post(format!("/lists/{id}/archive")).to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert!(fragment(&body) && !body.contains("<html"));
        assert_eq!(archived_section(&body), Some(closed));
    }

    // Actions inside the section: it comes back open.
    let resp = call_service(&app, post(format!("/lists/{yksi}/unarchive")).to_request()).await;
    let body = body_string(resp).await;
    assert!(fragment(&body));
    assert_eq!(archived_section(&body), Some(open), "after restore");
    assert!(main_part(&body).contains("Yksi"));
    let resp = call_service(
        &app,
        post(format!("/lists/{kaksi}/rename"))
            .set_form([("name", "Kaksi ja puoli")])
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert_eq!(archived_section(&body), Some(open), "after rename");
    assert!(archived_part(&body).contains("Kaksi ja puoli"));
    // A rejected rename inside the section: the error, section open.
    let resp = call_service(
        &app,
        post(format!("/lists/{kaksi}/rename"))
            .set_form([("name", "  ")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(r#"role="alert""#));
    assert_eq!(
        archived_section(&body),
        Some(open),
        "after a rejected rename"
    );
    let resp = call_service(&app, post(format!("/lists/{kaksi}/delete")).to_request()).await;
    let body = body_string(resp).await;
    assert_eq!(archived_section(&body), Some(open), "after delete");
    // An action outside it leaves it closed.
    let resp = call_service(
        &app,
        post(format!("/lists/{yksi}/rename"))
            .set_form([("name", "Ykkönen")])
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert_eq!(
        archived_section(&body),
        Some(closed),
        "after an outside action"
    );

    // Without htmx: 303s; following the restore's opens the section.
    let plain = |uri: String| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .to_request()
    };
    let resp = call_service(&app, plain(format!("/lists/{yksi}/archive"))).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp).as_deref(), Some("/lists"));
    let resp = call_service(&app, plain(format!("/lists/{kolme}/unarchive"))).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let to = location(&resp).expect("location");
    assert_eq!(to, "/lists?open=archived");
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&to)
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(body.contains("<html"));
    assert_eq!(archived_section(&body), Some(open));
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/lists?open=nonsense")
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert_eq!(archived_section(&body), Some(closed));

    // The cross-site check covers the new routes: refused, nothing changes.
    for action in ["archive", "unarchive"] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&format!("/lists/{kolme}/{action}"))
                .cookie(session.clone())
                .insert_header((header::ORIGIN, "https://evil.example"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{action}");
    }
    assert!(
        ShoppingListRepo::get(&state.pool, user, kolme)
            .await
            .expect("list")
            .archived_at
            .is_none()
    );
}
