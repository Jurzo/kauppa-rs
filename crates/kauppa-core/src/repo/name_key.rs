//! The one rule for matching category and product names (MB-0010, RR-0003
//! constraint 3): names match when their keys are equal, so "maito"/"Maito"
//! and "äyräs"/"ÄYRÄS" collide. Every write to a `name` column stores
//! `name_key(name)` next to it, and every lookup by name goes through the key.
//!
//! Rows that existed before `0004_name_keys.sql` got their key from a SQL
//! `lower()` + `replace()` chain instead; `matches_sql_backfill` pins that
//! expression to this helper.

/// The match key for a user-supplied name: trimmed and lowercased with
/// Rust's Unicode-aware `to_lowercase`.
pub(super) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

// MB-0010 acceptance tests (P4, P5, P7). They sit at module level rather than
// in a `tests` submodule so the brief's `cargo test -p kauppa-core
// repo::name_key::<name>` checks select them.

/// The backfill key expression in `0004_name_keys.sql`, over the `name`
/// column. SQLite's `lower()` folds only ASCII, so each non-ASCII capital is
/// replaced explicitly first.
#[cfg(test)]
const SQL_BACKFILL_KEY: &str = "lower(replace(replace(replace(replace(replace(replace(replace(replace(name, 'Å', 'å'), 'Ä', 'ä'), 'Ö', 'ö'), 'Õ', 'õ'), 'Ü', 'ü'), 'É', 'é'), 'Ø', 'ø'), 'Æ', 'æ'))";

#[cfg(test)]
const MIGRATION_0004: &str = include_str!("../../../../migrations/0004_name_keys.sql");

#[cfg(test)]
#[allow(clippy::expect_used)]
mod fixtures {
    use chrono::{DateTime, TimeZone, Utc};
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use uuid::Uuid;

    use crate::repo::SqlitePool;

    /// A shared-cache in-memory database: a raw sqlx pool for seeding and
    /// inspecting rows, plus a `SqlitePool` on the same database for repo calls
    /// (opened once the schema is current).
    pub(super) struct Db {
        url: String,
        pub(super) raw: sqlx::SqlitePool,
    }

    impl Db {
        pub(super) async fn new() -> Self {
            let url = format!(
                "sqlite:file:name-key-{}?mode=memory&cache=shared",
                Uuid::new_v4()
            );
            let options = SqliteConnectOptions::from_str(&url)
                .expect("parse url")
                .foreign_keys(true);
            let raw = SqlitePoolOptions::new()
                .connect_with(options)
                .await
                .expect("connect raw pool");
            Self { url, raw }
        }

        /// Apply migrations up to and including `version`.
        pub(super) async fn migrate_to(&self, version: i64) {
            let mut migrator: Migrator = sqlx::migrate!("../../migrations");
            migrator.migrations = migrator
                .migrations
                .iter()
                .filter(|m| m.version <= version)
                .cloned()
                .collect::<Vec<_>>()
                .into();
            migrator.run(&self.raw).await.expect("partial migrations");
        }

        /// Apply every migration, as the app does at startup.
        pub(super) async fn upgrade(&self) -> SqlitePool {
            let pool = SqlitePool::connect(&self.url).await.expect("connect pool");
            pool.run_migrations().await.expect("run migrations");
            pool
        }

        pub(super) async fn user(&self, email: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r"INSERT INTO users (id, email, display_name, password_hash, created_at)
                  VALUES (?1, ?2, 'Test User', 'argon2-hash', ?3)",
            )
            .bind(id.to_string())
            .bind(email)
            .bind(at(0))
            .execute(&self.raw)
            .await
            .expect("seed user");
            id
        }

        /// A category row as written before `0004` (no `name_key`).
        pub(super) async fn category(&self, id: Uuid, owner: Uuid, name: &str, created: i64) {
            sqlx::query(
                r"INSERT INTO categories (id, owner_id, name, position, created_at)
                  VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(id.to_string())
            .bind(owner.to_string())
            .bind(name)
            .bind(created)
            .bind(at(created))
            .execute(&self.raw)
            .await
            .expect("seed category");
        }

        /// A product row as written before `0004` (no `name_key`).
        pub(super) async fn product(
            &self,
            id: Uuid,
            owner: Uuid,
            name: &str,
            cat: Uuid,
            created: i64,
        ) {
            sqlx::query(
                r"INSERT INTO products
                      (id, owner_id, name, category_id, position, is_favorite,
                       created_at, updated_at)
                  VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
            )
            .bind(id.to_string())
            .bind(owner.to_string())
            .bind(name)
            .bind(cat.to_string())
            .bind(created)
            .bind(at(created))
            .execute(&self.raw)
            .await
            .expect("seed product");
        }

        /// `(name, name_key, position)` of one row of `table`.
        pub(super) async fn row(&self, table: &str, id: Uuid) -> (String, Option<String>, i64) {
            sqlx::query_as(&format!(
                "SELECT name, name_key, position FROM {table} WHERE id = ?1"
            ))
            .bind(id.to_string())
            .fetch_one(&self.raw)
            .await
            .expect("fetch row")
        }

        pub(super) async fn count(&self, table: &str) -> i64 {
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&self.raw)
                .await
                .expect("count rows")
        }
    }

    /// A fixed timestamp `seconds` after a base instant, so tests control
    /// which row is oldest.
    pub(super) fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_767_225_600 + seconds, 0)
            .single()
            .expect("valid timestamp")
    }

    /// A UUID with a chosen first byte, so tests control the `id` tiebreak.
    pub(super) fn id_with_prefix(prefix: u8) -> Uuid {
        let mut bytes = *Uuid::new_v4().as_bytes();
        bytes[0] = prefix;
        Uuid::from_bytes(bytes)
    }
}

/// P7: the Rust helper and the migration's SQL backfill expression agree on
/// ASCII and Å/Ä/Ö names (and the extra capitals the chain folds), so rows
/// keyed before and after the upgrade match each other.
#[cfg(test)]
#[tokio::test]
#[allow(clippy::expect_used)]
async fn matches_sql_backfill() {
    let db = fixtures::Db::new().await;

    assert_eq!(
        MIGRATION_0004.matches(SQL_BACKFILL_KEY).count(),
        2,
        "0004_name_keys.sql backfills both tables with exactly this expression"
    );

    let samples = [
        "maito",
        "Maito",
        "MAITO",
        "Hevi",
        "äyräs",
        "ÄYRÄS",
        "Äyräs",
        "ÅLAND",
        "Åland",
        "ÖLJY",
        "Öljy",
        "Kylmä",
        "KYLMÄ",
        "Ruoka-öljy",
        "SUKLAA 70 %",
        "Õun",
        "MÜSLI",
        "CAFÉ",
        "Smør",
        "SMØR",
        "ÆBLE",
        "Herkut & Sipsit",
    ];
    for name in samples {
        let sql: String = sqlx::query_scalar(&format!(
            "SELECT {SQL_BACKFILL_KEY} FROM (SELECT ?1 AS name)"
        ))
        .bind(name)
        .fetch_one(&db.raw)
        .await
        .expect("evaluate backfill expression");
        assert_eq!(sql, name_key(name), "key for {name:?}");
    }
}

/// P4: `0004_name_keys.sql` applies to a database that already has categories
/// and products, gives every existing row a key, and the case-insensitive
/// rule holds for writes after the upgrade.
#[cfg(test)]
#[tokio::test]
#[allow(clippy::unwrap_used, clippy::expect_used)]
async fn backfill_existing_rows() {
    use crate::CoreError;
    use crate::repo::{CategoryRepo, ProductRepo};
    use fixtures::id_with_prefix;
    use uuid::Uuid;

    let db = fixtures::Db::new().await;
    db.migrate_to(3).await;

    let u = db.user("u@example.com").await;
    let (hevi, kylma) = (Uuid::new_v4(), Uuid::new_v4());
    db.category(hevi, u, "HEVI", 0).await;
    db.category(kylma, u, "Kylmä", 1).await;
    let (ayras, omena, oljy) = (id_with_prefix(1), id_with_prefix(2), id_with_prefix(3));
    db.product(ayras, u, "ÄYRÄS", kylma, 0).await;
    db.product(omena, u, "omena", hevi, 1).await;
    db.product(oljy, u, "Öljy", kylma, 2).await;

    let pool = db.upgrade().await;

    for (table, id, name, key) in [
        ("categories", hevi, "HEVI", "hevi"),
        ("categories", kylma, "Kylmä", "kylmä"),
        ("products", ayras, "ÄYRÄS", "äyräs"),
        ("products", omena, "omena", "omena"),
        ("products", oljy, "Öljy", "öljy"),
    ] {
        let (stored, stored_key, _) = db.row(table, id).await;
        assert_eq!(stored, name, "{table} name unchanged");
        assert_eq!(stored_key.as_deref(), Some(key), "{table} key for {name}");
        assert_eq!(stored_key.as_deref(), Some(name_key(name).as_str()));
    }

    // Writes after the upgrade collide with pre-upgrade rows ignoring case.
    assert!(matches!(
        CategoryRepo::create(&pool, u, "hevi").await,
        Err(CoreError::Conflict(_))
    ));
    assert!(matches!(
        CategoryRepo::create(&pool, u, "KYLMÄ").await,
        Err(CoreError::Conflict(_))
    ));
    assert!(matches!(
        ProductRepo::create(&pool, u, "äyräs", hevi).await,
        Err(CoreError::Conflict(_))
    ));
    assert!(matches!(
        ProductRepo::rename(&pool, u, omena, "ÖLJY").await,
        Err(CoreError::Conflict(_))
    ));

    // And new rows are keyed by the Rust helper.
    let leipa = CategoryRepo::create(&pool, u, "LEIPÄ").await.unwrap();
    assert_eq!(
        db.row("categories", leipa.id).await.1.as_deref(),
        Some("leipä")
    );
}

/// P5: names that already differ only by case lose no data. The oldest row of
/// each group (by `created_at`, then `id`) keeps the plain key; every other row
/// keeps its id, name, and data and gets `<key>#<row id>`. Renaming such a row
/// to a non-clashing name gives it a plain key again.
#[cfg(test)]
#[tokio::test]
#[allow(clippy::unwrap_used, clippy::expect_used)]
async fn existing_case_duplicates() {
    use crate::CoreError;
    use crate::repo::{CategoryRepo, ProductRepo};
    use fixtures::id_with_prefix;
    use uuid::Uuid;

    let db = fixtures::Db::new().await;
    db.migrate_to(3).await;

    let u = db.user("u@example.com").await;
    let other = db.user("other@example.com").await;

    // Categories: three spellings of one name; the oldest isn't the smallest id.
    let (hevi_old, hevi_mid, hevi_new) = (id_with_prefix(9), id_with_prefix(1), id_with_prefix(5));
    db.category(hevi_mid, u, "HEVI", 20).await;
    db.category(hevi_old, u, "Hevi", 10).await;
    db.category(hevi_new, u, "hevi", 30).await;
    // Another owner's same name is not part of the group.
    let theirs = Uuid::new_v4();
    db.category(theirs, other, "HEVI", 40).await;

    // Products: a Finnish pair created at the same instant, so `id` decides.
    let (ayras_small, ayras_big) = (id_with_prefix(2), id_with_prefix(7));
    db.product(ayras_big, u, "äyräs", hevi_old, 5).await;
    db.product(ayras_small, u, "ÄYRÄS", hevi_old, 5).await;
    let maito = Uuid::new_v4();
    db.product(maito, u, "Maito", hevi_old, 6).await;

    let (categories_before, products_before) =
        (db.count("categories").await, db.count("products").await);

    let pool = db.upgrade().await;

    assert_eq!(db.count("categories").await, categories_before);
    assert_eq!(db.count("products").await, products_before);

    for (table, id, name, key, position) in [
        ("categories", hevi_old, "Hevi", "hevi".to_owned(), 10),
        (
            "categories",
            hevi_mid,
            "HEVI",
            format!("hevi#{hevi_mid}"),
            20,
        ),
        (
            "categories",
            hevi_new,
            "hevi",
            format!("hevi#{hevi_new}"),
            30,
        ),
        ("categories", theirs, "HEVI", "hevi".to_owned(), 40),
        ("products", ayras_small, "ÄYRÄS", "äyräs".to_owned(), 5),
        (
            "products",
            ayras_big,
            "äyräs",
            format!("äyräs#{ayras_big}"),
            5,
        ),
        ("products", maito, "Maito", "maito".to_owned(), 6),
    ] {
        let row = db.row(table, id).await;
        assert_eq!(row, (name.to_owned(), Some(key), position), "{table} {id}");
    }

    // Every row is still listed.
    assert_eq!(
        CategoryRepo::list_for_owner(&pool, u).await.unwrap().len(),
        3
    );
    let products: usize = ProductRepo::list_for_owner(&pool, u)
        .await
        .unwrap()
        .iter()
        .map(|g| g.products.len())
        .sum();
    assert_eq!(products, 3);

    // A suffixed row still can't be renamed into its group's plain key...
    assert!(matches!(
        CategoryRepo::rename(&pool, u, hevi_new, "HeVi").await,
        Err(CoreError::Conflict(_))
    ));
    // ...but a non-clashing name gives it a plain key again.
    CategoryRepo::rename(&pool, u, hevi_new, "Hedelmät")
        .await
        .unwrap();
    assert_eq!(
        db.row("categories", hevi_new).await.1.as_deref(),
        Some("hedelmät")
    );
    ProductRepo::rename(&pool, u, ayras_big, "Äyriäinen")
        .await
        .unwrap();
    assert_eq!(
        db.row("products", ayras_big).await.1.as_deref(),
        Some("äyriäinen")
    );
}
