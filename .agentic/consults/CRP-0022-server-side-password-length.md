# Consultation Request Pack — Server-side password length (MB-0034)

```yaml
---
id: CRP-0022
mission: MB-0034
owner: "@juuso-kalliomaki"
target_role: security
risk_tier: medium
urgency: medium
state: answered
runbook_trigger: "feature.md escalation path `crates/kauppa-web/src/routes/auth/` (security). MB-0034 autonomy.escalate: the minimum and maximum, and whether length counts characters, bytes or grapheme clusters (security); putting the rule in `password.rs` rather than the domain layer or the handler (security); the order of the check relative to the rate limiter and the invitation check (RR-0012 sub-answers 2 and 7, constraint 4; RR-0013 sub-answer 1)."
---
```

## Decision statement

Decide how `POST /auth/register` enforces password length on the server:

- the bounds (minimum and maximum);
- how length is counted, and whether the input is normalized or trimmed;
- where the rule lives;
- where the check sits in the register flow, so that it adds no oracle and
  no Argon2 work;
- what the user sees, and what the register page's `minlength` (and
  `maxlength`, if any) are derived from (P5).

**Routing.** Security only. No dependency, schema, session or KDF change is
proposed, so no platform or data-owner sign-off is needed.

## Current state

Verified on `origin/main` `50322c0`. No code was changed. The only build
was a throwaway Argon2 benchmark in the scratchpad (Evidence bundle).

**Register flow** (`crates/kauppa-web/src/routes/auth/mod.rs`):

1. **Limiter** (`mod.rs:215-222`). `register_submit` checks
   `REGISTER_PER_CLIENT` before anything else, including before a malformed
   form is answered (RR-0013 sub-answer 7).
2. **Form shape** (`mod.rs:213`, `223-226`). `web::Form<Registration>`
   (`mod.rs:50-57`). Missing fields fail deserialization and come back as an
   actix 400. `password` is a plain `String`; an empty one is accepted.
3. **Token shape** (`mod.rs:270-273`). `token::is_well_formed` rejects a
   missing or malformed invitation with `invalid_invitation`
   (`mod.rs:319-325`), before any DB work or Argon2 (RR-0012 sub-answer 7).
4. **Hash** (`mod.rs:274-276`). `password::hash` runs for every well-formed
   token, valid or not, so valid and invalid tokens take the same time
   (RR-0012 sub-answer 7).
5. **Consume** (`mod.rs:277-302`). `InvitationRepo::register` in one
   transaction. `Ok(None)` gives the one invalid-invitation page; a
   duplicate email gives 409 on the register page with the invitation kept
   (`mod.rs:291-300`, RR-0012 sub-answer 4).
6. **Counting** (`mod.rs:227-236`). Every 4xx answer, including actix's form
   400, is recorded as a register failure (RR-0013 sub-answer 1: "invalid
   input" is listed).

There is **no length check anywhere**. An empty or 30,000-character
password is hashed and stored if the invitation is valid.

**Page and view.**

- `templates/auth/register.hbs:12`:
  `<input type="password" name="password" required minlength="12" autocomplete="new-password" />`.
  No `maxlength`. The `12` is a literal.
- `RegisterView` (`mod.rs:81-87`) carries `invite`, `error`, `display_name`
  and `email`. Every register response renders through `render_register`
  (`mod.rs:327-336`). The password is never put back into the page: the
  input has no `value`.

**KDF** (`crates/kauppa-core/src/password.rs:14-29`). `hash` and `verify`
pass `password.as_bytes()` to `Argon2::default()` (Argon2id, m = 19 MiB,
t = 2, p = 1; CRP-0013 Evidence). The bytes are hashed as given: no
normalization, no trimming. `argon2` 0.5.3 accepts up to `MAX_PWD_LEN =
0xFFFFFFFF` bytes (`argon2-0.5.3/src/lib.rs:127`, `:549`).

**Input bound.** The app sets no `FormConfig`
(`grep -rn 'FormConfig\|PayloadConfig' crates/` finds nothing). In the
locked actix-web 4.18.2 the default form limit is **32 KiB**, not 16 KiB
(`actix-web-4.18.2/src/types/form.rs:255`, `const DEFAULT_LIMIT: usize =
32_768`; it was 16 KiB in earlier 4.x releases). So today a password can be
up to about 32 KB, all of which is hashed.

**Errors and logging.**

- `CoreError::Invalid(String)` maps to 400, and `error.rs` logs client
  errors at WARN with `error = ?self` and returns the message as a plain
  body (`crates/kauppa-web/src/error.rs:34`, `43-50`). Any message put into
  a `CoreError` is therefore logged. A plain body would also drop the
  register page and the invitation.
- `Registration`'s `Debug` prints no fields (`mod.rs:59-64`).

**Existing tests.**

- Every register test posts `test_support::PASSWORD`, "correct horse
  battery staple", which is 28 characters
  (`crates/kauppa-web/src/routes/test_support.rs:24`;
  `routes/auth/tests/mod.rs:36-46`). It passes any minimum up to 28.
- `csrf/tests/coverage.rs:151` posts `password = "x"` to every route,
  register included, but without `display_name` or `invite`. On register it
  fails the form shape (or the cross-site check) before any length check, so
  it is unaffected.

**Login** (`mod.rs:125-158`) never inspects the password beyond
`password::verify`. P4 and `autonomy.forbidden` keep it that way.

## Reference: NIST SP 800-63B (revision 4)

Read on 2026-11-09 from `https://pages.nist.gov/800-63-4/sp800-63b.html`
(page dated 2025-08-26), Sec. 3.1.1.2 (Password Verifiers):

- **Minimum.** Passwords "used as a single-factor authentication mechanism"
  SHALL be "a minimum of 15 characters in length". Passwords used only as
  part of multi-factor authentication may be shorter, but at least 8.
- **Maximum.** "Verifiers and CSPs SHOULD permit a maximum password length
  of at least 64 characters."
- **Characters.** All printing ASCII and space SHOULD be accepted; Unicode
  SHOULD be accepted.
- **Counting.** "Each Unicode code point SHALL be counted as a single
  character" when evaluating length.
- **Normalization.** SHOULD apply Unicode Normalization Form C (NFC,
  UAX #15 Sec. 12.1) before hashing. (Revision 3, 2017, said NFKC or NFKD;
  revision 4 says NFC.)
- **No truncation.** The entire submitted password SHALL be verified.
- **Trimming.** Verifiers MAY make "limited allowances for mistyping (e.g.,
  removing leading and trailing whitespace characters before verification)"
  if the result still meets the minimum.
- **No composition rules** (SHALL NOT).

kauppa-rs has passwords only: no second factor. Under revision 4 its
passwords are single-factor, so NIST's floor is **15**, not the form's 12.

## Questions

### Q1 — Minimum (security)

#### Option A — 12, as the form says

- Pros: matches the page and the brief's default; nothing a current user
  sees changes.
- Cons: below NIST revision 4's SHALL (15) for single-factor passwords.
- Risk: weaker floor; offset in part by the RR-0013 login limits (10 per
  client, 5 per account, per 15 min), which make online guessing slow, but
  not by anything against offline cracking of a stolen database.

#### Option B — 15, aligned with NIST SP 800-63B-4 (recommended)

- Pros: meets the current standard. The cost falls only on **new**
  registrations: login doesn't apply the rule (P4), and the brief's
  non-goal leaves existing accounts alone. The audience is a household
  registering a few times a year; a 15-character passphrase ("three words
  and a number") is easy to type on a phone.
- Cons: the page's minimum changes from 12 to 15, a small visible change.
  P1's text says 12 "unless the RR sets another", so the brief needs no
  delta.
- Risk: none beyond a little friction at sign-up.

#### Option C — 8 (NIST's multi-factor floor)

- Not appropriate without a second factor. Listed for completeness.

**Recommendation: B, 15.** If you prefer to keep the page as it is, A is
acceptable and the rest of this CRP is unchanged.

### Q2 — Maximum (security)

**What a maximum is for here.** Not CPU. Argon2 feeds the password once
into the initial Blake2b hash H0; the cost is the memory fill (m, t), which
doesn't depend on the input. Measured in release (Evidence bundle), one
`hash_password` takes about the same time for 12 bytes (median 26 ms,
noisy) as for 1 KiB (19 ms), 16 KiB (18 ms), 32 KiB (18 ms) and even 1 MiB
(15 ms). Every value is within run-to-run noise. So P2's "before any Argon2
work" matters because a rejected request should cost nothing, not because
long inputs are expensive. The 32 KiB form limit already bounds the input.

What a maximum does give:

- a clear, testable contract for the form and any future change-password
  flow;
- a bound on what is ever hashed and retained in memory per request;
- one less "accept anything" input on a public endpoint.

#### Option A — 64 characters (NIST's floor for the maximum)

- Pros: the smallest value NIST allows.
- Cons: some password managers generate or users paste longer passphrases;
  rejecting them is pure friction, since length costs nothing.

#### Option B — 128 characters (recommended)

- Pros: twice NIST's floor; covers generated passwords (common managers top
  out around 64–128) and long passphrases. At most 512 UTF-8 bytes.
- Cons: none known.

#### Option C — 1024 characters

- Pros: effectively unlimited for humans.
- Cons: no benefit over B; a larger value invites the question of why it is
  needed.

**Recommendation: B, 128.**

### Q3 — Counting, normalization and trimming (security)

**Counting.**

- **Unicode scalar values** (recommended): `password.chars().count()`. A
  Rust `&str` is valid UTF-8, so code points and scalar values coincide.
  This is NIST's "each Unicode code point SHALL be counted as a single
  character", needs no dependency, and costs microseconds even on a 32 KiB
  input.
- **Bytes:** `len()`. Penalizes nobody at the maximum but weakens the
  minimum for non-ASCII text. A Finnish user's "ä" or "ö" is two bytes, so
  15 bytes could be 8 characters. Rejected.
- **Grapheme clusters:** closest to "what the user sees", but needs
  `unicode-segmentation` as a new direct dependency (today it is only in the
  lock file through a proc-macro build dependency), whose tables change
  between Unicode versions, and it departs from NIST. Rejected.

**Normalization.** NIST revision 4 says SHOULD normalize to NFC before
hashing. Recommended: **not in this mission.**

- Normalizing changes what is hashed. Accounts created with a non-NFC
  password (for example, decomposed "a" + combining diaeresis instead of
  "ä", which some input methods produce) would stop working unless
  `verify` also tried the raw form. That touches `password.rs`, the KDF
  module, and the brief's non-goal ("changing the KDF or its parameters")
  and forbidden list ("change … the hash format") come close.
- It needs `unicode-normalization` as a new direct dependency (platform,
  dependency-bump runbook).
- The length check counts the raw string. A decomposed "ä" counts 2, so a
  password near the minimum could pass with one fewer visible character.
  That edge is small.

Suggest a follow-up brief for NFC normalization (hash and verify),
recorded in the Continuity Pack.

**Trimming.** Recommended: **no trimming, and no other transformation.**
The check counts exactly the bytes `password::hash` will hash. Trimming at
registration but not at login would break the account for anyone whose
password starts or ends with a space; trimming at login is forbidden by
the brief. NIST makes trimming optional (MAY). Spaces, including leading
and trailing ones, count as characters.

**Whitespace-only or repeated passwords** (15 spaces) pass a length-only
rule. Rejecting them is a strength rule, which is a non-goal. Noted, not
proposed.

### Q4 — Placement (security)

#### Option A — Domain layer in `kauppa-core` (recommended)

A new module `crates/kauppa-core/src/domain/password_rule.rs` (name is
`may_decide`-level), re-exported from `domain/mod.rs`:

```rust
pub const PASSWORD_MIN_CHARS: usize = 15;
pub const PASSWORD_MAX_CHARS: usize = 128;

/// Why a new password was refused. Carries no part of the password.
pub enum PasswordLengthError { TooShort, TooLong }

/// Length rule for a new password (MB-0034, RR-…): Unicode scalar
/// values, counted on the string as submitted.
pub fn check_new_password_length(password: &str) -> Result<(), PasswordLengthError>;
```

- Pros: HTTP-agnostic and unit-testable, as `AGENTS.md` places business
  rules. One source for the handler, the template (P5) and any later
  change-password flow. Not a security trigger path, so the rule can be
  tuned under this RR's terms without touching the KDF module. Mirrors
  `token::is_well_formed` (a pure check called from the handler).
- Cons: one more small module.

#### Option B — In `password.rs`, inside `hash` or beside it

- Cons: `hash` is also used for `DUMMY_HASH` (`mod.rs:106-112`), and
  `verify` must never apply the rule (P4). Mixing a policy into the KDF
  module makes every tweak of the bounds a KDF-module change.
- Rejected.

#### Option C — In the handler

- Pros: smallest diff.
- Cons: the constants then live in `kauppa-web`; a later flow would have to
  reach into `routes::auth`. Business rules belong in core.
- Acceptable fallback.

**Recommendation: A.** The check returns a plain enum, not
`CoreError::Invalid(String)`, so it can't be propagated with `?` into
`error.rs`'s WARN log and plain-text body (Current state → Errors).

### Q5 — Order in the register flow, and the oracle question (security)

Proposed order (only step 3a is new):

1. limiter (unchanged, RR-0013);
2. form shape (unchanged; actix extractor);
3. token shape: missing or malformed → the invalid-invitation page
   (unchanged, RR-0012 sub-answer 7);
   - **3a. password length: out of bounds → 400 on the register page with
     the rule message;** no hash, no DB work;
4. `password::hash` (unchanged);
5. `InvitationRepo::register` (unchanged).

**Which error wins for a short password with an invalid invitation?**

- **Missing or malformed token:** the invalid-invitation page, as today.
  The token's shape is a property of the client's own input, checked
  without the database; it reveals nothing about any stored invitation.
- **Well-formed but unknown, expired, revoked or used token:** the
  password-length page, exactly as for a valid token. After fixing the
  password, the client gets the invalid-invitation page.

Why this adds no oracle:

- The length check runs before any lookup, so its answer (status, body,
  timing) is identical for valid and invalid well-formed tokens. RR-0012
  constraint 4 ("the five invalid cases share status, body and code path")
  holds among the invalid cases for any given password: with a bad length,
  unknown, expired, revoked and used all get the password page; with a good
  length, all get the invalid-invitation page. Missing and malformed tokens
  already took a different, cheaper path under sub-answer 7, which is a
  property of the input, not of the database.
- RR-0012 sub-answer 7's timing uniformity ("a well-formed token always
  pays one hash before the transaction") still holds for every request that
  reaches the transaction. A length-rejected request reaches neither.
- Length is checked on the submitted string only; no stored data is read.

Please confirm explicitly that this reading of constraint 4 is right. The
alternative is to check length **after** the invitation is known valid,
which would reveal validity (the page differs by token), require the check
inside or after the transaction, and hash first, contradicting P2.

**Alternative order considered: length before the token shape.** A missing
token with a short password would get the password message on a page that,
with no invitation, shows no form (`register.hbs:7-19`). Confusing, and no
security gain. Rejected.

**Rate limiting.** A length rejection is a 4xx, so it counts as a register
failure ("invalid input", RR-0013 sub-answer 1). A browser user rarely
reaches it, because `minlength` stops the submit first. No change to
RR-0013 is proposed.

### Q6 — Response, message and logging (security; wording is `may_decide`)

- **Status 400**, rendered through `render_register` with the invitation
  kept in the hidden field and `display_name` and `email` refilled, like
  the duplicate-email page (`mod.rs:291-300`). The password field comes
  back empty, as it does today.
  - Keeping the invitation means echoing a well-formed token the client
    just sent, unchecked, as `GET /auth/register?invite=` already does
    (RR-0012 sub-answer 5). It reveals nothing.
- **One message for both bounds**, built from the constants, for example:
  "Choose a password of 15 to 128 characters." It names the rule (P1), and
  the same text for too short and too long keeps the code simple.
- **Never logged or echoed:** not the password, not its length, not a
  prefix (`autonomy.forbidden`). Proposed: at most
  `tracing::debug!("registration rejected: password length")`, no fields,
  matching the duplicate-email line (`mod.rs:292`). The rejection doesn't
  go through `error.rs`, so no WARN line carries anything.
  `PasswordLengthError` has no fields and its `Debug` shows only the
  variant.

### Q7 — Where the template's bounds come from (P5) (security)

- `RegisterView` gains `password_min_chars` (and, if Q7b is answered yes,
  `password_max_chars`), filled from the core constants inside
  `render_register`, so every register render path gets them.
  `register.hbs` uses `minlength="{{password_min_chars}}"`. The literal
  `12` goes.
- P5's test renders the form and asserts `minlength="{PASSWORD_MIN_CHARS}"`,
  and that the template source contains no numeric `minlength`/`maxlength`
  literal.

**Q7b — `maxlength` on the form? Recommended: no.**

- Browsers stop typing at `maxlength` and **silently truncate pasted
  text**. In a masked password field the user doesn't see it: they paste a
  200-character passphrase, the account is created with the first 128 (if
  the server accepted the truncated value), and their saved 200-character
  password then fails at login, which has no `maxlength`. NIST: never
  truncate.
- Without `maxlength`, an over-long password reaches the server and gets
  the explicit message. At 128, this is rare.
- **Counting mismatch at the minimum.** HTML counts `minlength` in UTF-16
  code units; the server counts scalar values. A character outside the
  Basic Multilingual Plane (most emoji) counts 2 in the browser and 1 on
  the server. So the browser can let through a password the server
  rejects, never the reverse at the minimum. The server message covers it.
  Worth noting in the template comment; not worth JavaScript.

## Recommendation and question

| # | Question | Recommendation |
| --- | --- | --- |
| Q1 | Minimum | **15** (NIST SP 800-63B-4, single-factor); 12 acceptable |
| Q2 | Maximum | **128** characters; Argon2 cost doesn't depend on length (measured) |
| Q3 | Counting | Unicode scalar values (`chars().count()`) on the raw string; no NFC normalization (follow-up brief); no trimming |
| Q4 | Placement | `kauppa-core` domain module with `PASSWORD_MIN_CHARS`, `PASSWORD_MAX_CHARS` and a pure check returning a field-less enum; `password.rs` untouched |
| Q5 | Order | limiter → form shape → token shape → **length** → hash → consume; well-formed invalid tokens get the length page like valid ones |
| Q6 | Response | 400 on the register page with invitation and fields kept; one message naming both bounds; no log line with any password data |
| Q7 | Template | `minlength` from `PASSWORD_MIN_CHARS` via `RegisterView`; **no `maxlength`** (paste truncation) |

Crisp questions to answer:

1. (security) Q1: minimum **15** (NIST revision 4), or keep **12**?
2. (security) Q2: maximum **128** characters?
3. (security) Q3: count **Unicode scalar values** on the string as
   submitted, with **no normalization and no trimming**, NFC left to a
   follow-up brief?
4. (security) Q4: the rule in a **`kauppa-core` domain module**, not in
   `password.rs` or the handler?
5. (security) Q5: the length check **after the token-shape check and
   before `password::hash`**, and agreement that a well-formed invalid
   token getting the length page (like a valid one) is consistent with
   RR-0012 constraint 4?
6. (security) Q6: 400 on the register page with the invitation kept, one
   message for both bounds, at most a field-less DEBUG line?
7. (security) Q7: `minlength` from the constant, and **no `maxlength`**
   attribute (P5 then checks `minlength` only and that `maxlength` is
   absent)?

## Blast radius and rollback

**What can break:**

- **Registration** for anyone whose chosen password is out of bounds; by
  design. If the bounds or the counting are wrong (for example, bytes
  instead of characters), non-ASCII users are refused or let through
  wrongly; P3 covers it.
- **The register page** if `RegisterView` loses the bound on some render
  path (P5's test renders the form).
- **Nothing at login**, by construction (P4).

**Who is affected:** new invitees only. Existing accounts and sessions are
untouched; no data, schema, cookie, session or KDF change.

**Rollback plan:** revert the commit. Accounts created under the rule stay
valid, since nothing about how they were hashed changes.

## Minimal context

- `.agentic/missions/MB-0034-server-side-password-length.md`: P1–P5,
  `autonomy` (forbidden: Argon2 parameters, logging the password or its
  length, the rule at login).
- RR-0012: sub-answer 4 (duplicate email on the register page, invitation
  kept), sub-answer 5 (no-store, no-referrer on every register response),
  sub-answer 7 and constraint 4 (one invalid-invitation response, token
  shape before hashing, hash before the transaction); "Does not cover"
  lists this follow-up.
- RR-0013 sub-answer 1: every 4xx register answer counts, "invalid input"
  included; sub-answer 7: the limiter runs first.
- `.agentic/mentorship/security.md`: threat 1 (weak passwords), Input
  (typed structs, reject on invalid), Logging, Escalation
  (`routes/auth/`, `password.rs`).

## Evidence bundle (Hardened)

All read on `agentic/mb-0034-password-length` at `50322c0`.

- **Register flow:** `crates/kauppa-web/src/routes/auth/mod.rs:209-238`
  (`register_submit`: limiter 215-222, failure counting 227-236),
  `263-317` (`register`: token shape 270-273, hash 276, consume 277-302),
  `81-87` (`RegisterView`), `319-336` (`invalid_invitation`,
  `render_register`).
- **Template:** `templates/auth/register.hbs:12` (`minlength="12"`, no
  `maxlength`).
- **KDF:** `crates/kauppa-core/src/password.rs:14-29`; `argon2` 0.5.3
  (`Cargo.lock`), `MAX_PWD_LEN` at `argon2-0.5.3/src/lib.rs:127`.
- **Form limit:** no `FormConfig` in `crates/`; actix-web 4.18.2
  (`Cargo.lock`) `src/types/form.rs:255`, `DEFAULT_LIMIT = 32_768`.
- **Error path:** `crates/kauppa-web/src/error.rs:34` (Invalid → 400),
  `43-50` (WARN with `error = ?self`, message as body).
- **Test fixtures:** `routes/test_support.rs:24` (`PASSWORD`, 28
  characters); `routes/auth/tests/mod.rs:36-46` (`register_request`);
  `csrf/tests/coverage.rs:151` (no `display_name`; fails form shape).
- **Dependencies:** `unicode-normalization` and `unicode-segmentation` are
  not direct dependencies of either crate; `cargo tree -i
  unicode-segmentation --offline` shows it only under `convert_case` →
  `derive_more-impl` (proc-macro).
- **Argon2id time by input length.** Throwaway crate in the scratchpad
  (`argon2 = "=0.5.3"`, `features = ["std"]`, `Argon2::default()`,
  `hash_password` with a fresh salt), `cargo run --release`, 9 runs per
  length, dev machine (WSL2, 32 threads):

  | Input | Median | Min | Max |
  | --- | --- | --- | --- |
  | 12 B | 26.40 ms | 21.22 ms | 50.46 ms |
  | 64 B | 19.29 ms | 17.41 ms | 31.72 ms |
  | 128 B | 21.10 ms | 17.50 ms | 32.94 ms |
  | 1 KiB | 18.77 ms | 14.49 ms | 20.56 ms |
  | 16 KiB | 17.56 ms | 13.86 ms | 20.00 ms |
  | 32 KiB | 17.60 ms | 16.81 ms | 29.06 ms |
  | 1 MiB | 15.35 ms | 13.56 ms | 17.38 ms |

  No trend with length; the spread is warm-up and scheduler noise. Argon2
  reads the password once into H0 (Blake2b); the memory fill dominates.
- **NIST SP 800-63B-4**, Sec. 3.1.1.2, read 2026-11-09 at
  `https://pages.nist.gov/800-63-4/sp800-63b.html` (quotes above).
- **HTML:** `minlength`/`maxlength` count UTF-16 code units (HTML Living
  Standard, "the length of the value"); user agents prevent typing or
  pasting beyond `maxlength`.
