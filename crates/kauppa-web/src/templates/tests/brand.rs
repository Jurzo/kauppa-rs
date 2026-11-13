//! MB-0054 P4 (RR-0034 sub-answer 5): the app calls itself
//! `kauppalista.app`. The `#[test]` function is in `tests.rs`, where the
//! brief's check names it; its body is here.
//!
//! 1. The layout, in each language: the `<title>` ends with
//!    ` — kauppalista.app`; the top bar's brand links to `/`, reads
//!    `kauppalista.app` and shows the app icon as decoration; an
//!    `apple-mobile-web-app-title` meta says `kauppalista`.
//! 2. The home page's heading is the name.
//! 3. The manifest's `name` is `kauppalista.app`, its `short_name`
//!    `kauppalista`.
//! 4. No template (comments aside), catalog entry or manifest value has the
//!    old name, the word `kauppa` on its own.

use kauppa_core::domain::Language;
use serde_json::Value;

use super::language::render;
use super::{TEMPLATES, attr, element, tags, template, text};

const NAME: &str = "kauppalista.app";
const SHORT_NAME: &str = "kauppalista";
/// The icon beside the name in the top bar (RR-0034 sub-answer 3).
const BRAND_ICON: &str = "/assets/icons/favicon.svg";

/// A file under the workspace root.
fn workspace_file(path: &str) -> String {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"))
}

/// Whether `text` has the word `kauppa` on its own, in any case:
/// `kauppalista` and the Finnish `Kauppaan` ("to the shop") are other
/// words.
fn has_old_name(text: &str) -> bool {
    text.split(|c: char| !c.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case("kauppa"))
}

/// `source` without its `{{!-- … --}}` comments, which no user sees.
fn without_template_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("{{!--") {
        out.push_str(&rest[..start]);
        let len = rest[start..].find("--}}").expect("closed comment") + 4;
        rest = &rest[start + len..];
    }
    out.push_str(rest);
    out
}

/// MB-0054 P4: see the module comment.
pub fn name() {
    // 1. The layout.
    for lang in Language::ALL {
        for signed_in in [true, false] {
            let page = render(lang, signed_in);

            let title = text(element(&page, "<title>", "title"));
            assert!(
                title.ends_with(&format!(" — {NAME}")) && title.len() > NAME.len() + 5,
                "{lang}: <title> is `<page> — {NAME}`: {title}"
            );

            let brands: Vec<&str> = tags(&page, "a")
                .into_iter()
                .filter(|tag| attr(tag, "class") == Some("brand"))
                .collect();
            assert_eq!(brands.len(), 1, "{lang}: one brand link");
            assert_eq!(attr(brands[0], "href"), Some("/"), "{}", brands[0]);
            let brand = element(&page, brands[0], "a");
            assert_eq!(text(brand), NAME, "{lang}: the brand's text");
            let icons = tags(brand, "img");
            assert_eq!(icons.len(), 1, "{lang}: the brand's icon: {brand}");
            assert_eq!(attr(icons[0], "src"), Some(BRAND_ICON), "{}", icons[0]);
            // Decoration: the name is the text beside it.
            assert_eq!(attr(icons[0], "alt"), Some(""), "{}", icons[0]);
            assert!(
                attr(icons[0], "width").is_some() && attr(icons[0], "height").is_some(),
                "the icon has its size, so the bar doesn't jump: {}",
                icons[0]
            );

            let titles: Vec<&str> = tags(&page, "meta")
                .into_iter()
                .filter(|tag| attr(tag, "name") == Some("apple-mobile-web-app-title"))
                .collect();
            assert_eq!(titles.len(), 1, "{lang}: one apple-mobile-web-app-title");
            assert_eq!(
                attr(titles[0], "content"),
                Some(SHORT_NAME),
                "{}",
                titles[0]
            );
        }
    }

    // 2. The home page's heading.
    let home = template("index");
    let headings = tags(home, "h1");
    assert_eq!(headings.len(), 1, "index: one heading");
    assert_eq!(text(element(home, headings[0], "h1")), NAME);

    // 3. The manifest.
    let manifest: Value =
        serde_json::from_str(&workspace_file("assets/manifest.webmanifest")).expect("manifest");
    assert_eq!(manifest["name"], NAME);
    assert_eq!(manifest["short_name"], SHORT_NAME);

    // 4. The old name is gone from everything a user reads.
    for (name, source) in TEMPLATES {
        assert!(
            !has_old_name(&without_template_comments(source)),
            "{name}: the old name `kauppa`"
        );
    }
    for lang in Language::ALL {
        let path = format!("locales/{}.json", lang.tag());
        let catalog: Value = serde_json::from_str(&workspace_file(&path)).expect("catalog");
        for (key, value) in catalog.as_object().expect("a catalog object") {
            // A string, or an object of plural forms.
            let forms: Vec<&str> = value.as_str().map_or_else(
                || {
                    value
                        .as_object()
                        .unwrap_or_else(|| panic!("{path}: {key}"))
                        .values()
                        .map(|form| form.as_str().unwrap_or_else(|| panic!("{path}: {key}")))
                        .collect()
                },
                |text| vec![text],
            );
            for form in forms {
                assert!(!has_old_name(form), "{path}: {key}: {form}");
            }
        }
    }
    for (key, value) in manifest.as_object().expect("manifest object") {
        if let Some(value) = value.as_str() {
            assert!(!has_old_name(value), "manifest {key}: {value}");
        }
    }

    // The scan itself.
    assert!(has_old_name("Sign in — kauppa"));
    assert!(has_old_name("<h1>Kauppa</h1>"));
    assert!(!has_old_name("kauppalista.app"));
    assert!(!has_old_name("Kauppaan"));
}
