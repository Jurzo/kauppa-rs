//! MB-0053 P1: every `<select>` hides the browser's arrow and shows the
//! app's chevron, drawn in `app.css` with gradients (the
//! Content-Security-Policy, RR-0019, has no `data:` images). The `#[test]`
//! function is in `tests.rs`, where the brief's check names it; its body is
//! here.
//!
//! The rule the test checks, on `app.css` without its comments:
//!
//! 1. One top-level rule has the bare type selector `select`, so it matches
//!    every `<select>` whatever its ancestors. It sets `appearance: none`
//!    (and the `-webkit-` form), two `linear-gradient` layers in
//!    `background-image` coloured by `var(--muted)` (defined for light and
//!    dark), `background-repeat: no-repeat`, a `background-size` of `W H`
//!    and a `background-position` of `right A center, right B center`, all
//!    in `rem`. The two layers are the chevron's halves: they touch
//!    (`A = B + W`) and the nearer one is at least [`MIN_EDGE`] from the
//!    edge.
//! 2. Its right padding is at least the chevron's far side (`A + W`) plus
//!    [`MIN_GAP`], so the selected text ends before the chevron.
//! 3. No other rule whose subject is a `select` sets a property in
//!    [`CHEVRON_PROPERTIES`], which could undo it whatever the specificity.
//! 4. No `<select>` in the templates has `multiple` or `size` (a list box
//!    has no arrow), or a `class` or `style` of its own.
//! 5. `app.css` has no `data:` URL.

use std::collections::BTreeSet;

use super::{TEMPLATES, attr, tags};

/// The `name` of every `<select>` the templates render: the category
/// picker, the recipe picker, the list picker and the share role.
const SELECT_NAMES: [&str; 4] = ["category_id", "list_id", "recipe_id", "role"];

/// The least space between the chevron and the select's right edge, and
/// between the selected text and the chevron, in thousandths of a `rem`.
const MIN_EDGE: u32 = 500;
const MIN_GAP: u32 = 250;

/// What draws the chevron and makes room for it. Only the `select` rule
/// may set these on a `<select>`. `outline` is here so that the browser's
/// focus ring stays.
const CHEVRON_PROPERTIES: [&str; 13] = [
    "appearance",
    "-webkit-appearance",
    "-moz-appearance",
    "background",
    "background-image",
    "background-position",
    "background-repeat",
    "background-size",
    "padding",
    "padding-right",
    "padding-inline",
    "padding-inline-end",
    "outline",
];

/// `app.css`, as the binary serves it.
fn app_css() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/app.css"))
        .expect("app.css")
}

/// `css` without its `/* … */` comments.
fn without_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let len = rest[start..].find("*/").expect("closed comment") + 2;
        rest = &rest[start + len..];
    }
    out.push_str(rest);
    out
}

/// A style rule: its selectors and its `property: value` declarations.
struct Rule<'a> {
    selectors: Vec<&'a str>,
    declarations: Vec<(&'a str, &'a str)>,
}

impl<'a> Rule<'a> {
    /// The value of `property`, declared exactly once.
    fn value(&self, property: &str) -> &'a str {
        let values: Vec<&str> = self
            .declarations
            .iter()
            .filter(|(name, _)| *name == property)
            .map(|(_, value)| *value)
            .collect();
        assert_eq!(values.len(), 1, "`select` declares {property} once");
        values[0]
    }

    /// Whether a selector's subject (its last compound) is a `select`
    /// element: `select`, `.add-product select`, `label > select:focus`.
    fn styles_a_select(&self) -> bool {
        self.selectors.iter().any(|selector| {
            let subject = selector
                .rsplit(|c: char| c.is_ascii_whitespace() || matches!(c, '>' | '+' | '~'))
                .next()
                .unwrap_or_default();
            subject.strip_prefix("select").is_some_and(|rest| {
                !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
        })
    }
}

/// Every style rule in `css` (comments removed), also those inside `@media`
/// and `@supports` blocks: the blocks that contain no other block.
fn rules(css: &str) -> Vec<Rule<'_>> {
    let mut out = Vec::new();
    let mut prelude = 0;
    let mut open = None;
    for (at, c) in css.char_indices() {
        match c {
            '{' => {
                if let Some(inner) = open {
                    // The block opened before was an at-rule's.
                    prelude = inner + 1;
                }
                open = Some(at);
            }
            '}' => {
                if let Some(start) = open.take() {
                    out.push(Rule {
                        selectors: css[prelude..start].split(',').map(str::trim).collect(),
                        declarations: css[start + 1..at]
                            .split(';')
                            .filter_map(|d| d.split_once(':'))
                            .map(|(name, value)| (name.trim(), value.trim()))
                            .collect(),
                    });
                }
                prelude = at + 1;
            }
            _ => {}
        }
    }
    out
}

/// A length in `rem` (`2.25rem`, `0.5rem`) in thousandths of a `rem`.
fn millirem(length: &str) -> u32 {
    let number = length
        .strip_suffix("rem")
        .unwrap_or_else(|| panic!("{length} is in rem"));
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    assert!(fraction.len() <= 3, "{length}: at most three decimals");
    let digits = format!("{whole}{fraction:0<3}");
    digits
        .parse()
        .unwrap_or_else(|_| panic!("{length} is a number"))
}

/// The offset `N` of a `right N center` background position.
fn right_offset(position: &str) -> u32 {
    let parts: Vec<&str> = position.split_whitespace().collect();
    assert!(
        matches!(parts[..], ["right", _, "center"]),
        "`{position}` is `right <offset> center`"
    );
    millirem(parts[1])
}

/// The right padding of the `select` rule: its `padding-right`, or the
/// second value of a four-value `padding`.
fn padding_right(rule: &Rule<'_>) -> u32 {
    if rule.declarations.iter().any(|(p, _)| *p == "padding-right") {
        return millirem(rule.value("padding-right"));
    }
    let parts: Vec<&str> = rule.value("padding").split_whitespace().collect();
    assert_eq!(parts.len(), 4, "`padding` names all four sides");
    millirem(parts[1])
}

/// MB-0053 P1: see the module comment for the rule.
pub fn chevron() {
    // 4. The selects, as the templates write them.
    let selects: Vec<(&str, &str)> = TEMPLATES
        .iter()
        .flat_map(|(name, source)| tags(source, "select").into_iter().map(|tag| (*name, tag)))
        .collect();
    let names: BTreeSet<&str> = selects
        .iter()
        .map(|(template, tag)| {
            attr(tag, "name").unwrap_or_else(|| panic!("{template}: {tag} has a name"))
        })
        .collect();
    assert_eq!(names, BTreeSet::from(SELECT_NAMES), "the selects");
    for (template, tag) in &selects {
        for word in tag
            .trim_matches(|c| c == '<' || c == '>')
            .split_whitespace()
        {
            let attribute = word.split('=').next().unwrap_or_default();
            assert!(
                !matches!(attribute, "multiple" | "size" | "class" | "style"),
                "{template}: {tag} has `{attribute}`"
            );
        }
    }

    let css = without_comments(&app_css());
    // 5. No `data:` URL (RR-0019: `img-src 'self'`).
    assert!(
        !css.to_ascii_lowercase().contains("data:"),
        "app.css has no data: URL"
    );

    // 1. The one `select` rule, at the top level.
    let rules = rules(&css);
    let bare: Vec<&Rule<'_>> = rules
        .iter()
        .filter(|rule| rule.selectors.contains(&"select"))
        .collect();
    assert_eq!(bare.len(), 1, "one rule with the bare `select` selector");
    let rule = bare[0];
    assert_eq!(rule.selectors, ["select"], "`select` alone");
    assert!(css.contains("\nselect {"), "`select` is a top-level rule");

    assert_eq!(rule.value("appearance"), "none");
    assert_eq!(rule.value("-webkit-appearance"), "none");
    let image = rule.value("background-image");
    assert_eq!(image.matches("linear-gradient(").count(), 2, "{image}");
    assert!(!image.contains("url("), "{image}");
    assert_eq!(
        image.matches("var(--muted)").count() % 2,
        0,
        "both halves use --muted: {image}"
    );
    assert!(image.contains("var(--muted)"), "{image}");
    assert!(
        !image.contains('#') && !image.contains("rgb"),
        "theme colours only: {image}"
    );
    assert_eq!(rule.value("background-repeat"), "no-repeat");
    assert!(
        !rule.declarations.iter().any(|(p, _)| *p == "outline"),
        "the browser's focus outline stays"
    );

    let size: Vec<u32> = rule
        .value("background-size")
        .split_whitespace()
        .map(millirem)
        .collect();
    assert_eq!(size.len(), 2, "background-size is `W H`");
    let width = size[0];
    assert!(width > 0 && size[1] > 0, "the chevron has a size");
    let offsets: Vec<u32> = rule
        .value("background-position")
        .split(',')
        .map(right_offset)
        .collect();
    assert_eq!(offsets.len(), 2, "one position per half");
    let (far, near) = (offsets[0].max(offsets[1]), offsets[0].min(offsets[1]));
    assert_eq!(far, near + width, "the halves touch");
    assert!(near >= MIN_EDGE, "space from the right edge: {near}");

    // 2. Room for the chevron and a gap.
    let padding = padding_right(rule);
    assert!(
        padding >= far + width + MIN_GAP,
        "padding-right {padding} holds the chevron ({}) and a gap",
        far + width
    );

    // The colour is themed: `--muted` in `:root` and in the dark block.
    let dark = css
        .split_once("@media (prefers-color-scheme: dark)")
        .expect("dark block");
    assert!(dark.0.contains("--muted:"), "light --muted");
    let dark_block = dark.1.split_once("}\n}").expect("end of dark block").0;
    assert!(dark_block.contains("--muted:"), "dark --muted");

    // 3. Nothing else restyles a select's arrow, padding or outline.
    for other in rules.iter().filter(|r| r.styles_a_select()) {
        if std::ptr::eq(other, rule) {
            continue;
        }
        for (property, value) in &other.declarations {
            assert!(
                !CHEVRON_PROPERTIES.contains(property),
                "`{}` sets {property}: {value}",
                other.selectors.join(", ")
            );
        }
    }
}
