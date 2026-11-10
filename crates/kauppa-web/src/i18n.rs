//! Translations (MB-0035, RR-0023): the catalogs `locales/<tag>.json`,
//! embedded at build time, and the `{{t}}` / `{{html_lang}}` handlebars
//! helpers bound to one catalog per registry.
//!
//! A catalog is a flat JSON object: dotted keys (`nav.lists`) to either a
//! string or a plural `{"one": "…", "other": "…"}`, chosen by the `count`
//! argument (`count == 1` is `one`; exact for `en` and `fi`). `{name}` in a
//! value is replaced by the argument `name`. Catalog text is trusted and
//! contains no `<` or `&` (`tests::catalogs_complete`); arguments are user
//! data and are HTML-escaped when a template renders them (RR-0023
//! constraint 2). A missing key, argument or plural form is an error, never
//! fallback text (constraint 3).

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};
use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
    RenderErrorReason, ScopedJson,
};
use serde_json::Value;

use kauppa_core::domain::Language;

pub mod messages;
mod request;

#[cfg(test)]
use request::from_accept_language;
pub use request::{COOKIE, Ui, VARY, language_cookie};

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// One catalog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Text(String),
    Plural { one: String, other: String },
}

#[cfg(test)]
impl Entry {
    /// Every text form the entry has.
    pub fn forms(&self) -> Vec<&str> {
        match self {
            Self::Text(text) => vec![text],
            Self::Plural { one, other } => vec![one, other],
        }
    }
}

/// Why a lookup failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nError {
    Key(String),
    Argument { key: String, name: String },
    Count(String),
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(key) => write!(f, "no translation for `{key}`"),
            Self::Argument { key, name } => {
                write!(f, "`{key}` needs the argument `{name}`")
            }
            Self::Count(key) => write!(f, "`{key}` is a plural and needs `count`"),
        }
    }
}

impl std::error::Error for I18nError {}

/// One language's catalog.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    entries: BTreeMap<String, Entry>,
}

impl Catalog {
    /// Parses a catalog's JSON source.
    pub fn parse(source: &str) -> anyhow::Result<Self> {
        let Value::Object(map) = serde_json::from_str(source)? else {
            anyhow::bail!("a catalog is a JSON object");
        };
        let mut entries = BTreeMap::new();
        for (key, value) in map {
            let entry = match value {
                Value::String(text) => Entry::Text(text),
                Value::Object(forms) => {
                    let form = |name: &str| match forms.get(name) {
                        Some(Value::String(text)) => Ok(text.clone()),
                        _ => Err(anyhow::anyhow!("`{key}`: plural needs a string `{name}`")),
                    };
                    if forms.len() != 2 {
                        anyhow::bail!("`{key}`: a plural has exactly `one` and `other`");
                    }
                    Entry::Plural {
                        one: form("one")?,
                        other: form("other")?,
                    }
                }
                _ => anyhow::bail!("`{key}`: a value is a string or a plural object"),
            };
            entries.insert(key, entry);
        }
        Ok(Self { entries })
    }

    /// Every entry, by key.
    #[cfg(test)]
    pub const fn entries(&self) -> &BTreeMap<String, Entry> {
        &self.entries
    }

    /// The text for `key` with `{name}` placeholders replaced by `args`,
    /// each passed through `escape`. A plural picks its form by the `count`
    /// argument.
    pub fn format(
        &self,
        key: &str,
        args: &[(&str, String)],
        escape: impl Fn(&str) -> String,
    ) -> Result<String, I18nError> {
        let entry = self
            .entries
            .get(key)
            .ok_or_else(|| I18nError::Key(key.to_owned()))?;
        let template = match entry {
            Entry::Text(text) => text,
            Entry::Plural { one, other } => {
                let count = args
                    .iter()
                    .find(|(name, _)| *name == "count")
                    .ok_or_else(|| I18nError::Count(key.to_owned()))?;
                if count.1 == "1" { one } else { other }
            }
        };
        let mut out = String::with_capacity(template.len());
        let mut rest = template.as_str();
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            if let Some(name) = placeholder(after) {
                let value =
                    args.iter()
                        .find(|(n, _)| *n == name)
                        .ok_or_else(|| I18nError::Argument {
                            key: key.to_owned(),
                            name: name.to_owned(),
                        })?;
                out.push_str(&escape(&value.1));
                rest = &after[name.len() + 1..];
            } else {
                out.push('{');
                rest = after;
            }
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// The placeholder name at the start of `text` (after a `{`), if `text`
/// starts with `name}` where `name` is lower-case letters, digits and `_`.
pub fn placeholder(text: &str) -> Option<&str> {
    let end = text.find('}')?;
    let name = &text[..end];
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    valid.then_some(name)
}

/// Every placeholder name in `text`, sorted, without duplicates.
#[cfg(test)]
pub fn placeholders(text: &str) -> Vec<&str> {
    let mut names: Vec<_> = text
        .match_indices('{')
        .filter_map(|(at, _)| placeholder(&text[at + 1..]))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Every language's catalog.
#[derive(Debug, Clone)]
pub struct I18n {
    /// Indexed by [`Language::index`].
    catalogs: [Catalog; Language::COUNT],
}

impl I18n {
    /// The embedded catalogs: one per supported language, and nothing else.
    pub fn load() -> anyhow::Result<Self> {
        Self::from_sources(LOCALES)
    }

    /// Catalogs from `(tag, JSON)` pairs.
    pub fn from_sources(sources: &[(&str, &str)]) -> anyhow::Result<Self> {
        let mut catalogs: [Option<Catalog>; Language::COUNT] = Default::default();
        for (tag, source) in sources {
            let lang = Language::parse(tag)
                .ok_or_else(|| anyhow::anyhow!("locales/{tag}.json: unsupported language"))?;
            let catalog =
                Catalog::parse(source).map_err(|e| anyhow::anyhow!("locales/{tag}.json: {e}"))?;
            catalogs[lang.index()] = Some(catalog);
        }
        let [en, fi] = catalogs;
        let missing = |lang: Language| anyhow::anyhow!("locales/{lang}.json is missing");
        Ok(Self {
            catalogs: [
                en.ok_or_else(|| missing(Language::En))?,
                fi.ok_or_else(|| missing(Language::Fi))?,
            ],
        })
    }

    /// `lang`'s catalog.
    pub const fn catalog(&self, lang: Language) -> &Catalog {
        &self.catalogs[lang.index()]
    }

    /// Plain text for Rust-built messages: placeholders are replaced
    /// unescaped (the caller renders or escapes the result).
    pub fn text(
        &self,
        lang: Language,
        key: &str,
        args: &[(&str, String)],
    ) -> Result<String, I18nError> {
        self.catalog(lang).format(key, args, str::to_owned)
    }

    /// `at` in `lang`'s date format (`format.date`), e.g. `9.11.2026`.
    pub fn date(&self, lang: Language, at: DateTime<Utc>) -> Result<String, I18nError> {
        Ok(at.format(&self.text(lang, "format.date", &[])?).to_string())
    }

    /// `at` in `lang`'s date-and-time format (`format.datetime`), in UTC.
    pub fn datetime(&self, lang: Language, at: DateTime<Utc>) -> Result<String, I18nError> {
        Ok(at
            .format(&self.text(lang, "format.datetime", &[])?)
            .to_string())
    }
}

/// `{{t "key" name=value …}}` over one catalog. As a subexpression
/// (`page_title=(t "…")`) it returns plain text, which the outer `{{…}}`
/// escapes as it would any value.
pub struct Translate {
    catalog: Catalog,
}

impl Translate {
    pub const fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    fn lookup(
        &self,
        h: &Helper<'_>,
        escape: impl Fn(&str) -> String,
    ) -> Result<String, RenderError> {
        let key = h.param(0).and_then(|p| p.value().as_str()).ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName("t", "0".into(), "string".into())
        })?;
        if h.params().len() != 1 {
            return Err(RenderErrorReason::Other(format!("`t \"{key}\"` takes one key")).into());
        }
        let mut args = Vec::new();
        for (name, value) in h.hash() {
            let text = match value.value() {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                other => {
                    return Err(RenderErrorReason::Other(format!(
                        "`t \"{key}\"`: argument `{name}` is {other}, not text or a number"
                    ))
                    .into());
                }
            };
            args.push((*name, text));
        }
        self.catalog
            .format(key, &args, escape)
            .map_err(|e| RenderErrorReason::Other(e.to_string()).into())
    }
}

impl HelperDef for Translate {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let text = self.lookup(h, str::to_owned)?;
        Ok(ScopedJson::Derived(Value::String(text)))
    }

    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        // Catalog text is trusted and markup-free; only arguments are
        // escaped (RR-0023 constraint 2).
        let text = self.lookup(h, handlebars::html_escape)?;
        out.write(&text)?;
        Ok(())
    }
}

/// `{{html_lang}}`: the registry's language tag, for `<html lang>`.
pub struct HtmlLang(pub Language);

impl HelperDef for HtmlLang {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        _: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        Ok(ScopedJson::Derived(Value::String(self.0.tag().to_owned())))
    }
}

#[cfg(test)]
mod tests;
