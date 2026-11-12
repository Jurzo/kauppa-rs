//! MB-0051 P1, P3 and P4 (RR-0031): the language selector in the top bar.
//! The `#[test]` functions are in `tests.rs`, where the brief's checks
//! name them; their bodies are here.

use std::path::Path;

use kauppa_core::domain::Language;

use super::{accessible_name, attr, element, panel, tags, text};
use crate::i18n::I18n;

/// Each language's catalog key part, full name and short name, as the
/// selector shows them in every page language.
const NAMES: [(Language, &str, &str); 2] = [
    (Language::En, "English", "EN"),
    (Language::Fi, "Suomi", "FI"),
];

/// `app.css`: the phone media query (MB-0040's 40rem breakpoint), and the
/// default rule that hides the short name.
const NARROW: &str = "@media (max-width: 39.9375rem) {";
const HIDE_SHORT: &str = ".lang-short { display: none; }";

/// The opener's accessible name on a page in each language.
const OPENER_NAMES: [(Language, &str); 2] = [
    (Language::En, "Language: English"),
    (Language::Fi, "Kieli: Suomi"),
];

/// The layout in `lang`, around a one-line body.
fn render(lang: Language, signed_in: bool) -> String {
    let mut hb =
        super::super::build_registry(&I18n::load().expect("catalogs"), lang).expect("registry");
    hb.register_template_string(
        "page",
        format!(
            r#"{{{{#> layout page_title=(t "nav.lists") page_signed_in={signed_in}}}}}<p>body</p>{{{{/layout}}}}"#
        ),
    )
    .expect("page template");
    hb.render("page", &serde_json::json!({}))
        .unwrap_or_else(|e| panic!("render in {lang}: {e}"))
}

/// Every page the tests look at: both languages, signed in and out.
fn pages() -> Vec<(Language, String)> {
    let mut pages = Vec::new();
    for lang in Language::ALL {
        for signed_in in [true, false] {
            pages.push((lang, render(lang, signed_in)));
        }
    }
    pages
}

/// The language selector of a rendered page.
struct Selector<'a> {
    /// The opening button's tag, and the whole button.
    opener: &'a str,
    opener_element: &'a str,
    /// The list's `<form …>` tag, and the whole form.
    form: &'a str,
    list: &'a str,
    /// Each language's submit button: its tag and the whole button.
    options: Vec<(&'a str, &'a str)>,
}

/// Whether `tag` has the bare or valued attribute `name`.
fn has_attr(tag: &str, name: &str) -> bool {
    attr(tag, name).is_some()
        || tag.contains(&format!(" {name} "))
        || tag.contains(&format!(" {name}>"))
        || tag.contains(&format!(" {name}/"))
}

fn selector(page: &str) -> Selector<'_> {
    let forms: Vec<&str> = tags(page, "form")
        .into_iter()
        .filter(|tag| attr(tag, "action") == Some("/language"))
        .collect();
    assert_eq!(forms.len(), 1, "one form posts to /language: {page}");
    let form = forms[0];
    let id = attr(form, "id").unwrap_or_else(|| panic!("the list has an id: {form}"));
    let openers: Vec<&str> = tags(page, "button")
        .into_iter()
        .filter(|tag| attr(tag, "popovertarget") == Some(id))
        .collect();
    assert_eq!(openers.len(), 1, "one button opens the list: {page}");
    let opener = openers[0];
    let list = element(page, form, "form");
    let options = tags(list, "button")
        .into_iter()
        .map(|tag| (tag, element(list, tag, "button")))
        .collect();
    Selector {
        opener,
        opener_element: element(page, opener, "button"),
        form,
        list,
        options,
    }
}

/// The one flag image in `element`: its `<img>` tag.
fn flag(element: &str) -> &str {
    let images = tags(element, "img");
    assert_eq!(images.len(), 1, "one flag: {element}");
    images[0]
}

/// The `<span>` in `element` with exactly the class `class`: its tag and
/// its text.
fn span<'a>(element: &'a str, class: &str) -> (&'a str, String) {
    let spans: Vec<&str> = tags(element, "span")
        .into_iter()
        .filter(|tag| attr(tag, "class") == Some(class))
        .collect();
    assert_eq!(spans.len(), 1, "one .{class}: {element}");
    (spans[0], text(super::element(element, spans[0], "span")))
}

/// `element` without its `aria-hidden="true"` spans: what assistive
/// technology reads.
fn spoken(element: &str) -> String {
    let mut rest = element.to_owned();
    while let Some(tag) = tags(&rest, "span")
        .into_iter()
        .find(|tag| attr(tag, "aria-hidden") == Some("true"))
    {
        let hidden = super::element(&rest, tag, "span").to_owned();
        rest = rest.replacen(&hidden, "", 1);
    }
    text(&rest)
}

/// The workspace's `assets/` folder.
fn assets() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
}

/// MB-0051 P1: one selector in the top bar of every page, signed in or
/// out: a closed control with the current language's flag and name that
/// opens a list (`popover`) of every language, each with its flag and its
/// own name, the current one marked. The footer switcher is gone.
pub fn dropdown() {
    for (lang, page) in pages() {
        let Selector {
            opener,
            opener_element,
            form,
            list,
            options,
        } = selector(&page);

        // In the top bar, outside the navigation panel; no footer.
        let topbar = element(&page, tags(&page, "header")[0], "header");
        assert!(
            topbar.contains(opener_element) && topbar.contains(list),
            "{topbar}"
        );
        let nav = panel(&page);
        assert!(
            !nav.panel.contains(opener) && !nav.panel.contains(form),
            "the selector sits outside the navigation panel"
        );
        assert_ne!(attr(opener, "popovertarget"), Some(nav.id));
        assert!(tags(&page, "footer").is_empty(), "{lang}: a footer remains");
        assert!(!page.contains("lang-switch"), "{lang}: the old switcher");
        assert!(!page.contains("aria-pressed"), "{lang}: aria-pressed");

        // The closed control: the current language's flag and name.
        assert_eq!(attr(opener, "type"), Some("button"), "{opener}");
        assert!(attr(opener, "popovertargetaction").is_none(), "{opener}");
        let (_, full, _) = NAMES
            .into_iter()
            .find(|(l, ..)| *l == lang)
            .expect("language");
        assert_eq!(
            attr(flag(opener_element), "src"),
            Some(format!("/assets/flags/{lang}.svg").as_str()),
            "{opener_element}"
        );
        assert_eq!(span(opener_element, "lang-full").1, full);

        // The list: a popover that is the one POST form.
        assert!(has_attr(form, "popover"), "{form}");
        assert_eq!(attr(form, "method"), Some("post"), "{form}");
        assert_eq!(attr(form, "hx-boost"), Some("false"), "{form}");
        let hidden: Vec<&str> = tags(list, "input")
            .into_iter()
            .filter(|tag| attr(tag, "type") == Some("hidden"))
            .collect();
        assert_eq!(hidden.len(), 1, "{list}");
        assert_eq!(attr(hidden[0], "name"), Some("return"), "{list}");

        // One submit button per language, in `Language::ALL` order, each
        // with its flag and its own name; only the current one is marked.
        assert_eq!(options.len(), NAMES.len(), "{list}");
        for ((tag, option), (option_lang, name, _)) in options.into_iter().zip(NAMES) {
            let value = option_lang.tag();
            assert_eq!(attr(tag, "type"), Some("submit"), "{tag}");
            assert_eq!(attr(tag, "name"), Some("lang"), "{tag}");
            assert_eq!(attr(tag, "value"), Some(value), "{tag}");
            let src = format!("/assets/flags/{value}.svg");
            assert_eq!(attr(flag(option), "src"), Some(src.as_str()), "{option}");
            assert!(
                assets().join(format!("flags/{value}.svg")).is_file(),
                "{src} isn't a file under assets/"
            );
            assert_eq!(span(option, "lang-full").1, name, "{option}");

            let current = option_lang == lang;
            assert_eq!(
                attr(tag, "aria-current"),
                current.then_some("true"),
                "{lang}: {tag}"
            );
            let checks = tags(option, "span")
                .into_iter()
                .filter(|tag| attr(tag, "class") == Some("lang-check"))
                .count();
            assert_eq!(checks, usize::from(current), "{lang}: {option}");
        }
    }
}

/// MB-0051 P3: the control and every option carry a short name (`EN`,
/// `FI`) and the full name, both from the catalogs. `app.css` shows the
/// short one below MB-0040's 40rem breakpoint and the full one above it;
/// the short name is hidden from assistive technology and the full name is
/// never `display: none`, so it is read at every width.
pub fn names() {
    let i18n = I18n::load().expect("catalogs");
    for (lang, page) in pages() {
        let Selector {
            opener_element,
            options,
            ..
        } = selector(&page);
        let current = NAMES
            .into_iter()
            .find(|(l, ..)| *l == lang)
            .expect("language");
        let shown = options
            .into_iter()
            .map(|(_, option)| option)
            .zip(NAMES)
            .chain([(opener_element, current)]);
        for (part, (name_of, full, short)) in shown {
            // From the catalogs, the same in every page language.
            let tag = name_of.tag();
            assert_eq!(
                i18n.text(lang, &format!("language.short.{tag}"), &[])
                    .expect("short name"),
                short
            );
            assert_eq!(
                i18n.text(lang, &format!("language.{tag}"), &[])
                    .expect("full name"),
                full
            );

            let (short_tag, short_text) = span(part, "lang-short");
            assert_eq!(short_text, short, "{part}");
            assert_eq!(attr(short_tag, "aria-hidden"), Some("true"), "{part}");
            let (full_tag, full_text) = span(part, "lang-full");
            assert_eq!(full_text, full, "{part}");
            assert!(attr(full_tag, "aria-hidden").is_none(), "{part}");
            assert!(!has_attr(full_tag, "hidden"), "{part}");
        }
    }

    // The stylesheet: the short name is hidden by default (wide screens);
    // a later rule below MB-0040's 40rem breakpoint shows it and hides the
    // full name, visually only.
    let css = std::fs::read_to_string(assets().join("app.css")).expect("app.css");
    let (narrow_at, _) = css
        .match_indices(NARROW)
        .find(|(at, _)| {
            let block = &css[at + NARROW.len()..];
            let end = block.find("@media").unwrap_or(block.len());
            block[..end].contains(".lang-full") && block[..end].contains(".lang-short")
        })
        .expect("a rule for the names below 40rem");
    assert_eq!(css.matches(HIDE_SHORT).count(), 1, "{HIDE_SHORT}");
    assert!(
        css.find(HIDE_SHORT).is_some_and(|at| at < narrow_at),
        "the default (wide) rule comes first, so the narrow one overrides it"
    );
    for (at, _) in css.match_indices(".lang-full") {
        let rule = &css[at..];
        let rule = &rule[..rule.find('}').expect("closed rule")];
        assert!(
            !rule.contains("display: none") && !rule.contains("visibility"),
            "the full name is hidden from assistive technology: {rule}"
        );
    }
}

/// MB-0051 P4: the control's accessible name says it chooses the language
/// ("Language: English"); it and the options are native buttons (keyboard
/// reachable and operable, no ARIA menu roles); flags are decorative; each
/// option has its own `lang` and is named by its full name; the list is an
/// auto `popover` (Escape and an outside click close it) and a choice is a
/// full page load (`hx-boost="false"`).
pub fn accessible() {
    for (lang, page) in pages() {
        let Selector {
            opener,
            opener_element,
            form,
            list,
            options,
        } = selector(&page);

        let (_, expected) = OPENER_NAMES
            .into_iter()
            .find(|(l, _)| *l == lang)
            .expect("language");
        assert_eq!(accessible_name(&page, opener, "button"), expected);
        // The list is named too: "Language".
        assert!(
            attr(form, "aria-label").is_some_and(|label| !label.is_empty()),
            "{form}"
        );

        // Light dismiss needs the `auto` state: bare, empty or "auto".
        assert!(
            matches!(attr(form, "popover"), None | Some("" | "auto")),
            "{form}"
        );
        assert_eq!(attr(form, "hx-boost"), Some("false"), "{form}");

        // Native controls only.
        for part in [opener_element, list] {
            for word in [" role=", " tabindex=", " aria-pressed=", " aria-haspopup="] {
                assert!(!part.contains(word), "{word} in {part}");
            }
            assert!(tags(part, "a").is_empty(), "a link in {part}");
            // Flags are decoration: the name is the text.
            for image in tags(part, "img") {
                assert_eq!(attr(image, "alt"), Some(""), "{image}");
                assert!(attr(image, "width").is_some(), "{image}");
                assert!(attr(image, "height").is_some(), "{image}");
            }
        }

        for ((tag, option), (option_lang, name, _)) in options.into_iter().zip(NAMES) {
            assert_eq!(attr(tag, "lang"), Some(option_lang.tag()), "{tag}");
            assert!(attr(tag, "aria-label").is_none(), "{tag}");
            assert!(!has_attr(tag, "disabled"), "{tag}");
            // What a screen reader says: the full name, not `EN` or `✓`.
            assert_eq!(spoken(option), name, "{option}");
        }
    }
}
