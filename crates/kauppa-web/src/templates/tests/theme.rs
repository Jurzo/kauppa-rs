//! MB-0055 P1 and P3–P7 (RR-0035): the theme switch in the top bar, and a
//! top bar that never wraps. Each `#[test]` here is named by the brief's
//! check (`templates::tests::theme::…`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use kauppa_core::domain::Language;

use super::select::{Rule, app_css, millirem, rules, without_comments};
use super::{attr, element, panel, tags, text};
use crate::i18n::I18n;

/// The three choices, in the order the switch shows them.
const CHOICES: [&str; 3] = ["system", "light", "dark"];

/// Per page language: the group's name, each choice's label, the no-JS
/// button's label.
const LABELS: [(Language, &str, [&str; 3], &str); 2] = [
    (Language::En, "Theme", ["System", "Light", "Dark"], "Save"),
    (
        Language::Fi,
        "Teema",
        ["Järjestelmä", "Vaalea", "Tumma"],
        "Tallenna",
    ),
];

/// What a request can hand the layout as `request_theme`: nothing (a page
/// rendered outside `Ui::render`), each choice, and a value no handler
/// sends.
const THEMES: [Option<&str>; 5] = [
    None,
    Some("system"),
    Some("light"),
    Some("dark"),
    Some("bogus"),
];

/// The page backgrounds `theme-color` takes (design-system/README.md).
const LIGHT_BG: &str = "#fbf8f5";
const DARK_BG: &str = "#121212";

/// The navigation's one breakpoint (RR-0035 sub-answer 6), as each side
/// of it is written.
const NAV_NARROW: &str = "max-width: 59.9375rem";
const NAV_WIDE: &str = "min-width: 60rem";

/// The layout in `lang`, around a one-line body, with `request_theme` and
/// `request_path` in the root context as `Ui::render` puts them.
fn render(lang: Language, signed_in: bool, theme: Option<&str>) -> String {
    let mut hb =
        super::super::build_registry(&I18n::load().expect("catalogs"), lang).expect("registry");
    hb.register_template_string(
        "page",
        format!(
            r#"{{{{#> layout page_title=(t "nav.lists") page_signed_in={signed_in}}}}}<p>body</p>{{{{/layout}}}}"#
        ),
    )
    .expect("page template");
    let mut root = serde_json::json!({ "request_path": "/lists?x=1" });
    if let Some(theme) = theme {
        root["request_theme"] = serde_json::Value::String(theme.to_owned());
    }
    hb.render("page", &root)
        .unwrap_or_else(|e| panic!("render in {lang} with {theme:?}: {e}"))
}

/// Every page the tests look at: both languages, signed in and out, every
/// `request_theme`.
fn pages() -> Vec<(Language, Option<&'static str>, String)> {
    let mut out = Vec::new();
    for lang in Language::ALL {
        for signed_in in [true, false] {
            for theme in THEMES {
                out.push((lang, theme, render(lang, signed_in, theme)));
            }
        }
    }
    out
}

/// The labels for `lang`.
fn labels(lang: Language) -> (&'static str, [&'static str; 3], &'static str) {
    let (_, legend, options, save) = LABELS
        .iter()
        .find(|(l, ..)| *l == lang)
        .expect("labels for every language");
    (legend, *options, save)
}

/// The choice whose radio should be checked for `theme`.
fn expected_choice(theme: Option<&str>) -> &'static str {
    match theme {
        Some("light") => "light",
        Some("dark") => "dark",
        _ => "system",
    }
}

/// One option of the switch: its `<label>` element and its radio's tag.
struct Option_<'a> {
    label_tag: &'a str,
    label: &'a str,
    radio: &'a str,
}

/// The theme switch of a rendered page.
struct Switch<'a> {
    form_tag: &'a str,
    form: &'a str,
    options: Vec<Option_<'a>>,
}

fn switch(page: &str) -> Switch<'_> {
    let forms: Vec<&str> = tags(page, "form")
        .into_iter()
        .filter(|tag| attr(tag, "action") == Some("/theme"))
        .collect();
    assert_eq!(forms.len(), 1, "one theme switch per page: {page}");
    let form_tag = forms[0];
    let form = element(page, form_tag, "form");
    let options = tags(form, "label")
        .into_iter()
        .map(|label_tag| {
            let label = element(form, label_tag, "label");
            let radios: Vec<&str> = tags(label, "input")
                .into_iter()
                .filter(|tag| attr(tag, "type") == Some("radio"))
                .collect();
            assert_eq!(radios.len(), 1, "one radio per option: {label}");
            Option_ {
                label_tag,
                label,
                radio: radios[0],
            }
        })
        .collect();
    Switch {
        form_tag,
        form,
        options,
    }
}

/// Whether `tag` carries the bare or valued attribute `name`.
fn has_attr(tag: &str, name: &str) -> bool {
    attr(tag, name).is_some()
        || tag
            .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
            .any(|word| word == name)
}

/// The text of the `<span class="theme-label">` inside an option.
fn option_label(label: &str) -> String {
    let span = tags(label, "span")
        .into_iter()
        .find(|tag| attr(tag, "class") == Some("theme-label"))
        .unwrap_or_else(|| panic!("no .theme-label: {label}"));
    text(element(label, span, "span"))
}

/// Every `@media` or `@supports` block of `css` (comments removed), nested
/// ones too: its prelude (between the keyword and `{`) and its body.
fn at_blocks<'a>(css: &'a str, keyword: &str) -> Vec<(&'a str, &'a str)> {
    css.match_indices(keyword)
        .map(|(at, _)| {
            let open = at + css[at..].find('{').expect("block opens");
            let mut depth = 0;
            let mut close = open;
            for (i, c) in css[open..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            close = open + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            (css[at + keyword.len()..open].trim(), &css[open + 1..close])
        })
        .collect()
}

/// The rules of `css` whose selector list contains `selector`.
fn rules_for<'r, 'a>(rules: &'r [Rule<'a>], selector: &str) -> Vec<&'r Rule<'a>> {
    rules
        .iter()
        .filter(|r| r.selectors.contains(&selector))
        .collect()
}

/// The value of `property` in the one rule of `rules` with `selector`.
fn declared<'a>(rules: &[Rule<'a>], selector: &str, property: &str) -> &'a str {
    let found = rules_for(rules, selector);
    assert_eq!(found.len(), 1, "one rule for `{selector}`");
    found[0].value(property)
}

/// The phone-side navigation block where popover is supported: the
/// `@media (max-width: 59.9375rem)` inside `@supports selector(:popover-open)`
/// that lays out the panel.
fn panel_block(css: &str) -> &str {
    at_blocks(css, "@media")
        .into_iter()
        .find(|(prelude, body)| {
            *prelude == format!("({NAV_NARROW})") && body.contains(".site-nav {")
        })
        .map(|(_, body)| body)
        .expect("the navigation panel's media block")
}

/// MB-0055 P1: one theme switch per page, a form posting to `/theme` with
/// htmx on change, a fieldset and legend, three radios with icons and
/// labels from the catalogs, the request's choice checked, and a no-JS
/// submit button.
#[test]
fn switch_markup() {
    for (lang, theme, page) in pages() {
        let (legend, names, save) = labels(lang);
        let Switch {
            form_tag,
            form,
            options,
        } = switch(&page);
        let topbar = element(&page, tags(&page, "header")[0], "header");
        assert!(topbar.contains(form), "the switch is in the top bar");

        assert_eq!(attr(form_tag, "method"), Some("post"), "{form_tag}");
        assert_eq!(attr(form_tag, "hx-post"), Some("/theme"), "{form_tag}");
        assert_eq!(attr(form_tag, "hx-trigger"), Some("change"), "{form_tag}");
        assert_eq!(attr(form_tag, "hx-swap"), Some("none"), "{form_tag}");
        assert!(
            !form.contains("hx-on"),
            "no inline handler (RR-0035 sub-answer 3): {form}"
        );

        let returns: Vec<&str> = tags(form, "input")
            .into_iter()
            .filter(|tag| attr(tag, "name") == Some("return"))
            .collect();
        assert_eq!(returns.len(), 1, "{form}");
        assert_eq!(attr(returns[0], "type"), Some("hidden"));
        assert_eq!(attr(returns[0], "value"), Some("/lists?x&#x3D;1"));

        let fieldsets = tags(form, "fieldset");
        assert_eq!(fieldsets.len(), 1, "{form}");
        let legends = tags(form, "legend");
        assert_eq!(legends.len(), 1, "{form}");
        assert_eq!(text(element(form, legends[0], "legend")), legend);

        assert_eq!(options.len(), 3, "three choices: {form}");
        let checked = expected_choice(theme);
        for ((option, value), name) in options.iter().zip(CHOICES).zip(names) {
            assert_eq!(attr(option.radio, "name"), Some("theme"));
            assert_eq!(attr(option.radio, "value"), Some(value));
            let id = format!("theme-{value}");
            assert_eq!(attr(option.radio, "id"), Some(id.as_str()));
            assert_eq!(
                has_attr(option.radio, "checked"),
                value == checked,
                "{lang} {theme:?}: {}",
                option.radio
            );
            assert_eq!(option_label(option.label), name);
            assert_eq!(
                tags(option.label, "svg").len(),
                1,
                "an icon: {}",
                option.label
            );
        }

        let noscript = element(form, tags(form, "noscript")[0], "noscript");
        let buttons = tags(noscript, "button");
        assert_eq!(buttons.len(), 1, "{noscript}");
        assert_eq!(attr(buttons[0], "type"), Some("submit"));
        assert_eq!(text(noscript), save);
    }
}

/// MB-0055 P3: first paint is already right: no `data-theme` on `<html>`
/// (the checked radio decides, P4), and `theme-color` is the chosen
/// theme's page background, or the two OS-keyed ones for System.
#[test]
fn first_paint() {
    for (lang, theme, page) in pages() {
        let html = tags(&page, "html")[0];
        assert!(!html.contains("data-theme"), "{html}");

        let metas: Vec<&str> = tags(&page, "meta")
            .into_iter()
            .filter(|tag| attr(tag, "name") == Some("theme-color"))
            .collect();
        let got: Vec<(Option<&str>, Option<&str>)> = metas
            .iter()
            .map(|tag| (attr(tag, "content"), attr(tag, "media")))
            .collect();
        let expected = match expected_choice(theme) {
            "light" => vec![(Some(LIGHT_BG), None)],
            "dark" => vec![(Some(DARK_BG), None)],
            _ => vec![
                (Some(LIGHT_BG), Some("(prefers-color-scheme: light)")),
                (Some(DARK_BG), Some("(prefers-color-scheme: dark)")),
            ],
        };
        assert_eq!(got, expected, "{lang} {theme:?}");
    }
}

/// MB-0055 P4 (RR-0035 sub-answer 3): the theme is applied by CSS from the
/// checked radio, so a click repaints at once with no script: dark when
/// Dark is checked, or when the OS is dark and Light isn't; light
/// otherwise. `app.css` has no `data-theme` selector. Browsers without
/// `:has()` follow the OS (a fallback block). The token values are
/// `design_tokens`' business; here, the selectors and the ids they name.
#[test]
fn applies() {
    let css = without_comments(&app_css());
    assert!(!css.contains("data-theme"), "no `data-theme` in app.css");
    let all = rules(&css);

    // Dark chosen: one rule, outside any media query.
    let chosen = rules_for(&all, ":root:has(#theme-dark:checked)");
    assert_eq!(chosen.len(), 1, "one rule for Dark chosen");
    assert_eq!(chosen[0].selectors.len(), 1);
    assert_eq!(chosen[0].value("color-scheme"), "dark");

    // Dark from the OS unless Light is chosen: the first thing in the dark
    // media query (System and no choice both land here).
    let dark_media: Vec<&str> = at_blocks(&css, "@media")
        .into_iter()
        .filter(|(prelude, _)| *prelude == "(prefers-color-scheme: dark)")
        .map(|(_, body)| body)
        .collect();
    assert_eq!(dark_media.len(), 1, "one dark media query");
    assert!(
        dark_media[0]
            .trim_start()
            .starts_with(":root:not(:has(#theme-light:checked)) {"),
        "{}",
        dark_media[0]
    );

    // Without `:has()`: the OS decides, from a block those browsers read.
    let fallback: Vec<&str> = at_blocks(&css, "@supports")
        .into_iter()
        .filter(|(prelude, _)| *prelude == "not selector(:has(*))")
        .map(|(_, body)| body)
        .collect();
    assert_eq!(
        fallback.len(),
        1,
        "one fallback for browsers without :has()"
    );
    assert!(
        fallback[0].contains("@media (prefers-color-scheme: dark)")
            && fallback[0].contains("html {"),
        "{}",
        fallback[0]
    );

    // The ids the selectors name are the switch's radios.
    let page = render(Language::En, true, Some("light"));
    let Switch { options, .. } = switch(&page);
    let ids: Vec<&str> = options.iter().filter_map(|o| attr(o.radio, "id")).collect();
    assert_eq!(ids, ["theme-system", "theme-light", "theme-dark"]);
}

/// MB-0055 P5: the group is named by its legend, each option by its label
/// (hidden visually only, never removed), icons are decorative, the radios
/// stay real radios under the option, the focus ring shows on the option,
/// and the selected option differs by more than colour.
#[test]
fn accessible() {
    for (lang, _, page) in pages() {
        let (legend, names, _) = labels(lang);
        let Switch { form, options, .. } = switch(&page);
        assert_eq!(
            text(element(form, tags(form, "legend")[0], "legend")),
            legend
        );
        for (option, name) in options.iter().zip(names) {
            assert_eq!(
                attr(option.label_tag, "title"),
                Some(name),
                "{}",
                option.label_tag
            );
            let svg = tags(option.label, "svg")[0];
            assert_eq!(attr(svg, "aria-hidden"), Some("true"), "{svg}");
            assert_eq!(attr(svg, "focusable"), Some("false"), "{svg}");
            assert!(!option.radio.contains("hidden"), "{}", option.radio);
        }
    }

    let css = without_comments(&app_css());
    let all = rules(&css);
    for rule in all.iter().filter(|r| {
        r.selectors.iter().any(|s| {
            s.contains("theme-label")
                || s.contains("theme-switch legend")
                || s.contains(".theme-option input")
        })
    }) {
        for (property, value) in &rule.declarations {
            assert!(
                !(*property == "display" && *value == "none")
                    && !(*property == "visibility" && *value == "hidden"),
                "`{}` removes a label or radio from assistive technology",
                rule.selectors.join(", ")
            );
        }
    }
    // Wide screens: the legend and labels are hidden visually only.
    let hidden: Vec<&Rule<'_>> = all
        .iter()
        .filter(|r| r.selectors == [".theme-switch legend", ".theme-label"])
        .collect();
    assert_eq!(hidden.len(), 1, "one visually-hidden rule");
    assert_eq!(hidden[0].value("position"), "absolute");
    assert_eq!(hidden[0].value("clip-path"), "inset(50%)");

    // The radio covers its option: a large target, native keyboard.
    assert_eq!(declared(&all, ".theme-option input", "opacity"), "0");
    assert_eq!(declared(&all, ".theme-option input", "inset"), "0");
    assert!(
        declared(&all, ".theme-option:has(input:focus-visible)", "outline")
            .contains("var(--focus)")
    );
    let selected = rules_for(&all, ".theme-option:has(input:checked)");
    assert_eq!(selected.len(), 1);
    assert!(
        selected[0]
            .declarations
            .iter()
            .any(|(p, _)| *p == "box-shadow"),
        "the selected option has an edge, not only a colour"
    );
    // At least 44px per option where it's a segment.
    assert!(millirem(declared(&all, ".theme-option", "min-height")) >= 2750);
}

/// MB-0055 P6 (RR-0035 sub-answer 6): one breakpoint for the navigation,
/// 60rem: every media query with a `.site-nav` rule is written with it,
/// the menu button appears below it, and above it, where popover is
/// supported, the inline navigation doesn't wrap. Without popover support
/// it still wraps, which beats running off the side of a phone.
#[test]
fn top_bar_breakpoint() {
    let css = without_comments(&app_css());
    let media = at_blocks(&css, "@media");
    let nav_blocks: Vec<&(&str, &str)> = media
        .iter()
        .filter(|(_, body)| body.contains(".site-nav"))
        .collect();
    assert!(nav_blocks.len() >= 3, "panel, motion and wide blocks");
    for (prelude, _) in &nav_blocks {
        assert!(
            prelude.contains(NAV_NARROW) || prelude.contains(NAV_WIDE),
            "`@media {prelude}` lays out the navigation at another width"
        );
    }
    let panel = rules(panel_block(&css));
    assert_eq!(declared(&panel, ".nav-open", "display"), "inline-flex");

    let all = rules(&css);
    let defaults = rules_for(&all, ".site-nav");
    assert_eq!(defaults[0].value("flex-wrap"), "wrap", "default: wraps");

    let popover = at_blocks(&css, "@supports")
        .into_iter()
        .filter(|(prelude, _)| *prelude == "selector(:popover-open)")
        .map(|(_, body)| body)
        .find(|body| body.contains(NAV_WIDE))
        .expect("a wide block where popover is supported");
    let wide = at_blocks(popover, "@media")
        .into_iter()
        .find(|(prelude, _)| prelude.contains(NAV_WIDE))
        .map(|(_, body)| rules(body))
        .expect("the wide block");
    assert_eq!(declared(&wide, ".site-nav", "flex-wrap"), "nowrap");
}

/// MB-0055 P7 (RR-0035 sub-answer 5): one switch, inside the navigation,
/// so on phones it is in the menu panel (the 320px top bar keeps brand,
/// language and menu button only); there it is a headed group of labelled
/// rows at least 48px tall.
#[test]
fn on_phones() {
    for (_, _, page) in pages() {
        let nav = panel(&page);
        let Switch { form, .. } = switch(&page);
        assert!(nav.panel.contains(form), "the switch is in the <nav>");
    }

    let css = without_comments(&app_css());
    let panel = rules(panel_block(&css));
    let shown = rules_for(&panel, ".site-nav .theme-label");
    assert_eq!(shown.len(), 1, "labels shown in the panel");
    assert!(
        shown[0]
            .selectors
            .contains(&".site-nav .theme-switch legend")
    );
    assert_eq!(shown[0].value("position"), "static");
    assert_eq!(
        declared(&panel, ".site-nav .theme-switch fieldset", "flex-direction"),
        "column"
    );
    assert!(millirem(declared(&panel, ".site-nav .theme-option", "min-height")) >= 3000);
}
