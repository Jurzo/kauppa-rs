//! MB-0024 P1–P3: the embedded `/assets/` service. MB-0051 P5 (RR-0031):
//! the flag images. MB-0054 P5 and P9 (RR-0034): the app icons and the
//! fonts.
#![allow(clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use actix_web::App;
use actix_web::http::{Method, StatusCode, header};
use actix_web::test::{self, TestRequest, call_service, read_body};
use sha2::{Digest, Sha256};

use super::{ASSETS, Assets};
use crate::routes::test_support::{TEST_ORIGIN, test_state};

/// The content type of each file in `assets/`: the contract (RR-0011
/// constraint 4, RR-0020 constraint 2). The first rows were measured on
/// `actix-files` 0.7.4, which served the folder before MB-0024. A new file
/// needs a row.
const SERVED_AS: &[(&str, &str)] = &[
    ("app.css", "text/css; charset=utf-8"),
    ("manifest.webmanifest", "application/manifest+json"),
    // MB-0027 (RR-0015): mime_guess 2.0.5 maps `js` to `text/javascript`.
    // MB-0030 (RR-0017): htmx 2.0.11 replaces 2.0.3.
    ("vendor/htmx-2.0.11.min.js", "text/javascript"),
    // MB-0029 (RR-0018): the response-targets extension.
    ("vendor/response-targets-2.0.4.min.js", "text/javascript"),
    // MB-0051 (RR-0031 sub-answer 6): the language selector's flags. No
    // charset: `actix-files` 0.7.4 added one only to text types.
    ("flags/en.svg", "image/svg+xml"),
    ("flags/fi.svg", "image/svg+xml"),
    // MB-0054 (RR-0034 sub-answer 2): the app icons and the fonts.
    ("icons/apple-touch-icon.png", "image/png"),
    ("icons/favicon.ico", "image/x-icon"),
    ("icons/favicon.svg", "image/svg+xml"),
    ("icons/icon-192.png", "image/png"),
    ("icons/icon-512.png", "image/png"),
    ("icons/icon-maskable-512.png", "image/png"),
    ("fonts/inter.woff2", "font/woff2"),
    ("fonts/jetbrains-mono.woff2", "font/woff2"),
    ("fonts/space-grotesk.woff2", "font/woff2"),
];

/// The app icons (MB-0054 P5, RR-0034 sub-answer 2): byte for byte the
/// files of the same name in `design-system/assets/`.
/// `(path under /assets/, bytes, SHA-256 hex)`. File and hash change
/// together, with the design system's file (RR-0034 constraint 2).
const ICONS: &[(&str, usize, &str)] = &[
    (
        "icons/apple-touch-icon.png",
        3758,
        "abbe3c733c5d5b809d391c803dd9f8d0b4138736ad3fa38860a695a54fc3b4d4",
    ),
    (
        "icons/favicon.ico",
        3628,
        "05127b260746e53f31bbf8838b89e1dbaa508b7e6b99cc9bc2c723a371b0ddca",
    ),
    (
        "icons/favicon.svg",
        1022,
        "cce6532f406b885b8583766dba3b70747b2ef00a947b0f469ae438b76c3cd5c7",
    ),
    (
        "icons/icon-192.png",
        6047,
        "33669056e90fe658b9edad757135990e5907792e7ad5f6fe854990ca05c8c348",
    ),
    (
        "icons/icon-512.png",
        17530,
        "3d992c9d1b04ea70f8721a4a50b88702e6dc8ac4bed542f9db76d05a79bd7888",
    ),
    (
        "icons/icon-maskable-512.png",
        9631,
        "16476c61a533ac218f4ae4c810dc4e47ce0b4e9002636cd8143b9dcf2a6d53a3",
    ),
];

/// The one SVG that may carry a `<style>` block (RR-0034 sub-answer 3 and
/// constraint 4): the favicon, which switches colours with the OS's colour
/// scheme. It is also the top bar's brand icon.
const FAVICON: &str = "icons/favicon.svg";

/// The fonts (MB-0054 P9, RR-0034 sub-answer 1): the Latin subset of each
/// family's variable font, as `design-system/fonts/build_fonts.py` writes
/// it with fonttools 4.65.0 and brotli 1.2.1 from the TTFs in
/// `design-system/fonts/`; SIL OFL 1.1 (see `THIRD_PARTY_NOTICES.md`).
/// `(path under /assets/, bytes, SHA-256 hex, family, weights)`. File,
/// hash and the script's inputs change together (RR-0034 constraint 3).
const FONTS: &[(&str, usize, &str, &str, &str)] = &[
    (
        "fonts/inter.woff2",
        105_860,
        "e6c39f376a193728734a01931ca81e25b0599f6c94d126e9f0ae7f5bdd70bef0",
        "Inter",
        "100 900",
    ),
    (
        "fonts/jetbrains-mono.woff2",
        40852,
        "bdb88efcd0b3f3290de90d815a641d446e32df3fdc1c9b3afc93839de3606f20",
        "JetBrains Mono",
        "100 800",
    ),
    (
        "fonts/space-grotesk.woff2",
        26644,
        "acce646b157752956cbb92b860866edd479375a2f778b567fbb0e03f1ee392a0",
        "Space Grotesk",
        "300 700",
    ),
];

/// The flag images (MB-0051, RR-0031 sub-answer 5): flag-icons 7.6.0, MIT
/// (see `THIRD_PARTY_NOTICES.md`), byte for byte from the npm registry
/// tarball <https://registry.npmjs.org/flag-icons/-/flag-icons-7.6.0.tgz>,
/// whose npm `dist.integrity` is
/// `sha512-hir/PsXwr0EESyizKrXJzmkzpLIMf+1vlNKLqAfvdh+bsU94a7LYWTiubkHKqptGoCPzOUGcm6c4PfC9/BaGSQ==`:
/// `package/flags/4x3/gb.svg` as `en.svg` and `package/flags/4x3/fi.svg`.
/// `(path under /assets/, bytes, SHA-256 hex)`. Never edit a file: file,
/// hash and source version change together, under an RR (RR-0031
/// constraint 2).
const FLAGS: &[(&str, usize, &str)] = &[
    (
        "flags/en.svg",
        504,
        "c8be1e7208798a4ae692ee1e937065d498bb29e741943f6172b29118b8ed8066",
    ),
    (
        "flags/fi.svg",
        234,
        "012edd984cf1879463c2855b7df02dea2f7de7f7a66b244fb1e22b4227f7aa65",
    ),
];

/// The workspace's `assets/` folder, for comparison only.
fn source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// Every non-hidden file under `assets/`, as its path under `/assets/`.
fn files_on_disk() -> Vec<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read assets dir") {
            let path = entry.expect("dir entry").path();
            let name = path.file_name().expect("name").to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let rel = format!("{prefix}{name}");
            if path.is_dir() {
                walk(&path, &format!("{rel}/"), out);
            } else {
                out.push(rel);
            }
        }
    }
    let mut out = Vec::new();
    walk(&source_dir(), "", &mut out);
    out.sort();
    out
}

fn get(uri: &str) -> TestRequest {
    TestRequest::get().uri(uri)
}

/// MB-0024 P1/P2: every file in `assets/` is served at the same
/// `/assets/<path>`, byte for byte, with its pinned [`SERVED_AS`] content
/// type, and the embedded list holds nothing else. `cargo test` runs in
/// the crate directory, which has no `assets/`. MB-0030 P3: the replaced
/// htmx 2.0.3 URL is gone.
#[actix_web::test]
async fn serves_every_asset() {
    assert!(!Path::new("./assets").exists());
    let on_disk = files_on_disk();
    let embedded: Vec<_> = ASSETS.iter().map(|(n, ..)| (*n).to_owned()).collect();
    assert_eq!(embedded, on_disk);
    assert!(on_disk.contains(&"app.css".to_owned()));
    // No row without its file (MB-0051 P5: the flags are there).
    for (name, _) in SERVED_AS {
        assert!(on_disk.iter().any(|n| n == name), "{name}: not in assets/");
    }

    let app = test::init_service(App::new().service(Assets)).await;
    for name in &on_disk {
        let uri = format!("/assets/{name}");
        let expected_type = SERVED_AS.iter().find(|(n, _)| n == name).map_or_else(
            || panic!("{name}: add its content type to SERVED_AS"),
            |(_, t)| *t,
        );

        for method in [Method::GET, Method::HEAD] {
            let res = call_service(&app, get(&uri).method(method.clone()).to_request()).await;
            assert_eq!(res.status(), StatusCode::OK, "{method} {uri}");
            assert_eq!(
                res.headers().get(header::CONTENT_TYPE).expect("header"),
                expected_type,
                "{method} {uri}"
            );
        }

        let res = call_service(&app, get(&uri).to_request()).await;
        let body = read_body(res).await;
        let bytes = std::fs::read(source_dir().join(name)).expect("asset on disk");
        assert_eq!(body.as_ref(), bytes.as_slice(), "{uri}");
    }

    // MB-0030 P3 (RR-0017): the replaced htmx file is no longer served.
    let old_htmx = "/assets/vendor/htmx-2.0.3.min.js";
    for method in [Method::GET, Method::HEAD] {
        let res = call_service(&app, get(old_htmx).method(method.clone()).to_request()).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{method} {old_htmx}");
    }
}

/// MB-0024 P3: only GET and HEAD, no path traversal (plain or encoded),
/// 404 for unknown files, and nothing under `/assets` falls through to the
/// cross-site scope's routes.
#[actix_web::test]
async fn no_traversal() {
    let state = test_state().await;
    // As `main.rs` registers it: first, beside `routes::configure`.
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .service(Assets)
            .configure(crate::routes::configure),
    )
    .await;

    let not_found = [
        "/assets",
        "/assets/",
        "/assets/nope.css",
        "/assets/APP.CSS",
        "/assets/app.css/",
        "/assets//app.css",
        "/assets/./app.css",
        "/assets/x/../app.css",
        "/assets/../Cargo.toml",
        "/assets/../../Cargo.toml",
        "/assets/../templates/layout.hbs",
        "/assets/%2e%2e/Cargo.toml",
        "/assets/%2E%2E%2FCargo.toml",
        "/assets/..%2FCargo.toml",
        "/assets/..%5CCargo.toml",
        "/assets/%252e%252e%252fCargo.toml",
        "/assets/.gitignore",
        "/assets/app.css%00",
        "/assets/healthz",
    ];
    for uri in not_found {
        for method in [Method::GET, Method::HEAD] {
            let res = call_service(&app, get(uri).method(method.clone()).to_request()).await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND, "{method} {uri}");
        }
    }

    // actix-web's router decodes unreserved characters (RFC 3986 2.3) before
    // matching, so this is the same URL as `/assets/app.css`, not a bypass.
    let res = call_service(&app, get("/assets/app%2Ecss").to_request()).await;
    assert_eq!(res.status(), StatusCode::OK);

    // Write methods are refused before anything else, with or without the
    // cross-site proof, on known and unknown paths alike.
    for method in [
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
        Method::TRACE,
    ] {
        for uri in ["/assets/app.css", "/assets/nope", "/assets/../Cargo.toml"] {
            for origin in [TEST_ORIGIN, "http://evil.example"] {
                let req = get(uri)
                    .method(method.clone())
                    .insert_header((header::ORIGIN, origin))
                    .to_request();
                let res = call_service(&app, req).await;
                assert_eq!(
                    res.status(),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "{method} {uri}"
                );
                assert_eq!(
                    res.headers().get(header::ALLOW).expect("header"),
                    "GET, HEAD",
                    "{method} {uri}"
                );
            }
        }
    }

    // The scope beside it still serves its routes.
    let res = call_service(&app, get("/healthz").to_request()).await;
    assert_eq!(res.status(), StatusCode::OK);
}

/// MB-0046 P3 (RR-0029 sub-answer 3): files under `vendor/` (versioned
/// names) are cacheable for a year and immutable; every other file is
/// revalidated (`no-cache`) against its `ETag`, which answers 304 when it
/// matches, so a deploy's CSS reaches browsers on the next load.
#[actix_web::test]
async fn cache_headers() {
    let app = test::init_service(App::new().service(Assets)).await;
    let value = |res: &actix_web::dev::ServiceResponse, name: &header::HeaderName| {
        res.headers()
            .get(name)
            .map(|v| v.to_str().expect("ascii").to_owned())
    };
    for (name, ..) in ASSETS {
        let uri = format!("/assets/{name}");
        let res = call_service(&app, get(&uri).to_request()).await;
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        let etag = value(&res, &header::ETAG).unwrap_or_else(|| panic!("{uri}: no ETag"));
        assert!(
            etag.len() > 2 && etag.starts_with('"') && etag.ends_with('"'),
            "{uri}: {etag}"
        );
        let expected = if name.starts_with("vendor/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        assert_eq!(
            value(&res, &header::CACHE_CONTROL).as_deref(),
            Some(expected),
            "{uri}"
        );

        // A matching validator, alone, in a list, or `*`: 304, no body,
        // the same validator and cache policy (GET and HEAD alike).
        for (method, given) in [
            (Method::GET, etag.clone()),
            (Method::GET, format!("\"other\", {etag}")),
            (Method::GET, "*".to_owned()),
            (Method::HEAD, etag.clone()),
        ] {
            let res = call_service(
                &app,
                get(&uri)
                    .method(method.clone())
                    .insert_header((header::IF_NONE_MATCH, given.as_str()))
                    .to_request(),
            )
            .await;
            assert_eq!(
                res.status(),
                StatusCode::NOT_MODIFIED,
                "{method} {uri} {given}"
            );
            assert_eq!(value(&res, &header::ETAG).as_deref(), Some(etag.as_str()));
            assert_eq!(
                value(&res, &header::CACHE_CONTROL).as_deref(),
                Some(expected)
            );
            assert!(read_body(res).await.is_empty(), "{uri}: 304 has a body");
        }
        // A stale validator gets the file.
        let res = call_service(
            &app,
            get(&uri)
                .insert_header((header::IF_NONE_MATCH, "\"stale\""))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK, "{uri}: stale validator");
    }

    // Each file's validator differs from the others' (they differ in bytes).
    let mut tags: Vec<_> = Vec::new();
    for (name, ..) in ASSETS {
        let res = call_service(&app, get(&format!("/assets/{name}")).to_request()).await;
        tags.push(value(&res, &header::ETAG).expect("etag"));
    }
    let count = tags.len();
    tags.sort();
    tags.dedup();
    assert_eq!(tags.len(), count, "ETags collide: {tags:?}");
}

/// Whether the asset `name` is an SVG file, by its extension.
fn is_svg(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

/// MB-0051 P5 (RR-0031 sub-answer 7, constraint 2): the flags are exactly
/// the pinned flag-icons 7.6.0 files, and they are the only SVG images
/// besides the favicon (MB-0054, pinned in [`ICONS`]).
#[test]
fn flag_checksums() {
    let svgs: Vec<&str> = ASSETS
        .iter()
        .map(|(name, ..)| *name)
        .filter(|name| (is_svg(name) && *name != FAVICON) || name.starts_with("flags/"))
        .collect();
    let pinned: Vec<&str> = FLAGS.iter().map(|(name, ..)| *name).collect();
    assert_eq!(svgs, pinned, "every SVG and every flag is pinned in FLAGS");

    for (name, len, sha256) in FLAGS {
        assert_pinned(name, *len, sha256);
    }
}

/// The embedded file `name`: its content type and bytes.
fn embedded(name: &str) -> (&'static str, &'static [u8]) {
    ASSETS
        .iter()
        .find(|(n, ..)| *n == name)
        .map_or_else(|| panic!("{name}: not embedded"), |(_, t, _, b)| (*t, *b))
}

/// The embedded file `name` has `len` bytes and the SHA-256 `sha256`.
fn assert_pinned(name: &str, len: usize, sha256: &str) {
    let (_, bytes) = embedded(name);
    assert_eq!(bytes.len(), len, "{name}");
    let hex = Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").expect("write to String");
            hex
        });
    assert_eq!(hex, sha256, "{name}");
}

/// What makes `svg` (lowercased) more than an inert image: script, an
/// event handler, a link or outside reference, embedded content or styles
/// (RR-0031 sub-answer 7).
fn active_content(svg: &str) -> Vec<String> {
    let mut found: Vec<String> = [
        "<script",
        "href",
        "xlink:href",
        "url(",
        "<use",
        "<image",
        "<foreignobject",
        "<style",
        "style=",
    ]
    .into_iter()
    .filter(|word| svg.contains(word))
    .map(str::to_owned)
    .collect();
    // An `on…=` attribute: whitespace, `on`, letters, `=`.
    for (at, _) in svg.match_indices("on") {
        let rest = &svg[at + 2..];
        let letters = rest
            .find(|c: char| !c.is_ascii_lowercase())
            .unwrap_or(rest.len());
        let after_space = svg[..at].ends_with(|c: char| c.is_ascii_whitespace());
        if after_space && letters > 0 && rest[letters..].trim_start().starts_with('=') {
            found.push(format!("on{}=", &rest[..letters]));
        }
    }
    found
}

/// MB-0051 P5 (RR-0031 sub-answer 7, constraint 1): every SVG under
/// `assets/` but the favicon (which `brand_icons` checks, RR-0034
/// constraint 4) is an inert image, with no `<script>`, `on…` attribute,
/// `href`, `xlink:href`, `url(`, `<use>`, `<image>`, `<foreignObject>`,
/// `<style>` or `style=`. The scan finds each of them when planted.
#[test]
fn flags_are_inert_images() {
    let mut checked = 0;
    for (name, _, _, bytes) in ASSETS {
        if !is_svg(name) || *name == FAVICON {
            continue;
        }
        let svg = std::str::from_utf8(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.trim_start().starts_with("<svg "), "{name}: not an SVG");
        let found = active_content(&svg.to_ascii_lowercase());
        assert!(found.is_empty(), "{name}: {found:?}");
        checked += 1;
    }
    assert_eq!(checked, FLAGS.len(), "every flag was scanned");

    // The scan itself.
    for (planted, expected) in [
        ("<svg><script>alert(1)</script></svg>", "<script"),
        (r#"<svg onload="alert(1)"></svg>"#, "onload="),
        ("<svg\nonclick = 'x'></svg>", "onclick="),
        (r##"<svg><a href="#x"/></svg>"##, "href"),
        (r##"<svg><use xlink:href="#x"/></svg>"##, "xlink:href"),
        (r#"<svg><path fill="url(#g)"/></svg>"#, "url("),
        ("<svg><use/></svg>", "<use"),
        ("<svg><image/></svg>", "<image"),
        ("<svg><foreignObject/></svg>", "<foreignobject"),
        ("<svg><style>a{}</style></svg>", "<style"),
        (r#"<svg><path style="fill:red"/></svg>"#, "style="),
    ] {
        let found = active_content(&planted.to_ascii_lowercase());
        assert!(
            found.iter().any(|f| f == expected),
            "{planted}: {found:?} lacks {expected}"
        );
    }
    assert_eq!(
        active_content(r#"<svg id="icon" viewbox="0 0 4 3"><path d="m0 0"/></svg>"#),
        Vec::<String>::new()
    );
}

/// A file under the workspace root, for comparison only.
fn workspace_file(path: &str) -> Vec<u8> {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    std::fs::read(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()))
}

/// A text file under the workspace root.
fn workspace_text(path: &str) -> String {
    String::from_utf8(workspace_file(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The opening tags named `name` in `html`.
fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name} ");
    html.match_indices(&open)
        .map(|(start, _)| {
            let len = html[start..].find('>').expect("closed tag");
            &html[start..=start + len]
        })
        .collect()
}

/// The value of the attribute `name` in `tag`.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let open = format!(" {name}=\"");
    let start = tag.find(&open)? + open.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// MB-0054 P5 (RR-0034 sub-answers 2, 3, 6 and 7): the app icons.
///
/// 1. The six icons under `assets/icons/` are pinned and equal their files
///    in `design-system/assets/`; nothing else is in that folder.
/// 2. The layout links the ICO, the SVG and the Apple touch icon, and the
///    manifest lists the 192 px, 512 px and maskable icons. Every icon
///    either names is embedded, under `/assets/icons/`; nothing points at
///    the site root (no `/favicon.ico` route).
/// 3. `theme-color` is the page background of each theme, as `app.css`
///    serves it (`--bg`); the manifest's two colours are the light one.
/// 4. The favicon's style block only colours shapes: of what the flags'
///    scan finds, it has `<style>` and nothing else, and no `@import`.
#[test]
fn brand_icons() {
    // 1. Pinned, and the design system's bytes.
    let in_folder: Vec<&str> = ASSETS
        .iter()
        .map(|(name, ..)| *name)
        .filter(|name| name.starts_with("icons/"))
        .collect();
    let pinned: Vec<&str> = ICONS.iter().map(|(name, ..)| *name).collect();
    assert_eq!(in_folder, pinned, "every icon is pinned in ICONS");
    for (name, len, sha256) in ICONS {
        assert_pinned(name, *len, sha256);
        let file = name.strip_prefix("icons/").expect("under icons/");
        assert_eq!(
            embedded(name).1,
            workspace_file(&format!("design-system/assets/{file}")).as_slice(),
            "{name} is design-system/assets/{file}"
        );
    }
    assert_eq!(embedded("icons/favicon.ico").0, "image/x-icon");
    assert_eq!(embedded(FAVICON).0, "image/svg+xml");
    assert_eq!(embedded("icons/icon-192.png").0, "image/png");

    // 2. The layout's links.
    let layout = workspace_text("templates/layout.hbs");
    let links = tags(&layout, "link");
    let link = |rel: &str| -> Vec<&str> {
        links
            .iter()
            .copied()
            .filter(|tag| attr(tag, "rel") == Some(rel))
            .collect()
    };
    let icons = link("icon");
    assert_eq!(icons.len(), 2, "an ICO and an SVG icon: {icons:?}");
    assert_eq!(attr(icons[0], "href"), Some("/assets/icons/favicon.ico"));
    assert_eq!(attr(icons[0], "sizes"), Some("48x48"));
    assert_eq!(attr(icons[1], "href"), Some("/assets/icons/favicon.svg"));
    assert_eq!(attr(icons[1], "type"), Some("image/svg+xml"));
    let touch = link("apple-touch-icon");
    assert_eq!(touch.len(), 1, "one Apple touch icon");
    assert_eq!(
        attr(touch[0], "href"),
        Some("/assets/icons/apple-touch-icon.png")
    );
    assert!(
        !layout.contains("\"/favicon") && !layout.contains("\"/apple-touch-icon"),
        "no icon at the site root (RR-0034 sub-answer 6)"
    );

    // The manifest's icons.
    let manifest_source = workspace_text("assets/manifest.webmanifest");
    let manifest: serde_json::Value = serde_json::from_str(&manifest_source).expect("manifest");
    let listed: Vec<(&str, &str, &str, Option<&str>)> = manifest["icons"]
        .as_array()
        .expect("icons")
        .iter()
        .map(|icon| {
            (
                icon["src"].as_str().expect("src"),
                icon["sizes"].as_str().expect("sizes"),
                icon["type"].as_str().expect("type"),
                icon["purpose"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("/assets/icons/icon-192.png", "192x192", "image/png", None),
            ("/assets/icons/icon-512.png", "512x512", "image/png", None),
            (
                "/assets/icons/icon-maskable-512.png",
                "512x512",
                "image/png",
                Some("maskable")
            ),
        ]
    );

    // Every icon the layout or the manifest names is served, and every
    // served icon is named by one of them.
    let mut named: Vec<&str> = Vec::new();
    for source in [layout.as_str(), manifest_source.as_str()] {
        for (at, _) in source.match_indices("/assets/icons/") {
            let rest = &source[at + "/assets/".len()..];
            let len = rest.find('"').expect("a quoted path");
            named.push(&rest[..len]);
        }
    }
    named.sort_unstable();
    named.dedup();
    assert_eq!(named, pinned, "the icons the layout and manifest name");

    assert_site_colours(&layout, &manifest);
    assert_favicon_only_colours_shapes();
}

/// `brand_icons` 3: `theme-color` and the manifest's colours.
fn assert_site_colours(layout: &str, manifest: &serde_json::Value) {
    // 3. Colours: the site's page background, not the icon's.
    let css = workspace_text("assets/app.css");
    let backgrounds: Vec<&str> = css
        .match_indices("--bg:")
        .map(|(at, _)| {
            let rest = &css[at + "--bg:".len()..];
            rest[..rest.find(';').expect("declaration end")].trim()
        })
        .collect();
    // Light, then dark from the OS, dark chosen and the no-`:has()`
    // fallback (MB-0055, RR-0035 sub-answer 3).
    let [light, dark, chosen_dark, fallback_dark] = backgrounds[..] else {
        panic!("--bg in the four token rules: {backgrounds:?}");
    };
    assert_eq!(dark, chosen_dark);
    assert_eq!(dark, fallback_dark);
    let theme_colors: Vec<(Option<&str>, Option<&str>)> = tags(layout, "meta")
        .into_iter()
        .filter(|tag| attr(tag, "name") == Some("theme-color"))
        .map(|tag| (attr(tag, "content"), attr(tag, "media")))
        .collect();
    // The layout's source has a branch per choice (MB-0055, RR-0035
    // sub-answer 3): Light, Dark, then System's OS-keyed pair, written
    // twice (no `request_theme` is System too). Which one a page renders is
    // `templates::tests::theme::first_paint`'s; here, only site colours.
    let system = [
        (Some(light), Some("(prefers-color-scheme: light)")),
        (Some(dark), Some("(prefers-color-scheme: dark)")),
    ];
    assert_eq!(
        theme_colors,
        [[(Some(light), None), (Some(dark), None)], system, system].concat()
    );
    assert_eq!((light, dark), ("#fbf8f5", "#121212"));
    assert_eq!(manifest["theme_color"], light);
    assert_eq!(manifest["background_color"], light);
}

/// `brand_icons` 4: the favicon's content test.
fn assert_favicon_only_colours_shapes() {
    // 4. The favicon's own content test.
    let svg = std::str::from_utf8(embedded(FAVICON).1).expect("utf-8");
    assert!(svg.trim_start().starts_with("<svg "), "not an SVG");
    let lower = svg.to_ascii_lowercase();
    assert_eq!(active_content(&lower), ["<style"], "only a style block");
    assert!(
        !lower.contains("@import"),
        "the style block imports nothing"
    );
    assert_eq!(lower.matches("<style").count(), 1, "one style block");
}

/// MB-0054 P9 (RR-0034 sub-answer 1): the fonts.
///
/// 1. `assets/fonts/` holds exactly the three pinned WOFF2 files, served
///    as `font/woff2`.
/// 2. `app.css` declares each with one `@font-face` rule: its family, its
///    same-origin file, the weights the variable font holds and
///    `font-display: swap`. No other rule of `app.css` has a `url(`.
/// 3. Nothing preloads a font, and the layout names no other host.
#[test]
fn brand_fonts() {
    let in_folder: Vec<&str> = ASSETS
        .iter()
        .map(|(name, ..)| *name)
        .filter(|name| name.starts_with("fonts/"))
        .collect();
    let pinned: Vec<&str> = FONTS.iter().map(|(name, ..)| *name).collect();
    assert_eq!(in_folder, pinned, "every font is pinned in FONTS");

    let css = workspace_text("assets/app.css");
    let faces: Vec<&str> = css
        .match_indices("@font-face {")
        .map(|(at, _)| {
            let rest = &css[at..];
            &rest[..rest.find('}').expect("closed rule")]
        })
        .collect();
    assert_eq!(faces.len(), FONTS.len(), "one @font-face per font");

    for (name, len, sha256, family, weights) in FONTS {
        assert_pinned(name, *len, sha256);
        let (content_type, bytes) = embedded(name);
        assert_eq!(content_type, "font/woff2", "{name}");
        assert!(bytes.starts_with(b"wOF2"), "{name} is WOFF2");

        let src = format!("src: url(\"/assets/{name}\") format(\"woff2\");");
        let face: Vec<&&str> = faces.iter().filter(|face| face.contains(&src)).collect();
        assert_eq!(face.len(), 1, "one @font-face with `{src}`");
        let face = face[0];
        for declaration in [
            format!("font-family: \"{family}\";"),
            format!("font-weight: {weights};"),
            "font-style: normal;".to_owned(),
            "font-display: swap;".to_owned(),
        ] {
            assert!(face.contains(&declaration), "{name}: {declaration}");
        }
        // The family is one the token stacks name.
        assert!(
            css.contains(&format!("\"{family}\", ")),
            "{family} leads a font stack"
        );
    }
    assert_eq!(
        css.matches("url(").count(),
        FONTS.len(),
        "the only url()s of app.css are the fonts'"
    );

    let layout = workspace_text("templates/layout.hbs");
    assert!(!layout.contains("preload"), "no font is preloaded");
    for host in ["http://", "https://", "fonts.googleapis", "fonts.gstatic"] {
        assert!(!layout.contains(host), "the layout names {host}");
    }
}
