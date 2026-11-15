# Mission Brief — Live list updates for everyone viewing a list (SSE)

```yaml
---
id: MB-0057
owner: "@juuso-kalliomaki"
goal: "Everyone viewing a shopping list, on its edit page or in shopping mode, sees every change to it (by another user, or by the same user in another tab or device) within a couple of seconds, without reloading, while each viewer still sees only what their own access allows."
non_goals:
  - "Live updates on any page other than a list's edit page (`/lists/{id}`) and its shopping view (`/lists/{id}/shop`): not the lists index, favorites, recipes or categories."
  - "Presence (who else is viewing), typing indicators, or merging two people's concurrent edits to the same field: the last write wins, as today."
  - "Offline support or queuing changes made without a connection."
  - "WebSockets, or a second process or broker: the app stays one process with one SQLite file (AGENTS.md goals)."
  - "Changing who may see or change a list (RR-0002, RR-0024, RR-0033)."
acceptance:
  - id: P1
    property: "A viewer of `/lists/{id}` sees, without reloading, any change another session makes to that list: an item added, edited (quantity, category), removed or restored, recipe ingredients added, an item checked or unchecked in shopping mode, the list renamed."
    check: "scripts/browser/run.sh (a new flow: two browser contexts, a change in one appears in the other)"
  - id: P2
    property: "A viewer of `/lists/{id}/shop` sees the same changes reflected in the shopping view (items, categories, the found count), without reloading."
    check: "scripts/browser/run.sh (the same flow in shopping mode)"
  - id: P3
    property: "The update stream for a list is refused (403/404, as the list's pages are) to anyone who can't view the list, and a viewer whose access ends (share revoked, list deleted, signed out or session expired) stops receiving updates and is shown the list's error page or sign-in on the next refresh; no update ever carries list content to a session that can't view it."
    check: cargo test -p kauppa-web routes::lists::tests::live::stream_requires_view_access
  - id: P4
    property: "Each viewer's update is rendered for that viewer: their own permissions (owner, `edit`, `view` grantee), language and theme, exactly as a reload would show it."
    check: cargo test -p kauppa-web routes::lists::tests::live::update_renders_per_viewer
  - id: P5
    property: "A change made while a viewer is typing into a form on the list (the add field, an open edit section) doesn't discard what they typed or close what they opened, as the RR decides."
    check: "scripts/browser/run.sh (a flow that types, receives an update, and submits)"
  - id: P6
    property: "Without JavaScript, or when the stream can't connect, the pages work exactly as today (no live updates); a dropped connection reconnects and catches up without a reload; the CSP is unchanged or changed only as the RR approves, and no request leaves the app's origin."
    check: "scripts/browser/run.sh (CSP, console and htmx error assertions) plus cargo test -p kauppa-web --all-targets"
  - id: P7
    property: "Open streams are bounded per user and in total, are closed on server shutdown without delaying it beyond the existing grace period, and work behind the deployment's Caddy proxy (`deploy/`)."
    check: "cargo test -p kauppa-web routes::lists::tests::live::stream_limits, plus manual: the live check through Caddy recorded in MB-0057's Continuity Pack"
  - id: P8
    property: "Two phones (or a phone and a desktop) on the deployed site, one in shopping mode and one editing, see each other's changes."
    check: "manual: owner's check on the deployed site, recorded in MB-0057's Continuity Pack"
context:
  - templates/lists/show.hbs
  - templates/lists/_list.hbs
  - templates/lists/shop.hbs
  - templates/lists/_shop.hbs
  - templates/layout.hbs
  - crates/kauppa-web/src/routes/lists/
  - crates/kauppa-core/src/repo/sqlite/list_access.rs
  - crates/kauppa-web/src/csp.rs
  - crates/kauppa-web/src/main.rs
  - assets/vendor/
  - deploy/Caddyfile
  - tests/browser/
  - .agentic/resolutions/RR-0002-list-sharing-authz.md
  - .agentic/resolutions/RR-0015-vendor-htmx.md
  - .agentic/resolutions/RR-0019-content-security-policy.md
  - .agentic/resolutions/RR-0026-session-lifetime-and-dev-secret.md
  - .agentic/resolutions/RR-0041-browser-level-test.md
  - .agentic/mentorship/security.md
autonomy:
  may_decide:
    - "event names, the stream's URL under `/lists/{id}/`, and test names inside `routes::lists::tests::live` (record renames here)"
    - "the browser flows added to `tests/browser/` for P1, P2, P5, P6"
  escalate:
    - { when: "the transport and client: Server-Sent Events with htmx's SSE extension (a new vendored file under `assets/vendor/`, RR-0015: platform), versus plain `EventSource` code (new JavaScript), versus htmx polling (`hx-trigger=\"every Ns\"`) as a no-new-code fallback", to: platform-owner }
    - { when: "what an event carries: a bare 'list changed' signal after which each viewer re-fetches its own fragment through the normal, authorized route (recommended), or rendered HTML pushed in the stream (must be rendered per viewer's access, language and theme)", to: security }
    - { when: "how a stream learns that access ended (re-check on every event, or a revocation signal), and what a long-lived stream does to the session's visit deadline (RR-0026)", to: security }
    - { when: "the in-process change notification: a broadcast channel in `AppState` fed by the list routes (one process, RR-0014), or polling the database's `updated_at`", to: platform-owner }
    - { when: "limits: streams per user and in total; what happens at the limit; keep-alive interval; Caddy and the Compose stack (deploy/ is a trigger path, RR-0014)", to: platform-owner }
    - { when: "P5's behaviour: skip an update while a form on the list has focus or unsent input and apply it after, or preserve the inputs across the swap, or update only the items and never the forms", to: owner }
    - { when: "whether the viewer's own change also arrives as an event (double swap) or is filtered out", to: owner }
  forbidden:
    - "send list content to a session that can't view the list"
    - "a query on the list tables outside the central access predicate (`list_access.rs`, RR-0036)"
    - "inline scripts, `unsafe-inline`, or loading anything from another origin"
    - "a new Rust dependency without the dependency-bump runbook"
runbook: .agentic/runbooks/feature.md@v1
mentorship:
  - AGENTS.md@v1
  - .agentic/mentorship/actix-web.md@v1
  - .agentic/mentorship/templating.md@v1
  - .agentic/mentorship/security.md@v1
resolutions: []
---
```

## Context

Requested by the owner (2026-11-15): "SSE for shared list updates. All list
updates should be rendered to users viewing the list in either edit or
shopping mode."

Today a list page changes only through the viewer's own htmx requests: each
form swaps `#list-{id}` (edit page) or `#shop-{id}` (shopping view) with the
fragment the server renders for that viewer. Two people shopping from one
shared list (MB-0008, RR-0002) don't see each other's ticks until they
reload.

Facts that shape the design:

- **One process.** The app is one actix-web process with one SQLite file
  (AGENTS.md goals, RR-0014), so changes can be announced in memory; no
  broker is needed.
- **Fragments are per viewer.** What a list renders depends on the viewer's
  role (owner, `edit`, `view`), language (RR-0023) and theme (RR-0035).
  Pushing one rendering to everyone would leak or mistranslate; the simplest
  safe design sends only "changed" and lets each viewer re-fetch through
  the routes that already enforce access (`list_access.rs`).
- **No custom JavaScript.** `assets/` holds nothing executable outside
  `assets/vendor/` (AGENTS.md). htmx has an official SSE extension
  (`htmx-ext-sse`), which would be vendored like `response-targets`
  (RR-0015, RR-0018), or htmx's built-in polling could do the same job
  without a stream.
- **CSP.** `default-src 'self'` already allows an `EventSource` to the
  app's own origin; no CSP change is expected (RR-0019).
- **Proxy.** Caddy's `reverse_proxy` streams `text/event-stream` responses
  without buffering; idle timeouts and the Compose stack need checking
  (P7).
- **Forms on the page.** An update that swaps `#list-{id}` while someone is
  typing in the add field or has an item's edit section open would discard
  their input (P5).
- **Browser test.** MB-0047's harness (`scripts/browser/run.sh`) can drive
  two browser contexts against one app, so P1, P2, P5 and P6 get automated
  checks.

Related: MB-0062 (archived lists) changes what a list page shows; whichever
lands second covers the other's events.

## Conceptual plan

1. **Clarify (S1).** One CRP: transport and client (platform), event
   content and access (security), notification and limits (platform), P5
   and own-change behaviour (owner).
2. Change notification in `AppState`, fed by every list write route.
3. The stream route behind the access predicate, with limits and
   keep-alive; route tests (P3, P4, P7).
4. Templates subscribe the edit page and shopping view; browser flows (P1,
   P2, P5, P6).
5. Deploy check through Caddy (P7) and the owner's two-device check (P8).

## Validation plan

- Route tests: stream refused without access, stops after revocation,
  per-viewer rendering, limits.
- Browser flows with two contexts; mutation: drop one write route's
  notification and see P1 fail.
- The owner's check on the deployed site.
