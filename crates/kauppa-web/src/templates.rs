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

/// MB-0016 P5 (RR-0006): inside the layout, a body field named like a layout
/// parameter renders its own value, at the top level and in a nested block,
/// and the layout still gets its own parameters. Handlebars turns a partial's
/// hash params into block params, which would shadow such fields.
#[cfg(test)]
#[test]
#[allow(clippy::expect_used)]
fn layout_params_do_not_shadow_body() {
    let mut hb = build_registry_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates"))
        .expect("load templates");
    hb.register_template_string(
        "guard",
        r#"{{#> layout page_title="Page" page_signed_in=true}}
<b>{{title}}|{{signed_in}}</b>
{{#each items}}<i>{{this.title}}|{{this.signed_in}}</i>{{/each}}
{{/layout}}"#,
    )
    .expect("guard template");

    let body = hb
        .render(
            "guard",
            &serde_json::json!({
                "title": "Body",
                "signed_in": false,
                "items": [{ "title": "Item", "signed_in": true }],
            }),
        )
        .expect("render");
    assert!(body.contains("<title>Page — kauppa</title>"), "{body}");
    assert!(body.contains("Sign out"), "the layout's own signed_in");
    assert!(body.contains("<b>Body|false</b>"), "{body}");
    assert!(body.contains("<i>Item|true</i>"), "{body}");
}
