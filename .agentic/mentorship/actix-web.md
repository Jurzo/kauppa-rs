# Mentorship Pack — actix-web handlers

Version: v1 · Scope: `crates/kauppa-web/src/routes/**` and middleware.

## Handler shape

- Signature: `async fn name(state: web::Data<AppState>, …) -> WebResult<HttpResponse>`.
- Order of extractors: `HttpRequest` → shared state → `Identity` (or `Option<Identity>`) → `web::Path`/`web::Query`/`web::Form`/`web::Json`.
- Thin handlers: parse input → call a `kauppa_core` function → render.
  Business logic in handlers is a smell.

## Request / response conventions

- HTML pages return `HttpResponse::Ok().content_type("text/html").body(...)`.
- htmx-triggered endpoints detect `HX-Request: true` and return a fragment;
  without the header they return the full page or 303-redirect to the page.
- Form POSTs that succeed respond `303 See Other` with `Location: /...`, so a
  browser refresh is safe.
- Error paths return the sanitized message from `WebError::error_response`,
  never a `Debug` of the inner error.

## State

- Shared data lives in `AppState` behind `web::Data<AppState>`.
- Repo trait objects are owned by `AppState` and cloned per-worker.
- Do not reach into `AppState` for ad-hoc globals; add a field with a reviewer.

## Authentication

- `Identity` as the extractor means "must be signed in"; use `Option<Identity>`
  for pages that render differently signed-in vs out.
- Never read the raw session cookie; always go through `actix-identity`.

## Testing

- Unit-test core logic in `kauppa-core`.
- For HTTP smoke tests, use `actix_web::test::init_service` + `TestRequest`.
- Reserve full E2E (Playwright) for user-visible flows.

## Red flags

- `.unwrap()` or `.expect(...)` on a request-path value → raise a consult.
- `web::block(...)` wrapping async code — don't; call the async API directly.
- `sqlx::query(...)` inside a handler — route it through a repo trait.
