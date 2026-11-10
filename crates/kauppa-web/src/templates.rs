//! The handlebars registries, built from templates compiled into the binary
//! (MB-0024): `build.rs` lists every file in the workspace's `templates/`, so
//! the app doesn't depend on its working directory and can't run with
//! templates other than the ones it was built with.
//!
//! One registry per language (MB-0035, RR-0023): each has the same
//! templates and its own `{{t}}` and `{{html_lang}}` helpers, so views carry
//! no language field.

use handlebars::Handlebars;

use kauppa_core::domain::Language;

use crate::i18n::{HtmlLang, I18n, Translate};

pub type Registry = Handlebars<'static>;

include!(concat!(env!("OUT_DIR"), "/templates.rs"));

/// A registry in strict mode with every embedded template under the name
/// handlebars' directory source gave it before MB-0024 (`lists/show`), and
/// the helpers for `lang`.
pub fn build_registry(i18n: &I18n, lang: Language) -> anyhow::Result<Registry> {
    let mut hb = Handlebars::new();
    hb.set_strict_mode(true);
    hb.register_helper("t", Box::new(Translate::new(i18n.catalog(lang).clone())));
    hb.register_helper("html_lang", Box::new(HtmlLang(lang)));
    for (name, source) in TEMPLATES {
        hb.register_template_string(name, source)?;
    }
    Ok(hb)
}

/// Every language's registry, and the catalogs for Rust-built text.
pub struct Templates {
    /// Indexed by [`Language::index`].
    registries: [Registry; Language::COUNT],
    i18n: I18n,
}

impl Templates {
    /// Registries for every supported language over `i18n`.
    pub fn build(i18n: I18n) -> anyhow::Result<Self> {
        let [en, fi] = Language::ALL;
        let registries = [build_registry(&i18n, en)?, build_registry(&i18n, fi)?];
        Ok(Self { registries, i18n })
    }

    /// Registries over the embedded catalogs.
    pub fn load() -> anyhow::Result<Self> {
        Self::build(I18n::load()?)
    }

    /// `lang`'s registry.
    pub const fn get(&self, lang: Language) -> &Registry {
        &self.registries[lang.index()]
    }

    /// The catalogs.
    pub const fn i18n(&self) -> &I18n {
        &self.i18n
    }
}

impl std::fmt::Debug for Templates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Templates").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
