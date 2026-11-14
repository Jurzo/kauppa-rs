# Resolution Record — Theme switch: the `kauppa_theme` cookie and `POST /theme`, repainting by `:has()`, icon-only segments in the `<nav>`, one 60rem breakpoint

```yaml
---
id: RR-0035
closes: CRP-0035
mission: MB-0055
decided_by: "@Jurzo"
role: security
risk_tier: medium
decided_at: 2026-11-14T07:50:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0055-theme-toggle-and-top-bar.md
  - .agentic/consults/CRP-0035-theme-switch-and-top-bar.md
  - .agentic/mentorship/security.md
  - design-system/README.md
---
```

## Decision and outcome

Approve CRP-0035's **recommended set, questions 1–6** (owner, 2026-11-14:
"All as recommended").

Sub-answers:

1. **Cookie (Q1, security).** `kauppa_theme`, holding `light` or `dark`.
   - Choosing System removes it (`Max-Age=0`). No cookie, or any value
     other than `light` or `dark`, reads as System.
   - `Path=/`, `SameSite=Lax`, `HttpOnly`, `Secure` per
     `KAUPPA_COOKIE_SECURE`, `Max-Age` one year: the same as `kauppa_lang`.
   - Unsigned. The server maps the value to an enum; templates see only
     `system`, `light` or `dark`.
   - Not stored on the account.
2. **Route (Q2, security and api-owner).** `POST /theme`.
   - Registered in `routes::protected`, behind the cross-site check
     (RR-0010).
   - Form fields `theme` (`system` | `light` | `dark`) and `return`. Any
     other `theme` is a 400 and sets nothing.
   - With `HX-Request: true`: `204 No Content`, no body. Otherwise: `303`
     to `same_site_path(return)`. Both set or remove the cookie.
   - `same_site_path` is imported from `routes/language.rs`, not copied.
   - The theme reaches templates as the root-context value
     `request_theme`, set in `Ui::render` (`i18n/request.rs`) beside
     `request_path`.
   - No GET changes the theme; no `?theme=`. No log line of its own.
3. **Repaint (Q3 A, owner).** CSS keyed on the checked radio.
   - In `assets/app.css`, the dark token blocks' selectors become
     `@media (prefers-color-scheme: dark) { :root:not(:has(<light radio>:checked)) }`
     and `:root:has(<dark radio>:checked)`. Light stays on `:root`. Token
     values don't change.
   - `data-theme` is dropped from `app.css`; the server renders no
     attribute on `<html>`. First paint comes from the saved radio being
     rendered `checked`.
   - Browsers without `:has()` follow the OS whatever is chosen. Accepted.
   - `design-system/tokens.css` keeps its `data-theme` selectors;
     `design-system/README.md` says how the app maps them.
   - The `theme-color` meta follows the saved choice on a full load (one
     meta for Light or Dark, the two media-keyed metas for System). After
     an htmx-only change it updates on the next full load. Accepted.
4. **Shape on wide screens (Q4 A, owner).** Three icon-only segments, the
   baseline's compact variant. Each option keeps its label for assistive
   technology (visually hidden) and a `title`.
5. **Phone placement (Q5 A, owner).** One switch per page, inside the
   `<nav>`, as its last group.
   - Wide screens: inline in the top bar with the rest of the `<nav>`,
     icon-only. CSS `order` may place it before sign in/out.
   - The menu panel: the legend shows as a visible small heading, and the
     three options are labelled rows, each at least 48 px tall.
   - The 320 px top bar keeps brand, language selector and menu button,
     unchanged (RR-0031 sub-answer 4).
6. **Breakpoint (Q6, owner).** One shared breakpoint for the navigation
   panel, **60rem**.
   - The `max-width: 39.9375rem` blocks that belong to the navigation
     become `max-width: 59.9375rem`.
   - Above the breakpoint, the inline `<nav>` is `flex-wrap: nowrap`, only
     inside `@supports selector(:popover-open)`. Without popover support
     it keeps `wrap`.
   - The language selector's short name stays below 40rem.
   - If the real markup measures wider than 60rem, the implementation
     takes the widest measured bar plus a margin and records it. Above
     64rem it stops and asks (MB-0055 `escalate`).

Covers:

- MB-0055's implementation, P1–P10;
- the new `kauppa_theme` cookie, `POST /theme` (`routes/theme.rs`), the
  `request_theme` value in `i18n/request.rs`;
- the changes to `layout.hbs`, `app.css`, the catalogs and the tests that
  the sub-answers name, including the updated selectors in
  `templates/tests/design.rs`;
- the roll-backs below.

Does not cover:

- storing the theme on the account, or any schema change;
- a script of the app's own, `hx-on`, or a change to the htmx config;
- any change to `csp.rs` or the other security headers;
- changing a token value, or `design-system/tokens.css`;
- moving `same_site_path` out of `routes/language.rs`;
- a breakpoint above 64rem, or hiding an item of the 320 px top bar.

## Rationale and trade-offs

- **Cookie, Light or Dark only.** "No cookie" already means System, so a
  third value adds a state and nothing else. Unsigned is enough: a forged
  value can only choose one of the three themes.
- **`:has()` over a reload or a script.** The click repaints at once and
  first paint is right, with no JavaScript and no change to the policy. A
  reload per click is slow on a phone (goal 2). A script of the app's own
  would set a precedent against AGENTS.md's `assets/` rule. Dropping
  `data-theme` avoids a stale attribute on `<html>` after a boosted
  navigation.
- **Icon-only segments.** The owner's baseline, one tap, the current choice
  always visible. It costs the inline nav on portrait tablets (60rem).
  Labelled segments would push the breakpoint to 70rem; a popover button
  needs two taps.
- **Inside the `<nav>`.** One switch for `:has()` to read. The phone top
  bar has no room.

## Approvals and authority

@Jurzo as security for Q1 and Q2; api-owner for Q2's route; product
(owner) for Q3–Q6.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0035-theme-switch-and-top-bar.md`
- Mission Brief: `.agentic/missions/MB-0055-theme-toggle-and-top-bar.md`
- Related: RR-0010 (cross-site check), RR-0019 (CSP, unchanged), RR-0023
  (`kauppa_lang`, the model), RR-0006 (`page_` prefix), RR-0031
  (sub-answer 4), RR-0034 (tokens).
- Evidence: CRP-0035 → Evidence bundle (top bar widths measured in
  headless Edge 157; htmx's `allowEval` gate around `hx-on`).

## Constraints and follow-on rules (Hardened)

1. **Only `POST /theme` changes the theme.** No GET, no query parameter,
   no script.
2. **One theme switch per page.** `:has()` reads its checked radio; a
   second copy would make the page's theme ambiguous.
3. **`app.css` picks the theme only by `:has()` on that radio.** No
   `data-theme` selector in `app.css`, and nothing renders one.
4. **Token values stay `tokens.json`'s** (RR-0034 constraint 6). The
   theme work changes selectors only.
5. **One navigation breakpoint.** Every rule that switches between inline
   nav and menu panel uses the same value.
6. **The 320 px top bar holds brand, language selector and menu button**
   (RR-0031 sub-answer 4), and nothing else.

## Roll-back into the system (Hardened)

To be applied after this record is signed, in the consult's PR or
MB-0055's first implementation PR:

- `MB-0055`:
  - `resolutions: []` → `[RR-0035]`.
  - P3: "the server renders `data-theme`…" → "the server renders the
    saved choice's radio `checked`, and `app.css` applies the theme from
    it, so first paint is already right; nothing is rendered on
    `<html>`". The `theme-color` sentence stays.
  - P4: "by the mechanism the owner chooses (S1)" → "by CSS keyed on the
    checked radio (`:has()`)".
  - P6: "Finnish, signed in, is expected to be the widest" → "Finnish,
    signed out, measured widest in CRP-0035"; add "60rem unless the real
    markup measures wider".
  - P7: "where the owner chooses (S1): inside the menu panel or in the top
    bar" → "inside the menu panel, as a headed group of three labelled
    rows".
  - `escalate`: remove the cookie-and-route, `POST /theme`, repaint
    mechanism and phone placement items (answered here); keep the 64rem,
    account storage, brand icon and CSP items.
  - Context: the "The owner chooses the replacement (S1)" and "Finnish
    labels" sentences record the answers. Conceptual plan step 1 gains
    "Answered by RR-0035."
- `CRP-0035`: `state: open` → `answered`.
- `.agentic/mentorship/security.md:29`: "The one other cookie is
  `kauppa_lang`…" → names both `kauppa_lang` and `kauppa_theme`
  (RR-0035), with the same attributes, both unsigned, and neither holding
  anything a forged value could abuse.
- `design-system/README.md`, the theming paragraph: the app applies dark
  from `:has()` on the theme switch's checked radio (and from the OS when
  System is chosen), not from a `data-theme` attribute; `tokens.css`
  keeps `data-theme` as the portable form.

## Supersession and history

Status: active. Superseded by: none.
