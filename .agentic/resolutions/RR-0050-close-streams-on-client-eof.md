# Resolution Record — Free a live stream as soon as its page closes: `h1_allow_half_closed(false)`, the early keep-alive kept

```yaml
---
id: RR-0050
closes: CRP-0050
mission: MB-0057
decided_by: "@Jurzo"
role: platform
risk_tier: low
decided_at: 2026-11-17t13:21:00z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0057-live-list-updates.md
  - .agentic/consults/CRP-0050-close-streams-on-client-eof.md
---
```

## Decision and outcome

Approve CRP-0050 Option B.

1. **The setting.** `crates/kauppa-web/src/main.rs` adds
   `.h1_allow_half_closed(false)` to the `HttpServer` builder, with a
   comment naming MB-0057 and RR-0050. Nothing else in `main.rs` changes:
   the session block, the identity middleware and the shutdown task stay
   as they are.
2. **The early keep-alive stays.** `LiveHub::reserve` keeps making the
   user's other streams write a keep-alive at once. Its doc comment says
   that a closed client is now noticed at once when its close reaches the
   server, and that the early keep-alive covers a proxy that doesn't pass
   the close on.
3. **The check.** After the deploy, the owner opens a list page, closes
   it, and sees "live stream closed" with reason `client_gone` within a
   second in `docker compose logs kauppa`. That is enough to record the
   result in MB-0057's Continuity Pack.
4. **Gates.** The usual gates, including `scripts/browser/run.sh`, pass
   with the setting on.

Covers:
- the `main.rs` line and the `live.rs` comment above;
- the check through Caddy above;
- the roll-back below.

Does not cover:
- removing the early keep-alive (CRP-0050 Option A); that is a later
  change once the check through Caddy has passed;
- a change to `STREAMS_PER_USER`, `STREAMS_TOTAL`, `KEEP_ALIVE` or
  `STREAM_MAX_AGE` (RR-0049 constraint 5);
- `client_disconnect_timeout`, other server settings, HTTP/2, `deploy/`
  or the session block.

## Rationale and trade-offs

- Measured: with the setting off, three closed streams were freed within
  1 ms; today none was freed within 3 s.
- Keeping the early keep-alive leaves today's safety net in place until
  the close through Caddy is proven.
- The cost is server-wide: a client that closes its sending side and then
  waits for the answer gets none. No browser, Caddy, curl, the browser
  suite or `deploy/healthcheck.sh` does that.
- Rejected:
  - A, removing the early keep-alive now: not yet proven through Caddy;
  - C, no change: a left page's stream keeps its slot for up to about
    40 s, and the early keep-alive frees only the streams of a user who
    connects again.

## Approvals and authority

@Jurzo as platform.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0050-close-streams-on-client-eof.md`
  (its evidence bundle)
- Mission Brief: `.agentic/missions/MB-0057-live-list-updates.md`
- Continuity Pack: `.agentic/continuity/MB-0057.md`
- Related: RR-0049 (sub-answer 5, constraint 5).

## Constraints and follow-on rules (Hardened)

1. **One setting.** Only `h1_allow_half_closed(false)` changes in the
   server's configuration. Any other server setting needs its own consult.
2. **The safety net stays** until the check through Caddy passes and a
   later change removes it with the owner's OK.
3. **Rollback** is removing the one line; no data, schema, session or
   deployment change is involved.

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0057`: `resolutions: [RR-0049, RR-0050]`.
- `CRP-0050`: `state: open` → `answered`.

## Supersession and history

Status: active once signed. Amends nothing; supersedes nothing.
