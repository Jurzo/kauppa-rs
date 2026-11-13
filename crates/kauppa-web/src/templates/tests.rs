//! MB-0024 P1/P2 (templates) and P4; MB-0016 P5; MB-0027 P1/P2 (vendored
//! htmx); MB-0040 P1/P2 (navigation panel); MB-0029 P1/P2/P5 (boosted
//! form errors, secret pages); MB-0031 P2 (CSP); MB-0051 P1/P3/P4 (language
//! selector, bodies in `tests/language.rs`); MB-0053 P1 (select chevron,
//! body in `tests/select.rs`); MB-0054 P1–P3 (design tokens, contrast and
//! components, bodies in `tests/design.rs`) and P4 (the name, body in
//! `tests/brand.rs`).
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256, Sha384};

use kauppa_core::domain::Language;

use super::{Registry, TEMPLATES};
use crate::i18n::I18n;

mod brand;
mod design;
mod language;
mod select;

/// The English registry over the embedded catalogs.
fn build_registry() -> anyhow::Result<Registry> {
    super::build_registry(&I18n::load()?, Language::En)
}

/// The vendored htmx (MB-0027, RR-0015; upgraded by MB-0030, RR-0017):
/// htmx 2.0.11, 0BSD, byte-for-byte `package/dist/htmx.min.js` from the npm
/// registry tarball
/// <https://registry.npmjs.org/htmx.org/-/htmx.org-2.0.11.tgz>, whose npm
/// `dist.integrity` is
/// `sha512-Thx/WtpeOQqSrqBCw/A1cwGJGg4UrVa3+sW0GmrM3p4gJgO89ecH4qtbnyzDDWFvBTqjnIMCgELTNt636dtamA==`.
/// [`HTMX_SHA256`] equals the GitHub release `v2.0.11` asset `htmx.min.js`
/// digest. Never edit the file: a new version is a new file name, new
/// constants and a new `integrity` in `layout.hbs`, under an RR (RR-0015
/// constraint 2).
const HTMX_PATH: &str = "/assets/vendor/htmx-2.0.11.min.js";
/// SHA-256 of the file at [`HTMX_PATH`] (52,182 bytes), hex.
const HTMX_SHA256: &str = "d6fdc75f204e6bdefa99b69bf1e6d4ac69b8a364f77929f45c13476b4000f717";
/// Its Subresource Integrity value, as htmx's docs publish it at `v2.0.11`
/// and `layout.hbs` carries it.
const HTMX_SRI_SHA384: &str =
    "sha384-2OatzQy1H+Zd/IIrjr1TcuDGqLXeHhbooAyJY1KdQMKnr4LZ22k31GBLdYKHmVjg";

/// The vendored htmx response-targets extension (MB-0029, RR-0018): 2.0.4,
/// 0BSD, byte-for-byte `package/dist/response-targets.min.js` from the npm
/// registry tarball
/// <https://registry.npmjs.org/htmx-ext-response-targets/-/htmx-ext-response-targets-2.0.4.tgz>,
/// whose npm `dist.integrity` is
/// `sha512-Q/yfH0N2A40j903mr6ldGV3qLWMQeufROylYIbYQLBGrCpmydflxXmQwDOEhEWdPnBTgiHofkAo0gQ3S1BmyxQ==`.
/// [`RESPONSE_TARGETS_SRI_SHA384`] equals the SRI htmx's docs publish for
/// it at `v2.0.11`. Never edit the file (RR-0018 constraint 2).
const RESPONSE_TARGETS_PATH: &str = "/assets/vendor/response-targets-2.0.4.min.js";
/// SHA-256 of the file at [`RESPONSE_TARGETS_PATH`] (1,692 bytes), hex.
const RESPONSE_TARGETS_SHA256: &str =
    "bde3061dd9b0c7a0b3c7bae63784312bbb7a3a798411b750fe1a5c07a286e63a";
/// Its Subresource Integrity value, as `layout.hbs` carries it.
const RESPONSE_TARGETS_SRI_SHA384: &str =
    "sha384-T41oglUPvXLGBVyRdZsVRxNWnOOqCynaPubjUVjxhsjFTKrFJGEMm3/0KGmNQ+Pg";
/// The layout's scripts, in load order: `(src, integrity)`.
const LAYOUT_SCRIPTS: [(&str, &str); 2] = [
    (HTMX_PATH, HTMX_SRI_SHA384),
    (RESPONSE_TARGETS_PATH, RESPONSE_TARGETS_SRI_SHA384),
];

/// The workspace's `templates/` folder, for comparison only.
fn source_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates"))
}

fn names(hb: &Registry) -> BTreeSet<String> {
    hb.get_templates().keys().cloned().collect()
}

/// Every template under `templates/`, named by handlebars' directory source
/// rules (default options): `.hbs` files only, the `/`-separated relative
/// path without `.hbs`, skipping hidden (`.x`) and temporary (`#x`) files.
fn names_on_disk() -> BTreeSet<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).expect("read templates dir") {
            let path = entry.expect("dir entry").path();
            let name = path.file_name().expect("name").to_string_lossy();
            let rel = format!("{prefix}{name}");
            if path.is_dir() {
                walk(&path, &format!("{rel}/"), out);
            } else if let Some(stem) = rel.strip_suffix(".hbs")
                && !name.starts_with('.')
                && !name.starts_with('#')
            {
                out.insert(stem.to_owned());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(source_dir(), "", &mut out);
    out
}

/// MB-0024 P1/P2: the registry comes from the binary, not the working
/// directory, and holds exactly what the `templates/` folder holds, under
/// the names directory loading gave them, so no `render` call changes.
/// Compared with a `read_dir` walk (RR-0020), not handlebars' `dir_source`.
#[test]
fn registry_is_embedded() {
    // `cargo test` runs in the crate directory, which has no `templates/`:
    // the old `build_registry()` failed here, the embedded one doesn't.
    assert!(!Path::new("./templates").exists());
    let embedded = build_registry().expect("embedded registry");
    assert!(embedded.strict_mode());

    assert_eq!(names(&embedded), names_on_disk());
    assert!(names(&embedded).contains("layout"));
    assert!(names(&embedded).contains("lists/_list"));

    // Byte for byte what's on disk: the build re-runs when a template changes.
    for (name, source) in TEMPLATES {
        let on_disk = std::fs::read_to_string(source_dir().join(format!("{name}.hbs")))
            .expect("template on disk");
        assert_eq!(*source, on_disk, "{name}");
    }
}

/// The one `htmx-config` meta tag (MB-0031, RR-0019 sub-answer 5).
const HTMX_CONFIG: &str = r#"<meta name="htmx-config" content='{"allowEval":false,"allowScriptTags":false,"includeIndicatorStyles":false}' />"#;

/// The indicator rules htmx 2.0.11 would inject as a `<style>`, which
/// `style-src 'self'` blocks; `app.css` carries them instead (RR-0019
/// constraint 7: re-copy them when the vendored htmx changes).
const INDICATOR_CSS: [&str; 2] = [
    ".htmx-indicator{opacity:0;visibility:hidden}",
    ".htmx-request .htmx-indicator,.htmx-request.htmx-indicator{opacity:1;visibility:visible;transition:opacity 200ms ease-in}",
];

/// MB-0031 P2 (RR-0019): htmx runs under the Content-Security-Policy.
/// The layout has exactly one `htmx-config`, before the htmx script, with
/// `allowEval`, `allowScriptTags` and `includeIndicatorStyles` off; the
/// indicator rules are in `app.css`, as htmx 2.0.11 writes them; and no
/// template has inline script, style, event handler or eval-based htmx
/// feature (RR-0019 constraint 3).
#[test]
fn csp_compatible_templates() {
    let (_, layout) = TEMPLATES
        .iter()
        .find(|(name, _)| *name == "layout")
        .expect("layout");
    assert_eq!(
        layout.matches(HTMX_CONFIG).count(),
        1,
        "layout's htmx-config"
    );
    let config = layout.find(HTMX_CONFIG).expect("htmx-config");
    let script = layout.find("<script").expect("htmx script");
    assert!(config < script, "htmx-config comes before the htmx script");
    assert!(config < layout.find("</head>").expect("</head>"));

    // htmx itself would inject exactly these rules (spaces aside).
    let htmx = std::fs::read_to_string(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join(HTMX_PATH.trim_start_matches('/')),
    )
    .expect("vendored htmx");
    let htmx: String = htmx.split_whitespace().collect();
    assert!(
        htmx.contains("`.${t}{opacity:0;visibility:hidden}`"),
        "htmx's own rule"
    );
    assert!(
        htmx.contains(
            "`.${n}.${t},.${n}.${t}{opacity:1;visibility:visible;transition:opacity200msease-in}`"
        ),
        "htmx's own rule"
    );
    let css = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/app.css"))
        .expect("app.css");
    for rule in INDICATOR_CSS {
        assert!(css.contains(rule), "app.css lacks {rule}");
    }

    for (name, source) in TEMPLATES {
        let lower = source.to_ascii_lowercase();
        assert!(
            lower.matches("htmx-config").count() == usize::from(*name == "layout"),
            "{name}: the one htmx-config lives in layout.hbs"
        );
        for tag in script_tags(source) {
            assert!(attr(tag, "src").is_some(), "{name}: inline script {tag}");
        }
        for word in [
            "<style",
            " style=",
            "javascript:",
            "hx-on",
            "js:",
            "hx-vals='js",
            "eval(",
        ] {
            assert!(!lower.contains(word), "{name}: `{word}` breaks the CSP");
        }
        // Event handler attributes (`onclick=`, `onsubmit=`, …).
        for (at, _) in lower.match_indices(" on") {
            let rest = &lower[at + 3..];
            let len = rest
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(rest.len());
            assert!(
                len == 0 || !rest[len..].starts_with('='),
                "{name}: inline event handler at byte {at}"
            );
        }
        // `hx-trigger` filters (`click[ctrlKey]`) are evaluated as script.
        for (at, _) in lower.match_indices("hx-trigger=\"") {
            let value = &lower[at + 12..];
            let value = &value[..value.find('"').expect("closed hx-trigger")];
            assert!(!value.contains('['), "{name}: hx-trigger filter {value}");
        }
    }
}

/// MB-0016 P5 (RR-0006), MB-0024 P4: inside the layout, a body field named
/// like a layout parameter renders its own value, at the top level and in a
/// nested block, and the layout still gets its own parameters. Handlebars
/// turns a partial's hash params into block params, which would shadow such
/// fields. Runs on the embedded registry, which stays in strict mode.
#[test]
fn layout_params_do_not_shadow_body() {
    let mut hb = build_registry().expect("load templates");
    hb.register_template_string(
        "guard",
        r#"{{#> layout page_title="Page" page_signed_in=true}}
<b>{{title}}|{{signed_in}}</b>
{{#each items}}<i>{{this.title}}|{{this.signed_in}}</i>{{/each}}
{{/layout}}"#,
    )
    .expect("guard template");

    let body = hb
        .render(
            "guard",
            &serde_json::json!({
                "title": "Body",
                "signed_in": false,
                "items": [{ "title": "Item", "signed_in": true }],
            }),
        )
        .expect("render");
    assert!(
        body.contains("<title>Page — kauppalista.app</title>"),
        "{body}"
    );
    assert!(body.contains("Sign out"), "the layout's own signed_in");
    assert!(body.contains("<b>Body|false</b>"), "{body}");
    assert!(body.contains("<i>Item|true</i>"), "{body}");

    // Strict mode: a missing field is an error, not an empty string.
    hb.register_template_string("missing", "{{nope}}")
        .expect("missing template");
    assert!(hb.render("missing", &serde_json::json!({})).is_err());
}

/// The opening `<script …>` tags in `source`.
fn script_tags(source: &str) -> Vec<&str> {
    source
        .to_ascii_lowercase()
        .match_indices("<script")
        .map(|(start, _)| {
            let len = source[start..].find('>').expect("closed <script> tag");
            &source[start..=start + len]
        })
        .collect()
}

/// The value of `name="…"` in `tag`.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// MB-0027 P1, MB-0030 P2, MB-0029 P1: every script a template loads comes
/// from the app's own `/assets/`, none is inline, and the layout loads the
/// vendored htmx and then its response-targets extension, each with its
/// SRI, as rendered pages show.
#[test]
fn no_external_scripts() {
    for (name, source) in TEMPLATES {
        for tag in script_tags(source) {
            let src = attr(tag, "src").unwrap_or_else(|| panic!("{name}: inline script {tag}"));
            assert!(
                src.starts_with("/assets/") && !src.contains("..") && !src.contains("//"),
                "{name}: {src} isn't served from this app's /assets/"
            );
        }
        assert!(!source.contains("unpkg"), "{name} mentions unpkg");
    }

    let (_, layout) = TEMPLATES
        .iter()
        .find(|(name, _)| *name == "layout")
        .expect("layout");
    let tags = script_tags(layout);
    assert_eq!(tags.len(), LAYOUT_SCRIPTS.len(), "{tags:?}");
    for (tag, (src, integrity)) in tags.iter().zip(LAYOUT_SCRIPTS) {
        assert_eq!(attr(tag, "src"), Some(src));
        assert_eq!(attr(tag, "integrity"), Some(integrity));
    }

    let mut hb = build_registry().expect("registry");
    hb.register_template_string(
        "page",
        r#"{{#> layout page_title="Page" page_signed_in=false}}<p>body</p>{{/layout}}"#,
    )
    .expect("page template");
    let page = hb.render("page", &serde_json::json!({})).expect("render");
    let mut at = 0;
    for (src, integrity) in LAYOUT_SCRIPTS {
        let tag = format!(r#"<script src="{src}" integrity="{integrity}" defer>"#);
        let found = page[at..]
            .find(&tag)
            .unwrap_or_else(|| panic!("{tag} (in order) in {page}"));
        at += found + tag.len();
    }
    assert!(!page.contains("unpkg"), "{page}");
}

/// Standard base64 with padding (RFC 4648), for the SRI value.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let byte = |i: usize| usize::from(chunk.get(i).copied().unwrap_or(0));
        let n = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i)) & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Asserts the vendored file at `path` (a `/assets/…` URL path) has `len`
/// bytes, SHA-256 `sha256` (hex) and SRI `sri`.
fn assert_vendored(path: &str, len: usize, sha256: &str, sri: &str) {
    let file =
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(path.trim_start_matches('/'));
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(bytes.len(), len, "{path}");

    let hex = Sha256::digest(&bytes)
        .iter()
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").expect("write to String");
            hex
        });
    assert_eq!(hex, sha256, "{path}");
    let actual = format!("sha384-{}", base64(&Sha384::digest(&bytes)));
    assert_eq!(actual, sri, "{path}");
}

/// MB-0027 P2, MB-0030 P1: the vendored file is exactly the published htmx
/// 2.0.11. Its SHA-256 and its SHA-384 SRI both match the pinned values, so
/// a changed byte fails CI.
#[test]
fn vendored_htmx_checksum() {
    assert_vendored(HTMX_PATH, 52_182, HTMX_SHA256, HTMX_SRI_SHA384);

    // The encoder itself, on RFC 4648's test vectors.
    for (input, expected) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64(input.as_bytes()), expected);
    }
}

/// The opening tags named `name` (`"button"`, `"a"`, …) in `html`.
fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    html.match_indices(&open)
        .filter(|(start, _)| {
            html[start + open.len()..]
                .starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        })
        .map(|(start, _)| {
            let len = html[start..].find('>').expect("closed tag");
            &html[start..=start + len]
        })
        .collect()
}

/// The element that `open` (an opening tag found in `html`) starts, up to
/// and including its closing `</name>` (no nesting of the same element).
fn element<'a>(html: &'a str, open: &str, name: &str) -> &'a str {
    let start = html.find(open).expect("tag in html");
    let close = format!("</{name}>");
    let len = html[start..].find(&close).expect("closing tag") + close.len();
    &html[start..start + len]
}

/// The text of an element, tags stripped and whitespace collapsed.
fn text(element: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in element.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The accessible name of the element opened by `tag`: its `aria-label`, or
/// its text.
fn accessible_name(html: &str, tag: &str, name: &str) -> String {
    attr(tag, "aria-label").map_or_else(|| text(element(html, tag, name)), str::to_owned)
}

/// The layout rendered around a one-line body, with the given parameters.
fn render_layout(params: &str) -> String {
    let mut hb = build_registry().expect("registry");
    hb.register_template_string(
        "page",
        format!("{{{{#> layout {params}}}}}<p>body</p>{{{{/layout}}}}"),
    )
    .expect("page template");
    hb.render("page", &serde_json::json!({}))
        .unwrap_or_else(|e| panic!("render with {params}: {e}"))
}

/// The navigation panel of a rendered page: the menu button that opens it,
/// the panel's id, and the panel element itself.
struct Nav<'a> {
    button: &'a str,
    id: &'a str,
    panel: &'a str,
}

fn panel(page: &str) -> Nav<'_> {
    // MB-0051: the language selector is a popover too, so only the
    // openers that target a `<nav>` count.
    let navs = tags(page, "nav");
    let openers: Vec<&str> = tags(page, "button")
        .into_iter()
        .filter(|tag| {
            attr(tag, "popovertargetaction").is_none()
                && attr(tag, "popovertarget")
                    .is_some_and(|id| navs.iter().any(|nav| attr(nav, "id") == Some(id)))
        })
        .collect();
    assert_eq!(openers.len(), 1, "one menu button opens the panel: {page}");
    let button = openers[0];
    let id = attr(button, "popovertarget").expect("popovertarget");
    let id_attr = format!(r#" id="{id}""#);
    let nav = tags(page, "nav")
        .into_iter()
        .find(|tag| tag.contains(&id_attr))
        .unwrap_or_else(|| panic!("no <nav id=\"{id}\">: {page}"));
    assert!(
        nav.contains(" popover>") || nav.contains(" popover ") || attr(nav, "popover").is_some(),
        "the panel is a native popover: {nav}"
    );
    Nav {
        button,
        id,
        panel: element(page, nav, "nav"),
    }
}

/// The sections a page can mark current: `page_nav` value, link, label.
const SECTIONS: [(&str, &str, &str); 4] = [
    ("lists", "/lists", "Lists"),
    ("recipes", "/recipes", "Recipes"),
    ("favorites", "/favorites", "Favorites"),
    ("categories", "/categories", "Categories"),
];

/// MB-0040 P1: the top bar has a menu button that opens a panel (`popover`)
/// with every navigation entry and a close control; the current section is
/// marked `aria-current="page"`, and each page names its section.
#[test]
fn navigation_panel() {
    for signed_in in [true, false] {
        let page = render_layout(&format!(r#"page_title="Page" page_signed_in={signed_in}"#));
        let Nav { button, id, panel } = panel(&page);

        // The menu button: a real button with an accessible name, in the
        // top bar, outside the panel.
        assert_eq!(attr(button, "type"), Some("button"), "{button}");
        let name = accessible_name(&page, button, "button");
        assert!(
            !name.is_empty(),
            "menu button has no accessible name: {button}"
        );
        let topbar = element(&page, tags(&page, "header")[0], "header");
        assert!(
            topbar.contains(button) && topbar.contains(panel),
            "{topbar}"
        );
        assert!(
            !panel.contains(button),
            "the menu button sits outside the panel"
        );

        // A close control inside the panel.
        let closers: Vec<&str> = tags(panel, "button")
            .into_iter()
            .filter(|tag| {
                attr(tag, "popovertarget") == Some(id)
                    && attr(tag, "popovertargetaction") == Some("hide")
            })
            .collect();
        assert_eq!(closers.len(), 1, "one close control in the panel: {panel}");
        assert_eq!(attr(closers[0], "type"), Some("button"), "{}", closers[0]);
        let close_name = accessible_name(panel, closers[0], "button");
        assert!(
            !close_name.is_empty(),
            "close control has no accessible name"
        );

        // Every entry, inside the panel, none current.
        for (_, href, label) in SECTIONS {
            let link = tags(panel, "a")
                .into_iter()
                .find(|tag| attr(tag, "href") == Some(href))
                .unwrap_or_else(|| panic!("no {href} link in the panel: {panel}"));
            assert_eq!(text(element(panel, link, "a")), label);
        }
        assert!(
            !page.contains(r#"aria-current="page""#),
            "no section is current: {page}"
        );

        let logout = tags(panel, "form")
            .into_iter()
            .find(|tag| attr(tag, "action") == Some("/auth/logout"));
        let login = tags(panel, "a")
            .into_iter()
            .find(|tag| attr(tag, "href") == Some("/auth/login"));
        if signed_in {
            // MB-0018: Sign out stays a POST form, never a link.
            let form = logout.unwrap_or_else(|| panic!("no Sign out form: {panel}"));
            assert_eq!(attr(form, "method"), Some("post"));
            assert_eq!(text(element(panel, form, "form")), "Sign out");
            assert!(login.is_none(), "{panel}");
        } else {
            let link = login.unwrap_or_else(|| panic!("no Sign in link: {panel}"));
            assert_eq!(text(element(panel, link, "a")), "Sign in");
            assert!(logout.is_none(), "{panel}");
        }
        assert!(
            !page.contains(r#"href="/auth/logout""#),
            "Sign out isn't a link"
        );
    }

    // `aria-current="page"` on exactly the current section's entry.
    for (nav, current, _) in SECTIONS {
        let page = render_layout(&format!(
            r#"page_title="Page" page_signed_in=true page_nav="{nav}""#
        ));
        let Nav { panel, .. } = panel(&page);
        assert_eq!(page.matches(r#"aria-current="page""#).count(), 1, "{page}");
        for (_, href, _) in SECTIONS {
            let link = tags(panel, "a")
                .into_iter()
                .find(|tag| attr(tag, "href") == Some(href))
                .expect("entry");
            let expected = (href == current).then_some("page");
            assert_eq!(attr(link, "aria-current"), expected, "{nav}: {link}");
        }
    }

    // Each section's pages name their section; the others name none.
    for (name, source) in TEMPLATES {
        let Some(call) = source.lines().find(|line| line.starts_with("{{#> layout")) else {
            continue;
        };
        let expected = match *name {
            n if n.starts_with("lists/") => Some("lists"),
            n if n.starts_with("recipes/") => Some("recipes"),
            "products/index" => Some("favorites"),
            "categories/index" => Some("categories"),
            _ => None,
        };
        assert_eq!(attr(call, "page_nav"), expected, "{name}: {call}");
    }
}

/// MB-0051 P1 (RR-0031): the language selector, in `tests/language.rs`.
#[test]
fn language_dropdown() {
    language::dropdown();
}

/// MB-0051 P3: short and full language names.
#[test]
fn language_dropdown_names() {
    language::names();
}

/// MB-0051 P4: the selector's accessibility.
#[test]
fn language_dropdown_accessible() {
    language::accessible();
}

/// MB-0053 P1: every `<select>` shows the app's chevron, in `tests/select.rs`.
#[test]
fn select_chevron() {
    select::chevron();
}

/// MB-0054 P4 (RR-0034 sub-answer 5): the app is called `kauppalista.app`,
/// in `tests/brand.rs`.
#[test]
fn brand_name() {
    brand::name();
}

/// MB-0054 P1 (RR-0034): `app.css` holds the design tokens, in
/// `tests/design.rs`.
#[test]
fn design_tokens() {
    design::tokens();
}

/// MB-0054 P2: the tokens' contrast, and `on-brand` on brand fills.
#[test]
fn design_contrast() {
    design::contrast();
}

/// MB-0054 P3: buttons, fields, rows and the one primary button per page.
#[test]
fn design_components() {
    design::components();
}

/// MB-0040 P2 (and MB-0031's CSP): the panel is native HTML (`popover`
/// opened by `popovertarget`), and no template has a script other than the
/// layout's vendored htmx and its extension, an inline `<style>`, a
/// `style=` or an event handler (`on…=`, `hx-on`, `javascript:`).
#[test]
fn navigation_needs_no_script() {
    let page = render_layout(r#"page_title="Page" page_signed_in=true page_nav="lists""#);
    panel(&page);

    let rendered = ("rendered layout", page.as_str());
    for (name, source) in TEMPLATES.iter().copied().chain([rendered]) {
        let lower = source.to_ascii_lowercase();
        let scripts = script_tags(source);
        if name == "layout" || name == "rendered layout" {
            let srcs: Vec<_> = scripts.iter().map(|tag| attr(tag, "src")).collect();
            let expected: Vec<_> = LAYOUT_SCRIPTS.iter().map(|(src, _)| Some(*src)).collect();
            assert_eq!(srcs, expected, "{name}");
        } else {
            assert!(scripts.is_empty(), "{name}: {scripts:?}");
        }
        assert!(!lower.contains("<style"), "{name}: inline <style>");
        assert!(!lower.contains(" style="), "{name}: style attribute");
        assert!(!lower.contains("hx-on"), "{name}: hx-on handler");
        assert!(!lower.contains("javascript:"), "{name}: javascript: URL");
        // An `on…=` attribute: whitespace, `on`, letters, `=`.
        let bytes = lower.as_bytes();
        for (at, _) in lower.match_indices("on") {
            let rest = &lower[at + 2..];
            let letters = rest.len()
                - rest
                    .trim_start_matches(|c: char| c.is_ascii_lowercase())
                    .len();
            let handler = at > 0
                && bytes[at - 1].is_ascii_whitespace()
                && letters > 0
                && rest[letters..].starts_with('=');
            assert!(
                !handler,
                "{name}: event handler attribute: {}",
                &rest[..letters]
            );
        }
    }
}

/// MB-0029 (RR-0018): the vendored response-targets extension is exactly the
/// published 2.0.4.
#[test]
fn vendored_response_targets_checksum() {
    assert_vendored(
        RESPONSE_TARGETS_PATH,
        1_692,
        RESPONSE_TARGETS_SHA256,
        RESPONSE_TARGETS_SRI_SHA384,
    );
}

/// The boosted full-page forms whose handler answers a 4xx with a rendered
/// page (MB-0029's inventory, RR-0018 sub-answer 3): `(template, action)`.
const ERROR_PAGE_FORMS: [(&str, &str); 3] = [
    ("auth/login", "/auth/login"),
    ("auth/register", "/auth/register"),
    ("admin/invitations", "/admin/invitations"),
];

/// The source of the template called `name`.
fn template(name: &str) -> &'static str {
    TEMPLATES
        .iter()
        .find(|(n, _)| *n == name)
        .map_or_else(|| panic!("template {name}"), |(_, source)| *source)
}

/// Every opening tag in `source`, comments and closing tags excluded.
fn opening_tags(source: &str) -> Vec<&str> {
    source
        .match_indices('<')
        .filter(|(start, _)| source[start + 1..].starts_with(|c: char| c.is_ascii_alphabetic()))
        .map(|(start, _)| {
            let len = source[start..].find('>').expect("closed tag");
            &source[start..=start + len]
        })
        .collect()
}

/// MB-0029 P1: the login, register and create-invitation forms swap their
/// 4xx page into `<body>` through the response-targets extension. htmx runs
/// an extension for `htmx:beforeSwap` only if it is declared on the swap
/// target or above it, and a boosted form's target is `<body>`, so `<body>`
/// declares it (RR-0025, amending RR-0018); each form names its 4xx target.
/// The layout loads the extension after htmx (checked with its SRI by
/// `no_external_scripts`).
#[test]
fn boosted_error_pages_swap() {
    for (name, action) in ERROR_PAGE_FORMS {
        let forms: Vec<_> = tags(template(name), "form")
            .into_iter()
            .filter(|tag| attr(tag, "action") == Some(action))
            .collect();
        assert_eq!(forms.len(), 1, "{name}: {forms:?}");
        let form = forms[0];
        assert_eq!(attr(form, "method"), Some("post"), "{name}");
        assert_eq!(attr(form, "hx-target-4xx"), Some("body"), "{name}");
    }

    let page = render_layout(r#"page_title="Page" page_signed_in=false"#);
    let htmx = page.find(HTMX_PATH).expect("htmx loaded");
    let ext = page.find(RESPONSE_TARGETS_PATH).expect("extension loaded");
    assert!(htmx < ext, "the extension loads after htmx");
    let body = tags(&page, "body");
    assert_eq!(body.len(), 1, "{page}");
    assert_eq!(attr(body[0], "hx-boost"), Some("true"));
    assert_eq!(attr(body[0], "hx-ext"), Some("response-targets"));
}

/// MB-0029 P2 (RR-0025 constraints 1 and 2): `hx-ext` appears only on the
/// layout's `<body>`, and an `hx-target-<code>` only on the three
/// error-page forms. The extension acts only where it finds one from the
/// requesting element upward, so no `hx-post`/`hx-get` fragment request (on
/// such a form or inside it) gets a 4xx swapped into its target, and no
/// `htmx-config` changes `responseHandling`.
#[test]
fn fragment_errors_not_swapped() {
    let mut declarations = Vec::new();
    let mut carriers = Vec::new();
    for (name, source) in TEMPLATES {
        assert!(!source.contains("responseHandling"), "{name}");
        for tag in opening_tags(source) {
            if tag.contains(" hx-ext=") {
                declarations.push((*name, tag));
            }
            if tag.contains(" hx-target-") {
                assert!(tag.starts_with("<form "), "{name}: {tag}");
                assert!(!tag.contains(" hx-ext="), "{name}: {tag}");
                let inner = element(source, tag, "form");
                assert!(
                    !inner.contains("hx-post") && !inner.contains("hx-get"),
                    "{name}: {inner}"
                );
                carriers.push((*name, attr(tag, "action").expect("form action")));
            }
        }
    }
    assert_eq!(declarations.len(), 1, "{declarations:?}");
    let (name, tag) = declarations[0];
    assert_eq!(name, "layout");
    assert!(tag.starts_with("<body "), "{tag}");
    assert_eq!(attr(tag, "hx-ext"), Some("response-targets"));

    carriers.sort_unstable();
    let mut expected = ERROR_PAGE_FORMS.to_vec();
    expected.sort_unstable();
    assert_eq!(carriers, expected);
}

/// MB-0029 P5 (RR-0018 sub-answer 6): the pages that render a secret (the
/// register page's invitation token, the invitations page's one-time link)
/// opt out of htmx's history cache on their outermost element; the layout
/// doesn't, so other pages keep Back restore.
#[test]
fn secret_pages_skip_history() {
    for name in ["auth/register", "admin/invitations"] {
        let source = template(name);
        let body = &source[source.find("}}").expect("layout opener") + 2..];
        let first = opening_tags(body)[0];
        assert!(first.starts_with("<section"), "{name}: {first}");
        assert_eq!(attr(first, "hx-history"), Some("false"), "{name}: {first}");
    }
    assert!(!template("layout").contains("hx-history"));
}
