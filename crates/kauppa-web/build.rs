//! Compiles the workspace's `templates/` and `assets/` into the binary
//! (MB-0024): writes `include_str!`/`include_bytes!` lists to `OUT_DIR`, which
//! `templates.rs` and `assets.rs` include. Cargo re-runs this when either
//! folder changes.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// Content types for `assets/`, by extension, as `actix-files` 0.7 served
/// them (`mime_guess`, plus `; charset=utf-8` on `text/css`, `text/plain` and
/// `text/html`). The build fails on a file whose extension isn't here.
const CONTENT_TYPES: &[(&str, &str)] = &[
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript"),
    ("webmanifest", "application/manifest+json"),
];

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(env("CARGO_MANIFEST_DIR")?);
    let root = manifest.join("../..");
    let out = PathBuf::from(env("OUT_DIR")?);
    println!("cargo::rerun-if-changed=build.rs");

    let templates = root.join("templates");
    println!("cargo::rerun-if-changed={}", templates.display());
    let mut code =
        String::from("/// `(name, source)` for every template, as handlebars' directory\n");
    code.push_str("/// source names it: relative path, `/`-separated, without `.hbs`.\n");
    code.push_str("const TEMPLATES: &[(&str, &str)] = &[\n");
    for (name, path) in files(&templates)? {
        // As `register_templates_directory` with default options: `.hbs`
        // files, minus hidden (`.x`) and temporary (`#x`) ones.
        let Some(stem) = name.strip_suffix(".hbs") else {
            continue;
        };
        let file = stem.rsplit('/').next().unwrap_or(stem);
        if file.starts_with('.') || file.starts_with('#') {
            continue;
        }
        let _ = writeln!(
            code,
            "    ({stem:?}, include_str!({:?})),",
            path_str(&path)?
        );
    }
    code.push_str("];\n");
    std::fs::write(out.join("templates.rs"), code)?;

    let assets = root.join("assets");
    println!("cargo::rerun-if-changed={}", assets.display());
    let mut code =
        String::from("/// `(path under /assets/, content type, bytes)` for every asset.\n");
    code.push_str("const ASSETS: &[(&str, &str, &[u8])] = &[\n");
    for (name, path) in files(&assets)? {
        // `actix-files` refuses hidden files by default; so does the build.
        if name.split('/').any(|seg| seg.starts_with('.')) {
            continue;
        }
        let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
        let Some((_, content_type)) = CONTENT_TYPES.iter().find(|(e, _)| *e == ext) else {
            return Err(io::Error::other(format!(
                "assets/{name}: no content type for `.{ext}`; add it to build.rs's CONTENT_TYPES"
            )));
        };
        let _ = writeln!(
            code,
            "    ({name:?}, {content_type:?}, include_bytes!({:?})),",
            path_str(&path)?
        );
    }
    code.push_str("];\n");
    std::fs::write(out.join("assets.rs"), code)
}

fn env(key: &str) -> io::Result<String> {
    std::env::var(key).map_err(|e| io::Error::other(format!("{key}: {e}")))
}

fn path_str(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| io::Error::other(format!("{}: not UTF-8", path.display())))
}

/// Every file under `dir`, as `(path relative to dir with '/', full path)`,
/// sorted so the generated lists are stable.
fn files(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| io::Error::other(format!("{}: not UTF-8", path.display())))?;
            let rel = format!("{prefix}{name}");
            if path.is_dir() {
                walk(&path, &format!("{rel}/"), out)?;
            } else {
                out.push((rel, path));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, "", &mut out)?;
    out.sort();
    Ok(out)
}
