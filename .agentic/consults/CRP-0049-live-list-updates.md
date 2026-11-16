# Consultation Request Pack — Live list updates: the transport and client, what an event carries, how access ends, the notification, limits, which changes count, typing, and the viewer's own change

```yaml
---
id: CRP-0049
mission: MB-0057
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: open
runbook_trigger: "MB-0057 autonomy.escalate (transport and client, notification, limits → platform; event content, access ending and the visit deadline → security; typing and own-change behaviour → owner); feature.md escalation trigger `crates/kauppa-web/src/routes/lists/` (RR-0027); a new vendored file under `assets/vendor/` (RR-0015 constraint 1, RR-0018 constraint 1); a second `hx-ext` value (RR-0025 constraint 1); a new `AppState` field (actix-web.md → State)"
---
```

## Decision statement

Decide how a change to a shopping list reaches everyone viewing it: the
transport and the client code, what a stream carries, how a stream ends
when access ends, how routes announce changes, the limits on open streams,
which changes count, what happens to a form someone is typing in, and
whether the viewer's own change comes back to them.

## Options and trade-offs

### Q1 — Transport and client (platform)

#### Option A — Server-Sent Events with the vendored htmx SSE extension (recommended)

A new route `GET /lists/{id}/events` answers `text/event-stream`. The list
page and the shopping view connect to it with the extension's
`sse-connect`, and re-fetch on an event with `hx-trigger="sse:changed"`.

1. **The file.** `htmx-ext-sse` **2.2.4** (0BSD), the latest on npm, from
   `https://registry.npmjs.org/htmx-ext-sse/-/htmx-ext-sse-2.2.4.tgz`,
   file `package/dist/sse.min.js`, vendored byte-for-byte as
   `assets/vendor/sse-2.2.4.min.js` (RR-0018's naming: the upstream
   basename plus the version, no `htmx-` prefix). It is the third
   vendored file (RR-0018 constraint 1). Checked already, on a scratch
   copy (see Evidence):
   - tarball sha512 equals npm `dist.integrity`
     `sha512-LJmxVhykyflBWgh5PvbRidcyuqMHlgfajmmzumvKctv9puvsufeH6OaejSMZTNTnEI8O2wXXn4ZZtBdpEMqmEQ==`;
   - `sse.min.js` is **2,853 B** with sha256
     `98a46496de0c3605fbffdce9167ba427bdd9553184f83f149c261891a92c0136`;
   - its sha384 equals the SRI htmx's docs publish for
     `htmx-ext-sse@2.2.4` at tag `v2.0.11`
     (`www/content/extensions/sse.md` l.33):
     `sha384-A986SAtodyH8eg8x8irJnYUk7i9inVQqYigD6qZ9evobksGNIXfeFvDwLSHcp31N`.
   - No GitHub release digest exists for extensions; two checks suffice
     (RR-0018 sub-answer 1).
   - Its `htmx.org` range is `^2.0.2`, which covers 2.0.11 (RR-0018
     constraint 7).
   - The source has no `eval`, `Function(` or inline script. It opens
     `new EventSource(url, { withCredentials: true })`.
2. **Loading.** One more `<script … integrity=… defer>` in `layout.hbs`,
   after htmx. Constants (sha256, SRI, length) beside the others in
   `templates/tests.rs`; a `SERVED_AS` row; `no_external_scripts` allows
   the third file.
3. **`hx-ext`.** The layout's `<body>` becomes
   `hx-ext="response-targets,sse"`. It stays the only element that
   declares `hx-ext`. This amends RR-0025 constraint 1 and sub-answer 1,
   which name the exact value, and the two tests that assert it
   (`boosted_error_pages_swap`, `fragment_errors_not_swapped`).
4. **CSP.** Unchanged. `connect-src` falls back to `default-src 'self'`
   (RR-0019 sub-answer 1), and the stream is same-origin. No `htmx-config`
   key is added.
5. **No new Rust dependency.** The stream body is a hand-written
   `MessageBody` over `tokio::sync` and `tokio::time`, both already
   enabled. No `actix-web-lab`, `tokio-stream` or `futures`. If that
   turns out not to work, the implementation stops and asks (the
   dependency-bump runbook).
6. **Without JavaScript** nothing connects and the pages work as today
   (P6).

- Pros: updates arrive in well under a second; no first-party
  JavaScript; idle viewers cost one open connection and no requests.
- Cons: a third vendored file; a long-lived connection per open page
  (Q5).
- Risk: the extension retries a refused or dropped stream for ever, with
  a delay that doubles from 0.5 s to 64 s. Q3 and Q5 are written so that
  planned stops don't retry.

#### Option B — htmx polling, no stream

`hx-get` on the fragment with `hx-trigger="every 3s"`.

- Pros: no new file, no new route, no long-lived connections.
- Cons:
  - every open page sends a request every few seconds, changed or not,
    each running three to seven queries; a phone on a slow network (goal
    2) pays for that all the time;
  - each poll swaps the fragment, so the typing problem (Q7) happens
    every few seconds instead of only on a change;
  - a poll can't be made conditional in markup: `hx-trigger` filters are
    forbidden (RR-0019 constraint 3);
  - the owner asked for SSE.

#### Option C — First-party `EventSource` code

- Cons: `assets/` may hold nothing executable outside `assets/vendor/`
  (AGENTS.md, RR-0015 constraint 1). It would be the app's first own
  script.

### Q2 — What an event carries (security)

#### Option A — A bare signal; each viewer re-fetches through the normal routes (recommended)

The stream sends only the event name: `changed`, with an empty data line.
No list content, no item, no name, no id beyond what the URL already
holds. On `changed` the page re-fetches its fragment with an ordinary
htmx `GET /lists/{id}` or `GET /lists/{id}/shop`.

- **Access.** Every re-fetch runs the central predicate, as today. The
  stream route reads no list rows of its own: it calls
  `ShoppingListRepo::access` and `::get`, nothing new in `list.rs` or
  `list_access.rs`, and no change to the bypass guard (RR-0036
  constraints 1 to 3).
- **Per viewer (P4).** Role, language and theme come from the re-fetch's
  own request, exactly as a reload (RR-0023, RR-0035).
- **What can leak.** Only timing: that the list changed at some moment.
  Q3 bounds who can see even that.
- Cons: one extra request per viewer per change.

#### Option B — Rendered HTML in the stream

The server renders each viewer's fragment and pushes it.

- Cons:
  - the stream route then renders list content outside a request, with
    the role, language and theme it captured at connect; a role change
    or a language switch in another tab makes it wrong;
  - a mistake sends one viewer's rendering to another (the brief's first
    `forbidden` entry);
  - list content would flow on a connection whose access was checked
    earlier, which RR-0002 constraint 4 ("no cached grants") rules out
    unless every push re-checks anyway.

### Q3 — How a stream ends when access ends, and the session (security)

RR-0002 constraint 4 says a revoke removes access on the next request,
with no cached grants. A stream is one long request, so it needs its own
rule.

#### Option A — Re-check on every event, a final signal, and a maximum age (recommended)

1. **Connect.** `Identity` first (401 as any page), then the limits
   (Q5), then the predicate at `view`: 404 without access, as the list's
   pages answer. No stream is opened before the predicate passes.
2. **Every event.** Before a `changed` is written to a viewer, the
   predicate runs again for that viewer (`ShoppingListRepo::access`).
   No grant is kept between events.
3. **Access ended** (revoked, list deleted). Share changes, revokes and
   deletes announce the list like any other change (Q4), so the re-check
   runs at once. The viewer who fails it gets one last `changed` and then
   the event `stop`, and the server closes the stream.
   - The page declares `sse-close="stop"`, so the extension closes the
     connection and doesn't retry.
   - The last `changed` makes the page re-fetch. That answer is the 404
     page, which the error-page middleware already swaps into `<body>`
     under htmx (RR-0038 sub-answer 5). The viewer sees the same page a
     reload would show, and the stale list leaves the screen.
   - The last `changed` carries nothing. It tells a former viewer that
     the list they were looking at changed once more.
4. **Sign-out and expiry.** The session is a cookie with no server-side
   revocation (`security.md`), so a stream can't see a sign-out in
   another tab. Each stream therefore ends after a maximum age,
   `STREAM_MAX_AGE` = **15 minutes**, without `stop`. The browser
   reconnects by itself with its current cookie, and the identity
   middleware checks it again: a signed-out or expired session gets 401
   and no stream.
   - So a signed-out session can receive bare signals for at most 15
     minutes, and never content: its re-fetches are refused at once.
   - Its page stays as it was until the next action, which shows the
     sign-in page (RR-0038 sub-answer 4).
5. **The visit deadline (RR-0026).** actix-identity marks a visit on
   every request with an identity. A stream connect is a request, and so
   is every re-fetch. A page left open on a list therefore keeps the
   30-day idle deadline from running out; the 90-day sign-in deadline
   still holds. Recommended: accept this. A reload every few minutes
   would do the same today. Nothing in the session block of `main.rs`
   changes (RR-0026 constraint 5).
6. **Safe method.** The route is a GET that writes nothing (RR-0010
   constraint 2). A cross-site page can't open it with the user's
   cookie (`SameSite=Lax`) and couldn't read it anyway (no CORS
   headers).
7. **Headers.** `Content-Type: text/event-stream`. The default
   `Cache-Control: no-store` stays (RR-0029 constraint 3), as do the CSP
   and companion headers (RR-0019 constraint 1).
8. **Logging.** Ids only, never emails (RR-0002 constraint 5):
   - DEBUG when a stream opens and when it closes, with `user_id`,
     `list_id` and the reason;
   - INFO when a stream is closed because access ended;
   - WARN when a limit refuses a stream (Q5), with `user_id` and the
     limit's name.
   The request span records the path only, so the URL's query (Q8) is
   never logged (RR-0012).
9. **No rate limit on connects.** RR-0013 covers sign-in and
   registration only. The route is behind sign-in, the caps in Q5 bound
   open streams, and a refused connect costs one predicate query.
10. **Trigger path.** The hub (`crates/kauppa-web/src/live.rs`, Q4)
    decides who is sent a signal. Recommended: it joins the "Shared-list
    authorization" row in `feature.md`, `check_triggers.py` and
    `CODEOWNERS`. The stream route is under `routes/lists/` and is one
    already.

- Risk: a page restored from the browser's history for a list the user
  has since lost gets a 404 on connect, and the extension retries at up
  to 64 s intervals while that tab stays open. Each retry is one refused
  request.

#### Option B — A revocation signal only

Revoke and delete tell the hub to drop that viewer; events aren't
re-checked.

- Cons: a grant is cached for the stream's life. Any path that ends
  access without telling the hub keeps signalling. It reads against
  RR-0002 constraint 4.

#### Option C — As A, but a silent stop

On access ended the stream sends only `stop`.

- Cons: the list stays on a revoked viewer's screen until they act.

### Q4 — How routes announce a change (platform)

#### Option A — A hub in `AppState`, fed by the routes (recommended)

A `LiveHub` in a new `crates/kauppa-web/src/live.rs`, held in `AppState`
as an `Arc` and built in `main.rs` and `test_support` (as the auth
limiter is, RR-0013 sub-answer 3). This RR approves the new field
(`actix-web.md` → State).

- A `std::sync::Mutex` over a map from list id to its subscribers. The
  lock is never held across `.await`; a poisoned lock is recovered,
  never unwrapped.
- A route calls `state.live.changed(list_id)` after its repo call
  returned `Ok`. The hub marks each subscriber "dirty". Signals
  coalesce: a viewer with one unsent `changed` doesn't get a second.
- A subscriber holds the viewer's user id, the list id and the list
  owner's id. Nothing else, and no list content.
- One process, one hub (RR-0014). Nothing is persisted. After a restart
  every page reconnects and refreshes once.
- **Catch-up (P6).** Every connection gets one `changed` at the start
  if the list changed after the page was rendered, and always when the
  server can't tell (after a restart). A dropped connection therefore
  catches up when it reconnects.
- **A missed route is the main risk.** A guard test lists the
  `ShoppingListRepo` write methods and fails if a route calls one
  without announcing. The validation plan's mutation (drop one
  announcement, see P1 fail) stays.

#### Option B — Each stream polls the database

- Cons: constant queries per open stream; `updated_at` isn't touched by
  archive, restore (RR-0045 sub-answer 1), share changes or category
  changes, so it would miss them.

### Q5 — Limits, keep-alive, shutdown and the proxy (platform)

#### Option A — Fixed caps, refuse at the cap, close on shutdown (recommended)

Constants in `live.rs`, not settings (as RR-0013 constraint 6):

| Constant | Value | Meaning |
| --- | --- | --- |
| `STREAMS_PER_USER` | **8** | open streams per signed-in user |
| `STREAMS_TOTAL` | **200** | open streams in the process |
| `KEEP_ALIVE` | **20 s** | an SSE comment line when nothing else was sent |
| `STREAM_MAX_AGE` | **15 min** | Q3 point 4 |

1. **At a cap** the connect is refused with **503** and `Retry-After`,
   before the predicate query. The page works as today, without live
   updates, and the extension tries again by itself (at most every
   64 s). An open stream is never evicted to make room: two tabs would
   then evict each other in turn.
2. **Keep-alive** keeps mobile networks and the proxy from dropping an
   idle connection, and lets the server notice a viewer who has gone: a
   failed write frees the slot.
3. **Shutdown.** actix-web waits up to 30 s for open connections on
   SIGTERM, and Compose kills the container at 30 s
   (`stop_grace_period`). With one open stream every update would take
   the full 30 s and could end in a kill. So `main.rs` gets one task
   that waits for SIGTERM or SIGINT and tells the hub to close every
   stream (without `stop`, so pages reconnect to the new process).
   actix-web's own signal handling is unchanged. tokio's `signal`
   feature is already enabled. This is outside the session block.
4. **Caddy and Compose.** No change expected. Caddy's `reverse_proxy`
   flushes `text/event-stream` at once, and neither the Caddyfile nor
   the app compresses responses. P7's check through Caddy proves it. If
   a file under `deploy/` has to change, the implementation stops and
   asks (trigger path, RR-0014).
5. **Connections per browser.** Browsers reach Caddy over HTTP/2, so
   many streams share one connection. Straight to the app over HTTP/1.1
   (local development) a browser allows six connections per origin, so
   six open list tabs stall the seventh request. The README gets one
   line.
6. **Tests** pass short durations into the hub; they never sleep on the
   real values. tokio's `test-util` feature is not added.

#### Option B — Higher or lower numbers

The owner names them.

#### Option C — At a cap, answer 200 and `stop`

- Pros: no retries from that tab.
- Cons: the tab never gets live updates, even after other tabs close.

### Q6 — Which changes count (owner)

Every route below announces the list after a successful write:

| Change | Route |
| --- | --- |
| item added | `lists::add_item` |
| recipe's ingredients added | `lists::add_recipe`, and `recipes::add_recipe_to_list` (outside `routes/lists/`) |
| item saved (quantity, category) | `lists::update_item` |
| item removed, restored | `lists::remove_item`, `lists::restore_item`, `lists::remove_from_shop` |
| item checked, unchecked | `lists::check_item` |
| list renamed | `lists::rename` |
| list archived, restored | `lists::archive`, `lists::unarchive` (MB-0062) |
| list deleted | `lists::delete` |
| shared, role changed, revoked | `lists::share`, `lists::revoke` |

Not announced: `lists::create` (nobody is viewing yet) and the export
routes (they write the user's catalog, not the list). Rejected writes
(`Invalid`, `Conflict::Archived`, 403, 404) announce nothing.

There is no route that reorders a list's items today. A later one joins
the table (the guard test in Q4 makes that a failing test, not a
memory).

The open question is the owner's **categories**. A list shows its
categories' live names and order (`ITEM_SELECT`), so renaming, moving or
deleting a category changes every list of that owner without touching a
list table.

#### Option A — List writes and the owner's category changes (recommended)

`categories::rename`, `::move_category` and `::delete` call
`state.live.owner_changed(user)`, which announces every list that user
owns and that has a viewer. The hub already holds the owner's id per
subscriber, so this needs no query.

- Pros: a shopper's view regroups when the owner reorders categories at
  home.
- Cons: three more call sites, outside the list routes.

#### Option B — List writes only

- Cons: after a category change, viewers see old names or order until
  the next list change.

Favorites and recipes edits change only the owner's own suggestions and
recipe picker, not what a list shows. They are not announced under
either option.

### Q7 — A change arrives while someone is typing (owner; P5)

Today every swap replaces all of `#list-{id}`: the add field, the recipe
form, every item's edit section. A live swap of that would discard typed
text and close open sections.

The brief's first idea, "skip an update while a form has focus and apply
it after", can't be written in markup: it needs an `hx-trigger` filter
or a script, and both are ruled out (RR-0019 constraint 3;
`allowEval:false`; no first-party JavaScript).

In every option the shopping view is swapped whole. It has no text
fields.

#### Option A — Live updates never touch the add forms; an open edit section is kept unless its own item changed (recommended)

1. **Add field and recipe form.** They move out of the part a live
   update swaps. A live update replaces only the items part (the item
   rows, the removed items, the "updated" line). Typed text, focus and
   the form's error message are untouched. The viewer's own submissions
   swap everything, as today.
2. **Edit sections.** Each item's `<details>` gets `hx-preserve` and an
   id made from the item's id and its saved state (quantity and
   category). htmx then keeps the element as it is, open and with its
   typed text, across a live swap, unless that item itself changed. If
   it did, the new section replaces it and the typed text is lost: that
   is two people editing one field, which the brief's non-goals leave
   as last write wins.
3. **To be proven in the browser before the implementation PR opens**
   (RR-0025 constraint 4): a P5 flow that types into the add field and
   into an open edit section, receives an update, and submits.
   - Known doubt: on a browser without `Element.moveBefore`, htmx
     2.0.11 keeps a preserved element by re-inserting it, which may
     drop focus (the text stays). The agent hasn't verified which
     phones this affects.
   - If point 2 can't be made to work, the implementation stops and
     asks, with Option B as the fallback.

- Pros: markup only; no new file; the common case (typing the next
  item while someone ticks things off) is fully safe.
- Cons: `_list.hbs` is restructured; point 2 has the doubt above.

#### Option B — As A point 1, but edit sections are not protected

A live update closes an open edit section and discards a typed
quantity.

- Pros: certain to work.
- Cons: P5 is reworded to cover only the add forms.

#### Option C — A small first-party script that holds updates back

Hold a live update while any form on the list has focus or unsent
input; apply it after.

- Pros: P5 exactly as first imagined.
- Cons: the app's first own JavaScript. AGENTS.md's do-not-touch rule
  and RR-0015 constraint 1 would both change (platform).

#### Option D — Vendor a morphing swap (idiomorph)

- Cons: a fourth vendored file. From the agent's reading (unverified),
  its defaults reset a focused field to the server's value and close
  open `<details>` unless configured, and its configuration is
  evaluated as script, which the CSP blocks.

### Q8 — The viewer's own change (owner)

The tab that made a change already swaps the answer to its own request.

#### Option A — Skip the tab that made the change (recommended)

Each page render makes a random id. It goes into the stream's URL
(`/lists/{id}/events?tab=…`) and, as an `hx-headers` attribute on the
page's `<section>`, into every htmx request from that page. A write that
carries the header skips the one subscriber with that id, if it is the
same user's and the same list's. The same user's other tabs and devices
are announced as usual (the brief's goal).

- Pros: no second request and no second swap after every tick; that
  matters on a slow phone (goal 2), where the second swap can land
  while the next item is being tapped.
- Cons: one attribute on two pages and one header read per write.
- Risk: none for access. The id authorizes nothing; a forged one can
  only suppress a signal to the forger's own tab. `hx-headers` here is
  plain JSON, not a `js:` value (RR-0019 constraint 3). The id is in
  the query, which is never logged.

#### Option B — Everyone is announced, the writer too

- Pros: simplest.
- Cons: every change costs its maker a second request and swap.

### Q9 — The reconnect check and the browser-test rule (platform)

On every reconnect the extension fires `htmx:sseError`, and htmx logs it
as a console error. The browser harness fails on any `htmx:*Error`
event or console error (RR-0041 constraint 2). P6 asks for a browser
check that a dropped connection reconnects and catches up.

#### Option A — Allow `htmx:sseError` in the one reconnect flow (recommended)

The reconnect flow takes the page offline and back (Playwright, no
sleeps). Inside that flow only, after the test itself drops the
connection, `htmx:sseError` and its console line are expected. Every
other flow stays as strict as today. This amends RR-0041 constraint 2.

- Side note: the same event and console line appear in real use once
  per 15 minutes per open page (Q3 point 4). Users don't see the
  console.

#### Option B — No browser reconnect check

Catch-up is covered by a route test (a new connection gets `changed`),
and reconnecting is left to the extension.

- Cons: P6's reconnect is then unproven in a browser.

### Q10 — Brief amendments (owner)

Recommended, as the roll-back after the RR is signed:

- **P1** also names: archived and restored, a share's role changed, the
  owner's category renamed, moved or deleted (if Q6 A). The rename
  reaches the page's heading and title, which sit outside the fragment
  today.
- **P3** is reworded to Q3: refused with 401 or 404 at connect; after a
  revoke or a delete the stream stops at once and the
  viewer is shown the error page; after a sign-out or expiry signals
  stop within `STREAM_MAX_AGE` and the next action shows the sign-in
  page; no content in the stream at any time.
- **P5** as Q7 decides.
- **P6** names the reconnect flow and its one exception (Q9).
- **P7** names the constants, the 503 and the shutdown task (Q5).
- **Plan step 2:** "fed by every route that changes what a list shows",
  not "every list write route": one such route is in `routes/recipes/`,
  and three are in `routes/categories.rs`.
- **`context`** gains `crates/kauppa-web/src/routes/recipes/add_to_list.rs`,
  `crates/kauppa-web/src/routes/categories.rs`,
  `crates/kauppa-web/src/routes/errors.rs`,
  `crates/kauppa-web/src/templates/tests.rs`, `deploy/compose.yaml`, and
  RR-0010, RR-0017, RR-0018, RR-0025, RR-0029, RR-0036, RR-0038,
  RR-0045.
- **`escalate`** entries become the decisions; `resolutions: [RR-0049]`.

## Recommendation and question

Recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5 A, Q6 A, Q7 A, Q8 A, Q9 A, and
Q10 as listed.
Because:
- a bare signal keeps every byte of list content behind the routes that
  already check access, language and theme;
- the predicate runs again before every signal, so no grant is cached;
- the client is one pinned, checksummed extension and no code of ours;
- nothing changes in the CSP, the session block, the schema, the list
  SQL, the guard, the manifests or `deploy/`.

Crisp questions to answer:
1. (platform) SSE with `htmx-ext-sse` 2.2.4 vendored as
   `assets/vendor/sse-2.2.4.min.js`, and `<body
   hx-ext="response-targets,sse">` amending RR-0025 constraint 1 (A);
   polling (B); or first-party script (C)?
2. (security) A bare `changed` signal and a re-fetch (A), or rendered
   HTML in the stream (B)?
3. (security) The predicate before every signal; a last `changed` and
   `stop` when access ends; a 15-minute maximum age for sign-out and
   expiry; an open page counts as a visit; the logging; no rate limit;
   `live.rs` as a trigger path (A)? Or a revocation signal only (B), or
   a silent stop (C)?
4. (platform) A `LiveHub` in `AppState` fed by the routes, with a guard
   test (A), or database polling (B)?
5. (platform) 8 streams per user, 200 in total, 20 s keep-alive, 503 at
   a cap, a shutdown task in `main.rs`, no `deploy/` change (A)? Other
   numbers (B)? 200 and `stop` at a cap (C)?
6. (owner) Announce the owner's category changes too (A), or list
   writes only (B)?
7. (owner) Add forms outside live updates and edit sections kept by
   `hx-preserve`, proven in the browser first (A); add forms only (B);
   a first-party script (C); or a morphing swap (D)?
8. (owner) Skip the tab that made the change (A), or announce everyone
   (B)?
9. (platform) Allow `htmx:sseError` in the reconnect flow only,
   amending RR-0041 constraint 2 (A), or no browser reconnect check
   (B)?
10. (owner) The brief amendments as listed?

## Blast radius and rollback

What can break:
- privacy, if a signal or a re-fetch reached someone without access.
  The signal carries nothing and the re-fetch is the existing route;
- the list pages, if a live swap discards input (Q7) or eats a tap in
  the shopping view;
- the server, through too many open connections (Q5), or a slow
  shutdown;
- the htmx error wiring, through the changed `hx-ext` value (RR-0025).

Who is affected: every signed-in user who opens a list.

Rollback plan:
- Revert the implementation PR. There is no migration and no stored
  state.
- Short of a revert: removing `sse-connect` from the two templates turns
  live updates off and leaves everything else as it is.

## Minimal context

- **Today.** A list page changes only through the viewer's own htmx
  requests. Each swaps `#list-{id}` or `#shop-{id}` with a fragment
  rendered for that viewer (`render_list`, `render_shop`). `GET
  /lists/{id}` and `GET /lists/{id}/shop` already answer a fragment to a
  non-boosted htmx request (`wants_fragment`).
- **Access.** One predicate (`ACCESS_SELECT` in `list_access.rs`), which
  alone makes a `ListGrant`. No access is a 404, a weak role a 403.
  `begin_write(…, WhenArchived)` refuses item writes on an archived list
  (`Conflict::Archived`, RR-0045 constraint 1).
- **Errors under htmx.** The error-page middleware turns a 401, 403 or
  404 into a page and tells htmx to put it in `<body>` (`HX-Retarget`,
  RR-0038). `response-targets` on `<body>` makes htmx swap it (RR-0025).
- **Sessions.** A signed cookie; 30 days idle, 90 days in total
  (RR-0026); no server-side revocation.
- **Vendored files.** htmx 2.0.11 (RR-0017) and response-targets 2.0.4
  (RR-0018), pinned by checksum and SRI in `templates/tests.rs`.
- **Templates' rules.** No inline script, `hx-on*`, `js:` value or
  `hx-trigger` filter (RR-0019 constraint 3). `hx-ext` on `<body>` only
  (RR-0025 constraint 1).
- **Process.** One actix-web process, several worker threads, one
  `AppState` shared by all (`web::Data::new` outside the factory). No
  compression middleware. No HTTP/2 in the app; Caddy speaks it to
  browsers.
- **The list's name** is in the page's `<h1>` and `<title>`, outside
  both fragments. A rename is made on the lists index.

## Evidence bundle (Hardened)

- **Constraints checked:**
  - RR-0002 constraints 1, 4 and 5;
  - RR-0010 constraints 1 and 2;
  - RR-0013 ("does not cover" other routes; constraints 2 and 6 as the
    pattern for constants);
  - RR-0014 (one process; `deploy/` is a trigger path);
  - RR-0015 constraints 1 to 4; RR-0017 constraints 1 to 4; RR-0018
    constraints 1 to 4 and 7;
  - RR-0019 constraints 1 to 5;
  - RR-0025 constraints 1, 2 and 4; RR-0038 sub-answer 5 and
    constraint 5;
  - RR-0026 constraints 1 and 5;
  - RR-0029 constraint 3;
  - RR-0036 constraints 1 to 3;
  - RR-0041 constraints 1 and 2;
  - RR-0045 constraint 1;
  - RR-0048 constraint 1 (catalogs: not read by this feature);
  - the dependency-bump runbook (no manifest change is proposed).
- **The extension, on a scratch copy outside the repository
  (2026-11-16):** npm `dist-tags.latest` is 2.2.4 (published
  2025-10-18); the tarball's sha512 matched `dist.integrity`;
  `dist/sse.min.js` is 2,853 B, sha256 `98a46496…c0136`, sha384 equal to
  the docs SRI at htmx tag `v2.0.11`; `LICENSE` is the BSD Zero Clause
  License; `package.json` depends on `htmx.org` `^2.0.2`. Nothing was
  added to the repository.
- **Read in the extension's source (`sse.js`, 2.2.4):** `sse-connect`,
  `sse-close`, `hx-trigger="sse:<name>"`; the retry delay (0.5 s
  doubling to 64 s) when the connection is closed; `htmx:sseError` on
  every `EventSource` error; the connection is closed when its element
  leaves the page.
- **Read in htmx 2.0.11:** an error event is logged with
  `console.error`; `hx-preserve` uses `moveBefore` when the browser has
  it and `replaceChild` otherwise.
- **Read in actix-identity 0.8.0:** `middleware.rs` sets the last-visit
  time on every request that has an identity.
- **Code:**
  - `crates/kauppa-core/src/repo/sqlite/list_access.rs`;
  - `crates/kauppa-web/src/routes/lists/` (all six files);
  - `crates/kauppa-web/src/routes/recipes/add_to_list.rs`;
  - `crates/kauppa-web/src/routes/categories.rs`;
  - `crates/kauppa-web/src/routes/errors.rs`, `routes/request.rs`;
  - `crates/kauppa-web/src/main.rs`, `state.rs`, `csp.rs`;
  - `crates/kauppa-web/src/templates/tests.rs` (the `hx-ext` and
    `hx-trigger` rules);
  - `templates/layout.hbs`, `templates/lists/{show,_list,shop,_shop}.hbs`;
  - `deploy/Caddyfile`, `deploy/compose.yaml`;
  - `tests/browser/flows/fixtures.js`.
- **Not verified, and said so above:** Caddy's flushing of event
  streams (P7's check); focus on browsers without `moveBefore` (Q7 A);
  idiomorph's defaults (Q7 D). No code was run for this consult.

## Owner's answer (2026-11-16)

As recommended: Q1 A, Q2 A, Q3 A, Q4 A, Q5 A, Q6 A, Q7 A, Q8 A, Q9 A, and
Q10 as listed.

Recorded in RR-0049 (scaffold; the owner signs).
