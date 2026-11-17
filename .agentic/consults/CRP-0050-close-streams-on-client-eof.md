# Consultation Request Pack — Free a live stream as soon as its page closes: `h1_allow_half_closed(false)`

```yaml
---
id: CRP-0050
mission: MB-0057
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: low
state: open
runbook_trigger: "follow-up named in MB-0057's Continuity Pack (a server-wide HTTP setting is outside RR-0049's Covers → platform); feature.md escalation trigger `crates/kauppa-web/src/main.rs` (security:session-middleware; the session block itself doesn't change); RR-0049 constraint 5 (stream limits and shutdown)"
---
```

## Decision statement

Should the HTTP server stop serving an HTTP/1 connection as soon as the
client closes it (`HttpServer::h1_allow_half_closed(false)` in `main.rs`),
so that a live stream frees its slot the moment its page closes?

## Options and trade-offs

### Option A — Turn half-closed connections off, and remove the early keep-alive

`main.rs` adds `.h1_allow_half_closed(false)` to the `HttpServer` builder.
`LiveHub::reserve` no longer makes the user's other streams write a
keep-alive comment at once (the MB-0057 workaround), and its test changes
with it.

- Pros:
  - A closed page's stream is freed at once (measured: under 1 ms, against
    more than 3 s today; see the evidence bundle), whatever page the user
    opens next and whoever's it is.
  - Simpler hub code: one workaround and its explanation go.
  - The same freeing works for every user, not only for the one who
    connects again.
- Cons:
  - Server-wide: a client that shuts down its sending side after the
    request and then waits for the answer (`shutdown(SHUT_WR)`) gets no
    answer. Browsers, Caddy, curl, the Playwright suite and
    `deploy/healthcheck.sh` don't do that; some scripted clients can.
  - Depends on the proxy passing the close on. Caddy is expected to close
    its upstream connection when the browser leaves (Go's reverse proxy
    cancels the upstream request), but this wasn't checked here: Docker
    didn't answer on this machine.
- Risk: low. If Caddy doesn't pass the close on, slots go back to being
  freed only by keep-alive writes (20 s), as before MB-0057's workaround,
  and quick page changes could reach `STREAMS_PER_USER` (8) again.
- Cost / timeline: one line in `main.rs`, about 15 lines out of `live.rs`,
  one test reworded; a check through Caddy after the deploy.

### Option B — Turn half-closed connections off, and keep the early keep-alive

As A, but `reserve` keeps the early keep-alive.

- Pros:
  - A's freeing at once, and the workaround stays in case a proxy doesn't
    pass the close on.
- Cons:
  - Two mechanisms for one problem; the workaround's explanation in
    `live.rs` has to say when each one applies.
  - The same server-wide cost as A.
- Risk: low, and lower than A behind Caddy.
- Cost / timeline: one line in `main.rs`, a comment in `live.rs`.

### Option C — Leave it as it is

- Pros:
  - No change; RR-0049 and its implementation already meet P7 (the owner's
    check through Caddy passed).
- Cons:
  - A left page's stream keeps its slot until that user connects twice
    more, or until two keep-alives (up to about 40 s) if they don't.
  - The workaround stays only for the user who connects; another user's
    left page isn't freed sooner by it.
- Risk: none new.
- Cost / timeline: none.

## Recommendation and question

Recommended option: **B**.

Because: it gives the freeing at once that A gives, and keeps today's
safety net until the close through Caddy is proven. A later change can
remove the workaround (A) once the check through Caddy shows the slot is
freed when a page closes. The server-wide cost is the same for A and B,
and no client of this app half-closes.

Crisp question to answer: **A, B or C?** With A or B, also: is the check
through Caddy after the deploy (open a list page, close it, see "live
stream closed" with reason `client_gone` within a second in `docker
compose logs kauppa`) enough to record the result in MB-0057's pack?

## Blast radius and rollback

What can break: a client that half-closes its connection while waiting
for an answer gets none. No page, form or stream of the app does this.
Who is affected: no one known: browsers, Caddy and the healthcheck keep
their sending side open.
Rollback plan: remove the one `main.rs` line (and with A, revert the
`live.rs` change); no data, schema, session or deployment change.

## Minimal context

- RR-0049 sub-answer 5 and constraint 5: `STREAMS_PER_USER` 8,
  `STREAMS_TOTAL` 200, `KEEP_ALIVE` 20 s, `STREAM_MAX_AGE` 15 min; a
  failed keep-alive write frees the slot; open streams are never evicted.
  None of the four constants changes here.
- MB-0057's Continuity Pack, "Decisions and constraints": actix-web learns
  that a client is gone only when a write fails, and the first write after
  the close still succeeds. The implementation made a user's connect write
  a keep-alive on their other streams, inside RR-0049.
- actix-web 4.18.2's `HttpServer::h1_allow_half_closed` documentation:
  "Setting this to `false` will cause the server to abort the connection
  handling as soon as it detects an EOF from the client. The default
  behavior is to allow, i.e. `true`." In actix-http 3.20.3
  (`h1/dispatcher.rs`), the read-side EOF starts the connection's shutdown
  while a response is still being sent only when this is `false`.
- `crates/kauppa-web/src/main.rs` is a security trigger path for its
  session block; that block, the identity middleware and the shutdown task
  don't change.
- Not in scope: `deploy/`, the constants, `client_disconnect_timeout`,
  HTTP/2.

## Evidence bundle (Hardened)

- Measured 2026-11-17 on `origin/main` (`9f64917`, after #124), release
  build, in a scratch worktree that read the setting from an environment
  variable (not committed). Three `curl -N` streams on one list, the clients
  killed together, then the log read 3 s later:
  - today (`true`): no "live stream closed" line within 3 s;
  - `false`: three "live stream closed" lines, reason `client_gone`, within
    1 ms of the kill.
- With `false` the same run registered, signed in, created a list over
  POST and answered `/healthz`, as before.
- `deploy/healthcheck.sh` writes its request on a read-write descriptor and
  doesn't close the sending side before it reads.
- Not measured: the behaviour through Caddy (2.11.9, `deploy/compose.yaml`);
  Docker didn't answer on this machine.
