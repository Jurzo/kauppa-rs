//! MB-0054 P1–P3 (RR-0034): `app.css` implements the design system in
//! `design-system/`. The `#[test]` functions are in `tests.rs`, where the
//! brief's checks name them; their bodies are here.
//!
//! - [`tokens`] (P1): the three token rules of `app.css` (light, dark from
//!   the OS, dark by `data-theme`) hold exactly the tokens of
//!   `design-system/tokens.json`, with its values; so does
//!   `design-system/tokens.css`. No old variable is left, every `var()`
//!   names a declared token, and no other rule has a raw colour.
//! - [`contrast`] (P2): the pairs of the design system's contrast table,
//!   and the other pairs `app.css` uses for text, meet their level in both
//!   themes; a rule that fills with the brand colour writes in `on-brand`.
//! - [`components`] (P3): buttons, text fields, selects and rows follow
//!   `design-system/components/`, and a page has at most one primary
//!   button.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::select::{Rule, app_css, millirem, rules, without_comments};
use super::{TEMPLATES, attr, tags, template};

/// The token rules: light (the default), dark when the OS says so and the
/// page didn't choose light, and dark chosen with `data-theme`.
const LIGHT: &str = ":root";
const DARK_OS: &str = r#":root:not([data-theme="light"])"#;
const DARK_CHOSEN: &str = r#":root[data-theme="dark"]"#;
/// The at-rule [`DARK_OS`] sits in.
const DARK_MEDIA: &str = "@media (prefers-color-scheme: dark) {";

/// The variables `app.css` had before the tokens.
const OLD_VARIABLES: [&str; 6] = [
    "--fg",
    "--muted",
    "--primary",
    "--primary-fg",
    "--error",
    "--radius",
];

/// Variables of the app's own that aren't design tokens: lengths only.
const APP_VARIABLES: [&str; 1] = ["--maxw"];

/// Colour functions and names that would be a raw colour in a rule.
const COLOUR_FUNCTIONS: [&str; 9] = [
    "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(",
];
const COLOUR_NAMES: [&str; 30] = [
    "aqua", "beige", "black", "blue", "brown", "coral", "crimson", "cyan", "fuchsia", "gold",
    "gray", "green", "grey", "indigo", "ivory", "khaki", "lime", "magenta", "maroon", "navy",
    "olive", "orange", "pink", "purple", "red", "silver", "teal", "violet", "white", "yellow",
];

/// The least height of a control (44px) and of a list row (48px), in
/// thousandths of a `rem` (16px).
const MIN_CONTROL: u32 = 2750;
const MIN_ROW: u32 = 3000;

/// `(text, background, least ratio)`: the design system's contrast table
/// (`design-system/README.md`), then the other pairs `app.css` writes text
/// in. 4.5:1 for text, 3:1 for the edge of a control.
const CONTRAST: [(&str, &str, f64); 17] = [
    ("ink", "surface", 4.5),
    ("ink-muted", "surface", 4.5),
    ("ink-subtle", "surface", 4.5),
    ("brand", "bg", 4.5),
    ("on-brand", "brand", 4.5),
    ("danger", "surface", 4.5),
    ("border-strong", "surface", 3.0),
    ("ink", "bg", 4.5),
    ("ink-muted", "bg", 4.5),
    ("ink-subtle", "bg", 4.5),
    ("brand", "surface", 4.5),
    ("danger", "bg", 4.5),
    ("on-brand", "brand-hover", 4.5),
    ("ink", "brand-subtle", 4.5),
    ("ink", "surface-raised", 4.5),
    ("danger", "danger-subtle", 4.5),
    ("border-strong", "bg", 3.0),
];

/// A file under the workspace's `design-system/`.
fn design_file(name: &str) -> String {
    let path = format!("{}/../../design-system/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A JSON string.
fn text<'a>(value: &'a Value, what: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{what} is a string: {value}"))
}

/// A token's value as CSS writes it: `{brand}` is `var(--brand)`.
fn css_value(value: &str) -> String {
    value
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .map_or_else(|| value.to_owned(), |name| format!("var(--{name})"))
}

/// What `tokens.json` says the token rules hold: `--name` to value.
struct Tokens {
    /// Every token, with its light value.
    light: BTreeMap<String, String>,
    /// The tokens that differ by theme (colours, shadows), with their dark
    /// value.
    dark: BTreeMap<String, String>,
}

fn tokens_json() -> Tokens {
    let json: Value = serde_json::from_str(&design_file("tokens.json")).expect("tokens.json");
    let list = |group: &str| -> Vec<Value> {
        json[group]["tokens"]
            .as_array()
            .unwrap_or_else(|| panic!("{group}.tokens"))
            .clone()
    };
    let mut light = BTreeMap::new();
    let mut dark = BTreeMap::new();
    for group in ["color", "shadow"] {
        for token in list(group) {
            let name = format!("--{}", text(&token["name"], "name"));
            light.insert(
                name.clone(),
                css_value(text(&token["value"]["light"], &name)),
            );
            dark.insert(
                name.clone(),
                css_value(text(&token["value"]["dark"], &name)),
            );
        }
    }
    for group in ["spacing", "radius"] {
        for token in list(group) {
            let name = format!("--{}", text(&token["name"], "name"));
            light.insert(name.clone(), text(&token["value"], &name).to_owned());
        }
    }
    let families = json["type"]["families"].as_object().expect("type.families");
    for (family, stack) in families {
        let name = format!("--font-{family}");
        light.insert(name.clone(), text(stack, &name).to_owned());
    }
    assert_eq!(light.len(), 19 + 2 + 8 + 5 + 3, "the tokens of tokens.json");
    Tokens { light, dark }
}

/// A value with its runs of whitespace as one space.
fn squeeze(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The one rule whose selector list is exactly `selectors`.
fn rule<'r, 'a>(rules: &'r [Rule<'a>], selectors: &[&str]) -> &'r Rule<'a> {
    let found: Vec<&Rule<'_>> = rules.iter().filter(|r| r.selectors == selectors).collect();
    assert_eq!(found.len(), 1, "one rule for `{}`", selectors.join(", "));
    found[0]
}

/// The custom properties a rule declares, each once.
fn variables<'a>(rule: &Rule<'a>) -> BTreeMap<&'a str, &'a str> {
    let mut out = BTreeMap::new();
    for (name, value) in &rule.declarations {
        if name.starts_with("--") {
            assert!(
                out.insert(*name, *value).is_none(),
                "`{}` declares {name} twice",
                rule.selectors.join(", ")
            );
        }
    }
    out
}

/// Whether a rule is one of the three token rules.
fn is_token_rule(rule: &Rule<'_>) -> bool {
    [LIGHT, DARK_OS, DARK_CHOSEN]
        .iter()
        .any(|selector| rule.selectors == [*selector])
}

/// The token rules of a stylesheet (`what`, comments removed) hold the
/// tokens of `tokens.json`, and nothing else but `color-scheme` and, in the
/// light rule, `extra`.
fn assert_token_rules(what: &str, css: &str, extra: &[&str]) {
    let expected = tokens_json();
    let rules = rules(css);

    let light = rule(&rules, &[LIGHT]);
    let declared = variables(light);
    for (name, value) in &expected.light {
        let got = declared
            .get(name.as_str())
            .unwrap_or_else(|| panic!("{what}: `{LIGHT}` declares {name}"));
        assert_eq!(&squeeze(got), value, "{what}: light {name}");
    }
    for name in declared.keys() {
        assert!(
            expected.light.contains_key(*name) || extra.contains(name),
            "{what}: `{LIGHT}` declares {name}, which is not a token"
        );
    }
    assert_eq!(light.value("color-scheme"), "light", "{what}");

    for selector in [DARK_OS, DARK_CHOSEN] {
        let dark = rule(&rules, &[selector]);
        let declared = variables(dark);
        let names: BTreeSet<&str> = declared.keys().copied().collect();
        let expected_names: BTreeSet<&str> = expected.dark.keys().map(String::as_str).collect();
        assert_eq!(names, expected_names, "{what}: `{selector}`");
        for (name, value) in &expected.dark {
            assert_eq!(
                &squeeze(declared[name.as_str()]),
                value,
                "{what}: dark {name} in `{selector}`"
            );
        }
        assert_eq!(dark.value("color-scheme"), "dark", "{what}: `{selector}`");
    }
    for token_rule in rules.iter().filter(|r| is_token_rule(r)) {
        for (property, _) in &token_rule.declarations {
            assert!(
                property.starts_with("--") || *property == "color-scheme",
                "{what}: a token rule sets {property}"
            );
        }
    }

    // The OS's dark rule is the first thing in the dark media query.
    let (_, after) = css
        .split_once(DARK_MEDIA)
        .unwrap_or_else(|| panic!("{what}: {DARK_MEDIA}"));
    assert!(
        after.trim_start().starts_with(DARK_OS),
        "{what}: `{DARK_OS}` is inside the dark media query"
    );
}

/// The names in every `var(--name…)` of `css`.
fn var_references(css: &str) -> BTreeSet<&str> {
    css.match_indices("var(")
        .map(|(at, _)| {
            let rest = &css[at + 4..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .collect()
}

/// What makes `value` a raw colour, if anything.
fn raw_colour(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    if lower.contains('#') {
        return Some("a `#` colour".to_owned());
    }
    if let Some(function) = COLOUR_FUNCTIONS.iter().find(|f| {
        lower.match_indices(**f).any(|(at, _)| {
            // `rgb(` itself, not the end of another function's name.
            !lower[..at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
        })
    }) {
        return Some(format!("`{function}…)`"));
    }
    lower
        .split(|c: char| !(c.is_ascii_alphabetic() || c == '-'))
        .find(|word| COLOUR_NAMES.contains(word))
        .map(|word| format!("the colour name `{word}`"))
}

/// MB-0054 P1: see the module comment.
pub fn tokens() {
    let css = without_comments(&app_css());
    assert_token_rules("app.css", &css, &APP_VARIABLES);
    // Nothing generates `tokens.css` in this repository: keep it true too.
    assert_token_rules(
        "design-system/tokens.css",
        &without_comments(&design_file("tokens.css")),
        &[],
    );

    let rules = rules(&css);
    let declared = variables(rule(&rules, &[LIGHT]));
    for name in var_references(&css) {
        assert!(
            !OLD_VARIABLES.contains(&name),
            "app.css still uses the old variable {name}"
        );
        assert!(
            declared.contains_key(name),
            "app.css uses var({name}), which `{LIGHT}` doesn't declare"
        );
    }
    for old in OLD_VARIABLES {
        assert!(
            !declared.contains_key(old),
            "app.css still declares the old variable {old}"
        );
    }

    // Outside the token rules, colours come from tokens only.
    for other in rules.iter().filter(|r| !is_token_rule(r)) {
        for (property, value) in &other.declarations {
            assert!(
                !property.starts_with("--"),
                "`{}` declares {property}: variables belong in `{LIGHT}`",
                other.selectors.join(", ")
            );
            if let Some(why) = raw_colour(value) {
                panic!(
                    "`{}` sets {property}: {value}, with {why}; use a token",
                    other.selectors.join(", ")
                );
            }
        }
    }
}

/// The relative luminance (WCAG 2) of a `#rrggbb` colour.
fn luminance(hex: &str) -> f64 {
    let digits = hex
        .strip_prefix('#')
        .filter(|d| d.len() == 6)
        .unwrap_or_else(|| panic!("{hex} is #rrggbb"));
    let channel = |at: usize| -> f64 {
        let byte = u8::from_str_radix(&digits[at..at + 2], 16)
            .unwrap_or_else(|_| panic!("{hex} is hexadecimal"));
        let c = f64::from(byte) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.0722f64.mul_add(
        channel(4),
        0.2126f64.mul_add(channel(0), 0.7152 * channel(2)),
    )
}

/// The contrast ratio (WCAG 2) of two `#rrggbb` colours.
fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// MB-0054 P2: see the module comment.
pub fn contrast() {
    let css = without_comments(&app_css());
    let rules = rules(&css);

    // The colours as served: the light rule, and the dark rules over it.
    let light = variables(rule(&rules, &[LIGHT]));
    for (theme, selector) in [("light", LIGHT), ("dark", DARK_OS), ("dark", DARK_CHOSEN)] {
        let mut colours = light.clone();
        colours.extend(variables(rule(&rules, &[selector])));
        let colour = |name: &str| -> &str {
            colours
                .get(format!("--{name}").as_str())
                .copied()
                .unwrap_or_else(|| panic!("`{selector}` has --{name}"))
        };
        for (text, background, least) in CONTRAST {
            let ratio = contrast_ratio(colour(text), colour(background));
            assert!(
                ratio >= least,
                "{theme} (`{selector}`): {text} on {background} is {ratio:.2}:1, below {least}:1"
            );
        }
    }

    // A brand fill carries `on-brand` text: white in light, steel black in
    // dark, never white on the dark theme's orange.
    let mut fills = 0;
    for other in rules.iter().filter(|r| !is_token_rule(r)) {
        let fills_with_brand = other.declarations.iter().any(|(property, value)| {
            matches!(*property, "background" | "background-color")
                && (value.contains("var(--brand)") || value.contains("var(--brand-hover)"))
        });
        if fills_with_brand {
            fills += 1;
            assert_eq!(
                other.value("color"),
                "var(--on-brand)",
                "`{}` fills with the brand colour",
                other.selectors.join(", ")
            );
        }
    }
    assert!(
        fills >= 2,
        "the primary button and the checked box fill with brand"
    );
}

/// A rule's length in `rem`, in thousandths.
fn length(rule: &Rule<'_>, property: &str) -> u32 {
    millirem(rule.value(property))
}

/// How many primary buttons a page built from `source` can show at once.
/// Partials (`{{> name}}`) count with their page; of the branches of an
/// `{{#if}}` or `{{#unless}}` only one shows, so the largest counts; a
/// primary button inside `{{#each}}` would repeat and is refused.
fn primary_buttons(name: &str, source: &str) -> usize {
    /// Counts from `rest` to the end of the enclosing block: the count, and
    /// what follows the block's closing tag.
    fn sequence<'a>(name: &str, mut rest: &'a str, in_block: bool) -> (Vec<usize>, &'a str) {
        // One count per `{{else}}`-separated branch.
        let mut branches = vec![0];
        loop {
            let Some(open) = rest.find("{{") else {
                assert!(!in_block, "{name}: an unclosed block");
                *branches.last_mut().expect("a branch") += primaries(rest);
                return (branches, "");
            };
            *branches.last_mut().expect("a branch") += primaries(&rest[..open]);
            rest = &rest[open..];
            if rest.starts_with("{{!--") {
                let end = rest.find("--}}").expect("closed comment") + 4;
                rest = &rest[end..];
                continue;
            }
            let end = rest.find("}}").expect("closed tag");
            // `~` is handlebars' whitespace control: `{{~#if x~}}`.
            let tag = rest[2..end].trim().trim_matches('~').trim();
            rest = &rest[end + 2..];
            if let Some(partial) = tag.strip_prefix('>') {
                let partial = partial.split_whitespace().next().expect("partial name");
                if !partial.starts_with('@') {
                    *branches.last_mut().expect("a branch") +=
                        primary_buttons(partial, template(partial));
                }
            } else if tag.starts_with('#') {
                let (inner, after) = sequence(name, rest, true);
                rest = after;
                let count = if tag.starts_with("#if") || tag.starts_with("#unless") {
                    inner.into_iter().max().unwrap_or(0)
                } else {
                    let total: usize = inner.iter().sum();
                    assert!(
                        !(tag.starts_with("#each") && total > 0),
                        "{name}: a primary button inside {{{{{tag}}}}} repeats"
                    );
                    total
                };
                *branches.last_mut().expect("a branch") += count;
            } else if tag.starts_with('/') {
                assert!(in_block, "{name}: {{{{{tag}}}}} closes nothing");
                return (branches, rest);
            } else if tag == "else" || tag.starts_with("else ") {
                branches.push(0);
            }
        }
    }
    fn primaries(html: &str) -> usize {
        html.matches(r#"class="button primary""#).count()
    }
    let (branches, _) = sequence(name, source, false);
    branches.into_iter().sum()
}

/// MB-0054 P3: see the module comment.
pub fn components() {
    let css = without_comments(&app_css());
    let rules = rules(&css);
    let edge = "1px solid var(--border-strong)";

    // Buttons (`components/Button.md`): secondary by default, one primary,
    // quiet (`.link`) and danger.
    let button = rule(&rules, &[".button"]);
    assert!(length(button, "min-height") >= MIN_CONTROL, "button height");
    assert_eq!(button.value("border"), edge);
    assert_eq!(button.value("border-radius"), "var(--radius-md)");
    assert_eq!(button.value("background"), "var(--surface)");
    assert_eq!(button.value("color"), "var(--ink)");
    assert_eq!(button.value("font-weight"), "600");
    let primary = rule(&rules, &[".button.primary"]);
    assert_eq!(primary.value("background"), "var(--brand)");
    assert_eq!(primary.value("border-color"), "var(--brand)");
    let hover = rule(&rules, &[".button.primary:hover"]);
    assert_eq!(hover.value("background"), "var(--brand-hover)");
    assert_eq!(
        rule(&rules, &[".button:hover"]).value("background"),
        "var(--brand-subtle)"
    );
    assert_eq!(rule(&rules, &[".link"]).value("color"), "var(--brand)");
    assert_eq!(
        rule(&rules, &[".link.danger"]).value("color"),
        "var(--danger)"
    );

    // Text fields (`components/TextField.md`) and selects: 16px text so
    // phones don't zoom, a 44px box, a control's edge.
    let field = rule(&rules, &["input", "textarea"]);
    let select = rule(&rules, &["select"]);
    for control in [field, select] {
        let what = control.selectors.join(", ");
        assert_eq!(control.value("font-size"), "1rem", "{what}");
        assert!(length(control, "min-height") >= MIN_CONTROL, "{what}");
        assert_eq!(control.value("border"), edge, "{what}");
        assert_eq!(control.value("border-radius"), "var(--radius-md)", "{what}");
        assert_eq!(control.value("color"), "var(--ink)", "{what}");
    }
    assert_eq!(field.value("background"), "var(--surface)");
    assert_eq!(select.value("background-color"), "var(--surface)");

    // Focus: one ring for every interactive element, never removed.
    let focus = rule(&rules, &[":focus-visible"]);
    assert_eq!(focus.value("outline"), "2px solid var(--focus)");
    assert_eq!(focus.value("outline-offset"), "2px");
    for other in &rules {
        for (property, value) in &other.declarations {
            assert!(
                !(property.starts_with("outline") && (value.starts_with("none") || *value == "0")),
                "`{}` removes the focus ring: {property}: {value}",
                other.selectors.join(", ")
            );
        }
    }

    // Rows (`components/ListItem.md`): 48px, in a surface card.
    let row = rule(&rules, &[".category-row", ".product-row"]);
    assert!(length(row, "min-height") >= MIN_ROW, "row height");
    let card = rule(&rules, &[".category-list", ".product-list"]);
    assert_eq!(card.value("background"), "var(--surface)");
    assert_eq!(card.value("border-radius"), "var(--radius-lg)");
    assert!(
        length(rule(&rules, &[".shop-toggle"]), "min-height") >= MIN_ROW,
        "shop row height"
    );

    // The shop view's checkbox: a drawn 22px box, filled with the brand
    // colour when the item is found; the found item's name is struck
    // through in `ink-subtle`.
    let open_box = rule(&rules, &[".shop-box"]);
    assert_eq!(open_box.value("width"), "1.375rem");
    assert_eq!(open_box.value("height"), "1.375rem");
    assert_eq!(open_box.value("border"), "2px solid var(--border-strong)");
    assert_eq!(open_box.value("border-radius"), "var(--radius-sm)");
    let checked_box = rule(&rules, &[".shop-row.found .shop-box"]);
    assert_eq!(checked_box.value("background"), "var(--brand)");
    assert_eq!(checked_box.value("border-color"), "var(--brand)");
    let found = rule(&rules, &[".shop-row.found .product-name"]);
    assert_eq!(found.value("color"), "var(--ink-subtle)");
    assert_eq!(found.value("text-decoration"), "line-through");

    // The box replaces the glyphs, in both branches of the shop row; it is
    // decoration (the button's label and the hidden text say the state).
    for (name, source) in TEMPLATES {
        assert!(
            !source.contains('☐') && !source.contains('☑'),
            "{name}: a checkbox glyph"
        );
    }
    let boxes: Vec<&str> = tags(template("lists/_shop"), "span")
        .into_iter()
        .filter(|tag| attr(tag, "class") == Some("shop-box"))
        .collect();
    assert_eq!(boxes.len(), 2, "the editor's and the viewer's box");
    for tag in boxes {
        assert_eq!(attr(tag, "aria-hidden"), Some("true"), "{tag}");
    }

    // One primary button per page (owner, 2026-11-13: "Add" on the list
    // and recipe pages; "Go shopping" and the details' "Save" are
    // secondary).
    let mut pages = 0;
    for (name, source) in TEMPLATES {
        let file = name.rsplit('/').next().unwrap_or(name);
        if file.starts_with('_') || *name == "layout" {
            continue;
        }
        pages += 1;
        let count = primary_buttons(name, source);
        assert!(count <= 1, "{name}: {count} primary buttons");
    }
    assert!(pages >= 10, "the pages were counted: {pages}");
    for page in ["lists/show", "recipes/show", "index"] {
        assert_eq!(primary_buttons(page, template(page)), 1, "{page}");
    }
}
