//! MB-0056 (RR-0038): translated error pages. One module per acceptance
//! property; shared helpers here.
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod error_pages;
mod no_detail;
mod render_failure_falls_back;
mod signed_out;
mod under_htmx;

use actix_web::dev::ServiceResponse;
use actix_web::http::header;

use kauppa_core::domain::Language;

use crate::state::AppState;

/// What an error page says for each kind of status: the catalog key
/// prefix (`error_page.<kind>.title` and `.text`).
pub const NOT_FOUND: &str = "not_found";
pub const SIGNED_OUT: &str = "signed_out";
pub const FORBIDDEN: &str = "forbidden";
pub const BAD_REQUEST: &str = "bad_request";
pub const OTHER: &str = "other";
pub const SERVER: &str = "server";

/// `key` in `lang`, HTML-escaped as a template renders it.
pub fn shown(state: &AppState, lang: Language, key: &str) -> String {
    let text = state
        .templates
        .i18n()
        .text(lang, key, &[])
        .expect("catalog key");
    handlebars::html_escape(&text)
}

/// The page's `<main>` content: what the page says, without the layout.
pub fn main_of(body: &str) -> &str {
    let start = body.find("<main>").expect("<main>");
    let end = body.find("</main>").expect("</main>");
    &body[start..end]
}

/// The sign-in form in `body`, from its tag to `</form>`, if there is one.
pub fn login_form(body: &str) -> Option<&str> {
    let start = body.find(r#"action="/auth/login""#)?;
    let start = body[..start].rfind("<form")?;
    let end = start + body[start..].find("</form>")?;
    Some(&body[start..end])
}

/// Whether `body` is the error page of `kind` in `lang`: the layout in that
/// language, the kind's title as `<title>` and heading, its text, and a
/// link to the front page.
pub fn assert_page(state: &AppState, body: &str, lang: Language, kind: &str, what: &str) {
    let title = shown(state, lang, &format!("error_page.{kind}.title"));
    let text = shown(state, lang, &format!("error_page.{kind}.text"));
    assert!(
        body.contains(&format!(r#"<html lang="{}">"#, lang.tag())),
        "{what}: {lang:?} layout\n{body}"
    );
    assert!(
        body.contains(&format!("<title>{title} — kauppalista.app</title>")),
        "{what}: title {title:?}\n{body}"
    );
    let main = main_of(body);
    assert!(
        main.contains(&format!("<h1>{title}</h1>")),
        "{what}: heading\n{main}"
    );
    assert!(main.contains(&text), "{what}: text {text:?}\n{main}");
    assert!(
        main.contains(r#"href="/""#),
        "{what}: a way to the front page\n{main}"
    );
}

/// The response's `name` header, as text.
pub fn header_of<B>(resp: &ServiceResponse<B>, name: &str) -> Option<String> {
    resp.headers()
        .get(name)
        .map(|v| v.to_str().expect("ascii header").to_owned())
}

/// Whether the response is an HTML page with the security headers and
/// `Vary`, as every page.
pub fn assert_html<B>(resp: &ServiceResponse<B>, what: &str) {
    let content_type = header_of(resp, header::CONTENT_TYPE.as_str()).unwrap_or_default();
    assert!(
        content_type.starts_with("text/html"),
        "{what}: {content_type:?}"
    );
    assert_eq!(
        header_of(resp, header::VARY.as_str()).as_deref(),
        Some("Cookie, Accept-Language"),
        "{what}: Vary"
    );
    assert!(
        resp.headers().contains_key(header::CONTENT_SECURITY_POLICY),
        "{what}: CSP"
    );
}
