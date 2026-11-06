# Resolution Record — Rename the layout partial's parameters

```yaml
---
id: RR-0006
closes: CRP-0006
mission: MB-0016
decided_by: "@juuso.kalliomaki"
role: platform
risk_tier: low
decided_at: 2026-11-06T20:49:42Z
status: approved
superseded_by: null
rolls_back_into:
  - AGENTS.md
  - .agentic/mentorship/templating.md
  - .agentic/missions/MB-0016-layout-title-shadowing.md
  - .agentic/consults/CRP-0006-layout-parameter-names.md
---
```

## Decision and outcome

Approve CRP-0006 **Option A, sub-option A1**. The layout partial's parameters
take a `page_` prefix: `{{#> layout page_title="…" page_signed_in=…}}`, and
`templates/layout.hbs` reads `{{page_title}}` and `{{#if page_signed_in}}`.

Covers:

- `templates/layout.hbs` and all ten `{{#> layout …}}` invocations;
- `templates/index.hbs` passing `page_signed_in=signed_in` explicitly;
- the roll-back edits to AGENTS.md and `templating.md` listed below.

All of it ships in MB-0016's fix PR, with the P1–P5 tests committed failing
first (bugfix runbook).

Does not cover: any change to page-title text or format, handlebars upgrades,
strict mode, or other template work.

## Rationale and trade-offs

- **A — chosen.** It removes the shadowing at its source. Handlebars 6.4.4
  turns a partial's hash params into block params, which resolve before the
  context in every nested block (MB-0016 → Context). With a `page_` prefix,
  page bodies keep natural field names (`title`), and no view field will
  plausibly be named `page_*`.
- **A1 over A2.** One rule — every layout parameter starts with `page_` —
  and `signed_in` can't hit the same trap later. The home page's reliance on
  the leak becomes an explicit `page_signed_in=signed_in`.
- **B — rejected by the owner (2026-11-01).** Renaming the colliding view
  fields avoids a convention change, but the next field called `title` would
  silently break again.

## Approvals and authority

Approved by platform-owner @juuso.kalliomaki

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0006-layout-parameter-names.md`
- Mission Brief: `.agentic/missions/MB-0016-layout-title-shadowing.md` (PR #19)
- Evidence links:
  - MB-0016 → Context: the repro table (rows 1–4 wrong on full pages, 1′
    and 4′ correct as htmx fragments);
  - `handlebars-6.4.4/src/partial.rs` (`set_block_param` for each hash
    entry).

## Constraints and follow-on rules (Hardened)

1. **Layout parameters are prefixed `page_`.** A page passes only
   `page_title` and, where it applies, `page_signed_in`. A new layout
   parameter needs a `page_` name, too.
2. **Bodies don't read layout parameters.** A value a page body needs comes
   from its own view (e.g. `index.hbs`'s `signed_in`), never from the
   layout's parameters.
3. **Full pages are tested, not only fragments.** A page whose body renders
   view fields has at least one test on the full-page response (MB-0016
   P1–P4). The P5 guard (`templates::layout_params_do_not_shadow_body`) stays
   in the suite. **confirmed by platform-owner @juuso.kalliomaki**
4. **Strict mode stays on.** It is what turns a missed rename into a render
   error rather than a silently wrong value.

## Roll-back into the system (Hardened)

To be applied in MB-0016's fix PR:

- `AGENTS.md` → Conventions → **Templates**: "Each page is
  `{{#> layout page_title="..."}}...{{/layout}}`. Keep layout markup in one
  place. Layout parameters are prefixed `page_` (RR-0006)."
- `.agentic/mentorship/templating.md` → Handlebars:
  - convention line → "Every page is
    `{{#> layout page_title="…" page_signed_in=…}}…{{/layout}}`";
  - a new item under Red flags: "A layout parameter named like a view field.
    Partial hash params become block params and shadow same-named body
    fields on full pages (RR-0006)."
- `MB-0016`: `resolutions: [RR-0006]`.
- `CRP-0006`: `state: open` → `answered`.

## Supersession and history

Status: active. Supersedes nothing.
