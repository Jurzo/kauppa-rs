//! P1's scan: literal prose in a template (MB-0035, RR-0023 constraint 1).

/// Visible text a template may hold as is: the brand, and the page title's
/// brand suffix.
const ALLOWED: &[&str] = &["kauppa", "— kauppa"];

/// Attributes whose value a person reads (or a screen reader speaks).
const READ_ATTRS: &[&str] = &[
    "placeholder",
    "aria-label",
    "title",
    "alt",
    "label",
    "hx-confirm",
];

/// `source` with handlebars comments removed and every other `{{…}}` (or
/// `{{{…}}}`) replaced by `\u{1}`.
pub fn mask(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let at = &rest[open..];
        let close_pat = if at.starts_with("{{!--") {
            "--}}"
        } else {
            "}}"
        };
        let mut close = at.find(close_pat).expect("closed mustache") + close_pat.len();
        if at.starts_with("{{{") && at[close..].starts_with('}') {
            close += 1;
        }
        if !at.starts_with("{{!") {
            out.push('\u{1}');
        }
        rest = &at[close..];
    }
    out.push_str(rest);
    out
}

/// The text a reader sees in `masked` (markers dropped, whitespace
/// collapsed), if it has letters and isn't allowed as is.
fn prose(masked: &str) -> Option<String> {
    let text = masked
        .replace('\u{1}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (text.chars().any(char::is_alphabetic) && !ALLOWED.contains(&text.as_str())).then_some(text)
}

/// Every piece of literal prose in a template's `source`: text between
/// tags, and the values of [`READ_ATTRS`].
pub fn literal_text(source: &str) -> Vec<String> {
    let masked = mask(source);
    let mut found = Vec::new();
    let mut rest = masked.as_str();
    loop {
        let Some(lt) = rest.find('<') else {
            found.extend(prose(rest));
            break;
        };
        found.extend(prose(&rest[..lt]));
        let gt = rest[lt..].find('>').expect("closed tag");
        let tag = &rest[lt..=lt + gt];
        for attr in READ_ATTRS {
            let key = format!(" {attr}=\"");
            for (at, _) in tag.match_indices(&key) {
                let value = &tag[at + key.len()..];
                let value = &value[..value.find('"').expect("closing quote")];
                found.extend(prose(value).map(|t| format!("{attr}=\"{t}\"")));
            }
        }
        rest = &rest[lt + gt + 1..];
    }
    found
}

/// The text a page shows, plus [`READ_ATTRS`] values, with the entities
/// handlebars writes decoded: what a reader of the rendered page gets.
pub fn rendered_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        out.push('\n');
        let Some(gt) = rest[lt..].find('>') else {
            break;
        };
        let tag = &rest[lt..=lt + gt];
        for attr in READ_ATTRS {
            let key = format!(" {attr}=\"");
            if let Some(at) = tag.find(&key) {
                let value = &tag[at + key.len()..];
                out.push_str(&value[..value.find('"').unwrap_or(value.len())]);
                out.push('\n');
            }
        }
        rest = &rest[lt + gt + 1..];
    }
    out.push_str(rest);
    out.replace("&#x27;", "'")
        .replace("&quot;", "\"")
        .replace("&#x3D;", "=")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Whether `needle` occurs in `text` as whole words (not inside a longer
/// word).
pub fn contains_words(text: &str, needle: &str) -> bool {
    text.match_indices(needle).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + needle.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}
