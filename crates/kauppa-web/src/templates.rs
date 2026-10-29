use handlebars::Handlebars;

pub type Registry = Handlebars<'static>;

pub fn build_registry() -> anyhow::Result<Registry> {
    build_registry_from("./templates")
}

/// Build the registry from `dir`. Tests pass an absolute path: `cargo test`
/// runs with the crate directory as CWD, where `./templates` doesn't resolve.
pub fn build_registry_from(dir: &str) -> anyhow::Result<Registry> {
    let mut hb = Handlebars::new();
    hb.set_strict_mode(true);
    hb.register_templates_directory(dir, handlebars::DirectorySourceOptions::default())?;
    Ok(hb)
}
