# Consultation Request Pack — MB-0049 amendment: the shared ingredient query stays runtime, and the image build gets `.sqlx/`

```yaml
---
id: CRP-0040
mission: MB-0049
owner: "@juuso-kalliomaki"
target_role: platform
risk_tier: low
urgency: medium
state: answered
runbook_trigger: "MB-0049 autonomy.escalate (a query left runtime against RR-0037 sub-answer 1 → platform); feature.md escalation trigger `deploy/Dockerfile` (platform:deploy), outside RR-0037's Covers"
---
```

## Decision statement

Amend RR-0037 for two things its implementation found: whether
`recipe.rs`'s ingredient query, whose SQL `list.rs` shares, stays a
runtime query; and whether the deploy image's build gets the `.sqlx/`
cache, which RR-0037 didn't cover.

## Current state

Implementation on `agentic/mb-0049-query-macros` (from `main` `1fee551`):
50 of RR-0037's 51 queries use the macros; `scripts/sqlx-prepare.sh`, the
`ci.yml` step and `SQLX_OFFLINE=true` are in place; the fast gate and
`cargo test -p kauppa-core` (94) pass. Two items remain:

1. **`fetch_ingredients` (`recipe.rs`).** It runs `INGREDIENT_SELECT`, a
   constant in `sqlite/mod.rs` that `list.rs` also runs (`add_recipe`,
   `list.rs:300`) and reads by column name. Without aliases the macro gets
   the types wrong: it infers `product_id` and `category_id` as NOT NULL
   across the LEFT JOINs, `text` (a `COALESCE`) as `Option<()>`, and the
   timestamp as `String` (probe against a schema database, 2026-11-14).
   With aliases the result columns are renamed, so `list.rs` can't run the
   same text.
2. **The image build.** `deploy/Dockerfile` compiles `kauppa-core`, and
   `.dockerignore` is an allowlist without `.sqlx/`. With no cache and no
   `DATABASE_URL`, the macros fail ("set `DATABASE_URL` to use query
   macros online"), so every image build after this PR would fail.

## Options and trade-offs

### Q1: `fetch_ingredients`

- **A — runtime by decision, with `list.rs` (recommended).** One copy of
  the owner-checked SQL (RR-0001 constraint 9), the argument RR-0037 used
  for `list.rs`. The query stays covered by the repository tests.
- **B — a second, aliased copy in `recipe.rs`**, with a test that the two
  texts match once aliases are stripped. Compile-time checked, but two
  copies of security-relevant SQL to keep in step.

### Q2: the image build

- **A — ship the cache (recommended).** `.dockerignore`: `!.sqlx/`;
  `deploy/Dockerfile` builder: `COPY .sqlx ./.sqlx` and
  `ENV SQLX_OFFLINE=true`. The runtime stage doesn't change.
- **B — a database in the build.** Build a schema database inside the
  builder and set `DATABASE_URL`. More moving parts, Python in the
  builder; rejected.

## Recommendation and question

Recommended: Q1 A, Q2 A.

Crisp question: keep `fetch_ingredients` a runtime query by decision
(`INGREDIENT_SELECT` shared with `list.rs`), and ship `.sqlx/` into the
image build as above, amending RR-0037's scope from 51 to 50 macro queries
and adding the two deploy files to its Covers?

**Answered by the owner, 2026-11-14:** "Go with the recommendations",
"Create an amendment for the RR-0037" (Q1 A, Q2 A). Recorded in RR-0040.

## Blast radius and rollback

What can break: the image build, if the cache is missing or stale (CI's
`--check` keeps it current). Nothing at run time.

Rollback plan: revert with the MB-0049 PR.

## Minimal context

- RR-0037 (sub-answer 1: the five modules, 51 queries; Covers: the five
  files, `.sqlx/`, the script, `ci.yml`).
- `crates/kauppa-core/src/repo/sqlite/mod.rs` (`INGREDIENT_SELECT`);
  `recipe.rs` (`fetch_ingredients`); `list.rs:300`.
- `deploy/Dockerfile` (builder stage), `.dockerignore`.

## Evidence bundle (Hardened)

- Type probe (2026-11-14): `query!` on `INGREDIENT_SELECT`'s text against
  a database made from `migrations/*.sql` inferred `(String, String,
  String, Option<()>, Option<String>, i64, String, String, i64, String)`
  for `id, recipe_id, product_id, text, quantity, position, category_id,
  category_name, category_position, category_created_at`.
- `sqlx-macros-core` 0.8.6 `src/query/mod.rs:166-183`: with no
  `DATABASE_URL` and no cache file the macro errors.
- The local Docker daemon didn't answer (2026-11-14), so the image change
  is checked by CI's image job on the PR.
