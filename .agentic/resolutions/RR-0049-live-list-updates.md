# Resolution Record — Live list updates: a bare signal over SSE, re-checked before every event, from a hub fed by the routes

```yaml
---
id: RR-0049
closes: CRP-0049
mission: MB-0057
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-16t14:10:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0057-live-list-updates.md
  - .agentic/consults/CRP-0049-live-list-updates.md
  - AGENTS.md
  - .agentic/runbooks/feature.md
  - scripts/agentic/check_triggers.py
  - .github/CODEOWNERS
---
```

## Decision and outcome

Approve CRP-0049 as recommended: Q1–Q9 A, and Q10 as listed.

1. **Transport and client (Q1 A, platform).**
   - A new route, `GET /lists/{id}/events`, answers `text/event-stream`.
     The list page and the shopping view connect with `sse-connect` and
     re-fetch their fragment on `hx-trigger="sse:changed"`.
   - `htmx-ext-sse` **2.2.4** (0BSD) is vendored byte-for-byte from the
     npm tarball's `package/dist/sse.min.js` as
     `assets/vendor/sse-2.2.4.min.js`: 2,853 B, sha256
     `98a46496de0c3605fbffdce9167ba427bdd9553184f83f149c261891a92c0136`,
     SRI `sha384-A986SAtodyH8eg8x8irJnYUk7i9inVQqYigD6qZ9evobksGNIXfeFvDwLSHcp31N`.
     The implementation re-checks both against the tarball (sha512 equal
     to npm `dist.integrity`) before committing it. It is the third
     vendored file (amends RR-0018 constraint 1).
   - It is loaded with `integrity` and `defer` after htmx in
     `layout.hbs`, and pinned in `templates/tests.rs` beside the others.
   - `<body hx-ext="response-targets,sse">`. `<body>` stays the only
     element that declares `hx-ext`. This amends RR-0025 constraint 1 and
     sub-answer 1, and the two tests that assert the exact value.
   - No CSP change, no `htmx-config` key, no new Rust dependency (the
     stream body is a hand-written `MessageBody` over the enabled tokio
     features). If that doesn't work, the implementation stops and asks.
   - Without JavaScript the pages work as today.
2. **What an event carries (Q2 A, security).** The event `changed`, with
   an empty data line, and nothing else. Each viewer re-fetches its own
   fragment through `GET /lists/{id}` or `GET /lists/{id}/shop`, so every
   byte of list content passes the existing predicate, role, language and
   theme. The stream route calls only `ShoppingListRepo::access` and
   `::get`; `list.rs`, `list_access.rs` and the bypass guard don't change
   (RR-0036 constraints 1–3).
3. **How a stream ends (Q3 A, security).**
   - **Connect:** identity (401), then the limits (sub-answer 5), then the
     predicate at `view` (404). No stream opens before the predicate
     passes.
   - **Every event:** the predicate runs again for that viewer before a
     `changed` is written. No grant is kept between events (RR-0002
     constraint 4).
   - **Access ended** (revoked, list deleted): one last `changed`, then the
     event `stop`, then the server closes the stream. Pages declare
     `sse-close="stop"`. The re-fetch answers the 404 page, swapped into
     `<body>` (RR-0038 sub-answer 5).
   - **Sign-out and expiry:** every stream ends after `STREAM_MAX_AGE`
     (15 minutes) without `stop`; the reconnect is checked by the identity
     middleware like any request.
   - **The visit deadline:** an open page counts as a visit (RR-0026's
     30-day idle deadline keeps running from each connect and re-fetch;
     the 90-day deadline holds). The session block of `main.rs` doesn't
     change.
   - **Headers:** `Cache-Control: no-store` (RR-0029 constraint 3), the
     CSP and companion headers (RR-0019 constraint 1).
   - **Logging,** ids only: DEBUG on open and close (`user_id`, `list_id`,
     reason); INFO when access ended; WARN when a limit refuses
     (`user_id`, the limit). The stream URL's query is never logged.
   - **No rate limit** on connects.
   - **Trigger path:** `crates/kauppa-web/src/live.rs` joins the
     "Shared-list authorization" row of `feature.md`, `check_triggers.py`
     and `CODEOWNERS`.
4. **The hub (Q4 A, platform).**
   - `LiveHub` in `crates/kauppa-web/src/live.rs`, held in `AppState` as
     an `Arc`, built in `main.rs` and `test_support`. This record approves
     the new `AppState` field (`actix-web.md` → State).
   - A `std::sync::Mutex` over list id → subscribers; never held across
     `.await`; a poisoned lock is recovered, never unwrapped.
   - A subscriber holds the viewer's user id, the list id and the list
     owner's id only.
   - Routes call `state.live.changed(list_id)` after their repo call
     returns `Ok`. Signals coalesce.
   - **Catch-up:** a new connection gets one `changed` at once if the list
     changed after the page was rendered, or when the server can't tell.
   - **Guard test:** a route that calls a `ShoppingListRepo` write method
     without announcing fails a test.
5. **Limits (Q5 A, platform).** Constants in `live.rs`:
   `STREAMS_PER_USER` = 8, `STREAMS_TOTAL` = 200, `KEEP_ALIVE` = 20 s,
   `STREAM_MAX_AGE` = 15 min.
   - At a cap: **503** with `Retry-After`, before the predicate query.
     Open streams are never evicted.
   - A failed keep-alive write frees the slot.
   - **Shutdown:** one task in `main.rs` waits for SIGTERM or SIGINT and
     closes every stream (without `stop`). actix-web's own signal handling
     and the session block don't change.
   - **No `deploy/` change.** If one is needed, the implementation stops
     and asks (RR-0014).
   - The README gets one line on HTTP/1.1's six connections per origin.
   - Tests pass short durations to the hub; tokio's `test-util` isn't
     added.
6. **Which changes count (Q6 A, owner).** Every route in CRP-0049's Q6
   table announces after a successful write, including
   `recipes::add_recipe_to_list`, archive and restore, and share role
   changes and revokes. The owner's category rename, move and delete call
   `state.live.owner_changed(user)`. Not announced: `lists::create`, the
   export routes, favorites and recipe edits, and rejected writes.
7. **Typing (Q7 A, owner).**
   - The add field and the recipe form move outside the part a live update
     swaps; a live update replaces only the items part. The viewer's own
     submissions swap everything, as today.
   - Each item's edit `<details>` gets `hx-preserve` with an id made from
     the item's id and its saved quantity and category.
   - This is proven in a browser before the implementation PR opens. If
     the edit sections can't be kept, the implementation stops and asks,
     with Option B (add forms only) as the fallback.
8. **The viewer's own change (Q8 A, owner).** Each page render makes a
   random tab id, carried in the stream URL (`?tab=…`) and in a plain-JSON
   `hx-headers` attribute on the page's `<section>`. A write carrying it
   skips that one subscriber when it is the same user's and the same
   list's. The id authorizes nothing.
9. **The reconnect check (Q9 A, platform).** The one P6 reconnect flow may
   see `htmx:sseError` and its console line after it drops the connection
   itself. Every other flow stays as strict as today. This amends RR-0041
   constraint 2.
10. **Brief amendments (Q10, owner):** as listed in CRP-0049 Q10.

Covers:
- MB-0057's implementation as above;
- the amendments to RR-0018 constraint 1, RR-0025 constraint 1 (and
  sub-answer 1) and RR-0041 constraint 2 named above;
- the roll-back below.

Does not cover:
- rendered content in the stream;
- first-party JavaScript, or another vendored file (idiomorph);
- live updates for anything but shopping lists;
- a change to the CSP, the session block, the schema, `list.rs`,
  `list_access.rs`, the bypass guard, the manifests or `deploy/`;
- a route that reorders items (none exists; a later one joins sub-answer
  6 through the guard test).

## Rationale and trade-offs

- A bare signal keeps all list content behind the routes that already
  check access, language and theme. Nothing new can leak but timing.
- Re-checking before every signal means no grant is cached, as RR-0002
  requires. A maximum age covers sign-outs that cookies can't report.
- The client is one pinned, checksummed extension; no code of ours runs
  in the browser.
- Closing streams on SIGTERM keeps deploys from waiting out the grace
  period.
- Rejected:
  - polling, or our own script (Q1 B, C);
  - HTML in the stream (Q2 B);
  - a revocation signal only, or a silent stop (Q3 B, C);
  - database polling (Q4 B);
  - other caps, or 200 and `stop` at a cap (Q5 B, C);
  - list writes only (Q6 B);
  - add forms only, a script, or a morphing swap (Q7 B, C, D) — B stays
    the fallback;
  - announcing the writer too (Q8 B);
  - no browser reconnect check (Q9 B).

## Approvals and authority

@Jurzo as security for sub-answers 2 and 3; as platform for sub-answers 1, 4, 5 and 9; as owner for sub-answers 6, 7, 8 and 10.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0049-live-list-updates.md`
- Mission Brief: `.agentic/missions/MB-0057-live-list-updates.md`
- Related: RR-0002, RR-0010, RR-0013, RR-0014, RR-0015, RR-0017, RR-0018,
  RR-0019, RR-0025, RR-0026, RR-0029, RR-0036, RR-0038, RR-0041, RR-0045,
  RR-0048.

## Constraints and follow-on rules (Hardened)

1. **No content in the stream.** A stream writes only `changed`, `stop`
   and keep-alive comments. Sending list content on a stream is a review
   failure.
2. **Re-check before every signal.** The stream route reads lists only
   through `ShoppingListRepo::access` and `::get`, at connect and before
   every `changed`. No grant is held between events.
3. **Every write announces.** A route that changes what a list shows
   announces it after a successful write; the guard test enforces it for
   `ShoppingListRepo` writes. A new such route joins the guard.
4. **Vendored file.** `sse-2.2.4.min.js` is pinned by sha256, length and
   SRI in `templates/tests.rs`. Upgrading or replacing it is a platform
   decision.
5. **Limits and shutdown.** The four constants change only through a
   consult. Streams close on SIGTERM.
6. **Browser checks.** Only the reconnect flow may expect `htmx:sseError`;
   P5 is proven in a browser before the implementation PR opens.
7. **Tests.** Route tests cover connect (401, 404, 503 at each cap),
   a viewer revoked mid-stream (last `changed`, then `stop`), the maximum
   age, catch-up, the writer's tab skipped, and the other roles and tabs
   announced.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0057`:
  - P1, P3, P5, P6 and P7 reworded as CRP-0049 Q10 lists;
  - plan step 2: "fed by every route that changes what a list shows";
  - `context` gains the files and RRs CRP-0049 Q10 lists;
  - the seven escalate entries marked decided (RR-0049);
  - `resolutions: [RR-0049]`;
  - plan step 1 marked done.
- `AGENTS.md`, do-not-touch boundaries: the vendored files include
  htmx's SSE extension 2.2.4 (RR-0049).
- `.agentic/runbooks/feature.md`, `scripts/agentic/check_triggers.py`,
  `.github/CODEOWNERS`: `crates/kauppa-web/src/live.rs` joins the
  "Shared-list authorization" trigger.
- `CRP-0049`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Amends RR-0018 constraint 1, RR-0025
constraint 1 and RR-0041 constraint 2 as above; supersedes nothing.
