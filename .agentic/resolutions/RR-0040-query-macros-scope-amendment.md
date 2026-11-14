# Resolution Record — Amend RR-0037: the shared ingredient query stays runtime, the image build ships `.sqlx/`

```yaml
---
id: RR-0040
closes: CRP-0040
mission: MB-0049
decided_by: "<resolver's GitHub handle, e.g. @Jurzo>"
role: platform
risk_tier: low
decided_at: <RFC 3339 UTC time of signing, e.g. 2026-11-14T00:00:00Z>
status: <approved | rejected>
superseded_by: null
rolls_back_into:
  - .agentic/missions/MB-0049-compile-time-checked-queries.md
  - .agentic/consults/CRP-0040-query-macros-scope-amendment.md
  - .agentic/resolutions/RR-0037-compile-time-checked-queries.md
  - .agentic/mentorship/persistence.md
---
```

> **Scaffold, not yet signed.** The owner answered CRP-0040 in the session
> of 2026-11-14 ("Go with the recommendations", "Create an amendment for
> the RR-0037"). Before signing, the resolver checks the sub-answers,
> removes this note, and fills `decided_by`, `decided_at`, `status` and
> the Approvals line. `validate_artifacts.py` fails on those placeholders
> until then.

## Decision and outcome

Approve CRP-0040 Q1 A and Q2 A. This amends RR-0037.

Sub-answers:

1. **`fetch_ingredients` stays runtime (Q1 A, platform).** Of RR-0037's
   51 queries in the five modules, 50 use the macros. `recipe.rs`'s
   `fetch_ingredients` runs `INGREDIENT_SELECT` (`sqlite/mod.rs`) as a
   runtime query by decision, as `list.rs` does: the constant is shared
   with `list.rs`, which reads its columns by name, and the macro version
   needs aliases that rename them. RR-0037 constraint 2's runtime list
   gains `INGREDIENT_SELECT`.
2. **The image build ships the cache (Q2 A, platform).** `.dockerignore`
   lets `.sqlx/` in; `deploy/Dockerfile`'s builder copies it and sets
   `SQLX_OFFLINE=true`. The runtime stage doesn't change.

Covers:

- MB-0049's implementation PR with those two changes (commit `5223347` on
  `agentic/mb-0049-query-macros`);
- the roll-backs below.

Does not cover:

- any other query left runtime, or a second copy of `INGREDIENT_SELECT`;
- any other change to `deploy/`.

## Rationale and trade-offs

- **One copy of the owner-checked SQL** over a compile-checked second
  copy: the same reasoning RR-0037 applied to `list.rs` (CRP-0037, "12
  copies a test must prove identical").
- **A cache in the image** over a database in the build: the same
  mechanism CI uses, nothing new in the builder.

## Approvals and authority

<resolver: e.g. "@Jurzo as platform for sub-answers 1 and 2.">

## Linked artifacts and evidence

- Consult: `.agentic/consults/CRP-0040-query-macros-scope-amendment.md`
- Mission Brief: `.agentic/missions/MB-0049-compile-time-checked-queries.md`
- Amends: RR-0037 (sub-answer 1, constraint 2, Covers).
- Evidence: CRP-0040 → Evidence bundle.

## Constraints and follow-on rules (Hardened)

1. `INGREDIENT_SELECT` stays one constant, used by `list.rs` and
   `recipe.rs`. It isn't in `.sqlx/`; the repository tests of both check
   it.
2. The image build is offline (`SQLX_OFFLINE=true`), like CI (RR-0037
   constraint 4).

## Roll-back into the system (Hardened)

To be applied after this record is signed, with the owner's OK:

- `MB-0049`: P2 "Every query in … (51)" → "Every query … but
  `recipe.rs`'s `fetch_ingredients` (50; RR-0040)"; P3's runtime list adds
  `INGREDIENT_SELECT`; `resolutions: [RR-0037, RR-0040]`.
- `CRP-0040`: `state: open` → `answered`.
- `RR-0037`: under "Supersession and history", add "Amended by RR-0040
  (2026-11-14): `recipe.rs`'s `fetch_ingredients` stays runtime
  (`INGREDIENT_SELECT`, shared with `list.rs`); the image build ships
  `.sqlx/`."
- `.agentic/mentorship/persistence.md`: the runtime exception names
  `INGREDIENT_SELECT` (used by `list.rs` and `recipe.rs`).

## Supersession and history

Status: active once signed. Amends RR-0037; supersedes nothing.
