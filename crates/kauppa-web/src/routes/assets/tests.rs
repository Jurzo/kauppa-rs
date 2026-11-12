//! MB-0024 P1–P3: the embedded `/assets/` service. MB-0051 P5 (RR-0031):
//! the flag images.
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
/// the pinned flag-icons 7.6.0 files, and they are the only SVG images.
#[test]
fn flag_checksums() {
    let svgs: Vec<&str> = ASSETS
        .iter()
        .map(|(name, ..)| *name)
        .filter(|name| is_svg(name) || name.starts_with("flags/"))
        .collect();
    let pinned: Vec<&str> = FLAGS.iter().map(|(name, ..)| *name).collect();
    assert_eq!(svgs, pinned, "every SVG and every flag is pinned in FLAGS");

    for (name, len, sha256) in FLAGS {
        let (.., bytes) = ASSETS
            .iter()
            .find(|(n, ..)| n == name)
            .unwrap_or_else(|| panic!("{name}: not embedded"));
        assert_eq!(bytes.len(), *len, "{name}");
        let hex = Sha256::digest(bytes)
            .iter()
            .fold(String::new(), |mut hex, b| {
                write!(hex, "{b:02x}").expect("write to String");
                hex
            });
        assert_eq!(hex, *sha256, "{name}");
    }
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
/// `assets/` is an inert image, with no `<script>`, `on…` attribute,
/// `href`, `xlink:href`, `url(`, `<use>`, `<image>`, `<foreignObject>`,
/// `<style>` or `style=`. The scan finds each of them when planted.
#[test]
fn flags_are_inert_images() {
    let mut checked = 0;
    for (name, _, _, bytes) in ASSETS {
        if !is_svg(name) {
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
