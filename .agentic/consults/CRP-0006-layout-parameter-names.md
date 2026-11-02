# Consultation Request Pack — Rename the layout partial's parameters (MB-0016 Option A)

```yaml
---
id: CRP-0006
mission: MB-0016
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: open
runbook_trigger: "MB-0016 autonomy.escalate (changing the documented layout convention, AGENTS.md → Conventions and .agentic/mentorship/templating.md); AGENTS.md → Agent role (never modify Mentorship Packs without explicit approval) and Lifecycle (agent-proposed changes go through a CRP and an RR); feature.md escalation trigger `.agentic/{mentorship,runbooks,resolutions}/**`"
---
```

## Decision statement

Change the page-layout convention from `{{#> layout title=… signed_in=…}}` to
names that page bodies can't collide with, so the layout's parameters stop
shadowing view fields (MB-0016). This updates AGENTS.md → Conventions and
`templating.md` to match.

The owner chose Option A for MB-0016 on 2026-11-01. This pack records that
choice and the one open sub-question, for the RR that closes it.

## Options and trade-offs

### Option A — rename the layout's parameters (owner's choice)

`layout.hbs` reads namespaced names, and every page passes them. Bodies keep
natural field names such as `title`.

- Pros:
  - removes the trap at its source;
  - a domain or view field is very unlikely ever to be named `page_*`;
  - the P5 guard test pins "body fields are never shadowed".
- Cons:
  - touches all ten `{{#> layout …}}` invocations and `layout.hbs`;
  - changes two governance documents.
- Risk: low. Templates only, no data. Any missed invocation fails loudly:
  strict mode errors on the missing `page_title`.
- Cost / timeline: one PR (MB-0016), after this pack's RR is approved.

#### Sub-question: `signed_in` too?

- **A1 — rename both (recommended):** `page_title` and `page_signed_in`.
  - One rule: every layout parameter starts with `page_`.
  - `signed_in` can't hit the same trap later.
  - `templates/index.hbs` passes no `signed_in` today; the layout reads it
    from the handler's context. So index must pass
    `page_signed_in=signed_in` explicitly, which makes that dependency
    visible.
- **A2 — rename only `title` → `page_title`:** a smaller diff, but it leaves
  `signed_in` as a latent collision and makes the rule "except `signed_in`".

### Option B — rename the colliding view fields (rejected by the owner)

Fields such as `title` → `name`/`draft_title`. No convention change, but the
next field named `title` silently breaks again.

## Recommendation and question

Recommended: **Option A, sub-option A1.**

Crisp question to answer: **Approve Option A with the `page_` prefix for both
layout parameters (A1)?** That covers:

- `templates/layout.hbs` reading `{{page_title}}` and `{{#if page_signed_in}}`;
- every `{{#> layout …}}` invocation updated;
- AGENTS.md → Conventions: "Each page is `{{#> layout page_title="..."}}...{{/layout}}`";
- `.agentic/mentorship/templating.md` → Handlebars: the same convention, plus
  a red flag: "naming a layout parameter like a view field — partial hash
  params become block params and shadow body fields on full pages".

## Blast radius and rollback

- **What can break:** page titles and the signed-in navigation on full pages.
  Both are covered by MB-0016's tests (P1–P4 assert `<title>`) and by strict
  mode (a missed rename is a render error, not a silent wrong value).
- **Who is affected:** every full-page render. htmx fragments don't use the
  layout.
- **Rollback plan:** revert the MB-0016 PR. There are no data or schema
  changes.

## Minimal context

- Root cause (MB-0016 → Context): handlebars 6.4.4 sets each partial hash
  entry as a block param of the partial's block (`src/partial.rs`,
  `set_block_param`). Block params are resolved before the context and
  inherited by nested blocks, so `title` and `this.title` in a page body
  resolve to the layout's `title`.
- Today's convention text: AGENTS.md line 115 (`{{#> layout title="..."}}`),
  and `templating.md` lines 7–8
  (`{{#> layout title="…" signed_in=…}}…{{/layout}}`).
- The ten invocations: `index`, `auth/login`, `auth/register`,
  `categories/index`, `products/index`, `recipes/index`, `recipes/show`,
  `lists/index`, `lists/show`, `lists/shop`.

## Evidence bundle (Hardened)

- Repro (curl against the built binary, fresh DB, 2026-11-01): MB-0016 →
  Context, rows 1–4, with their correct htmx counterparts 1′ and 4′.
- `grep -rn 'title' templates`: the only body references to `title` are the
  four affected sites plus `recipes/show.hbs`'s `<h1>`, which is correct by
  coincidence (`title=title`).
