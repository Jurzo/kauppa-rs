//! MB-0035 acceptance tests (RR-0023): no literal text (P1), the catalogs
//! (P2), choosing the language (P3), every page in Finnish (P4) and
//! localized dates (P5).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;

use actix_web::cookie::Cookie;
use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use actix_web::web;
use chrono::{TimeZone, Utc};

use kauppa_core::domain::Language;
use kauppa_core::repo::UserRepo;

use super::{Catalog, Entry, I18n, I18nError, from_accept_language, placeholders};
use crate::routes::auth::tests::{invitation, register_request};
use crate::routes::test_support::{
    body_string, login_request, make_user, session_cookie, test_app, test_state,
};
use crate::state::AppState;
use crate::templates::TEMPLATES;

mod pages;
mod scan;

/// Every key a template passes to `t`, as `{{t "key"` or `(t "key"`.
fn template_keys() -> BTreeSet<(String, &'static str)> {
    let mut keys = BTreeSet::new();
    for (name, source) in TEMPLATES {
        for opener in ["{{t \"", "(t \""] {
            for (at, _) in source.match_indices(opener) {
                let rest = &source[at + opener.len()..];
                let key = &rest[..rest.find('"').expect("closing quote")];
                keys.insert((key.to_owned(), *name));
            }
        }
    }
    keys
}

/// The keys Rust code looks up by name.
fn rust_keys() -> Vec<String> {
    let mut keys = vec!["format.date".to_owned(), "format.datetime".to_owned()];
    keys.extend(Language::ALL.map(|l| format!("language.{l}")));
    keys
}

/// MB-0035 P2: every catalog has the same keys, each with the same shape
/// (text or plural) and the same placeholders in every form; no value has
/// markup; every key a template or Rust uses exists.
#[test]
fn catalogs_complete() {
    let i18n = I18n::load().expect("catalogs load");
    let en = i18n.catalog(Language::En).entries();
    assert!(!en.is_empty());
    for lang in Language::ALL {
        let catalog = i18n.catalog(lang).entries();
        let keys: Vec<_> = catalog.keys().collect();
        assert_eq!(
            keys,
            en.keys().collect::<Vec<_>>(),
            "{lang}: keys differ from en"
        );
        for (key, entry) in catalog {
            let base = &en[key];
            match (base, entry) {
                (Entry::Text(_), Entry::Text(_)) | (Entry::Plural { .. }, Entry::Plural { .. }) => {
                }
                _ => panic!("{lang}: `{key}` is text in one catalog and plural in the other"),
            }
            for (ours, theirs) in entry.forms().into_iter().zip(base.forms()) {
                assert_eq!(
                    placeholders(ours),
                    placeholders(theirs),
                    "{lang}: `{key}` placeholders differ from en"
                );
                assert!(
                    !ours.contains('<') && !ours.contains('&'),
                    "{lang}: `{key}` contains markup: {ours:?}"
                );
            }
        }
    }
    for (key, template) in template_keys() {
        assert!(
            en.contains_key(&key),
            "{template} uses `{key}`, which no catalog has"
        );
    }
    for key in rust_keys() {
        assert!(
            en.contains_key(&key),
            "Rust uses `{key}`, which no catalog has"
        );
    }
}

/// The catalog's own rules: parsing, placeholders, plurals, escaping and
/// strict errors (RR-0023 constraints 2 and 3).
#[test]
fn catalog_rules() {
    let catalog = Catalog::parse(
        r#"{"hi": "Hi {name}!", "items": {"one": "{count} item", "other": "{count} items"},
            "brace": "a {not a placeholder} and {}"}"#,
    )
    .expect("parse");
    let escape = |s: &str| handlebars::html_escape(s);
    assert_eq!(
        catalog.format("hi", &[("name", "<b>Pat & co</b>".to_owned())], escape),
        Ok("Hi &lt;b&gt;Pat &amp; co&lt;/b&gt;!".to_owned())
    );
    assert_eq!(
        catalog.format("items", &[("count", "1".to_owned())], escape),
        Ok("1 item".to_owned())
    );
    for n in ["0", "2", "21"] {
        assert_eq!(
            catalog.format("items", &[("count", n.to_owned())], escape),
            Ok(format!("{n} items"))
        );
    }
    assert_eq!(
        catalog.format("brace", &[], escape),
        Ok("a {not a placeholder} and {}".to_owned())
    );
    assert_eq!(
        catalog.format("nope", &[], escape),
        Err(I18nError::Key("nope".to_owned()))
    );
    assert!(matches!(
        catalog.format("hi", &[], escape),
        Err(I18nError::Argument { .. })
    ));
    assert!(matches!(
        catalog.format("items", &[], escape),
        Err(I18nError::Count(_))
    ));

    for bad in [
        "[]",
        r#"{"a": 1}"#,
        r#"{"a": {"one": "x"}}"#,
        r#"{"a": {"one": "x", "other": "y", "few": "z"}}"#,
        r#"{"a": {"one": "x", "many": "y"}}"#,
    ] {
        assert!(Catalog::parse(bad).is_err(), "{bad}");
    }
    assert!(I18n::from_sources(&[("en", "{}")]).is_err(), "fi missing");
    assert!(
        I18n::from_sources(&[("en", "{}"), ("fi", "{}"), ("sv", "{}")]).is_err(),
        "unsupported language"
    );
}

/// The `{{t}}` helper: escapes arguments, writes catalog text as is,
/// fails on a missing key instead of falling back, and works as a
/// subexpression.
#[test]
fn helper_escapes_and_fails_strictly() {
    let i18n = I18n::from_sources(&[
        ("en", r#"{"hi": "Hi {name}", "title": "Lists"}"#),
        ("fi", r#"{"hi": "Hei {name}", "title": "Listat"}"#),
    ])
    .unwrap();
    for (lang, hi) in [(Language::En, "Hi"), (Language::Fi, "Hei")] {
        let mut hb = handlebars::Handlebars::new();
        hb.set_strict_mode(true);
        hb.register_helper(
            "t",
            Box::new(super::Translate::new(i18n.catalog(lang).clone())),
        );
        hb.register_helper("html_lang", Box::new(super::HtmlLang(lang)));
        let data = serde_json::json!({ "who": "<i>x</i>" });
        let out = hb
            .render_template(r#"{{html_lang}}|{{t "hi" name=who}}"#, &data)
            .unwrap();
        assert_eq!(out, format!("{lang}|{hi} &lt;i&gt;x&lt;/i&gt;"));
        hb.register_template_string("p", "<{{title}}>").unwrap();
        let out = hb
            .render_template(r#"{{> p title=(t "hi" name=who)}}"#, &data)
            .unwrap();
        assert_eq!(out, format!("<{hi} &lt;i&gt;x&lt;/i&gt;>"), "subexpression");
        assert!(hb.render_template(r#"{{t "missing"}}"#, &data).is_err());
        assert!(hb.render_template(r#"{{t "hi"}}"#, &data).is_err());
    }
}

/// `Accept-Language`: the supported language with the highest `q`.
#[test]
fn accept_language_parsing() {
    for (header, expected) in [
        ("fi", Some(Language::Fi)),
        ("fi-FI,fi;q=0.9,en-US;q=0.8,en;q=0.7", Some(Language::Fi)),
        ("en-GB,en;q=0.9,fi;q=0.8", Some(Language::En)),
        ("sv-SE, de;q=0.9, fi;q=0.5", Some(Language::Fi)),
        ("en;q=0.5, FI;q=0.9", Some(Language::Fi)),
        ("fi;q=0, en;q=0.1", Some(Language::En)),
        ("en;q=0.8, fi;q=0.8", Some(Language::En)),
        ("fi;q=abc", None),
        ("fi;q=2", None),
        ("*", None),
        ("sv, de", None),
        ("", None),
    ] {
        assert_eq!(from_accept_language(header), expected, "{header:?}");
    }
}

/// The `lang` attribute of a rendered page's `<html>`.
fn html_lang(page: &str) -> &str {
    let start = page.find("<html lang=\"").expect("<html lang>") + 12;
    &page[start..start + page[start..].find('"').expect("closing quote")]
}

/// The `kauppa_lang` cookie a response sets, as its header line.
fn set_lang_cookie<B>(resp: &actix_web::dev::ServiceResponse<B>) -> Option<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("kauppa_lang="))
        .map(str::to_owned)
}

fn location<B>(resp: &actix_web::dev::ServiceResponse<B>) -> &str {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location")
}

/// MB-0035 P3: the language is the signed-in user's choice, then the
/// cookie, then `Accept-Language`, then English (RR-0023 sub-answer 3);
/// `<html lang>` matches; `POST /language` switches it signed out (cookie)
/// and signed in (column and cookie), returning to a same-site path only
/// (sub-answers 4 and 5); a new account keeps the cookie's language.
#[actix_web::test]
async fn language_selection() {
    let state = test_state().await;
    let app = test_app!(state);
    let page = |req: TestRequest| async {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::VARY)
                .map(|v| v.to_str().unwrap()),
            Some("Cookie, Accept-Language"),
            "HTML varies by the language inputs"
        );
        body_string(resp).await
    };
    let login_page = || TestRequest::get().uri("/auth/login?next=1");

    // Nothing chosen: English.
    let body = page(login_page()).await;
    assert_eq!(html_lang(&body), "en");
    // The switcher: a plain POST form returning here, one button per
    // language named in its own language, the current one pressed.
    assert!(
        body.contains(
            r#"<form class="lang-switch" method="post" action="/language" hx-boost="false""#
        ),
        "{body}"
    );
    assert!(
        body.contains(r#"<input type="hidden" name="return" value="/auth/login?next&#x3D;1" />"#),
        "{body}"
    );
    assert!(body.contains(r#"<button type="submit" name="lang" value="en" lang="en" aria-pressed="true">English</button>"#), "{body}");
    assert!(body.contains(r#"<button type="submit" name="lang" value="fi" lang="fi" aria-pressed="false">Suomi</button>"#), "{body}");

    // Accept-Language, then the cookie over it; an unknown cookie value is
    // ignored.
    let body =
        page(login_page().insert_header((header::ACCEPT_LANGUAGE, "fi-FI,fi;q=0.9,en;q=0.5")))
            .await;
    assert_eq!(html_lang(&body), "fi");
    assert!(
        body.contains(r#"value="fi" lang="fi" aria-pressed="true""#),
        "{body}"
    );
    let body = page(
        login_page()
            .insert_header((header::ACCEPT_LANGUAGE, "fi"))
            .cookie(Cookie::new("kauppa_lang", "en")),
    )
    .await;
    assert_eq!(html_lang(&body), "en", "cookie over Accept-Language");
    let body = page(
        login_page()
            .insert_header((header::ACCEPT_LANGUAGE, "fi"))
            .cookie(Cookie::new("kauppa_lang", "sv")),
    )
    .await;
    assert_eq!(html_lang(&body), "fi", "unknown cookie value ignored");

    // Signed out: the switch sets the cookie and returns.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/language")
            .set_form([("lang", "fi"), ("return", "/auth/login?next=1")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp), "/auth/login?next=1");
    let cookie = set_lang_cookie(&resp).expect("kauppa_lang set");
    for attr in [
        "kauppa_lang=fi",
        "HttpOnly",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=31536000",
    ] {
        assert!(cookie.contains(attr), "{cookie}: {attr}");
    }
    assert!(
        !cookie.contains("Secure"),
        "{cookie}: not Secure without KAUPPA_COOKIE_SECURE"
    );

    // Only a same-site path is a return target.
    for bad in [
        "",
        "lists",
        "//evil.example/x",
        "/\\evil.example",
        "https://evil.example/",
        "/lists\r\nX-Bad: 1",
        "/lists\u{0}",
    ] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri("/language")
                .set_form([("lang", "en"), ("return", bad)])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{bad:?}");
        assert_eq!(location(&resp), "/", "{bad:?}");
    }
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/language")
            .set_form([("lang", "en")])
            .to_request(),
    )
    .await;
    assert_eq!(location(&resp), "/", "no return field");
    // An unsupported language is refused and sets nothing.
    for bad in ["sv", "EN", ""] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri("/language")
                .set_form([("lang", bad), ("return", "/")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{bad:?}");
        assert_eq!(set_lang_cookie(&resp), None, "{bad:?}");
    }

    // Signed in: the choice is saved on the account and follows it to a
    // new session on another device, over that device's cookie and header.
    let user = make_user(&state, "lang@example.com").await;
    let session =
        session_cookie(&call_service(&app, login_request("lang@example.com").to_request()).await);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/language")
            .cookie(session.clone())
            .set_form([("lang", "fi"), ("return", "/lists")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp), "/lists");
    assert!(set_lang_cookie(&resp).is_some_and(|c| c.starts_with("kauppa_lang=fi")));
    assert_eq!(
        UserRepo::language(&state.pool, user).await.unwrap(),
        Some(Language::Fi)
    );

    let other_device =
        session_cookie(&call_service(&app, login_request("lang@example.com").to_request()).await);
    let body = page(
        TestRequest::get()
            .uri("/lists")
            .cookie(other_device)
            .cookie(Cookie::new("kauppa_lang", "en"))
            .insert_header((header::ACCEPT_LANGUAGE, "en")),
    )
    .await;
    assert_eq!(html_lang(&body), "fi", "the account's choice wins");

    // A new account keeps the language chosen before it existed.
    let token = invitation(&state).await;
    let resp = call_service(
        &app,
        register_request(Some(&token), "new@example.com")
            .cookie(Cookie::new("kauppa_lang", "fi"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let (created, _) = UserRepo::find_by_email(&state.pool, "new@example.com")
        .await
        .unwrap()
        .expect("account created");
    assert_eq!(
        UserRepo::language(&state.pool, created.id).await.unwrap(),
        Some(Language::Fi)
    );
    // Never from Accept-Language alone.
    let token = invitation(&state).await;
    call_service(
        &app,
        register_request(Some(&token), "plain@example.com")
            .insert_header((header::ACCEPT_LANGUAGE, "fi"))
            .to_request(),
    )
    .await;
    let (plain, _) = UserRepo::find_by_email(&state.pool, "plain@example.com")
        .await
        .unwrap()
        .expect("account created");
    assert_eq!(
        UserRepo::language(&state.pool, plain.id).await.unwrap(),
        None
    );
}

/// RR-0023 sub-answer 4: the cookie is `Secure` when cookies are.
#[actix_web::test]
async fn language_cookie_secure_with_secure_cookies() {
    let base = test_state().await;
    let state = web::Data::new(AppState {
        cookie_secure: true,
        ..(*base.into_inner()).clone()
    });
    let app = test_app!(state);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/language")
            .set_form([("lang", "fi"), ("return", "/")])
            .to_request(),
    )
    .await;
    let cookie = set_lang_cookie(&resp).expect("kauppa_lang set");
    assert!(cookie.contains("Secure"), "{cookie}");
}

/// MB-0035 P5: dates and times use the language's format from the catalog
/// (English unchanged; times stay UTC), on the pages that show them.
#[actix_web::test]
async fn dates_localized() {
    let i18n = I18n::load().unwrap();
    let at = Utc.with_ymd_and_hms(2026, 11, 9, 8, 5, 0).unwrap();
    assert_eq!(i18n.date(Language::En, at).unwrap(), "9.11.2026");
    assert_eq!(i18n.date(Language::Fi, at).unwrap(), "9.11.2026");
    assert_eq!(
        i18n.datetime(Language::En, at).unwrap(),
        "2026-11-09 08:05 UTC"
    );
    assert_eq!(
        i18n.datetime(Language::Fi, at).unwrap(),
        "9.11.2026 klo 8.05 UTC"
    );

    // The admin portal's invitation times follow the request's language.
    let state = test_state().await;
    let app = test_app!(state);
    let admin = make_user(&state, "admin@example.com").await;
    kauppa_core::repo::AdminRepo::grant(&state.pool, admin, None, Utc::now())
        .await
        .unwrap();
    invitation(&state).await;
    let session =
        session_cookie(&call_service(&app, login_request("admin@example.com").to_request()).await);
    let today = Utc::now();
    for (lang, expected) in [
        (Language::En, today.format("%Y-%m-%d ").to_string()),
        (Language::Fi, today.format("%-d.%-m.%Y klo ").to_string()),
    ] {
        UserRepo::set_language(&state.pool, admin, Some(lang))
            .await
            .unwrap();
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri("/admin/invitations")
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await;
        assert!(body.contains(&expected), "{lang}: {expected} in {body}");
    }
}

/// MB-0035 P1 (RR-0023 constraint 1): no template holds literal prose: text
/// between tags and the attributes a person reads come from `{{t}}`, and
/// `page_title` is never a literal. The scan finds a planted literal.
#[test]
fn templates_have_no_literal_text() {
    let mut found = Vec::new();
    for (name, source) in TEMPLATES {
        for text in scan::literal_text(source) {
            found.push(format!("{name}: {text}"));
        }
        if source.contains("page_title=\"") {
            found.push(format!("{name}: a literal page_title"));
        }
        if source.contains("{{{t ") {
            found.push(format!("{name}: a translation in a raw {{{{{{…}}}}}}"));
        }
    }
    assert!(
        found.is_empty(),
        "literal text in templates:\n{}",
        found.join("\n")
    );

    // The scan itself.
    assert_eq!(scan::literal_text("<p>Hello {{name}}</p>"), ["Hello"]);
    assert_eq!(
        scan::literal_text(r#"<input placeholder="e.g. Maito" />"#),
        [r#"placeholder="e.g. Maito""#]
    );
    assert_eq!(
        scan::literal_text(r#"<b aria-label="{{#if x}}Mark {{y}}{{/if}}">✕</b>"#),
        [r#"aria-label="Mark""#]
    );
    assert!(
        scan::literal_text(
            r#"{{!-- a comment --}}<h1>kauppa</h1><p>{{t "a.b"}}</p><input placeholder="{{t "c.d"}}" />↑ ·"#
        )
        .is_empty()
    );
}

/// The English text the catalog has that its Finnish doesn't: each literal
/// stretch (between placeholders) of an English value with three or more
/// letters that no Finnish value contains as words.
fn english_only(i18n: &I18n) -> Vec<String> {
    let fi: Vec<&str> = i18n
        .catalog(Language::Fi)
        .entries()
        .values()
        .flat_map(Entry::forms)
        .collect();
    let mut chunks = BTreeSet::new();
    for entry in i18n.catalog(Language::En).entries().values() {
        for form in entry.forms() {
            let mut rest = form;
            loop {
                let (chunk, next) = match rest.find('{') {
                    Some(open) => (
                        &rest[..open],
                        rest[open..].find('}').map(|c| &rest[open + c + 1..]),
                    ),
                    None => (rest, None),
                };
                let chunk =
                    chunk.trim_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation());
                if chunk.chars().filter(|c| c.is_alphabetic()).count() >= 3
                    && !fi.iter().any(|f| scan::contains_words(f, chunk))
                {
                    chunks.insert(chunk.to_owned());
                }
                match next {
                    Some(next) => rest = next,
                    None => break,
                }
            }
        }
    }
    chunks.into_iter().collect()
}

/// MB-0035 P4: every page and fragment, and the main rejections, render for
/// a Finnish user: no missing key (a render error would be a 500), full
/// pages say `<html lang="fi">`, and no English-only catalog text appears.
#[actix_web::test]
async fn every_page_renders_in_finnish() {
    let i18n = I18n::load().unwrap();
    let english = english_only(&i18n);
    assert!(!english.is_empty());
    let mut problems = Vec::new();
    for page in pages::walk(Language::Fi).await {
        let what = &page.what;
        if page.status.is_server_error() {
            problems.push(format!("{what}: {} {}", page.status, page.body));
            continue;
        }
        if page.full && !page.body.contains(r#"<html lang="fi">"#) {
            problems.push(format!("{what}: not <html lang=\"fi\">"));
        }
        let text = scan::rendered_text(&page.body);
        for chunk in &english {
            if scan::contains_words(&text, chunk) {
                problems.push(format!("{what}: English {chunk:?}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
