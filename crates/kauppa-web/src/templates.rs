//! The handlebars registry, built from templates compiled into the binary
//! (MB-0024): `build.rs` lists every file in the workspace's `templates/`, so
//! the app doesn't depend on its working directory and can't run with
//! templates other than the ones it was built with.

use handlebars::Handlebars;

pub type Registry = Handlebars<'static>;

include!(concat!(env!("OUT_DIR"), "/templates.rs"));

/// The registry, in strict mode, with every embedded template under the
/// name handlebars' directory source gave it before MB-0024 (`lists/show`).
pub fn build_registry() -> anyhow::Result<Registry> {
    let mut hb = Handlebars::new();
    hb.set_strict_mode(true);
    for (name, source) in TEMPLATES {
        hb.register_template_string(name, source)?;
    }
    Ok(hb)
}

#[cfg(test)]
mod tests;
