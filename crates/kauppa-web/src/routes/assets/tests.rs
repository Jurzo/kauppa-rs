//! MB-0024 P1–P3: the embedded `/assets/` service.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use actix_web::App;
use actix_web::http::{Method, StatusCode, header};
use actix_web::test::{self, TestRequest, call_service, read_body};

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
