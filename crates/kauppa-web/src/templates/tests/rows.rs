//! MB-0068 P3 (RR-0055): the reorderable rows of the categories page, a
//! favorites group and a recipe's ingredients. The `#[test]` function,
//! `icon_buttons`, is in `tests.rs`, where the brief's check names it; its
//! body is here.
//!
//! Each list is rendered in every language with two rows, and each row is
//! checked for:
//!
//! 1. **Order.** With the edit panel closed, the row's keyboard stops are,
//!    in order: ↑, ↓, the pencil (the panel's `<summary>`) and the trash
//!    button, all after the name; the drag handle is not one of them.
//! 2. **Names.** Each has the accessible name its old text gave it, with
//!    the row's name ("Move Voi up", "Edit Voi", "Delete Voi").
//! 3. **Icons.** The pencil and the trash show one SVG each and no text;
//!    the SVG is `aria-hidden="true" focusable="false"`.
//! 4. **Forms.** Every action is a `method="post"` form whose `action` is
//!    its `hx-post` (so it works without JavaScript); the trash's form asks
//!    for a confirmation (`hx-confirm`), the ingredient's newly.
//! 5. **Sizes** (`app.css`): the pencil, the trash and the drag handle are
//!    at least 44 × 44 px; each stacked arrow at least 44 × 30 px.

use kauppa_core::domain::Language;
use serde_json::{Value, json};

use super::select::{Rule, app_css, millirem, rules, without_comments};
use super::{TEMPLATES, attr, tags, text};
use crate::i18n::I18n;

/// 44px and 30px, in thousandths of a `rem`.
const MIN_TARGET: u32 = 2750;
const MIN_ARROW_HEIGHT: u32 = 1875;

/// The rows' names, in order.
const NAMES: [&str; 2] = ["Voi", "Maito"];

/// The accessible names a row's controls have, per language: `{}` is the
/// row's name. `(language, up, down, edit, rename, delete, remove)`.
const LABELS: [(Language, [&str; 6]); 2] = [
    (
        Language::En,
        [
            "Move {} up",
            "Move {} down",
            "Edit {}",
            "Rename {}",
            "Delete {}",
            "Remove {}",
        ],
    ),
    (
        Language::Fi,
        [
            "Siirrä ylös: {}",
            "Siirrä alas: {}",
            "Muokkaa: {}",
            "Nimeä uudelleen: {}",
            "Poista: {}",
            "Poista: {}",
        ],
    ),
];

/// The confirmations, per language: `(delete, remove)`.
const CONFIRMS: [(Language, [&str; 2]); 2] = [
    (Language::En, ["Delete “{}”?", "Remove “{}”?"]),
    (Language::Fi, ["Poistetaanko ”{}”?", "Poistetaanko ”{}”?"]),
];

/// The three lists.
#[derive(Clone, Copy, Debug)]
enum List {
    Categories,
    Favorites,
    Ingredients,
}

impl List {
    const ALL: [Self; 3] = [Self::Categories, Self::Favorites, Self::Ingredients];

    fn template(self) -> &'static str {
        match self {
            Self::Categories => "categories/_section",
            Self::Favorites => "products/_group",
            Self::Ingredients => "recipes/_ingredients",
        }
    }

    /// The template's data, with a row per name in [`NAMES`].
    fn data(self) -> Value {
        match self {
            Self::Categories => json!({
                "categories": NAMES.iter().enumerate()
                    .map(|(i, n)| json!({ "id": format!("c{i}"), "name": n }))
                    .collect::<Vec<_>>(),
                "draft_name": "",
                "error": null,
            }),
            Self::Favorites => json!({
                "dom_id": "group-k",
                "open": true,
                "name": "Kylmä",
                "order_key": "k",
                "count": NAMES.len(),
                "products": NAMES.iter().enumerate()
                    .map(|(i, n)| json!({
                        "id": format!("p{i}"),
                        "name": n,
                        // The second is used by a recipe, so its confirmation
                        // carries the warning.
                        "recipe_count": i,
                        "picker": {
                            "options": [{ "id": "k", "name": "Kylmä", "selected": true }],
                            "has_selection": true,
                            "required": true,
                            "no_category": true,
                        },
                    }))
                    .collect::<Vec<_>>(),
            }),
            Self::Ingredients => json!({
                "recipe_id": "r",
                "error": null,
                "has_categories": false,
                "add": {},
                "items": NAMES.iter().enumerate()
                    .map(|(i, n)| json!({
                        "id": format!("i{i}"),
                        "name": n,
                        "quantity": if i == 0 { "50 g" } else { "" },
                        "category": if i == 0 { Value::from("Kylmä") } else { Value::Null },
                    }))
                    .collect::<Vec<_>>(),
            }),
        }
    }

    /// The pencil's and the trash's label, and the confirmation, as indexes
    /// into [`LABELS`] and [`CONFIRMS`].
    fn labels(self) -> (usize, usize, usize) {
        match self {
            Self::Categories => (3, 4, 0),
            Self::Favorites => (2, 4, 0),
            Self::Ingredients => (2, 5, 1),
        }
    }
}

/// `pattern` with `{}` replaced by `name`.
fn fill(pattern: &str, name: &str) -> String {
    pattern.replace("{}", name)
}

fn render(lang: Language, list: List) -> String {
    let hb =
        super::super::build_registry(&I18n::load().expect("catalogs"), lang).expect("registry");
    hb.render(list.template(), &list.data())
        .unwrap_or_else(|e| panic!("{list:?} in {lang}: {e}"))
}

/// The `<li>` elements whose class includes `reorder-row`, in order.
fn rows(html: &str) -> Vec<&str> {
    tags(html, "li")
        .into_iter()
        .filter(|tag| {
            attr(tag, "class").is_some_and(|c| c.split_whitespace().any(|w| w == "reorder-row"))
        })
        .scan(0, |from, tag| {
            let start = *from + html[*from..].find(tag).expect("row in html");
            let len = html[start..].find("</li>").expect("closed row") + "</li>".len();
            *from = start + len;
            Some(&html[start..start + len])
        })
        .collect()
}

/// The element `open` starts inside `html`, up to its matching close
/// (`summary` and `button` don't nest).
fn element_at<'a>(html: &'a str, open: &str, name: &str) -> &'a str {
    let start = html.find(open).expect("tag in row");
    let close = format!("</{name}>");
    let len = html[start..].find(&close).expect("closing tag") + close.len();
    &html[start..start + len]
}

/// The form that holds the element at byte `at` of `row`: its opening tag.
fn form_around(row: &str, at: usize) -> &str {
    let start = row[..at].rfind("<form").expect("inside a form");
    let len = row[start..].find('>').expect("closed tag");
    assert!(
        !row[start..at].contains("</form>"),
        "the control is inside the form"
    );
    &row[start..=start + len]
}

/// The row's keyboard stops while its panel is closed, in order: the opening
/// tags of its `<button>`s, `<summary>`s and visible form fields, outside
/// the `<details>` panel's content.
fn stops(row: &str) -> Vec<&str> {
    let closed = match (row.find("</summary>"), row.find("</details>")) {
        (Some(a), Some(b)) => [&row[..a], &row[b..]],
        _ => [row, ""],
    };
    let mut out = Vec::new();
    for part in closed {
        let mut found: Vec<(usize, &str)> = Vec::new();
        for name in ["button", "summary", "input", "select", "textarea", "a"] {
            for tag in tags(part, name) {
                if name == "input" && attr(tag, "type") == Some("hidden") {
                    continue;
                }
                found.push((part.find(tag).expect("tag"), tag));
            }
        }
        assert!(
            !part.contains("tabindex"),
            "no tabindex changes the order: {part}"
        );
        found.sort_by_key(|(at, _)| *at);
        out.extend(found.into_iter().map(|(_, tag)| tag));
    }
    out
}

/// A row's icon control (`summary` or `button`): one aria-hidden SVG, no
/// text.
fn assert_icon_only(row: &str, open: &str, name: &str, what: &str) {
    let control = element_at(row, open, name);
    let svgs = tags(control, "svg");
    assert_eq!(svgs.len(), 1, "{what}: one icon: {control}");
    assert_eq!(attr(svgs[0], "aria-hidden"), Some("true"), "{what}");
    assert_eq!(attr(svgs[0], "focusable"), Some("false"), "{what}");
    assert_eq!(text(control), "", "{what}: no visible text: {control}");
    assert!(
        attr(open, "class").is_some_and(|c| c.split_whitespace().any(|w| w == "icon-button")),
        "{what}: an .icon-button"
    );
}

/// 1–4 of the module comment, for one row of `list`.
fn check_row(lang: Language, list: List, row: &str, name: &str) {
    let what = format!("{list:?} in {lang}, {name}");
    let labels = LABELS.iter().find(|(l, _)| *l == lang).expect("labels").1;
    let confirms = CONFIRMS
        .iter()
        .find(|(l, _)| *l == lang)
        .expect("confirms")
        .1;
    let (edit, delete, confirm) = list.labels();

    // 1, 2: the stops, in order, with their names.
    let stops = stops(row);
    let names: Vec<String> = stops
        .iter()
        .map(|tag| attr(tag, "aria-label").unwrap_or_default().to_owned())
        .collect();
    let want: Vec<String> = [labels[0], labels[1], labels[edit], labels[delete]]
        .iter()
        .map(|p| fill(p, name))
        .collect();
    assert_eq!(names, want, "{what}: keyboard order and names: {row}");
    let kinds: Vec<&str> = stops
        .iter()
        .map(|tag| tag[1..].split([' ', '>']).next().unwrap_or_default())
        .collect();
    assert_eq!(kinds, ["button", "button", "summary", "button"], "{what}");
    let name_at = row.find("row-name").expect("the name");
    assert!(
        name_at < row.find(stops[0]).expect("↑"),
        "{what}: name first"
    );
    let handle = tags(row, "span")
        .into_iter()
        .find(|t| attr(t, "class") == Some("drag-handle"))
        .expect("the drag handle");
    assert_eq!(attr(handle, "role"), Some("img"), "{what}: the handle");

    // The arrows keep their glyphs.
    assert_eq!(text(element_at(row, stops[0], "button")), "↑", "{what}");
    assert_eq!(text(element_at(row, stops[1], "button")), "↓", "{what}");

    // 3: the pencil opens the panel; it and the trash are icons only.
    let details = tags(row, "details");
    assert_eq!(details.len(), 1, "{what}: one panel");
    assert_eq!(attr(details[0], "class"), Some("row-edit"), "{what}");
    assert!(
        row[row.find(details[0]).expect("details") + details[0].len()..]
            .trim_start()
            .starts_with(stops[2]),
        "{what}: the pencil is the panel's summary"
    );
    assert_icon_only(row, stops[2], "summary", &format!("{what}, pencil"));
    assert_icon_only(row, stops[3], "button", &format!("{what}, trash"));

    // 4: every form works without JavaScript; the trash confirms.
    for tag in tags(row, "form") {
        assert_eq!(attr(tag, "method"), Some("post"), "{what}: {tag}");
        assert!(
            attr(tag, "action").is_some() && attr(tag, "action") == attr(tag, "hx-post"),
            "{what}: {tag}"
        );
    }
    let trash_at = row.find(stops[3]).expect("trash");
    let trash_form = form_around(row, trash_at);
    assert!(
        attr(trash_form, "action").is_some_and(|a| a.ends_with("/delete")),
        "{what}: {trash_form}"
    );
    let asked = attr(trash_form, "hx-confirm").unwrap_or_default();
    assert!(
        asked.starts_with(&fill(confirms[confirm], name)),
        "{what}: the confirmation: {trash_form}"
    );
    let arrows_form = form_around(row, row.find(stops[0]).expect("↑"));
    assert!(
        attr(arrows_form, "action").is_some_and(|a| a.ends_with("/move")),
        "{what}: {arrows_form}"
    );
}

/// The rule whose selector list is exactly `selector`, outside `@supports`
/// and `@media` blocks or not: the first.
fn rule<'r, 'a>(rules: &'r [Rule<'a>], selector: &str) -> &'r Rule<'a> {
    rules
        .iter()
        .find(|r| r.selectors == [selector])
        .unwrap_or_else(|| panic!("app.css has a rule for `{selector}`"))
}

/// 5 of the module comment.
fn check_sizes() {
    let css = without_comments(&app_css());
    let rules = rules(&css);
    for (selector, width, height) in [
        (".icon-button", MIN_TARGET, MIN_TARGET),
        (".sortable-ready .drag-handle", MIN_TARGET, MIN_TARGET),
        (".row-move .link", MIN_TARGET, MIN_ARROW_HEIGHT),
    ] {
        let rule = rule(&rules, selector);
        assert!(
            millirem(rule.value("min-width")) >= width,
            "{selector}: min-width"
        );
        assert!(
            millirem(rule.value("min-height")) >= height,
            "{selector}: min-height"
        );
    }
    // Nothing else sets the size of these controls.
    for other in &rules {
        let targets = other.selectors.iter().any(|s| {
            s.contains("icon-button") || s.contains("row-move") || s.contains("drag-handle")
        });
        if !targets {
            continue;
        }
        for (property, value) in &other.declarations {
            if matches!(
                *property,
                "width" | "height" | "max-width" | "max-height" | "min-width" | "min-height"
            ) && !other.selectors.iter().any(|s| s.ends_with(" .icon"))
            {
                let known = [
                    ".icon-button",
                    ".sortable-ready .drag-handle",
                    ".row-move .link",
                ];
                assert!(
                    other.selectors.iter().all(|s| known.contains(s)),
                    "`{}` sets {property}: {value}",
                    other.selectors.join(", ")
                );
            }
        }
    }
}

/// The icon partials: an SVG each, hidden from assistive technology, not
/// focusable, drawn in strokes only (the README's line; `app.css` sets the
/// stroke).
fn check_icons() {
    let mut icons: Vec<(&str, &str)> = TEMPLATES
        .iter()
        .filter(|(name, _)| name.starts_with("icons/"))
        .copied()
        .collect();
    icons.sort_unstable();
    let names: Vec<&str> = icons.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["icons/_pencil", "icons/_trash"], "the icons");
    for (name, source) in icons {
        let svgs = tags(source, "svg");
        assert_eq!(svgs.len(), 1, "{name}");
        let svg = svgs[0];
        assert_eq!(attr(svg, "viewBox"), Some("0 0 24 24"), "{name}");
        assert_eq!(attr(svg, "aria-hidden"), Some("true"), "{name}");
        assert_eq!(attr(svg, "focusable"), Some("false"), "{name}");
        assert_eq!(attr(svg, "class"), Some("icon"), "{name}");
        for word in ["<title", "<text", "fill=", "stroke=", "<image", "href"] {
            assert!(!source.contains(word), "{name}: `{word}`");
        }
    }
}

/// MB-0068 P3: see the module comment.
pub fn icon_buttons() {
    for lang in Language::ALL {
        for list in List::ALL {
            let html = render(lang, list);
            let rows = rows(&html);
            assert_eq!(rows.len(), NAMES.len(), "{list:?} in {lang}: {html}");
            for (row, name) in rows.into_iter().zip(NAMES) {
                check_row(lang, list, row, name);
            }
        }
    }
    check_sizes();
    check_icons();
}
