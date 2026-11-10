//! MB-0024 P1/P2 (templates) and P4; MB-0016 P5; MB-0027 P1/P2 (vendored
//! htmx); MB-0040 P1/P2 (navigation panel).
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256, Sha384};

use super::{Registry, TEMPLATES, build_registry};

/// The vendored htmx (MB-0027, RR-0015): htmx 2.0.3, 0BSD, byte-for-byte
/// `package/dist/htmx.min.js` from the npm registry tarball
/// <https://registry.npmjs.org/htmx.org/-/htmx.org-2.0.3.tgz>, whose npm
/// `dist.integrity` is
/// `sha512-AeoJUAjkCVVajbfKX+3sVQBTCt8Ct4lif1T+z/tptTXo8+8yyq3QIMQQe/IT+R8ssfrO1I0DeX4CAronzCL6oA==`.
/// Never edit the file: a new version is a new file name, new constants and
/// a new `integrity` in `layout.hbs`, under an RR (RR-0015 constraint 2).
const HTMX_PATH: &str = "/assets/vendor/htmx-2.0.3.min.js";
/// SHA-256 of the file at [`HTMX_PATH`] (50,387 bytes), hex.
const HTMX_SHA256: &str = "491955cd1810747d7d7b9ccb936400afb760e06d25d53e4572b64b6563b2784e";
/// Its Subresource Integrity value, as htmx's docs publish it at `v2.0.3`
/// and `layout.hbs` carries it.
const HTMX_SRI_SHA384: &str =
    "sha384-0895/pl2MU10Hqc6jd4RvrthNlDiE9U1tWmX7WRESftEDRosgxNsQG/Ze9YMRzHq";

/// The workspace's `templates/` folder, for comparison only.
fn source_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates"))
}

fn names(hb: &Registry) -> BTreeSet<String> {
    hb.get_templates().keys().cloned().collect()
}

/// MB-0024 P1/P2: the registry comes from the binary, not the working
/// directory, and holds exactly what the `templates/` folder holds, under
/// the names directory loading gave them, so no `render` call changes.
#[test]
fn registry_is_embedded() {
    // `cargo test` runs in the crate directory, which has no `templates/`:
    // the old `build_registry()` failed here, the embedded one doesn't.
    assert!(!Path::new("./templates").exists());
    let embedded = build_registry().expect("embedded registry");
    assert!(embedded.strict_mode());

    let mut from_dir = Registry::new();
    from_dir
        .register_templates_directory(source_dir(), handlebars::DirectorySourceOptions::default())
        .expect("directory registry");
    assert_eq!(names(&embedded), names(&from_dir));
    assert!(names(&embedded).contains("layout"));
    assert!(names(&embedded).contains("lists/_list"));

    // Byte for byte what's on disk: the build re-runs when a template changes.
    for (name, source) in TEMPLATES {
        let on_disk = std::fs::read_to_string(source_dir().join(format!("{name}.hbs")))
            .expect("template on disk");
        assert_eq!(*source, on_disk, "{name}");
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
    assert!(body.contains("<title>Page — kauppa</title>"), "{body}");
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

/// MB-0027 P1: every script a template loads comes from the app's own
/// `/assets/`, none is inline, and the layout loads the vendored htmx with
/// its SRI, as rendered pages show.
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
    assert_eq!(tags.len(), 1, "{tags:?}");
    assert_eq!(attr(tags[0], "src"), Some(HTMX_PATH));
    assert_eq!(attr(tags[0], "integrity"), Some(HTMX_SRI_SHA384));

    let mut hb = build_registry().expect("registry");
    hb.register_template_string(
        "page",
        r#"{{#> layout page_title="Page" page_signed_in=false}}<p>body</p>{{/layout}}"#,
    )
    .expect("page template");
    let page = hb.render("page", &serde_json::json!({})).expect("render");
    let tag = format!(r#"<script src="{HTMX_PATH}" integrity="{HTMX_SRI_SHA384}" defer>"#);
    assert!(page.contains(&tag), "{page}");
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

/// MB-0027 P2: the vendored file is exactly the published htmx 2.0.3. Its
/// SHA-256 and its SHA-384 SRI both match the pinned values, so a changed
/// byte fails CI.
#[test]
fn vendored_htmx_checksum() {
    let file = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join(HTMX_PATH.trim_start_matches('/'));
    let bytes = std::fs::read(&file).expect("vendored htmx");
    assert_eq!(bytes.len(), 50_387);

    let sha256 = Sha256::digest(&bytes)
        .iter()
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").expect("write to String");
            hex
        });
    assert_eq!(sha256, HTMX_SHA256);
    let sri = format!("sha384-{}", base64(&Sha384::digest(&bytes)));
    assert_eq!(sri, HTMX_SRI_SHA384);

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
    let openers: Vec<&str> = tags(page, "button")
        .into_iter()
        .filter(|tag| {
            attr(tag, "popovertarget").is_some() && attr(tag, "popovertargetaction").is_none()
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
            !page.contains("aria-current"),
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
        assert_eq!(page.matches("aria-current").count(), 1, "{page}");
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

/// MB-0040 P2 (and MB-0031's CSP): the panel is native HTML (`popover`
/// opened by `popovertarget`), and no template has a script other than the
/// layout's vendored htmx, an inline `<style>`, a `style=` or an event
/// handler (`on…=`, `hx-on`, `javascript:`).
#[test]
fn navigation_needs_no_script() {
    let page = render_layout(r#"page_title="Page" page_signed_in=true page_nav="lists""#);
    panel(&page);

    let rendered = ("rendered layout", page.as_str());
    for (name, source) in TEMPLATES.iter().copied().chain([rendered]) {
        let lower = source.to_ascii_lowercase();
        let scripts = script_tags(source);
        if name == "layout" || name == "rendered layout" {
            assert_eq!(scripts.len(), 1, "{name}: {scripts:?}");
            assert_eq!(attr(scripts[0], "src"), Some(HTMX_PATH), "{name}");
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
