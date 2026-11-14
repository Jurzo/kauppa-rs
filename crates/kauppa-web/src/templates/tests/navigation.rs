//! MB-0040 P1 and P2: the navigation panel, and no script in any template.
//! The `#[test]` functions are in `tests.rs`, where the brief's checks name
//! them; their bodies are here (MB-0048 P3: `tests.rs` back under 800
//! lines). `panel` is shared with the language and theme tests.

use super::{
    LAYOUT_SCRIPTS, TEMPLATES, accessible_name, attr, element, render_layout, script_tags, tags,
    text,
};

/// The navigation panel of a rendered page: the menu button that opens it,
/// the panel's id, and the panel element itself.
pub(super) struct Nav<'a> {
    pub(super) button: &'a str,
    pub(super) id: &'a str,
    pub(super) panel: &'a str,
}

pub(super) fn panel(page: &str) -> Nav<'_> {
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

/// MB-0040 P1: see `tests.rs`'s `navigation_panel`.
pub fn entries() {
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

/// MB-0040 P2: see `tests.rs`'s `navigation_needs_no_script`.
pub fn no_script() {
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
