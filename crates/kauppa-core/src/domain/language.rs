//! The languages the app is shown in (MB-0035, RR-0023).
//!
//! The tag is the BCP 47 primary language subtag: it names the catalog file
//! (`locales/<tag>.json`), the `<html lang>` value, the `kauppa_lang` cookie
//! value and what `users.language` stores. Adding a language adds a variant
//! here and a catalog file.

use std::fmt;

/// A supported language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    /// English, the default.
    En,
    /// Finnish.
    Fi,
}

impl Language {
    /// Every supported language, the default first.
    pub const ALL: [Self; 2] = [Self::En, Self::Fi];

    /// The language used when nothing else chooses one.
    pub const DEFAULT: Self = Self::En;

    /// The language whose tag is exactly `tag` (`"en"`, `"fi"`); anything
    /// else, including other case or whitespace, is `None`.
    #[must_use]
    pub fn parse(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.tag() == tag)
    }

    /// The number of supported languages.
    pub const COUNT: usize = Self::ALL.len();

    /// This language's position in [`Self::ALL`], for per-language arrays.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The BCP 47 tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fi => "fi",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn parses_only_exact_tags() {
        for lang in Language::ALL {
            assert_eq!(Language::parse(lang.tag()), Some(lang));
            assert_eq!(lang.to_string(), lang.tag());
        }
        for bad in ["", "EN", "Fi", " en", "en-US", "fi_FI", "sv", "en\0"] {
            assert_eq!(Language::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Language::ALL[0], Language::DEFAULT);
        for (i, lang) in Language::ALL.into_iter().enumerate() {
            assert_eq!(lang.index(), i);
        }
    }
}
