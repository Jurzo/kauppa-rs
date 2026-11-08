//! MB-0024 P1/P2 (templates) and P4; MB-0016 P5.
#![allow(clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::Path;

use super::{Registry, TEMPLATES, build_registry};

/// The workspace's `templates/` folder, for comparison only.
fn source_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates"))
}

fn names(hb: &Registry) -> BTreeSet<String> {
    hb.get_templates().keys().cloned().collect()
}

/// MB-0024 P1/P2: the registry comes from the binary, not the working
/// directory, and holds exactly what the `templates/` folder holds, under
/// the names directory loading gave them, so no `render` call changes.
#[test]
fn registry_is_embedded() {
    // `cargo test` runs in the crate directory, which has no `templates/`:
    // the old `build_registry()` failed here, the embedded one doesn't.
    assert!(!Path::new("./templates").exists());
    let embedded = build_registry().expect("embedded registry");
    assert!(embedded.strict_mode());

    let mut from_dir = Registry::new();
    from_dir
        .register_templates_directory(source_dir(), handlebars::DirectorySourceOptions::default())
        .expect("directory registry");
    assert_eq!(names(&embedded), names(&from_dir));
    assert!(names(&embedded).contains("layout"));
    assert!(names(&embedded).contains("lists/_list"));

    // Byte for byte what's on disk: the build re-runs when a template changes.
    for (name, source) in TEMPLATES {
        let on_disk = std::fs::read_to_string(source_dir().join(format!("{name}.hbs")))
            .expect("template on disk");
        assert_eq!(*source, on_disk, "{name}");
    }
}

/// MB-0016 P5 (RR-0006), MB-0024 P4: inside the layout, a body field named
/// like a layout parameter renders its own value, at the top level and in a
/// nested block, and the layout still gets its own parameters. Handlebars
/// turns a partial's hash params into block params, which would shadow such
/// fields. Runs on the embedded registry, which stays in strict mode.
#[test]
fn layout_params_do_not_shadow_body() {
    let mut hb = build_registry().expect("load templates");
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

    // Strict mode: a missing field is an error, not an empty string.
    hb.register_template_string("missing", "{{nope}}")
        .expect("missing template");
    assert!(hb.render("missing", &serde_json::json!({})).is_err());
}
