//! MB-0008 acceptance tests for shared lists at the repo layer (P1, P6), plus
//! the role matrix and share-by-email behavior behind P2–P5.
//! Test-only module: the share methods live on `ShoppingListRepo` and the
//! central access predicate in `repo::sqlite`; this module exists so the
//! brief's `cargo test -p kauppa-core repo::share::<name>` checks select these
//! tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use uuid::Uuid;

use crate::CoreError;
use crate::domain::{ListAccess, NewIngredient, RecipeInput, ShareRole};
use crate::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo, SqlitePool, UserRepo};

/// A fresh, isolated in-memory database with migrations applied, plus a raw
/// sqlx pool on the same database for inspecting rows.
async fn test_db() -> (SqlitePool, sqlx::SqlitePool) {
    let url = format!(
        "sqlite:file:share-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    let options = SqliteConnectOptions::from_str(&url)
        .expect("parse url")
        .foreign_keys(true);
    let raw = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("connect raw pool");
    (pool, raw)
}

async fn make_user(pool: &SqlitePool, email: &str) -> Uuid {
    UserRepo::create(pool, email, "Test User", "argon2-hash")
        .await
        .expect("create user")
        .id
}

/// Everything a list can hold, owned by `owner`: one active and one removed
/// item, plus a category, a favorite and a recipe in the owner's catalog.
struct Fixture {
    list: Uuid,
    active: Uuid,
    removed: Uuid,
    category: Uuid,
    product: Uuid,
    recipe: Uuid,
}

async fn make_fixture(pool: &SqlitePool, owner: Uuid) -> Fixture {
    let category = CategoryRepo::create(pool, owner, "Kylmä").await.unwrap().id;
    let product = ProductRepo::create(pool, owner, "Maito", category)
        .await
        .unwrap()
        .id;
    let recipe = RecipeRepo::create(
        pool,
        owner,
        &RecipeInput {
            title: "Puuro".to_owned(),
            ..RecipeInput::default()
        },
    )
    .await
    .unwrap()
    .id;
    let ingredient = NewIngredient {
        product_name: "Kaurahiutale".to_owned(),
        quantity: Some("2 dl".to_owned()),
        category_id: Some(category),
    };
    RecipeRepo::add_ingredient(pool, owner, recipe, &ingredient)
        .await
        .unwrap();
    let list = ShoppingListRepo::create(pool, owner, "Kauppa")
        .await
        .unwrap()
        .id;
    let active = ShoppingListRepo::add_item(pool, owner, list, "Leipä", None)
        .await
        .unwrap()
        .id;
    let removed = ShoppingListRepo::add_item(pool, owner, list, "Kynttilä", None)
        .await
        .unwrap()
        .id;
    ShoppingListRepo::remove_item(pool, owner, list, removed)
        .await
        .unwrap();
    Fixture {
        list,
        active,
        removed,
        category,
        product,
        recipe,
    }
}

/// A snapshot of every row a list owns, to prove a denied call wrote nothing.
async fn list_state(raw: &sqlx::SqlitePool, list: Uuid) -> String {
    type ItemRow = (String, Option<String>, i64, Option<String>, Option<String>);
    let rows: Vec<ItemRow> = sqlx::query_as(
        r"SELECT text, quantity, checked, category_id, removed_at
            FROM list_items WHERE list_id = ?1 ORDER BY position, id",
    )
    .bind(list.to_string())
    .fetch_all(raw)
    .await
    .unwrap();
    let list_row: Option<(String, String)> =
        sqlx::query_as(r"SELECT name, updated_at FROM shopping_lists WHERE id = ?1")
            .bind(list.to_string())
            .fetch_optional(raw)
            .await
            .unwrap();
    let shares: Vec<(String, String)> = sqlx::query_as(
        r"SELECT grantee_id, role FROM list_shares WHERE list_id = ?1 ORDER BY grantee_id",
    )
    .bind(list.to_string())
    .fetch_all(raw)
    .await
    .unwrap();
    format!("{list_row:?} {rows:?} {shares:?}")
}

/// What `user` gets from every `ShoppingListRepo` method that takes a list
/// id, as `(method, outcome)`: `Ok` or the error. `own` is a category, a
/// favorite and a recipe in `user`'s own catalog, so a catalog check can't be
/// what denies the call.
#[allow(clippy::too_many_lines)]
async fn every_method(
    pool: &SqlitePool,
    user: Uuid,
    f: &Fixture,
    own: &Fixture,
    share_id: Uuid,
) -> Vec<(&'static str, Result<(), CoreError>)> {
    let list = f.list;
    vec![
        (
            "get",
            ShoppingListRepo::get(pool, user, list).await.map(drop),
        ),
        (
            "access",
            ShoppingListRepo::access(pool, user, list).await.map(drop),
        ),
        (
            "items",
            ShoppingListRepo::items(pool, user, list, false)
                .await
                .map(drop),
        ),
        (
            "items(removed)",
            ShoppingListRepo::items(pool, user, list, true)
                .await
                .map(drop),
        ),
        (
            "store_view",
            ShoppingListRepo::store_view(pool, user, list)
                .await
                .map(drop),
        ),
        (
            "shares",
            ShoppingListRepo::shares(pool, user, list).await.map(drop),
        ),
        (
            "add_item",
            ShoppingListRepo::add_item(pool, user, list, "Munat", Some("6"))
                .await
                .map(drop),
        ),
        (
            "add_product",
            ShoppingListRepo::add_product(pool, user, list, own.product, None)
                .await
                .map(drop),
        ),
        (
            "add_recipe",
            ShoppingListRepo::add_recipe(pool, user, list, own.recipe, 1.0)
                .await
                .map(drop),
        ),
        (
            "assign_category",
            ShoppingListRepo::assign_category(pool, user, list, f.active, own.category)
                .await
                .map(drop),
        ),
        (
            "set_checked",
            ShoppingListRepo::set_checked(pool, user, list, f.active, true)
                .await
                .map(drop),
        ),
        (
            "remove_item",
            ShoppingListRepo::remove_item(pool, user, list, f.active)
                .await
                .map(drop),
        ),
        (
            "restore_item",
            ShoppingListRepo::restore_item(pool, user, list, f.removed)
                .await
                .map(drop),
        ),
        (
            "share",
            ShoppingListRepo::share(pool, user, list, "x@example.com", ShareRole::Edit)
                .await
                .map(drop),
        ),
        (
            "revoke",
            ShoppingListRepo::revoke(pool, user, list, share_id)
                .await
                .map(drop),
        ),
        (
            "rename",
            ShoppingListRepo::rename(pool, user, list, "Hijacked")
                .await
                .map(drop),
        ),
        (
            "delete",
            ShoppingListRepo::delete(pool, user, list).await.map(drop),
        ),
    ]
}

/// The methods `every_method` calls that only read.
const READS: [&str; 5] = ["get", "access", "items", "items(removed)", "store_view"];
/// The item writes an `edit` grantee may make.
const ITEM_WRITES: [&str; 4] = ["add_item", "set_checked", "remove_item", "restore_item"];

/// MB-0008 P1: one predicate decides access, and a user who neither owns
/// the list nor holds a grant on it is denied — as `NotFound`, so the list's
/// existence doesn't leak — on every read and every mutation, and nothing is
/// written. The list doesn't appear in any of their indexes either.
#[tokio::test]
async fn central_predicate_denies_non_grantee() {
    let (pool, raw) = test_db().await;
    let owner = make_user(&pool, "owner@example.com").await;
    let grantee = make_user(&pool, "grantee@example.com").await;
    let stranger = make_user(&pool, "stranger@example.com").await;
    let f = make_fixture(&pool, owner).await;
    let own = make_fixture(&pool, stranger).await;
    // A real grant exists on the list (for someone else), so the stranger is
    // denied by the predicate, not by an empty share table.
    let share =
        ShoppingListRepo::share(&pool, owner, f.list, "grantee@example.com", ShareRole::Edit)
            .await
            .unwrap()
            .expect("grantee matched");
    assert_eq!(share.grantee_id, grantee);
    // The stranger shares their own list with the grantee too: a grant on
    // another list must not open this one.
    ShoppingListRepo::share(
        &pool,
        stranger,
        own.list,
        "grantee@example.com",
        ShareRole::Edit,
    )
    .await
    .unwrap();

    let before = list_state(&raw, f.list).await;
    let outcomes = every_method(&pool, stranger, &f, &own, share.id).await;
    assert_eq!(
        outcomes.len(),
        READS.len() + 12,
        "every method is exercised"
    );
    for (method, outcome) in outcomes {
        assert!(
            matches!(outcome, Err(CoreError::NotFound)),
            "{method}: a non-grantee must get NotFound, got {outcome:?}"
        );
    }
    assert_eq!(
        list_state(&raw, f.list).await,
        before,
        "nothing was written"
    );

    let own_lists: Vec<Uuid> = ShoppingListRepo::list_for_owner(&pool, stranger)
        .await
        .unwrap()
        .into_iter()
        .map(|l| l.id)
        .collect();
    assert_eq!(own_lists, [own.list], "only their own list");
    assert!(
        ShoppingListRepo::shared_with(&pool, stranger)
            .await
            .unwrap()
            .is_empty()
    );

    // Unknown list ids are the same NotFound as a foreign list.
    assert!(matches!(
        ShoppingListRepo::get(&pool, owner, Uuid::new_v4()).await,
        Err(CoreError::NotFound)
    ));
}

/// RR-0002 constraint 2 at the repo layer: `view` reads only; `edit` also
/// writes items but can't touch the owner's catalog or administer the list;
/// the owner can do everything. Denials of a grantee are `Forbidden`.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn role_matrix() {
    let (pool, raw) = test_db().await;
    let owner = make_user(&pool, "owner@example.com").await;
    let viewer = make_user(&pool, "viewer@example.com").await;
    let editor = make_user(&pool, "editor@example.com").await;
    let f = make_fixture(&pool, owner).await;
    let viewer_own = make_fixture(&pool, viewer).await;
    let editor_own = make_fixture(&pool, editor).await;
    ShoppingListRepo::share(&pool, owner, f.list, "viewer@example.com", ShareRole::View)
        .await
        .unwrap();
    let editor_share =
        ShoppingListRepo::share(&pool, owner, f.list, "editor@example.com", ShareRole::Edit)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        ShoppingListRepo::access(&pool, viewer, f.list)
            .await
            .unwrap(),
        ListAccess::View
    );
    assert_eq!(
        ShoppingListRepo::access(&pool, editor, f.list)
            .await
            .unwrap(),
        ListAccess::Edit
    );
    assert_eq!(
        ShoppingListRepo::access(&pool, owner, f.list)
            .await
            .unwrap(),
        ListAccess::Owner
    );

    let before = list_state(&raw, f.list).await;
    for (method, outcome) in every_method(&pool, viewer, &f, &viewer_own, editor_share.id).await {
        if READS.contains(&method) {
            assert!(outcome.is_ok(), "viewer {method}: {outcome:?}");
        } else {
            assert!(
                matches!(outcome, Err(CoreError::Forbidden)),
                "viewer {method}: expected Forbidden, got {outcome:?}"
            );
        }
    }
    assert_eq!(
        list_state(&raw, f.list).await,
        before,
        "the viewer wrote nothing"
    );

    for (method, outcome) in every_method(&pool, editor, &f, &editor_own, editor_share.id).await {
        if READS.contains(&method) || ITEM_WRITES.contains(&method) {
            assert!(outcome.is_ok(), "editor {method}: {outcome:?}");
        } else {
            assert!(
                matches!(outcome, Err(CoreError::Forbidden)),
                "editor {method}: expected Forbidden, got {outcome:?}"
            );
        }
    }
    // The editor's writes landed; the list is still there with its grants.
    let texts: Vec<String> = ShoppingListRepo::items(&pool, owner, f.list, false)
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.text)
        .collect();
    assert_eq!(
        texts,
        ["Kynttilä", "Munat"],
        "removed Leipä, restored Kynttilä, added Munat"
    );
    assert_eq!(
        ShoppingListRepo::get(&pool, owner, f.list)
            .await
            .unwrap()
            .name,
        "Kauppa"
    );
    assert_eq!(
        ShoppingListRepo::shares(&pool, owner, f.list)
            .await
            .unwrap()
            .len(),
        2
    );

    // The owner can do everything, ending with delete.
    let outcomes = every_method(&pool, owner, &f, &f, editor_share.id).await;
    for (method, outcome) in outcomes {
        // `active` was removed by the editor, so the item calls on it are
        // NotFound; every other call must succeed.
        if [
            "assign_category",
            "set_checked",
            "remove_item",
            "restore_item",
        ]
        .contains(&method)
        {
            continue;
        }
        assert!(outcome.is_ok(), "owner {method}: {outcome:?}");
    }
    assert!(matches!(
        ShoppingListRepo::get(&pool, viewer, f.list).await,
        Err(CoreError::NotFound)
    ));
    let shares: i64 = sqlx::query_scalar(r"SELECT COUNT(*) FROM list_shares")
        .fetch_one(&raw)
        .await
        .unwrap();
    assert_eq!(shares, 0, "deleting the list deleted its grants");
}

/// MB-0008 P6: after the owner revokes a grant, the former grantee is denied
/// on their very next call — read or write — with nothing cached, and the
/// list leaves their "shared with me" index.
#[tokio::test]
async fn revocation_is_immediate() {
    let (pool, raw) = test_db().await;
    let owner = make_user(&pool, "owner@example.com").await;
    let grantee = make_user(&pool, "grantee@example.com").await;
    let f = make_fixture(&pool, owner).await;
    let own = make_fixture(&pool, grantee).await;
    let share =
        ShoppingListRepo::share(&pool, owner, f.list, "grantee@example.com", ShareRole::Edit)
            .await
            .unwrap()
            .unwrap();

    // Granted: the grantee reads, writes, and sees the list as shared.
    ShoppingListRepo::items(&pool, grantee, f.list, false)
        .await
        .unwrap();
    ShoppingListRepo::add_item(&pool, grantee, f.list, "Munat", None)
        .await
        .unwrap();
    let shared = ShoppingListRepo::shared_with(&pool, grantee).await.unwrap();
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].list.id, f.list);
    assert_eq!(shared[0].role, ShareRole::Edit);

    let revoked = ShoppingListRepo::revoke(&pool, owner, f.list, share.id)
        .await
        .unwrap();
    assert_eq!(revoked.grantee_id, grantee);

    // The very next call of every kind is denied, as for any stranger.
    let before = list_state(&raw, f.list).await;
    for (method, outcome) in every_method(&pool, grantee, &f, &own, share.id).await {
        assert!(
            matches!(outcome, Err(CoreError::NotFound)),
            "{method} after revoke: expected NotFound, got {outcome:?}"
        );
    }
    assert_eq!(list_state(&raw, f.list).await, before);
    assert!(
        ShoppingListRepo::shared_with(&pool, grantee)
            .await
            .unwrap()
            .is_empty()
    );
    // Revoking twice is NotFound; a grant id from another list is too.
    assert!(matches!(
        ShoppingListRepo::revoke(&pool, owner, f.list, share.id).await,
        Err(CoreError::NotFound)
    ));

    // A role downgrade is just as immediate: edit → view stops writes at once.
    ShoppingListRepo::share(&pool, owner, f.list, "grantee@example.com", ShareRole::Edit)
        .await
        .unwrap();
    ShoppingListRepo::add_item(&pool, grantee, f.list, "Voi", None)
        .await
        .unwrap();
    ShoppingListRepo::share(&pool, owner, f.list, "grantee@example.com", ShareRole::View)
        .await
        .unwrap();
    assert!(matches!(
        ShoppingListRepo::add_item(&pool, grantee, f.list, "Juusto", None).await,
        Err(CoreError::Forbidden)
    ));
}

/// Share by email: a single matching account (ignoring ASCII case) other
/// than the owner gets the grant; anything else is `None` and writes
/// nothing; sharing again changes the role in place.
#[tokio::test]
async fn share_by_email() {
    let (pool, raw) = test_db().await;
    let owner = make_user(&pool, "owner@example.com").await;
    let grantee = make_user(&pool, "Grantee@Example.com").await;
    let f = make_fixture(&pool, owner).await;
    let count = || async {
        sqlx::query_scalar::<_, i64>(r"SELECT COUNT(*) FROM list_shares")
            .fetch_one(&raw)
            .await
            .unwrap()
    };

    for nobody in [
        "nobody@example.com",
        "owner@example.com",
        "OWNER@example.com",
    ] {
        let outcome = ShoppingListRepo::share(&pool, owner, f.list, nobody, ShareRole::Edit)
            .await
            .unwrap();
        assert!(outcome.is_none(), "{nobody}");
    }
    assert_eq!(count().await, 0);
    assert!(matches!(
        ShoppingListRepo::share(&pool, owner, f.list, "  ", ShareRole::View).await,
        Err(CoreError::Invalid(_))
    ));

    let first = ShoppingListRepo::share(
        &pool,
        owner,
        f.list,
        " grantee@example.COM ",
        ShareRole::View,
    )
    .await
    .unwrap()
    .expect("case-insensitive match");
    assert_eq!(first.grantee_id, grantee);
    assert_eq!(first.role, ShareRole::View);
    let again =
        ShoppingListRepo::share(&pool, owner, f.list, "grantee@example.com", ShareRole::Edit)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(again.id, first.id, "same grant");
    assert_eq!(again.role, ShareRole::Edit);
    assert_eq!(count().await, 1);

    // Two accounts that differ only by case are ambiguous: nobody is granted.
    make_user(&pool, "twin@example.com").await;
    make_user(&pool, "TWIN@example.com").await;
    assert!(
        ShoppingListRepo::share(&pool, owner, f.list, "twin@example.com", ShareRole::View)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(count().await, 1);
}

// The structural half of P1: a test that fails if a list or list-item query
// goes around the central predicate. It reads the SQLite repo's source.

// The list repo. Every other file in `repo/sqlite/` is read when the test runs
// (`other_sqlite_files`), so a new file can't escape RR-0009 constraint 3:
// only `sqlite/list.rs` may name a list table.
const LIST_RS: &str = include_str!("sqlite/list.rs");
const MOD_RS: &str = include_str!("mod.rs");

/// The text from `header` to the end of its top-level block.
fn block<'a>(src: &'a str, header: &str) -> &'a str {
    let start = src.find(header).expect("block header");
    let end = src[start..].find("\n}\n").expect("block end") + start;
    &src[start..end]
}

/// Top-level items of a Rust file as `(first line, text)`.
fn items(src: &str) -> Vec<(&str, String)> {
    const STARTS: [&str; 11] = [
        "fn ",
        "async fn ",
        "const ",
        "impl ",
        "struct ",
        "type ",
        "pub ",
        "enum ",
        "use ",
        "mod ",
        "static ",
    ];
    let mut out: Vec<(&str, String)> = Vec::new();
    for line in src.lines() {
        if STARTS.iter().any(|s| line.starts_with(s)) {
            out.push((line, String::new()));
        }
        if let Some((_, text)) = out.last_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    out
}

/// Every `.rs` file in `repo/sqlite/` except `list.rs`, as `(name, source)`.
fn other_sqlite_files() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/repo/sqlite");
    let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("read repo/sqlite")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .filter(|path| path.file_name().is_some_and(|n| n != "list.rs"))
        .map(|path| {
            let name = format!("sqlite/{}", path.file_name().unwrap().to_string_lossy());
            (name, std::fs::read_to_string(&path).expect("read source"))
        })
        .collect();
    files.sort();
    files
}

/// P1's bypass check. Fails if:
/// - a `ShoppingListRepo` method (other than `create`, which makes a new
///   list) doesn't open its transaction through `begin_read` /
///   `begin_write` or call `accessible_lists`, or runs a query before that;
/// - the trait gains a method this check didn't see;
/// - any code outside that impl and the grant-taking helpers mentions a list
///   table (`shopping_lists`, `list_items`, `list_shares`);
/// - any SQLite repo file other than `sqlite/list.rs` mentions one (RR-0009).
#[test]
fn list_queries_route_through_central_predicate() {
    const GATES: [&str; 3] = ["self.begin_read(", "self.begin_write(", "accessible_lists("];
    // Outside the impl, only these items may touch list tables; the helpers
    // that query rows take a `&ListGrant`, which only `authorize` makes.
    const ALLOWED: [&str; 7] = [
        "const ACCESS_SELECT",
        "const ITEM_SELECT",
        "const STORE_VIEW_SELECT",
        "const SHARE_SELECT",
        "async fn touch_list",
        "async fn fetch_items",
        "impl ShoppingListRepo for SqlitePool",
    ];
    let repo = block(LIST_RS, "impl ShoppingListRepo for SqlitePool {");
    let mut seen = 0;
    for method in repo.split("\n    async fn ").skip(1) {
        seen += 1;
        let name = method.split('(').next().unwrap();
        if name == "create" {
            continue;
        }
        // Whitespace-free, so rustfmt's line breaks don't matter.
        let method: String = method.split_whitespace().collect();
        let gate = GATES.iter().filter_map(|g| method.find(g)).min();
        assert!(
            gate.is_some(),
            "ShoppingListRepo::{name} skips the access predicate"
        );
        let gate = gate.unwrap();
        if let Some(query) = method.find("sqlx::query") {
            assert!(
                gate < query,
                "ShoppingListRepo::{name} queries before authorizing"
            );
        }
    }
    let declared = block(MOD_RS, "pub trait ShoppingListRepo")
        .matches("\n    async fn ")
        .count();
    assert_eq!(seen, declared, "every trait method was checked");

    for (header, text) in items(LIST_RS) {
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let touches = ["shopping_lists", "list_items", "list_shares"]
            .iter()
            .any(|t| code.contains(t));
        let uses_select = ["ITEM_SELECT)", "STORE_VIEW_SELECT)", "SHARE_SELECT)"]
            .iter()
            .any(|t| code.contains(t));
        if !(touches || uses_select) {
            continue;
        }
        assert!(
            ALLOWED.iter().any(|a| header.starts_with(a)),
            "`{header}` touches a list table outside the central predicate"
        );
        if header.starts_with("async fn") {
            assert!(
                code.contains("grant: &ListGrant"),
                "`{header}` must take a `&ListGrant`"
            );
        }
    }

    let others = other_sqlite_files();
    assert!(others.len() >= 5, "read the other repo/sqlite files");
    for (file, src) in &others {
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for table in ["shopping_lists", "list_items", "list_shares"] {
            assert!(
                !code.contains(table),
                "{file} names `{table}`; list SQL belongs in sqlite/list.rs"
            );
        }
        for select in [
            "ACCESS_SELECT",
            "ITEM_SELECT)",
            "STORE_VIEW_SELECT",
            "SHARE_SELECT",
        ] {
            assert!(
                !code.contains(select),
                "{file} uses `{select}` outside sqlite/list.rs"
            );
        }
    }
}
