# Resolution Record — Server-side password length: 15–128 Unicode scalar values, checked before hashing

```yaml
---
id: RR-0022
closes: CRP-0022
mission: MB-0034
decided_by: "@juuso.kalliomaki"
role: security
risk_tier: medium
decided_at: 2026-11-10T08:23:00Z
status: approved
superseded_by: null
rolls_back_into:
  - .agentic/mentorship/security.md
  - .agentic/missions/MB-0034-server-side-password-length.md
  - .agentic/consults/CRP-0022-server-side-password-length.md
---
```

## Decision and outcome

Approve CRP-0022's recommendation on every question: `POST /auth/register`
refuses a new password of fewer than **15** or more than **128** Unicode
scalar values, counted on the string as submitted, with a pure check in a
`kauppa-core` domain module that runs after the token-shape check and
before `password::hash`.

Sub-answers:

1. **Minimum (Q1 B).** `PASSWORD_MIN_CHARS = 15`, per NIST SP 800-63B
   revision 4, Sec. 3.1.1.2 (single-factor passwords: at least 15). The
   page's `minlength` changes from 12 to 15.
2. **Maximum (Q2 B).** `PASSWORD_MAX_CHARS = 128`.
3. **Counting (Q3).** `password.chars().count()` (Unicode scalar values) on
   the password exactly as submitted. No normalization, no trimming, no
   other transformation: the check counts what `password::hash` hashes.
   Leading, trailing and inner spaces count. NFC normalization (hash and
   verify) is a possible follow-up brief, not part of MB-0034.
4. **Placement (Q4 A).** A new `kauppa-core` domain module (name
   `may_decide`, e.g. `domain/password_rule.rs`) exporting
   `PASSWORD_MIN_CHARS`, `PASSWORD_MAX_CHARS` and a pure check
   (`check_new_password_length(&str) -> Result<(), PasswordLengthError>`)
   whose error is a field-less enum (`TooShort`, `TooLong`), **not**
   `CoreError::Invalid`, so it can't reach `error.rs`'s WARN log or plain
   body. `password.rs` is untouched.
5. **Order (Q5).** `register_submit`: limiter → form shape → token shape →
   **length** → `password::hash` → `InvitationRepo::register`.
   - A missing or malformed token gets the invalid-invitation page, as
     today.
   - A well-formed token with an out-of-bounds password gets the length
     page whether the invitation is valid, unknown, expired, revoked or
     used. **This is consistent with RR-0012 constraint 4**: the check reads
     no stored data, so for any given password the four invalid stored
     cases (unknown, expired, revoked, used) still share status, body and
     code path (all the length page, or all the invalid-invitation page),
     and the missing/malformed cases keep the separate input-only path
     RR-0012 sub-answer 7 already allows. RR-0012 sub-answer 7's "a
     well-formed token always pays one hash before the transaction" holds
     for every request that reaches the transaction.
   - A length rejection is a 4xx and counts as a register failure
     ("invalid input", RR-0013 sub-answer 1). RR-0013 is unchanged.
6. **Response and logging (Q6).** **400**, rendered through
   `render_register`, with the hidden `invite` field, `display_name` and
   `email` kept and the password field empty. One message naming both
   bounds, built from the constants (wording `may_decide`, e.g. "Choose a
   password of 15 to 128 characters."), the same for too short and too
   long. At most one field-less DEBUG line (e.g. `tracing::debug!
   ("registration rejected: password length")`). Never the password, its
   length or any part of it, at any level.
7. **Template (Q7).** `RegisterView` carries the minimum from
   `PASSWORD_MIN_CHARS`, set in `render_register`; `register.hbs` uses
   `minlength="{{…}}"` and loses the literal `12`. **No `maxlength`**
   (browsers truncate pasted passwords silently). P5's test checks both:
   the rendered `minlength` equals `PASSWORD_MIN_CHARS`, and the page has no
   `maxlength` (and the template no numeric `minlength`/`maxlength`
   literal).

**Login is unaffected (P4).** `POST /auth/login` never applies the rule;
existing accounts keep working whatever their password's length.

Covers:

- MB-0034's implementation PR, P1–P5 as amended below;
- the new `kauppa-core` domain module and its unit tests;
- the `register_submit` / `register` change in
  `crates/kauppa-web/src/routes/auth/mod.rs`, `RegisterView` and
  `templates/auth/register.hbs`;
- tests in `routes::auth::tests::password_length`;
- the `security.md` rule and brief deltas below.

Does not cover:

- any change to `password.rs`, Argon2 parameters or the hash format;
- NFC (or any) normalization, trimming, or any change at login;
- strength rules beyond length (character classes, breached-password lists,
  whitespace-only or repeated-character passwords);
- password change or reset flows (they adopt the same constants when a
  later brief adds them);
- a `FormConfig` / form size limit change (actix-web's 32 KiB default
  stays);
- any new dependency (`unicode-normalization`, `unicode-segmentation`);
- any change to RR-0012's or RR-0013's responses, limits or counting.

## Rationale and trade-offs

- **15 (Q1 B) — chosen.** kauppa-rs has passwords only, so they are
  single-factor and NIST revision 4's SHALL is 15. The cost falls only on
  new registrations; existing accounts and login are untouched. Not chosen:
  A (12, the form today; below the standard), C (8, multi-factor floor).
- **128 (Q2 B) — chosen.** Argon2id time doesn't depend on input length
  (CRP-0022's measurement: 12 B to 1 MiB all within noise), so the maximum
  is a contract and a bound on what is hashed, not a CPU guard. 128 covers
  generated passwords and passphrases. Not chosen: 64 (NIST's floor; pure
  friction), 1024 (no benefit).
- **Scalar values, as submitted (Q3) — chosen.** NIST counts code points;
  no dependency. Not chosen: bytes (an "ä" counts 2, weakening the minimum
  for Finnish text), grapheme clusters (new dependency, Unicode-version
  drift). No NFC now: it changes what is hashed and needs `verify` and a new
  dependency, close to the brief's KDF non-goal. No trimming: login doesn't
  trim, so trimming at registration would break accounts.
- **Domain module (Q4 A) — chosen.** Business rules live in core, one
  source for handler, template and later flows, and tuning the bounds
  doesn't touch the KDF module. Not chosen: B (`password.rs`; `hash` also
  builds `DUMMY_HASH`, and `verify` must never apply it), C (handler; an
  acceptable fallback, but the constants would live in `kauppa-web`).
- **After token shape, before hash (Q5) — chosen.** No database read and
  no Argon2 for a rejected request, and no oracle (sub-answer 5). Not
  chosen: length before token shape (the no-invitation page has no form),
  length after the invitation is known valid (reveals validity, hashes
  first).
- **One message, register page (Q6) — chosen.** Same shape as the
  duplicate-email page (RR-0012 sub-answer 4); the plain `CoreError` body
  would drop the invitation and log through WARN.
- **No `maxlength` (Q7) — chosen.** Silent paste truncation would create
  accounts whose saved password then fails at login. The browser's
  UTF-16 `minlength` may let through a password the server refuses (non-BMP
  characters), never the reverse; the server message covers it.

## Approvals and authority

Approved by security-owner @juuso.kalliomaki.

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0022-server-side-password-length.md`
- Mission Brief: `.agentic/missions/MB-0034-server-side-password-length.md`
- Continuity Pack: `.agentic/continuity/MB-0034.md`
- Related: RR-0012 (sub-answers 4, 5, 7; constraint 4; "Does not cover"
  lists this follow-up), RR-0013 (sub-answers 1 and 7: limiter first, 4xx
  counted).
- Evidence links: CRP-0022 → Current state and Evidence bundle:
  - register flow `crates/kauppa-web/src/routes/auth/mod.rs:209-336` and
    `templates/auth/register.hbs:12` at 50322c0;
  - `error.rs:34`, `43-50` (Invalid → 400, WARN with `error = ?self`);
  - actix-web 4.18.2 form `DEFAULT_LIMIT = 32_768`;
  - the Argon2id timing table by input length;
  - NIST SP 800-63B-4 Sec. 3.1.1.2, read 2026-11-09 at
    https://pages.nist.gov/800-63-4/sp800-63b.html.

## Constraints and follow-on rules (Hardened)

1. **One rule, one place.** New passwords are length-checked only through
   the `kauppa-core` domain check and its two constants; the handler and
   the template read the constants, never literals. Any future flow that
   sets a password (change, reset, admin) uses the same check.
2. **Never at login.** `POST /auth/login` and `password::verify` never apply
   the rule.
3. **Count what is hashed.** Unicode scalar values of the submitted string,
   untransformed. Adding normalization or trimming needs a new consult and
   RR (it changes what is hashed).
4. **Before cost, after token shape.** The check runs after the limiter,
   form shape and token-shape checks and before `password::hash` and any
   database work. Moving it after the invitation lookup is a review failure
   (it would reveal invitation validity).
5. **No password data in logs or responses.** Not the password, its length
   or any part of it; at most one field-less DEBUG line. The check's error
   type carries no data.
6. **No `maxlength`.** The register page sets `minlength` from
   `PASSWORD_MIN_CHARS` and no `maxlength`.
7. **Changing the bounds** is a reviewed code change citing this RR, with a
   security consult if the minimum goes down.

## Roll-back into the system (Hardened)

To be applied in MB-0034's implementation PR:

- `.agentic/mentorship/security.md` → Rules (must), after
  **Registration**, add:

  > - **Password length.** New passwords are 15 to 128 Unicode scalar
  >   values, counted on the string exactly as submitted (no
  >   normalization, no trimming), checked on the server by the
  >   `kauppa-core` domain rule (`PASSWORD_MIN_CHARS`,
  >   `PASSWORD_MAX_CHARS`) after the token-shape check and before
  >   `password::hash` (MB-0034, RR-0022). Forms take `minlength` from
  >   the same constant and set no `maxlength`. Login never applies the
  >   rule. The password and its length are never logged or echoed.

- `MB-0034`:
  - `resolutions: []` → `[RR-0022]`;
  - P1: "(12 characters, as the form says, unless the RR sets another)" →
    "(15 characters, RR-0022)";
  - P3: "(characters, not bytes, unless it says otherwise)" → "(Unicode
    scalar values on the string as submitted, RR-0022)";
  - P5: "The register page's `minlength` (and `maxlength`, if any) come from
    the same constants as the server check." → "The register page's
    `minlength` comes from the same constant as the server check, and the
    page has no `maxlength` (RR-0022)."
- `CRP-0022`: `state: open` → `answered`.
- `.agentic/continuity/MB-0034.md` (agent-owned, no consult): records the
  NFC normalization follow-up brief candidate.

## Supersession and history

Status: active once signed. Supersedes nothing. Closes the server-side
password length follow-up listed in RR-0012 and RR-0013 "Does not cover".
Reads RR-0012 constraint 4 as stated in sub-answer 5; RR-0012's decision is
unchanged.
