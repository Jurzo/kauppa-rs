//! MB-0023 acceptance tests for invitations and admins at the repo layer
//! (P2, P6). Test-only module: the methods live on `InvitationRepo` and
//! `AdminRepo`, implemented in `repo::sqlite::invitation`; this module exists
//! so the brief's `cargo test -p kauppa-core repo::invite::<name>` checks
//! select these tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

use crate::CoreError;
use crate::domain::invitation::{INVITATION_TTL_DAYS, NOTE_MAX_CHARS};
use crate::domain::{InvitationState, ShareRole};
use crate::repo::{AdminRepo, InvitationRepo, ShoppingListRepo, SqlitePool, UserRepo};
use crate::{password, token};

/// A fresh in-memory database with migrations applied, plus a raw sqlx pool
/// on it for inspecting rows.
async fn test_db() -> (SqlitePool, sqlx::SqlitePool) {
    let url = format!(
        "sqlite:file:invite-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    (pool, raw_pool(&url).await)
}

async fn raw_pool(url: &str) -> sqlx::SqlitePool {
    let options = SqliteConnectOptions::from_str(url)
        .expect("parse url")
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("connect raw pool")
}

fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

async fn users(raw: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(raw)
        .await
        .unwrap()
}

async fn register(
    pool: &SqlitePool,
    token_hash: &str,
    email: &str,
    now: DateTime<Utc>,
) -> Option<crate::domain::Registered> {
    InvitationRepo::register(pool, token_hash, email, "Pat", "argon2-hash", now)
        .await
        .expect("register")
}

/// P2: token entropy and hashing, single use, expiry, revocation, and one
/// winner under concurrency.
mod single_use_and_expiry {
    use super::*;

    /// The token is 256 bits as 64 hex characters; only its SHA-256 is
    /// stored, and no column holds the plaintext.
    #[tokio::test]
    async fn token_is_random_and_stored_hashed() {
        let (pool, raw) = test_db().await;
        let now = Utc::now();
        let a = InvitationRepo::create(&pool, None, Some("for Mum"), now)
            .await
            .unwrap();
        let b = InvitationRepo::create(&pool, None, None, now)
            .await
            .unwrap();
        let token = a.token.as_str();
        assert_eq!(token.len(), 64, "32 bytes = 256 bits, hex");
        assert!(token::is_well_formed(token));
        assert_ne!(token, b.token.as_str());

        let rows: Vec<(String, String, Option<String>)> =
            sqlx::query_as("SELECT id, token_hash, note FROM invitations ORDER BY id")
                .fetch_all(&raw)
                .await
                .unwrap();
        assert_eq!(rows.len(), 2);
        let (_, stored, note) = rows
            .iter()
            .find(|(id, ..)| *id == a.invitation.id.to_string())
            .unwrap();
        assert_eq!(*stored, token::hash(token));
        assert_eq!(note.as_deref(), Some("for Mum"));
        // Nothing in the table holds the plaintext.
        let dump: String = sqlx::query_scalar(
            r"SELECT group_concat(id || token_hash || COALESCE(note, '') || created_at
                                  || expires_at, '|') FROM invitations",
        )
        .fetch_one(&raw)
        .await
        .unwrap();
        assert!(!dump.contains(token));
        assert!(!dump.contains(b.token.as_str()));
        assert!(!format!("{a:?}").contains(token), "Debug never shows it");
    }

    /// A valid token creates exactly one account and is then used up.
    #[tokio::test]
    async fn token_is_single_use() {
        let (pool, raw) = test_db().await;
        let now = Utc::now();
        let created = InvitationRepo::create(&pool, None, None, now)
            .await
            .unwrap();
        let hash = created.token.hash();

        let first = register(&pool, &hash, "pat@example.com", now)
            .await
            .expect("pending invitation");
        assert_eq!(first.invitation_id, created.invitation.id);
        let (user, stored) = UserRepo::find_by_email(&pool, "pat@example.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.id, first.user.id);
        assert_eq!(stored, "argon2-hash");

        assert!(
            register(&pool, &hash, "sam@example.com", now)
                .await
                .is_none(),
            "used"
        );
        assert_eq!(users(&raw).await, 1);
        assert!(
            UserRepo::find_by_email(&pool, "sam@example.com")
                .await
                .unwrap()
                .is_none()
        );

        let listed = InvitationRepo::list(&pool).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].state(now), InvitationState::Used);
        let used_by = listed[0].used_by.as_ref().unwrap();
        assert_eq!(used_by.id, user.id);
        assert_eq!(used_by.email, "pat@example.com");
        assert_eq!(used_by.display_name, "Pat");
    }

    /// An unknown token creates nothing, the same as a used one.
    #[tokio::test]
    async fn unknown_token_creates_nothing() {
        let (pool, raw) = test_db().await;
        let unknown = token::hash(&"ab".repeat(32));
        assert!(
            register(&pool, &unknown, "pat@example.com", Utc::now())
                .await
                .is_none()
        );
        assert_eq!(users(&raw).await, 0);
    }

    /// Expiry: the default lifetime, compared at whole seconds; a fraction
    /// of a second in the clock changes nothing.
    #[tokio::test]
    async fn token_expires() {
        assert_eq!(INVITATION_TTL_DAYS, 7, "the brief's default");
        let (pool, raw) = test_db().await;
        let created_at = at("2026-01-01T12:00:00.750Z");
        let created = InvitationRepo::create(&pool, None, None, created_at)
            .await
            .unwrap();
        assert_eq!(created.invitation.created_at, at("2026-01-01T12:00:00Z"));
        assert_eq!(created.invitation.expires_at, at("2026-01-08T12:00:00Z"));
        // Stored without a fraction, at a fixed width.
        let stored: Vec<(String, String)> =
            sqlx::query_as("SELECT created_at, expires_at FROM invitations")
                .fetch_all(&raw)
                .await
                .unwrap();
        assert_eq!(
            stored,
            [(
                "2026-01-01T12:00:00+00:00".to_owned(),
                "2026-01-08T12:00:00+00:00".to_owned()
            )]
        );
        let hash = created.token.hash();

        // At and after the expiry second: expired, nothing written.
        for now in [
            "2026-01-08T12:00:00Z",
            "2026-01-08T12:00:00.999Z",
            "2026-01-09T00:00:00Z",
        ] {
            assert!(
                register(&pool, &hash, "late@example.com", at(now))
                    .await
                    .is_none(),
                "{now}"
            );
            let listed = InvitationRepo::list(&pool).await.unwrap();
            assert_eq!(listed[0].state(at(now)), InvitationState::Expired, "{now}");
        }
        assert_eq!(users(&raw).await, 0);

        // The last second before it: still pending.
        let now = at("2026-01-08T11:59:59.999Z");
        assert_eq!(
            InvitationRepo::list(&pool).await.unwrap()[0].state(now),
            InvitationState::Pending
        );
        assert!(
            register(&pool, &hash, "just@example.com", now)
                .await
                .is_some()
        );
        let used_at: String = sqlx::query_scalar("SELECT used_at FROM invitations")
            .fetch_one(&raw)
            .await
            .unwrap();
        assert_eq!(used_at, "2026-01-08T11:59:59+00:00");
    }

    /// A revoked invitation creates nothing; only a pending one can be
    /// revoked, anything else is `NotFound`.
    #[tokio::test]
    async fn revoked_token_creates_nothing() {
        let (pool, raw) = test_db().await;
        let now = Utc::now();
        let created = InvitationRepo::create(&pool, None, None, now)
            .await
            .unwrap();
        let revoked = InvitationRepo::revoke(&pool, created.invitation.id, now)
            .await
            .unwrap();
        assert_eq!(revoked.state(now), InvitationState::Revoked);
        assert!(
            register(&pool, &created.token.hash(), "pat@example.com", now)
                .await
                .is_none()
        );
        assert_eq!(users(&raw).await, 0);

        // Again, unknown, used and expired: NotFound, nothing changes.
        let used = InvitationRepo::create(&pool, None, None, now)
            .await
            .unwrap();
        register(&pool, &used.token.hash(), "pat@example.com", now)
            .await
            .unwrap();
        let old = InvitationRepo::create(&pool, None, None, now - Duration::days(8))
            .await
            .unwrap();
        let before = InvitationRepo::list(&pool).await.unwrap();
        for id in [
            created.invitation.id,
            Uuid::new_v4(),
            used.invitation.id,
            old.invitation.id,
        ] {
            assert!(
                matches!(
                    InvitationRepo::revoke(&pool, id, now).await,
                    Err(CoreError::NotFound)
                ),
                "{id}"
            );
        }
        assert_eq!(InvitationRepo::list(&pool).await.unwrap(), before);
    }

    /// An invalid token with a registered email is the same `None` as with
    /// a new one, never `Conflict`: only the holder of a pending invitation
    /// learns that an email is taken (RR-0012 sub-answer 4, constraint 4).
    #[tokio::test]
    async fn taken_email_with_an_invalid_token_is_just_invalid() {
        let (pool, raw) = test_db().await;
        UserRepo::create(&pool, "taken@example.com", "Old", "h")
            .await
            .unwrap();
        let now = Utc::now();
        let used = InvitationRepo::create(&pool, None, None, now)
            .await
            .unwrap();
        register(&pool, &used.token.hash(), "pat@example.com", now)
            .await
            .unwrap();
        let revoked = InvitationRepo::create(&pool, None, None, now)
            .await
            .unwrap();
        InvitationRepo::revoke(&pool, revoked.invitation.id, now)
            .await
            .unwrap();
        let expired = InvitationRepo::create(&pool, None, None, now - Duration::days(8))
            .await
            .unwrap();
        for (label, hash) in [
            ("unknown", token::hash(&"ab".repeat(32))),
            ("used", used.token.hash()),
            ("revoked", revoked.token.hash()),
            ("expired", expired.token.hash()),
        ] {
            let outcome =
                InvitationRepo::register(&pool, &hash, "taken@example.com", "New", "h", now).await;
            assert!(matches!(outcome, Ok(None)), "{label}: {outcome:?}");
        }
        assert_eq!(users(&raw).await, 2);
    }

    /// A duplicate email rolls back: no second account, the invitation
    /// stays pending, and the error doesn't name the email.
    #[tokio::test]
    async fn duplicate_email_keeps_the_invitation() {
        let (pool, raw) = test_db().await;
        UserRepo::create(&pool, "taken@example.com", "Old", "h")
            .await
            .unwrap();
        let now = Utc::now();
        let created = InvitationRepo::create(&pool, None, None, now)
            .await
            .unwrap();
        let hash = created.token.hash();
        let err = InvitationRepo::register(&pool, &hash, "taken@example.com", "New", "h", now)
            .await
            .unwrap_err();
        let CoreError::Conflict(message) = &err else {
            panic!("{err:?}")
        };
        assert!(!message.contains("taken"), "{message}");
        assert!(!err.to_string().contains("example.com"), "{err}");
        assert_eq!(users(&raw).await, 1);
        assert_eq!(
            InvitationRepo::list(&pool).await.unwrap()[0].state(now),
            InvitationState::Pending
        );
        assert!(
            register(&pool, &hash, "new@example.com", now)
                .await
                .is_some()
        );
    }

    /// Two registrations racing with one token create one account, on a
    /// file database with the app's connection settings.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_registrations_create_one_account() {
        let dir = TempDir::new();
        let url = format!("sqlite://{}", dir.path().join("race.db").display());
        let pool = SqlitePool::connect(&url).await.unwrap();
        pool.run_migrations().await.unwrap();
        let raw = raw_pool(&url).await;

        for round in 0..10 {
            let now = Utc::now();
            let created = InvitationRepo::create(&pool, None, None, now)
                .await
                .unwrap();
            let hash = created.token.hash();
            let racers: Vec<_> = (0..4)
                .map(|n| {
                    let pool = pool.clone();
                    let hash = hash.clone();
                    tokio::spawn(async move {
                        let email = format!("racer{round}-{n}@example.com");
                        InvitationRepo::register(&pool, &hash, &email, "Racer", "h", now).await
                    })
                })
                .collect();
            let mut winners = 0;
            for racer in racers {
                // A loser sees no pending invitation (or, at worst, a busy
                // database); either way it creates nothing.
                if let Ok(Some(_)) = racer.await.unwrap() {
                    winners += 1;
                }
            }
            assert_eq!(winners, 1, "round {round}");
            assert_eq!(users(&raw).await, round + 1, "round {round}");
        }
        raw.close().await;
    }
}

/// A directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kauppa-invite-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Notes are trimmed and capped; a blank one is none.
#[tokio::test]
async fn notes_are_validated() {
    let (pool, _raw) = test_db().await;
    let now = Utc::now();
    let blank = InvitationRepo::create(&pool, None, Some("   "), now)
        .await
        .unwrap();
    assert_eq!(blank.invitation.note, None);
    let long = "x".repeat(NOTE_MAX_CHARS + 1);
    assert!(matches!(
        InvitationRepo::create(&pool, None, Some(&long), now).await,
        Err(CoreError::Invalid(_))
    ));
    assert_eq!(InvitationRepo::list(&pool).await.unwrap().len(), 1);
}

/// The list is newest first, each with its derived state and creator.
#[tokio::test]
async fn list_is_newest_first_with_states() {
    let (pool, _raw) = test_db().await;
    let admin = UserRepo::create(&pool, "admin@example.com", "Admin", "h")
        .await
        .unwrap()
        .id;
    let now = at("2026-03-01T00:00:00Z");
    let expired = InvitationRepo::create(&pool, Some(admin), None, now - Duration::days(9))
        .await
        .unwrap();
    let revoked = InvitationRepo::create(&pool, Some(admin), None, now - Duration::days(3))
        .await
        .unwrap();
    InvitationRepo::revoke(&pool, revoked.invitation.id, now - Duration::days(2))
        .await
        .unwrap();
    let used = InvitationRepo::create(&pool, None, None, now - Duration::days(2))
        .await
        .unwrap();
    register(
        &pool,
        &used.token.hash(),
        "pat@example.com",
        now - Duration::days(1),
    )
    .await
    .unwrap();
    let pending = InvitationRepo::create(&pool, Some(admin), Some("Sam"), now - Duration::hours(1))
        .await
        .unwrap();

    let listed = InvitationRepo::list(&pool).await.unwrap();
    let got: Vec<_> = listed.iter().map(|i| (i.id, i.state(now))).collect();
    assert_eq!(
        got,
        [
            (pending.invitation.id, InvitationState::Pending),
            (used.invitation.id, InvitationState::Used),
            (revoked.invitation.id, InvitationState::Revoked),
            (expired.invitation.id, InvitationState::Expired),
        ]
    );
    assert_eq!(listed[0].created_by, Some(admin));
    assert_eq!(listed[0].note.as_deref(), Some("Sam"));
    assert_eq!(listed[1].created_by, None, "host command");
}

/// Admin grants are idempotent and read from the table.
#[tokio::test]
async fn admin_grant_is_idempotent() {
    let (pool, raw) = test_db().await;
    let user = UserRepo::create(&pool, "pat@example.com", "Pat", "h")
        .await
        .unwrap()
        .id;
    assert!(!AdminRepo::is_admin(&pool, user).await.unwrap());
    let t0 = at("2026-03-01T10:00:00.5Z");
    assert!(AdminRepo::grant(&pool, user, None, t0).await.unwrap());
    assert!(AdminRepo::is_admin(&pool, user).await.unwrap());
    assert!(
        !AdminRepo::grant(&pool, user, None, t0 + Duration::days(1))
            .await
            .unwrap()
    );
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT user_id, granted_at, granted_by FROM admins")
            .fetch_all(&raw)
            .await
            .unwrap();
    assert_eq!(
        rows,
        [(
            user.to_string(),
            "2026-03-01T10:00:00+00:00".to_owned(),
            None
        )]
    );
}

/// Migrates `raw` to the schema as of 0007, through sqlx's migrator over
/// copies of the committed files (same checksums, so the app's full run
/// accepts them afterwards).
async fn migrate_to_0007(dir: &TempDir, raw: &sqlx::SqlitePool) {
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let old = dir.path().join("migrations");
    std::fs::create_dir_all(&old).unwrap();
    let mut copied = 0;
    for entry in std::fs::read_dir(&migrations).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.as_str() < "0008" {
            std::fs::copy(&path, old.join(&name)).unwrap();
            copied += 1;
        }
    }
    assert_eq!(copied, 7);
    sqlx::migrate::Migrator::new(old.as_path())
        .await
        .unwrap()
        .run(raw)
        .await
        .unwrap();
}

/// P6: migrating a database that already has accounts, lists and shares to
/// 0008 changes none of them: everyone can still sign in and keeps their
/// data, and nobody becomes an admin.
#[tokio::test]
async fn existing_users_unaffected() {
    let dir = TempDir::new();
    let url = format!("sqlite://{}", dir.path().join("old.db").display());
    let raw = raw_pool(&url).await;

    migrate_to_0007(&dir, &raw).await;
    let has_new_tables = |raw: sqlx::SqlitePool| async move {
        let admins = sqlx::query("SELECT 1 FROM admins").fetch_all(&raw).await;
        let invitations = sqlx::query("SELECT 1 FROM invitations")
            .fetch_all(&raw)
            .await;
        (admins.is_ok(), invitations.is_ok())
    };
    assert_eq!(
        has_new_tables(raw.clone()).await,
        (false, false),
        "before 0008"
    );

    // Existing accounts, with a list shared between them.
    let pool = SqlitePool::connect(&url).await.unwrap();
    let hash = password::hash("correct horse battery staple").unwrap();
    let owner = UserRepo::create(&pool, "owner@example.com", "Owner", &hash)
        .await
        .unwrap();
    let friend = UserRepo::create(&pool, "friend@example.com", "Friend", &hash)
        .await
        .unwrap();
    let list = ShoppingListRepo::create(&pool, owner.id, "Kauppa")
        .await
        .unwrap();
    ShoppingListRepo::add_item(&pool, owner.id, list.id, "Maito", Some("1 l"))
        .await
        .unwrap();
    ShoppingListRepo::share(
        &pool,
        owner.id,
        list.id,
        "friend@example.com",
        ShareRole::Edit,
    )
    .await
    .unwrap()
    .unwrap();
    let dump = |raw: sqlx::SqlitePool| async move {
        let users: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, email, display_name, password_hash, created_at FROM users ORDER BY id",
        )
        .fetch_all(&raw)
        .await
        .unwrap();
        let lists: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, owner_id, name FROM shopping_lists ORDER BY id")
                .fetch_all(&raw)
                .await
                .unwrap();
        let shares: Vec<(String, String, String)> =
            sqlx::query_as("SELECT list_id, grantee_id, role FROM list_shares ORDER BY id")
                .fetch_all(&raw)
                .await
                .unwrap();
        format!("{users:?}\n{lists:?}\n{shares:?}")
    };
    let before = dump(raw.clone()).await;

    // The app's own migration run applies 0008.
    pool.run_migrations().await.unwrap();
    assert_eq!(
        has_new_tables(raw.clone()).await,
        (true, true),
        "after 0008"
    );
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&raw)
            .await
            .unwrap();
    assert_eq!(versions, (1..=8).collect::<Vec<i64>>());

    assert_eq!(dump(raw.clone()).await, before, "no existing row changed");
    for user in [&owner, &friend] {
        let (found, stored) = UserRepo::find_by_email(&pool, &user.email)
            .await
            .unwrap()
            .expect("still there");
        assert_eq!(found.id, user.id);
        assert!(password::verify("correct horse battery staple", &stored).unwrap());
        assert!(!AdminRepo::is_admin(&pool, user.id).await.unwrap());
    }
    let items = ShoppingListRepo::items(&pool, owner.id, list.id, false)
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        ShoppingListRepo::shared_with(&pool, friend.id)
            .await
            .unwrap()
            .len(),
        1
    );
    let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admins")
        .fetch_one(&raw)
        .await
        .unwrap();
    assert_eq!(admins, 0, "no default admin");
    raw.close().await;
}
