//! MB-0048 P1 (RR-0036 sub-answer 1): `sqlx` is a dev-dependency of
//! `kauppa-web`. The web layer reaches the database only through
//! `kauppa-core`'s repo traits (`persistence.md`); with `sqlx` out of
//! `[dependencies]` the compiler enforces that for non-test code, and this
//! test keeps the line from moving back.
#![allow(clippy::expect_used, clippy::panic)]

/// The dependency names declared in `section` of a `Cargo.toml`, up to the
/// next `[section]` header.
fn names_in<'a>(manifest: &'a str, section: &str) -> Vec<&'a str> {
    let header = format!("[{section}]");
    let start = manifest
        .lines()
        .position(|line| line.trim() == header)
        .unwrap_or_else(|| panic!("no {header}"));
    manifest
        .lines()
        .skip(start + 1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('#'))
                .then(|| line.split('=').next().expect("a key").trim())
        })
        .collect()
}

#[test]
fn sqlx_is_a_dev_dependency() {
    let manifest = include_str!("../../Cargo.toml");
    assert!(
        !names_in(manifest, "dependencies").contains(&"sqlx"),
        "`sqlx` is under [dependencies] of kauppa-web"
    );
    assert!(
        names_in(manifest, "dev-dependencies").contains(&"sqlx"),
        "`sqlx` is a dev-dependency of kauppa-web, for its tests"
    );
}
