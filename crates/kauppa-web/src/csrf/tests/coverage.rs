//! MB-0018 P4: the route-coverage guard. A route accepting a non-safe method
//! can only be registered behind the cross-site check:
//!
//! 1. handlers are registered only by attribute macros (`#[post("…")]`), not
//!    by builder calls the scan can't see (`web::resource`, `.route`, …);
//! 2. `main.rs` registers nothing besides `routes::assets::Assets` and
//!    `routes::configure`;
//! 3. `routes::configure` registers one scope, wrapped in `csrf::guard`,
//!    around `routes::protected`;
//! 4. every handler attribute in the source is registered in
//!    `routes::protected`, and every registration there is a handler found;
//! 5. behind the real wiring, each write route answers a forged request
//!    with 403 and a same-origin one with something else, so the route
//!    exists and the 403 is the check's;
//! 6. behind the real wiring, every non-safe method on `/assets/…` answers
//!    405 with `Allow: GET, HEAD`, so the static files accept no write
//!    (RR-0011).

use actix_web::App;
use actix_web::http::{Method, StatusCode, header};
use actix_web::test::{TestRequest, call_service, init_service};

use super::{handler_attrs, protected_services, source_files, write_routes};
use crate::routes::assets::Assets;
use crate::routes::test_support::{TEST_HOST, TEST_ORIGIN, bare_test_app, test_state};

/// Route registrations other than handler attribute macros.
const BUILDER_CALLS: &[&str] = &[
    "web::resource(",
    "web::route(",
    "web::to(",
    "web::service(",
    "web::method(",
    "web::post(",
    "web::put(",
    "web::patch(",
    "web::delete(",
    "Resource::new(",
    "Route::new(",
    ".route(",
    ".default_service(",
    ".external_resource(",
];

const NIL: &str = "00000000-0000-0000-0000-000000000000";

/// The code of `text`, without `//` comment lines.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
}

/// The text of `fn <name>(` up to its closing brace at column 0.
fn fn_body<'a>(text: &'a str, name: &str) -> &'a str {
    let start = text.find(&format!("fn {name}(")).expect("fn");
    let body = &text[start..];
    &body[..body.find("\n}\n").expect("end of fn")]
}

#[actix_web::test]
async fn every_write_route_is_covered() {
    let sources = source_files();

    // 1. No builder-style registrations anywhere.
    for (file, text) in &sources {
        for (i, line) in code_lines(text) {
            for call in BUILDER_CALLS {
                assert!(
                    !line.contains(call),
                    "{file}:{}: `{call}` registers a route the coverage guard can't see; \
                     use a handler attribute inside routes::protected",
                    i + 1
                );
            }
        }
    }

    // 2. main.rs: static files (GET/HEAD only, rule 6) and routes::configure,
    //    nothing else.
    let main = &sources
        .iter()
        .find(|(f, _)| f == "main.rs")
        .expect("main.rs")
        .1;
    let services: Vec<_> = code_lines(main)
        .filter(|(_, l)| l.contains(".service(") || l.contains(".configure("))
        .map(|(_, l)| l.trim())
        .collect();
    assert_eq!(
        services,
        [
            ".service(routes::assets::Assets)",
            ".configure(routes::configure)"
        ],
        "main.rs registers routes only through routes::configure"
    );

    // 3. routes::configure: one scope, wrapped in the guard, around `protected`.
    let routes_mod = &sources
        .iter()
        .find(|(f, _)| f == "routes/mod.rs")
        .expect("routes/mod.rs")
        .1;
    let configure: String = fn_body(routes_mod, "configure")
        .split_whitespace()
        .collect();
    assert_eq!(
        configure,
        "fnconfigure(cfg:&mutweb::ServiceConfig){cfg.service(web::scope(\"\")\
         .wrap(from_fn(crate::csrf::guard)).configure(protected),);",
        "routes::configure wraps every route in the cross-site check"
    );

    // 4. Every handler is registered in `protected`, and every registration
    //    there is a handler the scan found.
    let attrs = handler_attrs();
    let registered = protected_services();
    for (handler, path, method) in &attrs {
        assert!(
            registered.iter().any(|(h, _)| h == handler),
            "{method} {path} ({handler}) isn't registered in routes::protected"
        );
    }
    for (handler, _) in &registered {
        assert!(
            attrs.iter().any(|(h, ..)| h == handler),
            "routes::protected registers {handler}, which has no handler attribute"
        );
    }

    // 5. Behind the real wiring, each write route is the check's to reject.
    let state = test_state().await;
    let app = bare_test_app!(state);
    let routes = write_routes();
    for route in &routes {
        // Any id will do: without a session the handler answers 401 (or the
        // login form's error), never 403, 404 or 405.
        let uri = route
            .path
            .split('/')
            .map(|seg| if seg.starts_with('{') { NIL } else { seg })
            .collect::<Vec<_>>()
            .join("/");
        let send = |origin: &'static str| {
            TestRequest::default()
                .method(route.method.clone())
                .uri(&uri)
                .insert_header((header::HOST, TEST_HOST))
                .insert_header((header::ORIGIN, origin))
                .set_form([("email", "guard@example.com"), ("password", "x")])
                .to_request()
        };
        let forged = call_service(&app, send("http://evil.example")).await;
        assert_eq!(forged.status(), StatusCode::FORBIDDEN, "{route:?}");
        let genuine = call_service(&app, send(TEST_ORIGIN)).await.status();
        assert!(
            ![
                StatusCode::FORBIDDEN,
                StatusCode::NOT_FOUND,
                StatusCode::METHOD_NOT_ALLOWED
            ]
            .contains(&genuine),
            "{route:?}: a same-origin request reaches the handler, got {genuine}"
        );
    }

    // 6. Behind the real wiring (rule 2: `Assets` first, then
    //    `routes::configure`), `/assets` accepts no write method, on a known
    //    and an unknown path, with or without cross-site proof: 405 from
    //    `Assets`, never 2xx and never the check's 403 (RR-0011).
    let app = init_service(
        App::new()
            .app_data(state.clone())
            .service(Assets)
            .configure(crate::routes::configure),
    )
    .await;
    for method in [
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
        Method::TRACE,
    ] {
        for uri in ["/assets/app.css", "/assets/x"] {
            for origin in [TEST_ORIGIN, "http://evil.example"] {
                let req = TestRequest::default()
                    .method(method.clone())
                    .uri(uri)
                    .insert_header((header::HOST, TEST_HOST))
                    .insert_header((header::ORIGIN, origin))
                    .to_request();
                let res = call_service(&app, req).await;
                assert_eq!(
                    res.status(),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "{method} {uri} from {origin}: /assets accepts no write"
                );
                assert_eq!(
                    res.headers().get(header::ALLOW).expect("Allow"),
                    "GET, HEAD",
                    "{method} {uri} from {origin}"
                );
            }
        }
    }
}
