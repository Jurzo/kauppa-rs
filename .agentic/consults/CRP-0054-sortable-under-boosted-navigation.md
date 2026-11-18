# Consultation Request Pack — Drag to reorder under boosted navigation: where Sortable is loaded

```yaml
---
id: CRP-0054
mission: MB-0066
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: open
runbook_trigger: "MB-0066 slow-mode trigger (the connecting code doesn't fit the approved loading); RR-0053 sub-answer 2 and constraint 3 (scripts only where `page_sortable` is set) would change; AGENTS.md change procedure if the rule's wording changes"
---
```

## Decision statement

RR-0053 loads Sortable and `assets/app/sortable.js` only on the three
drag pages, from their `<head>`. Under the app's boosted navigation those
scripts don't load on most visits. Where should they load instead (Q1)?

## Current state

- `<body hx-boost="true">`: links and forms swap the body in place.
  htmx 2.0.11 drops the response's `<head>` on a boosted swap and keeps only
  its `<title>` (its `D()` parser strips `<head>…</head>`); there is no
  head-merging without htmx's separate head-support extension.
- RR-0053 (sub-answer 2, constraint 3): a layout parameter `page_sortable`
  adds the two `<script defer>` tags on the categories, favorites and recipe
  pages only. That is what MB-0066's branch does now.
- **Measured** (Playwright, release binary, the branch): from `/lists`,
  clicking the nav link to Categories gives `typeof Sortable ===
  "undefined"`, no `sortable-ready` class and no handles: only the ↑/↓
  buttons. After a reload of the same URL, Sortable is loaded and dragging
  works. So dragging appears only after a full page load (typing the URL,
  a reload, or opening a new tab), not after a normal in-app click or a
  form's redirect.
- Nothing breaks: without Sortable the handles stay hidden and the ↑/↓
  buttons work, as without JavaScript.
- Caching: `/assets/vendor/*` is served `immutable` for a year (RR-0029);
  other assets revalidate by ETag (304). Caddy doesn't compress responses;
  Sortable is 45,478 bytes (15,009 gzipped, if compression is ever added).

## Q1 — Where the scripts load (platform)

### Option A — Both on every page, from the layout (recommended)

The layout loads Sortable and `assets/app/sortable.js` after the other
three scripts, with SRI, on every page; `page_sortable` goes away. The
script does nothing on a page without a `[data-sortable]` list except mark
`<html>` (`sortable-ready`, which only shows handles).

- Pros:
  - Dragging works however the user reaches the page.
  - The simplest change: two tags in the layout, as htmx and its
    extensions are; the browser fetches Sortable once and keeps it a year.
- Cons:
  - Every first visit downloads 45 KB more (once per browser per year),
    and every page parses it (a few milliseconds). This is RR-0053 Q2 B,
    rejected then for AGENTS.md goal 2 (a slow phone over 3G), before the
    boost problem was known.
- Risk: low.

### Option B — `sortable.js` on every page; it loads Sortable when a list needs it

The layout loads only `assets/app/sortable.js` (about 1.5 KB) on every
page. When htmx loads content with a `[data-sortable]` list and Sortable
isn't there yet, the script adds one `<script src="/assets/vendor/Sortable-1.15.7.min.js" integrity="…">`
element (the path and SRI fixed in the file, pinned by the test) and
connects the lists when it has loaded.

- Pros:
  - Sortable's 45 KB is fetched only by someone who opens a drag page.
  - Works after boosted navigation.
- Cons:
  - More first-party code: a script element created at run time (allowed
    by `script-src 'self'`, no inline code), and a short wait for the
    first drag after the page appears.
  - RR-0053 constraint 2's rule gains "may load the vendored file it names".
- Risk: low.

### Option C — Keep per-page loading; no boost into the three pages

`hx-boost="false"` on every link and form whose result is one of the three
pages.

- Cons: fragile: every redirect or link into those pages (the nav, the
  recipe list, the review's back links, form redirects) must opt out, and a
  missed one silently drops dragging; those navigations also lose boost's
  in-place swap.

### Option D — Keep it as it is

- Cons: dragging appears only after a reload, so most users won't see it.

### Rejected outright — htmx's head-support extension

A fourth vendored htmx file to merge `<head>` on each swap; it would have
to run new `<script>` elements, which the htmx config turns off
(`allowScriptTags: false`, RR-0019).

**Recommended: A.**

## Recommendation and question

Recommended: **Q1 A.**

Because: it makes dragging available wherever the page is reached, with the
least code; the cost is one cached 45 KB download per browser per year.
If that weight matters more, B keeps it to the drag pages at the price of
about 15 more lines of our own JavaScript.

Crisp question to answer:
- **Q1:** every page (A), loaded on demand by `sortable.js` (B), no boost
  into the three pages (C), or as it is (D)?

## Blast radius and rollback

What can break: page load on every page (A, B: one or two more deferred
scripts).
Who is affected: every user.
Rollback plan: revert the layout change; with D nothing changes.

## Minimal context

- RR-0053 sub-answer 2 and constraint 3 (amended by A or B); RR-0029
  (asset caching); RR-0019 (htmx config); AGENTS.md goal 2.
- `templates/layout.hbs`, `templates/sortable/_scripts.hbs`,
  `assets/app/sortable.js`, `crates/kauppa-web/src/templates/tests.rs`
  (`sortable_scripts_pinned`).

## Evidence bundle (Hardened)

- The probe above (a throwaway Playwright flow on the branch, not
  committed): after a boosted click `{"ready":false,"sortable":"undefined",
  "scripts":[htmx, response-targets, sse]}`; after a reload
  `{"ready":true,"sortable":"function"}`.
- htmx 2.0.11 (`assets/vendor/htmx-2.0.11.min.js`): the swap parser removes
  `<head(\s[^>]*)?>[\s\S]*?</head>` from the response and copies only the
  title.
- The rest of MB-0066 is done on the branch and passes every gate: the
  order routes, the templates, the vendored file, the script, 108 + 3 +
  241 tests, the browser suite 15 of 15 (the drag flows use full page
  loads).
