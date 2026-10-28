use handlebars::Handlebars;

pub type Registry = Handlebars<'static>;

pub fn build_registry() -> anyhow::Result<Registry> {
    let mut hb = Handlebars::new();
    hb.set_strict_mode(true);
    hb.register_templates_directory("./templates", handlebars::DirectorySourceOptions::default())?;
    Ok(hb)
}
