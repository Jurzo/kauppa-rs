//! MB-0057 (RR-0049): the list's event stream and the live re-fetch.
//!
//! The brief's checks: `stream_requires_view_access` (P3),
//! `update_renders_per_viewer` (P4), `stream_limits` (P7). The rest cover
//! RR-0049 constraint 7: the maximum age, catch-up, the writer's tab, the
//! other roles and tabs, and that every announcing route announces. The
//! hub's own tests are in `crate::live::tests`.
#![allow(clippy::expect_used, clippy::panic)]

use std::future::poll_fn;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use actix_web::body::MessageBody;
use actix_web::dev::ServiceResponse;
use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use actix_web::web;
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;

use super::{list_posts, shared_list};
use crate::live::{Limits, LiveHub};
use crate::routes::test_support::{
    body_string, login_request, make_category, make_list, make_user, session_cookie, test_app,
    test_state,
};
use crate::state::AppState;

const CHANGED: &str = "event: changed\ndata:\n\n";
const STOP: &str = "event: stop\ndata:\n\n";
const KEEP_ALIVE: &str = ": keep-alive\n\n";

/// An open event stream, read one chunk (one event) at a time.
struct Stream<B>(Pin<Box<B>>);

impl<B: MessageBody> Stream<B> {
    /// The open stream in `resp`, after checking its headers: an event
    /// stream that is never stored, under the CSP (RR-0049 sub-answer 3).
    fn open(resp: ServiceResponse<B>) -> Self {
        assert_eq!(resp.status(), StatusCode::OK);
        let value = |name| resp.headers().get(name).and_then(|v| v.to_str().ok());
        assert_eq!(value(header::CONTENT_TYPE), Some("text/event-stream"));
        assert_eq!(value(header::CACHE_CONTROL), Some("no-store"));
        assert!(value(header::CONTENT_SECURITY_POLICY).is_some());
        Self(Box::pin(resp.into_body()))
    }

    async fn poll(&mut self) -> Option<String> {
        let chunk = poll_fn(|cx| self.0.as_mut().poll_next(cx)).await?;
        let bytes = chunk.map_err(Into::into).expect("a stream chunk");
        Some(String::from_utf8(bytes.to_vec()).expect("utf-8 event"))
    }

    /// The next chunk, keep-alive comments included, or `None` when the
    /// server ended the stream.
    async fn raw(&mut self) -> Option<String> {
        tokio::time::timeout(Duration::from_secs(5), self.poll())
            .await
            .expect("the stream wrote or ended within 5 s")
    }

    /// The next event, or `None` when the server ended the stream.
    /// Keep-alive comments are skipped: a stream writes one whenever its
    /// user opens another.
    async fn next(&mut self) -> Option<String> {
        loop {
            match self.raw().await {
                Some(chunk) if chunk == KEEP_ALIVE => {}
                other => return other,
            }
        }
    }

    /// Whether no event is written for a moment (keep-alive comments
    /// aside) and the stream stays open.
    async fn quiet(&mut self) -> bool {
        let wait = tokio::time::sleep(Duration::from_millis(150));
        tokio::pin!(wait);
        loop {
            tokio::select! {
                () = &mut wait => return true,
                chunk = self.poll() => {
                    if chunk.as_deref() != Some(KEEP_ALIVE) {
                        return false;
                    }
                }
            }
        }
    }
}

/// The session cookie of the user with `email`.
macro_rules! login {
    ($app:expr, $email:expr) => {
        session_cookie(&call_service(&$app, login_request($email).to_request()).await)
    };
}

/// `GET uri` with `cookie`.
macro_rules! get {
    ($app:expr, $cookie:expr, $uri:expr) => {
        call_service(
            &$app,
            TestRequest::get()
                .uri(&$uri)
                .cookie($cookie.clone())
                .to_request(),
        )
        .await
    };
}

/// An htmx `POST uri` with `cookie` and `form`; `tab` is the page's
/// `X-Live-Tab`.
macro_rules! post {
    ($app:expr, $cookie:expr, $uri:expr, $form:expr) => {
        post!($app, $cookie, $uri, $form, None::<Uuid>)
    };
    ($app:expr, $cookie:expr, $uri:expr, $form:expr, $tab:expr) => {{
        let mut req = TestRequest::post()
            .uri(AsRef::<str>::as_ref(&$uri))
            .cookie($cookie.clone())
            .insert_header(("HX-Request", "true"))
            .set_form($form);
        if let Some(tab) = $tab {
            req = req.insert_header(("X-Live-Tab", tab.to_string()));
        }
        call_service(&$app, req.to_request()).await
    }};
}

/// A list page's live re-fetch, as htmx sends it: `target` is
/// `list-items-{id}` or `shop-{id}`.
macro_rules! refetch {
    ($app:expr, $cookie:expr, $uri:expr, $target:expr) => {
        refetch!($app, $cookie, $uri, $target, "en")
    };
    ($app:expr, $cookie:expr, $uri:expr, $target:expr, $lang:expr) => {
        call_service(
            &$app,
            TestRequest::get()
                .uri(&$uri)
                .cookie($cookie.clone())
                .insert_header(("HX-Request", "true"))
                .insert_header(("HX-Target", $target))
                .insert_header((header::ACCEPT_LANGUAGE, $lang))
                .to_request(),
        )
        .await
    };
}

/// The stream URL for `list` from a page rendered now: nothing to catch up.
fn events(state: &AppState, list: Uuid, tab: Option<Uuid>) -> String {
    let since = state.live.mark();
    tab.map_or_else(
        || format!("/lists/{list}/events?since={since}"),
        |tab| format!("/lists/{list}/events?tab={tab}&since={since}"),
    )
}

/// The test state with its hub's limits replaced.
async fn state_with(limits: Limits) -> web::Data<AppState> {
    let state = test_state().await;
    web::Data::new(AppState {
        live: Arc::new(LiveHub::new(limits)),
        ..(**state).clone()
    })
}

/// Wait until the hub counts `open` streams: a stream's slot is freed by
/// its task, a moment after the response is dropped or the stream ends.
async fn open_streams(state: &AppState, open: usize) {
    for _ in 0..200 {
        if state.live.open_streams() == open {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(state.live.open_streams(), open);
}

/// `html`'s lines, trimmed, without the empty ones: a partial is indented
/// where a page includes it.
fn lines(html: &str) -> Vec<&str> {
    html.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

/// The items part in `html` (`open` is its opening tag's start), as
/// trimmed lines: it ends with the "updated" paragraph and its `</div>`.
fn items_part<'a>(html: &'a str, open: &str) -> Vec<&'a str> {
    let all = lines(html);
    let start = all
        .iter()
        .position(|l| l.starts_with(open))
        .unwrap_or_else(|| panic!("no {open} in {html}"));
    let end = (start..all.len() - 1)
        .find(|&i| all[i].ends_with("</p>") && all[i + 1] == "</div>")
        .unwrap_or_else(|| panic!("{open} has no end in {html}"));
    all[start..end + 2].to_vec()
}

/// P3: the stream is refused with 401 signed out and 404 without access,
/// and no slot stays taken; any role may listen. After a revoke the
/// viewer's stream writes one last `changed`, then `stop`, and ends; their
/// re-fetch is the 404 page. The same after the list is deleted. A stream
/// never carries anything else.
#[actix_web::test]
async fn stream_requires_view_access() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    make_user(&state, "stranger@example.com").await;
    let owner = login!(app, "owner@example.com");
    let viewer = login!(app, "viewer@example.com");
    let editor = login!(app, "editor@example.com");
    let stranger = login!(app, "stranger@example.com");
    let url = events(&state, s.list, None);

    // Signed out: 401. Signed in without access, or no such list: 404.
    let resp = call_service(&app, TestRequest::get().uri(&url).to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(get!(app, stranger, url).status(), StatusCode::NOT_FOUND);
    let missing = events(&state, Uuid::new_v4(), None);
    assert_eq!(get!(app, owner, missing).status(), StatusCode::NOT_FOUND);
    open_streams(&state, 0).await;

    // Every role may listen.
    let mut owner_stream = Stream::open(get!(app, owner, url));
    let mut viewer_stream = Stream::open(get!(app, viewer, url));
    let mut editor_stream = Stream::open(get!(app, editor, url));
    open_streams(&state, 3).await;
    assert!(owner_stream.quiet().await && viewer_stream.quiet().await);

    // A user's new connect makes their other streams write a keep-alive
    // comment at once (a gone client is found by a failed write); no one
    // else's stream writes anything.
    let mut owner_second = Stream::open(get!(app, owner, url));
    assert_eq!(owner_stream.raw().await.as_deref(), Some(KEEP_ALIVE));
    assert!(owner_second.quiet().await && editor_stream.quiet().await);
    drop(owner_second);
    open_streams(&state, 3).await;

    // The viewer is revoked: a last `changed`, `stop`, and the end. The
    // others are told the list changed, and stay.
    let revoke = format!("/lists/{}/shares/{}/revoke", s.list, s.viewer_share);
    let resp = post!(app, owner, revoke, Vec::<(&str, String)>::new());
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(viewer_stream.next().await.as_deref(), Some(CHANGED));
    assert_eq!(viewer_stream.next().await.as_deref(), Some(STOP));
    assert_eq!(viewer_stream.next().await, None);
    assert_eq!(editor_stream.next().await.as_deref(), Some(CHANGED));
    assert_eq!(owner_stream.next().await.as_deref(), Some(CHANGED));
    assert!(editor_stream.quiet().await && owner_stream.quiet().await);
    open_streams(&state, 2).await;

    // The former viewer's re-fetch and reconnect are both the 404 page.
    let items = format!("list-items-{}", s.list);
    let resp = refetch!(app, viewer, format!("/lists/{}", s.list), items.as_str());
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let retarget = resp
        .headers()
        .get("hx-retarget")
        .and_then(|v| v.to_str().ok());
    assert_eq!(retarget, Some("body"), "htmx puts the error page in <body>");
    let page = body_string(resp).await;
    assert!(!page.contains("Leipä"), "no list content: {page}");
    assert_eq!(get!(app, viewer, url).status(), StatusCode::NOT_FOUND);

    // The list is deleted: the same for everyone left.
    let delete = format!("/lists/{}/delete", s.list);
    let resp = post!(app, owner, delete, Vec::<(&str, String)>::new());
    assert_eq!(resp.status(), StatusCode::OK);
    for stream in [&mut editor_stream, &mut owner_stream] {
        assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
        assert_eq!(stream.next().await.as_deref(), Some(STOP));
        assert_eq!(stream.next().await, None);
    }
    open_streams(&state, 0).await;
}

/// P7: streams are bounded per user and in total. At a cap the connect is
/// a 503 with `Retry-After`, before the predicate runs (it isn't the 404
/// an unknown list would get); open streams stay and still work; a closed
/// one frees its slot. A silent stream writes keep-alive comments.
/// Shutdown ends every stream without `stop` and refuses new ones.
#[actix_web::test]
async fn stream_limits() {
    let state = state_with(Limits {
        per_user: 2,
        total: 3,
        keep_alive: Duration::from_millis(40),
        ..Limits::default()
    })
    .await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let owner = login!(app, "owner@example.com");
    let viewer = login!(app, "viewer@example.com");
    let editor = login!(app, "editor@example.com");
    let url = events(&state, s.list, None);
    let missing = events(&state, Uuid::new_v4(), None);

    let refused = |resp: &ServiceResponse<_>| {
        let value = |name| resp.headers().get(name).and_then(|v| v.to_str().ok());
        resp.status() == StatusCode::SERVICE_UNAVAILABLE
            && value(header::RETRY_AFTER) == Some("30")
            && value(header::CACHE_CONTROL) == Some("no-store")
    };

    // Per user: two streams, the third refused, even for an unknown list.
    let mut first = Stream::open(get!(app, owner, url));
    let second = Stream::open(get!(app, owner, url));
    assert!(refused(&get!(app, owner, url)));
    assert!(refused(&get!(app, owner, missing)));
    // In total: a third stream for someone else, then no more for anyone.
    let third = Stream::open(get!(app, viewer, url));
    assert!(refused(&get!(app, editor, url)));
    assert!(refused(&get!(app, editor, missing)));
    open_streams(&state, 3).await;

    // Nothing was evicted: the first stream still hears a change, and
    // keep-alive comments while nothing changes.
    let add = format!("/lists/{}/items", s.list);
    assert_eq!(
        post!(app, editor, add, [("text", "Munat")]).status(),
        StatusCode::OK
    );
    let mut seen = Vec::new();
    while !(seen.contains(&CHANGED.to_owned()) && seen.contains(&KEEP_ALIVE.to_owned())) {
        let chunk = first.raw().await.expect("the stream stays open");
        assert!(chunk == CHANGED || chunk == KEEP_ALIVE, "{chunk:?}");
        seen.push(chunk);
    }

    // A closed stream frees its slot, for its user and in total.
    drop(second);
    open_streams(&state, 2).await;
    let mut again = Stream::open(get!(app, owner, url));
    assert!(refused(&get!(app, owner, url)));
    drop(third);
    open_streams(&state, 2).await;
    let mut editors = Stream::open(get!(app, editor, url));

    // Shutdown: every stream ends, without `stop`; new ones are refused.
    state.live.close_all();
    for stream in [&mut first, &mut again, &mut editors] {
        loop {
            match stream.next().await.as_deref() {
                None => break,
                Some(CHANGED) => {}
                Some(other) => panic!("a closing stream wrote {other:?}"),
            }
        }
    }
    open_streams(&state, 0).await;
    assert_eq!(
        get!(app, owner, url).status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

/// RR-0049 sub-answer 3: a stream ends at its maximum age without `stop`,
/// so the page reconnects through the identity middleware.
#[actix_web::test]
async fn stream_ends_at_max_age() {
    let state = state_with(Limits {
        max_age: Duration::from_millis(120),
        ..Limits::default()
    })
    .await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let owner = login!(app, "owner@example.com");
    let url = events(&state, s.list, None);

    let started = std::time::Instant::now();
    let mut stream = Stream::open(get!(app, owner, url));
    assert_eq!(stream.next().await, None, "ended without an event");
    assert!(started.elapsed() >= Duration::from_millis(120));
    open_streams(&state, 0).await;
    // The reconnect is a new stream.
    let _reconnected = Stream::open(get!(app, owner, url));
}

/// RR-0049 sub-answer 4: a new stream starts with one `changed` when the
/// list changed after the page was rendered, or when the server can't
/// tell; otherwise it starts silent. The page's stream URL carries the
/// mark.
#[actix_web::test]
async fn catch_up_on_connect() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let owner = login!(app, "owner@example.com");
    let editor = login!(app, "editor@example.com");

    // The stream URL as the page gives it.
    let stream_url = |page: &str| {
        let start = page.find("sse-connect=\"").expect("sse-connect") + "sse-connect=\"".len();
        let len = page[start..].find('"').expect("closed attribute");
        page[start..start + len].replace("&amp;", "&")
    };
    for page in [
        format!("/lists/{}", s.list),
        format!("/lists/{}/shop", s.list),
    ] {
        let url = stream_url(&body_string(get!(app, editor, page)).await);
        assert!(
            url.starts_with(&format!("/lists/{}/events?tab=", s.list)),
            "{url}"
        );

        // Nothing changed since the render: silent.
        let mut stream = Stream::open(get!(app, editor, url));
        assert!(stream.quiet().await, "{page}");
        drop(stream);

        // The list changed after the render, before the stream opened
        // (or while it was down): one `changed` at once.
        let add = format!("/lists/{}/items", s.list);
        assert_eq!(
            post!(app, owner, add, [("text", "Munat")]).status(),
            StatusCode::OK
        );
        let mut stream = Stream::open(get!(app, editor, url));
        assert_eq!(stream.next().await.as_deref(), Some(CHANGED), "{page}");
        assert!(stream.quiet().await, "one signal, not one per change");
    }

    // The owner's categories changed after the render.
    let url = stream_url(&body_string(get!(app, editor, format!("/lists/{}", s.list))).await);
    let rename = format!("/categories/{}/rename", s.category);
    assert_eq!(
        post!(app, owner, rename, [("name", "Kylmät")]).status(),
        StatusCode::OK
    );
    let mut stream = Stream::open(get!(app, editor, url));
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));

    // The server can't tell: no mark, or one it doesn't know.
    for query in ["", "?since=", "?since=abc.1", "?tab=not-a-uuid&since=1"] {
        let url = format!("/lists/{}/events{query}", s.list);
        let mut stream = Stream::open(get!(app, editor, url));
        assert_eq!(stream.next().await.as_deref(), Some(CHANGED), "{query}");
    }
}

/// RR-0049 sub-answer 8 and constraint 7: a write that carries its page's
/// tab id skips that one stream; the same user's other tab and every other
/// role are told. The id authorizes nothing: it doesn't skip another
/// user's stream, and a rejected or refused write announces nothing.
#[actix_web::test]
async fn writers_tab_is_skipped() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let owner = login!(app, "owner@example.com");
    let viewer = login!(app, "viewer@example.com");
    let editor = login!(app, "editor@example.com");
    let (tab, other_tab) = (Uuid::new_v4(), Uuid::new_v4());

    let mut writing = Stream::open(get!(app, owner, events(&state, s.list, Some(tab))));
    let mut second = Stream::open(get!(app, owner, events(&state, s.list, Some(other_tab))));
    let mut shop = Stream::open(get!(app, owner, events(&state, s.list, None)));
    let mut editors = Stream::open(get!(app, editor, events(&state, s.list, Some(tab))));
    let mut viewers = Stream::open(get!(app, viewer, events(&state, s.list, None)));

    let add = format!("/lists/{}/items", s.list);
    let resp = post!(app, owner, add, [("text", "Munat")], Some(tab));
    assert_eq!(resp.status(), StatusCode::OK);
    for stream in [&mut second, &mut shop, &mut editors, &mut viewers] {
        assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
    }
    assert!(writing.quiet().await, "the writer's own page isn't told");

    // The editor's write with the same tab id skips only the editor's own
    // stream.
    let resp = post!(app, editor, add, [("text", "Voi")], Some(tab));
    assert_eq!(resp.status(), StatusCode::OK);
    for stream in [&mut writing, &mut second, &mut shop, &mut viewers] {
        assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
    }
    assert!(editors.quiet().await);

    // Rejected input (200 with a message), a refused write (403) and a
    // write to another list: no one is told.
    let resp = post!(app, owner, add, [("text", " ")]);
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = post!(app, viewer, add, [("text", "Suola")]);
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let other = make_list(&state, s.owner, "Toinen").await;
    let resp = post!(
        app,
        owner,
        format!("/lists/{other}/items"),
        [("text", "Suola")]
    );
    assert_eq!(resp.status(), StatusCode::OK);
    for stream in [
        &mut writing,
        &mut second,
        &mut shop,
        &mut editors,
        &mut viewers,
    ] {
        assert!(stream.quiet().await);
    }
}

/// A POST with a name for the assertion messages: `(name, uri, form)`.
type NamedPost = (String, String, Vec<(&'static str, String)>);

/// RR-0049 sub-answer 6 and constraint 3, by behaviour: every announcing
/// route makes a viewer's stream write `changed`: the item, shopping,
/// rename, archive and sharing routes, the recipe page's "add to a list",
/// and the owner's category changes. `lists::create` and the export
/// announce nothing. (`live_guard` checks the same from the source.)
#[actix_web::test]
async fn every_list_change_is_announced() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let second = make_category(&state, s.owner, "Hevi").await;
    let owner = login!(app, "owner@example.com");
    let editor = login!(app, "editor@example.com");
    let mut stream = Stream::open(get!(app, editor, events(&state, s.list, None)));

    let base = format!("/lists/{}", s.list);
    let mut posts: Vec<NamedPost> = list_posts(&s)
        .into_iter()
        // Deleting ends the stream (`stream_requires_view_access`); the
        // shopping view's remove comes below, for an item still there.
        .filter(|(name, ..)| !["delete", "shop_remove"].contains(name))
        .map(|(name, uri, form)| (name.to_owned(), uri, form))
        .collect();
    posts.extend([
        (
            "shop remove".to_owned(),
            format!("{base}/shop/items/{}/remove", s.sugar),
            vec![],
        ),
        ("archive".to_owned(), format!("{base}/archive"), vec![]),
        ("unarchive".to_owned(), format!("{base}/unarchive"), vec![]),
        (
            "role change".to_owned(),
            format!("{base}/shares"),
            vec![
                ("email", "viewer@example.com".to_owned()),
                ("role", "edit".to_owned()),
            ],
        ),
        (
            "recipe page".to_owned(),
            format!("/recipes/{}/add-to-list", s.recipe),
            vec![
                ("list_id", s.list.to_string()),
                ("multiplier", "1".to_owned()),
            ],
        ),
        (
            "category rename".to_owned(),
            format!("/categories/{}/rename", s.category),
            vec![("name", "Kylmät".to_owned())],
        ),
        (
            "category move".to_owned(),
            format!("/categories/{second}/move"),
            vec![("dir", "up".to_owned())],
        ),
        (
            "category delete".to_owned(),
            format!("/categories/{second}/delete"),
            vec![],
        ),
    ]);
    for (name, uri, form) in posts {
        let resp = post!(app, owner, uri, form);
        assert_eq!(resp.status(), StatusCode::OK, "{name}");
        assert_eq!(stream.next().await.as_deref(), Some(CHANGED), "{name}");
        assert!(stream.quiet().await, "{name}: one signal");
    }

    // Not announced: a new list (nobody is viewing it) and the export.
    let resp = post!(app, owner, "/lists", [("name", "Uusi")]);
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = post!(
        app,
        owner,
        format!("{base}/export"),
        Vec::<(&str, String)>::new()
    );
    assert!(resp.status().is_success() || resp.status().is_redirection());
    assert!(stream.quiet().await);
}

/// P4: a live re-fetch renders for the viewer who makes it, exactly as
/// their own page shows it: the items part and the shopping view equal the
/// page's, per role (the owner's category picker, an `edit` grantee's
/// quantity field, a `view` grantee's plain rows) and per language. The
/// theme is the layout's only, so a fragment has none to differ in.
#[actix_web::test]
async fn update_renders_per_viewer() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let items_id = format!("list-items-{}", s.list);
    let shop_id = format!("shop-{}", s.list);
    let list_url = format!("/lists/{}", s.list);
    let shop_url = format!("/lists/{}/shop", s.list);

    for email in [
        "owner@example.com",
        "editor@example.com",
        "viewer@example.com",
    ] {
        let cookie = login!(app, email);
        for lang in ["en", "fi"] {
            let page = |uri: &str| {
                TestRequest::get()
                    .uri(uri)
                    .cookie(cookie.clone())
                    .insert_header((header::ACCEPT_LANGUAGE, lang))
                    .to_request()
            };
            let what = format!("{email} {lang}");

            // The edit page's items part.
            let full = body_string(call_service(&app, page(&list_url)).await).await;
            let live = refetch!(app, cookie, list_url, items_id.as_str(), lang);
            assert_eq!(live.status(), StatusCode::OK, "{what}");
            let live = body_string(live).await;
            assert!(!live.contains("<html"), "{what}: a fragment");
            let open = format!(r#"<div id="{items_id}""#);
            let shown = items_part(&full, &open);
            assert_eq!(items_part(&live, &open), shown, "{what}");
            let has = |needle: &str| shown.iter().any(|l| l.contains(needle));
            assert!(has("Leipä"), "{what}");
            assert_eq!(
                has("<details class=\"product-edit\""),
                !email.starts_with("viewer"),
                "{what}"
            );
            assert_eq!(
                has("name=\"category_id\""),
                email.starts_with("owner"),
                "{what}"
            );
            assert!(
                has(if lang == "fi" {
                    "Päivitetty"
                } else {
                    "Updated"
                }),
                "{what}: {shown:?}"
            );

            // The shopping view, whole.
            let full = body_string(call_service(&app, page(&shop_url)).await).await;
            let live = body_string(refetch!(app, cookie, shop_url, shop_id.as_str(), lang)).await;
            let open = format!(r#"<div id="{shop_id}""#);
            let all = lines(&full);
            let start = all
                .iter()
                .position(|l| l.starts_with(&open))
                .expect("shop part");
            let end = all
                .iter()
                .rposition(|l| *l == "</section>")
                .expect("page section");
            let shown = &all[start..end];
            let live = lines(&live);
            let at = live
                .iter()
                .position(|l| l.starts_with(&open))
                .expect("live shop part");
            assert_eq!(&live[at..], shown, "{what}");
            let has = |needle: &str| shown.iter().any(|l| l.contains(needle));
            assert_eq!(
                has("class=\"shop-check\""),
                !email.starts_with("viewer"),
                "{what}"
            );
        }
    }
}

/// RR-0049 sub-answer 6 (archive, restore, a share's role): the forms part
/// of the page isn't in a live update, unless the page's no longer fits.
/// The re-fetch says which forms the page shows (`shown`); when the list
/// was archived or restored, or the viewer's role changed, the answer also
/// carries the forms part, to swap out of band. The stream itself still
/// says only `changed`.
#[actix_web::test]
async fn archive_and_role_changes_update_the_forms() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let owner = login!(app, "owner@example.com");
    let editor = login!(app, "editor@example.com");
    let items_id = format!("list-items-{}", s.list);
    let forms = format!(
        r#"<div id="list-forms-{}" class="list-forms" hx-swap-oob="true">"#,
        s.list
    );
    let mut stream = Stream::open(get!(app, editor, events(&state, s.list, None)));

    // What the editor's page shows, from its hidden field.
    let shown = |page: &str| {
        let field = format!(r#"id="list-shown-{}" name="shown" value=""#, s.list);
        let start = page.find(&field).expect("the shown field") + field.len();
        page[start..start + page[start..].find('"').expect("closed")].to_owned()
    };
    let page = body_string(get!(app, editor, format!("/lists/{}", s.list))).await;
    let active = shown(&page);
    assert_eq!(active, "edit-shared-active");
    assert!(
        !page.contains("hx-swap-oob"),
        "a page has no out-of-band part"
    );

    let live = |shown: &str| format!("/lists/{}?shown={shown}", s.list);
    macro_rules! live_body {
        ($cookie:expr, $shown:expr) => {
            body_string(refetch!(app, $cookie, live($shown), items_id.as_str())).await
        };
    }

    // An ordinary change: the items part only.
    let add = format!("/lists/{}/items", s.list);
    assert_eq!(
        post!(app, owner, add, [("text", "Munat")]).status(),
        StatusCode::OK
    );
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
    let body = live_body!(editor, &active);
    assert!(body.contains("Munat") && body.contains(&format!(r#"<div id="{items_id}""#)));
    assert!(!body.contains("list-forms-"), "{body}");
    assert!(!body.contains(r#"action="/lists/"#) || !body.contains("id=\"new-item\""));

    // Archived by the owner: the notice replaces the add form.
    let archive = format!("/lists/{}/archive", s.list);
    let resp = post!(app, owner, archive, Vec::<(&str, String)>::new());
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
    let body = live_body!(editor, &active);
    assert!(body.contains(&forms), "{body}");
    assert!(
        body.contains("Archived on") && !body.contains("id=\"new-item\""),
        "{body}"
    );
    assert!(
        !body.contains("<details class=\"product-edit\""),
        "read-only items"
    );
    let archived = shown(&body);
    assert_eq!(archived, "read-shared-archived");
    // The page now shows that state: later updates leave the forms alone.
    assert!(!live_body!(editor, &archived).contains("list-forms-"));
    // The owner's page gets Restore.
    let body = live_body!(owner, "edit-owner-active");
    assert!(body.contains(&forms) && body.contains(&format!("/lists/{}/unarchive", s.list)));

    // Restored: the add form is back.
    let unarchive = format!("/lists/{}/unarchive", s.list);
    let resp = post!(app, owner, unarchive, Vec::<(&str, String)>::new());
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
    let body = live_body!(editor, &archived);
    assert!(
        body.contains(&forms) && body.contains("id=\"new-item\""),
        "{body}"
    );
    assert_eq!(shown(&body), active);

    // The editor's role becomes `view`: the add form goes.
    let share = format!("/lists/{}/shares", s.list);
    let form = [("email", "editor@example.com"), ("role", "view")];
    assert_eq!(post!(app, owner, share, form).status(), StatusCode::OK);
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
    let body = live_body!(editor, &active);
    assert!(
        body.contains(&forms) && body.contains("Shared with you to view."),
        "{body}"
    );
    assert!(!body.contains("id=\"new-item\""));
    assert_eq!(shown(&body), "read-shared-active");

    // A re-fetch that doesn't say what its page shows gets the forms.
    let resp = refetch!(app, editor, format!("/lists/{}", s.list), items_id.as_str());
    assert!(body_string(resp).await.contains(&forms));
}

/// P1 (the rename): the list's name is in the page's heading and title,
/// outside the swapped parts, so a live update carries both: a `<title>`
/// htmx applies, equal to the page's, and the heading to swap out of band.
#[actix_web::test]
async fn rename_reaches_heading_and_title() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let owner = login!(app, "owner@example.com");
    let editor = login!(app, "editor@example.com");
    let mut stream = Stream::open(get!(app, editor, events(&state, s.list, None)));

    let rename = format!("/lists/{}/rename", s.list);
    let resp = post!(app, owner, rename, [("name", "Mökki & <sauna>")]);
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));

    let title = "<title>Mökki &amp; &lt;sauna&gt; — kauppalista.app</title>";
    let heading = |oob: &str| {
        format!(
            r#"<h1 id="list-name-{}"{oob}>Mökki &amp; &lt;sauna&gt;</h1>"#,
            s.list
        )
    };
    for (uri, target) in [
        (
            format!("/lists/{}", s.list),
            format!("list-items-{}", s.list),
        ),
        (
            format!("/lists/{}/shop", s.list),
            format!("shop-{}", s.list),
        ),
    ] {
        let page = body_string(get!(app, editor, uri)).await;
        assert!(page.contains(title), "{uri}: the page's title: {page}");
        assert!(page.contains(&heading("")), "{uri}: the page's heading");
        let live = body_string(refetch!(app, editor, uri, target.as_str())).await;
        assert!(live.contains(title), "{uri}: {live}");
        assert!(
            live.contains(&heading(r#" hx-swap-oob="true""#)),
            "{uri}: {live}"
        );
    }

    // The viewer's own htmx requests (not a live re-fetch) get neither.
    let add = format!("/lists/{}/items", s.list);
    let own = body_string(post!(app, editor, add, [("text", "Munat")])).await;
    assert!(
        !own.contains("<title>") && !own.contains("hx-swap-oob"),
        "{own}"
    );
}

/// RR-0002 constraint 4 through the stream: a stream holds no grant. Its
/// user's access is read again for every event, so a change made straight
/// in the database (no route, no announcement) is honoured by the next
/// event.
#[actix_web::test]
async fn access_is_rechecked_for_every_event() {
    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let owner = login!(app, "owner@example.com");
    let editor = login!(app, "editor@example.com");
    let mut stream = Stream::open(get!(app, editor, events(&state, s.list, None)));

    let add = format!("/lists/{}/items", s.list);
    assert_eq!(
        post!(app, owner, add, [("text", "Munat")]).status(),
        StatusCode::OK
    );
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));

    // Revoked without an announcement: the stream stays open, unaware.
    ShoppingListRepo::revoke(&state.pool, s.owner, s.list, s.editor_share)
        .await
        .expect("revoke");
    assert!(stream.quiet().await);
    // The next change is checked first: the last `changed`, then `stop`.
    assert_eq!(
        post!(app, owner, add, [("text", "Voi")]).status(),
        StatusCode::OK
    );
    assert_eq!(stream.next().await.as_deref(), Some(CHANGED));
    assert_eq!(stream.next().await.as_deref(), Some(STOP));
    assert_eq!(stream.next().await, None);
}
