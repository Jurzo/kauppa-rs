//! Compiles the workspace's `templates/`, `locales/` and `assets/` into the binary
//! (MB-0024, MB-0035): writes `include_str!`/`include_bytes!` lists to
//! `OUT_DIR`, which `templates.rs`, `i18n.rs` and `assets.rs` include. Cargo
//! re-runs this when any of the folders changes.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// Content types for `assets/`, by extension: the contract, pinned to what
/// `actix-files` 0.7 served before MB-0024 (`mime_guess`, plus
/// `; charset=utf-8` on `text/css`, `text/plain` and `text/html`). The build
/// fails on a file whose
/// extension isn't here. `svg` is MB-0051's (RR-0031 sub-answer 6): the
/// flag images under `assets/flags/`.
const CONTENT_TYPES: &[(&str, &str)] = &[
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript"),
    ("svg", "image/svg+xml"),
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
    code.push_str("pub const TEMPLATES: &[(&str, &str)] = &[\n");
    for (name, path) in files(&templates)? {
        // As handlebars' directory source named them before MB-0024: `.hbs`
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

    // MB-0035 (RR-0023): the translation catalogs, `locales/<tag>.json`.
    let locales = root.join("locales");
    println!("cargo::rerun-if-changed={}", locales.display());
    let mut code = String::from("/// `(language tag, JSON source)` for every catalog.\n");
    code.push_str("const LOCALES: &[(&str, &str)] = &[\n");
    for (name, path) in files(&locales)? {
        let Some(tag) = name.strip_suffix(".json") else {
            continue;
        };
        if tag.contains('/') || tag.starts_with('.') {
            continue;
        }
        let _ = writeln!(code, "    ({tag:?}, include_str!({:?})),", path_str(&path)?);
    }
    code.push_str("];\n");
    std::fs::write(out.join("locales.rs"), code)?;

    let assets = root.join("assets");
    println!("cargo::rerun-if-changed={}", assets.display());
    let mut code =
        String::from("/// `(path under /assets/, content type, ETag, bytes)` for every asset.\n");
    code.push_str("const ASSETS: &[(&str, &str, &str, &[u8])] = &[\n");
    for (name, path) in files(&assets)? {
        // Hidden files are never served (as before MB-0024).
        if name.split('/').any(|seg| seg.starts_with('.')) {
            continue;
        }
        let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
        let Some((_, content_type)) = CONTENT_TYPES.iter().find(|(e, _)| *e == ext) else {
            return Err(io::Error::other(format!(
                "assets/{name}: no content type for `.{ext}`; add it to build.rs's CONTENT_TYPES"
            )));
        };
        let etag = etag(&std::fs::read(&path)?);
        let _ = writeln!(
            code,
            "    ({name:?}, {content_type:?}, {etag:?}, include_bytes!({:?})),",
            path_str(&path)?
        );
    }
    code.push_str("];\n");
    std::fs::write(out.join("assets.rs"), code)
}

/// A strong `ETag` for `bytes` (MB-0046, RR-0029): a quoted 64-bit FNV-1a
/// hash, stable across builds and toolchains. It only has to change when
/// the bytes do; it isn't a security check.
fn etag(bytes: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let hash = bytes
        .iter()
        .fold(OFFSET, |h, b| (h ^ u64::from(*b)).wrapping_mul(PRIME));
    format!("\"{hash:016x}-{}\"", bytes.len())
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
