//! MB-0055 P2 (RR-0035 sub-answers 1 and 2): `POST /theme` stores the
//! choice in the `kauppa_theme` cookie, answers htmx with 204 and a plain
//! post with 303, accepts only the three choices, sits behind the
//! cross-site check, and the next page renders the saved choice.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use actix_web::cookie::Cookie;
use actix_web::dev::ServiceResponse;
use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service, read_body};

use crate::routes::test_support::{body_string, test_app, test_state};
use crate::theme::{COOKIE, Theme, cookie};

/// The `kauppa_theme` cookie a response sets, as its header line.
fn set_theme_cookie<B>(resp: &ServiceResponse<B>) -> Option<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("kauppa_theme="))
        .map(str::to_owned)
}

fn location<B>(resp: &ServiceResponse<B>) -> &str {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location")
}

/// `POST /theme` with `theme` and `return`, from htmx or a plain form.
fn post(theme: &str, back: &str, htmx: bool) -> TestRequest {
    let req = TestRequest::post()
        .uri("/theme")
        .set_form([("theme", theme), ("return", back)]);
    if htmx {
        req.insert_header(("HX-Request", "true"))
    } else {
        req
    }
}

/// Light and Dark set the cookie with `kauppa_lang`'s attributes; System
/// removes it. htmx gets 204 with no body, a plain post 303 back.
#[actix_web::test]
async fn stores_the_choice() {
    let state = test_state().await;
    let app = test_app!(state);

    for (theme, htmx) in [
        ("light", true),
        ("dark", true),
        ("light", false),
        ("dark", false),
    ] {
        let resp = call_service(&app, post(theme, "/lists?x=1", htmx).to_request()).await;
        let set = set_theme_cookie(&resp).unwrap_or_else(|| panic!("{theme}: cookie set"));
        for part in [
            format!("kauppa_theme={theme}"),
            "HttpOnly".to_owned(),
            "SameSite=Lax".to_owned(),
            "Path=/".to_owned(),
            "Max-Age=31536000".to_owned(),
        ] {
            assert!(set.contains(&part), "{set}: {part}");
        }
        assert!(
            !set.contains("Secure"),
            "{set}: not Secure without KAUPPA_COOKIE_SECURE"
        );
        if htmx {
            assert_eq!(resp.status(), StatusCode::NO_CONTENT, "{theme}");
            assert!(read_body(resp).await.is_empty(), "{theme}: no body");
        } else {
            assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{theme}");
            assert_eq!(location(&resp), "/lists?x=1");
        }
    }

    // System: the cookie is removed (no cookie means System).
    for htmx in [true, false] {
        let resp = call_service(&app, post("system", "/lists", htmx).to_request()).await;
        let set = set_theme_cookie(&resp).expect("removal cookie");
        assert!(set.starts_with("kauppa_theme=;"), "{set}");
        assert!(set.contains("Max-Age=0"), "{set}");
        assert!(set.contains("Path=/"), "{set}");
    }
}

/// Anything but the three choices is a 400 that sets nothing and doesn't
/// echo the value; a missing field too.
#[actix_web::test]
async fn rejects_other_values() {
    let state = test_state().await;
    let app = test_app!(state);
    for bad in ["", "Dark", "blue", "system ", "<b>x</b>"] {
        for htmx in [true, false] {
            let resp = call_service(&app, post(bad, "/lists", htmx).to_request()).await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{bad:?}");
            assert!(set_theme_cookie(&resp).is_none(), "{bad:?}: nothing set");
            let body = String::from_utf8(read_body(resp).await.to_vec()).unwrap();
            assert!(
                bad.is_empty() || !body.contains(bad),
                "{bad:?} echoed: {body}"
            );
        }
    }
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/theme")
            .set_form([("return", "/lists")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "no theme field");
    assert!(set_theme_cookie(&resp).is_none());
}

/// Only a same-site path is a return target (`same_site_path`).
#[actix_web::test]
async fn returns_to_this_site_only() {
    let state = test_state().await;
    let app = test_app!(state);
    for bad in [
        "",
        "lists",
        "//evil.example/x",
        "/\\evil.example",
        "https://evil.example/",
        "/lists\r\nX-Bad: 1",
    ] {
        let resp = call_service(&app, post("dark", bad, false).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{bad:?}");
        assert_eq!(location(&resp), "/", "{bad:?}");
    }
}

/// Behind the cross-site check: a forged origin gets 403 and no cookie,
/// htmx or not; no GET changes the theme.
#[actix_web::test]
async fn cross_site_and_get_change_nothing() {
    let state = test_state().await;
    let app = test_app!(state);
    for htmx in [true, false] {
        let resp = call_service(
            &app,
            post("dark", "/lists", htmx)
                .insert_header((header::ORIGIN, "https://evil.example"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert!(set_theme_cookie(&resp).is_none());
    }
    for uri in ["/theme", "/theme?theme=dark", "/?theme=dark"] {
        let resp = call_service(&app, TestRequest::get().uri(uri).to_request()).await;
        assert!(set_theme_cookie(&resp).is_none(), "GET {uri}");
    }
}

/// The next page renders the saved choice: its radio checked and its
/// `theme-color`; an unknown cookie value reads as System.
#[actix_web::test]
async fn page_renders_the_saved_choice() {
    let state = test_state().await;
    let app = test_app!(state);
    for (value, checked, colour) in [
        (None, "system", None),
        (Some("light"), "light", Some("#fbf8f5")),
        (Some("dark"), "dark", Some("#121212")),
        (Some("system"), "system", None),
        (Some("blue"), "system", None),
    ] {
        let mut req = TestRequest::get().uri("/auth/login");
        if let Some(value) = value {
            req = req.cookie(Cookie::new(COOKIE, value));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        let radio = format!(r#"id="theme-{checked}" checked"#);
        assert!(body.contains(&radio), "{value:?}: {radio}");
        let start = body.find(r#"action="/theme""#).expect("the switch");
        let form = &body[start..start + body[start..].find("</form>").expect("form ends")];
        assert_eq!(
            form.matches(" checked").count(),
            1,
            "{value:?}: one checked"
        );
        assert!(!body.contains("blue"), "the cookie value isn't echoed");
        let metas = body.matches(r#"name="theme-color""#).count();
        match colour {
            Some(colour) => {
                assert_eq!(metas, 1, "{value:?}");
                assert!(body.contains(&format!(r#"content="{colour}""#)));
            }
            None => assert_eq!(metas, 2, "{value:?}"),
        }
    }
}

/// The cookie is `Secure` when the deployment says so, and parsing knows
/// only the three choices.
#[test]
fn cookie_and_parse() {
    assert_eq!(cookie(Theme::Dark, true).secure(), Some(true));
    assert_eq!(cookie(Theme::Light, false).secure(), Some(false));
    assert_eq!(
        cookie(Theme::System, false)
            .max_age()
            .map(|d| d.whole_seconds()),
        Some(0)
    );
    for (raw, theme) in [
        ("system", Some(Theme::System)),
        ("light", Some(Theme::Light)),
        ("dark", Some(Theme::Dark)),
        ("Dark", None),
        ("", None),
    ] {
        assert_eq!(Theme::parse(raw), theme, "{raw:?}");
    }
    // Only Light and Dark are stored; anything read back that isn't one of
    // them is System.
    for (raw, theme) in [
        ("light", Theme::Light),
        ("dark", Theme::Dark),
        ("system", Theme::System),
        ("blue", Theme::System),
    ] {
        assert_eq!(Theme::from_cookie(Some(raw)), theme, "{raw:?}");
    }
    assert_eq!(Theme::from_cookie(None), Theme::System);
}
