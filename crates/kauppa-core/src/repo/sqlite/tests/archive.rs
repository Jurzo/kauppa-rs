//! MB-0062 P6 (RR-0045): `shopping_lists.archived_at` (migration 0011), the
//! access predicate's `archived_at`, and `begin_write`'s refusal of item and
//! recipe writes on an archived list. Every list write is covered (RR-0045
//! constraint 2): refused, or allowed for the owner actions.

use uuid::Uuid;

use crate::domain::{ListAccess, ShareRole};
use crate::error::{Conflict, CoreError};
use crate::repo::{ShoppingListRepo, SqlitePool, UserRepo};

async fn memory_pool() -> SqlitePool {
    let url = format!(
        "sqlite:file:archive-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    pool
}

async fn user(pool: &SqlitePool, email: &str) -> Uuid {
    UserRepo::create(pool, email, email, "argon2-hash")
        .await
        .expect("create user")
        .id
}

async fn own_names(pool: &SqlitePool, user: Uuid) -> Vec<String> {
    ShoppingListRepo::list_for_owner(pool, user)
        .await
        .unwrap()
        .into_iter()
        .map(|l| l.name)
        .collect()
}

async fn archived_names(pool: &SqlitePool, user: Uuid) -> Vec<(String, ListAccess)> {
    ShoppingListRepo::archived(pool, user)
        .await
        .unwrap()
        .into_iter()
        .map(|a| (a.list.name, a.access))
        .collect()
}

fn is_archived_conflict<T: std::fmt::Debug>(result: &Result<T, CoreError>) -> bool {
    matches!(result, Err(CoreError::Conflict(Conflict::Archived)))
}

/// Lists start active; archiving moves a list out of the owner's lists into
/// `archived`, without changing `updated_at`; archiving again keeps the first
/// time; restoring brings it back to its place, `updated_at` still unchanged.
#[tokio::test]
async fn archive_and_restore() {
    let pool = memory_pool().await;
    let u = user(&pool, "u@example.com").await;
    let old = ShoppingListRepo::create(&pool, u, "Vanha").await.unwrap();
    let new = ShoppingListRepo::create(&pool, u, "Uusi").await.unwrap();
    assert_eq!(old.archived_at, None, "a list starts active");
    assert_eq!(own_names(&pool, u).await, ["Uusi", "Vanha"]);
    assert_eq!(archived_names(&pool, u).await, []);

    let archived = ShoppingListRepo::archive(&pool, u, old.id).await.unwrap();
    let at = archived.archived_at.expect("archived_at set");
    assert_eq!(archived.updated_at, old.updated_at, "updated_at unchanged");
    assert_eq!(own_names(&pool, u).await, ["Uusi"]);
    assert_eq!(
        archived_names(&pool, u).await,
        [("Vanha".to_owned(), ListAccess::Owner)]
    );
    // The predicate still reaches it: reads work and show the state.
    let got = ShoppingListRepo::get(&pool, u, old.id).await.unwrap();
    assert_eq!(got.archived_at, Some(at));

    let again = ShoppingListRepo::archive(&pool, u, old.id).await.unwrap();
    assert_eq!(again.archived_at, Some(at), "the first time is kept");

    let restored = ShoppingListRepo::unarchive(&pool, u, old.id).await.unwrap();
    assert_eq!(restored.archived_at, None);
    assert_eq!(restored.updated_at, old.updated_at, "updated_at unchanged");
    assert_eq!(
        own_names(&pool, u).await,
        ["Uusi", "Vanha"],
        "back in its usual place"
    );
    assert_eq!(archived_names(&pool, u).await, []);
    // Restoring an active list is a no-op.
    let noop = ShoppingListRepo::unarchive(&pool, u, new.id).await.unwrap();
    assert_eq!(noop.archived_at, None);
}

/// Most recently archived first (RR-0045 sub-answer 5), own and shared
/// lists together, each with the user's access.
#[tokio::test]
async fn archived_order_and_access() {
    let pool = memory_pool().await;
    let u = user(&pool, "u@example.com").await;
    let friend = user(&pool, "friend@example.com").await;
    let first = ShoppingListRepo::create(&pool, u, "Ensin").await.unwrap();
    let second = ShoppingListRepo::create(&pool, u, "Toiseksi")
        .await
        .unwrap();
    let theirs = ShoppingListRepo::create(&pool, friend, "Heidän")
        .await
        .unwrap();
    ShoppingListRepo::share(&pool, friend, theirs.id, "u@example.com", ShareRole::View)
        .await
        .unwrap()
        .expect("shared");

    for id in [second.id, first.id] {
        ShoppingListRepo::archive(&pool, u, id).await.unwrap();
    }
    ShoppingListRepo::archive(&pool, friend, theirs.id)
        .await
        .unwrap();
    assert_eq!(
        archived_names(&pool, u).await,
        [
            ("Heidän".to_owned(), ListAccess::View),
            ("Ensin".to_owned(), ListAccess::Owner),
            ("Toiseksi".to_owned(), ListAccess::Owner),
        ]
    );
    assert!(
        ShoppingListRepo::shared_with(&pool, u)
            .await
            .unwrap()
            .is_empty(),
        "an archived shared list leaves 'shared with you'"
    );
}

/// Every write on an archived list: item and recipe writes are refused with
/// `Conflict::Archived` and change nothing; rename, share, revoke, delete,
/// archive and restore are allowed for the owner. Grantees and strangers get
/// their usual `Forbidden` / `NotFound` first, and can't archive or restore.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn writes_on_an_archived_list() {
    let pool = memory_pool().await;
    let owner = user(&pool, "owner@example.com").await;
    let editor = user(&pool, "editor@example.com").await;
    let viewer = user(&pool, "viewer@example.com").await;
    let stranger = user(&pool, "stranger@example.com").await;
    let list = ShoppingListRepo::create(&pool, owner, "Mökki")
        .await
        .unwrap();
    let item = ShoppingListRepo::add_item(&pool, owner, list.id, "Maito", Some("1 l"))
        .await
        .unwrap();
    let gone = ShoppingListRepo::add_item(&pool, owner, list.id, "Leipä", None)
        .await
        .unwrap();
    ShoppingListRepo::remove_item(&pool, owner, list.id, gone.id)
        .await
        .unwrap();
    for (email, role) in [
        ("editor@example.com", ShareRole::Edit),
        ("viewer@example.com", ShareRole::View),
    ] {
        ShoppingListRepo::share(&pool, owner, list.id, email, role)
            .await
            .unwrap()
            .expect("shared");
    }

    // Only the owner archives: a grantee is refused, a stranger sees nothing.
    for (who, expected) in [
        (editor, "Forbidden"),
        (viewer, "Forbidden"),
        (stranger, "NotFound"),
    ] {
        for result in [
            ShoppingListRepo::archive(&pool, who, list.id).await,
            ShoppingListRepo::unarchive(&pool, who, list.id).await,
        ] {
            let got = match &result {
                Err(CoreError::Forbidden) => "Forbidden".to_owned(),
                Err(CoreError::NotFound) => "NotFound".to_owned(),
                other => format!("{other:?}"),
            };
            assert_eq!(got, expected);
        }
    }
    ShoppingListRepo::archive(&pool, owner, list.id)
        .await
        .unwrap();
    let before = (
        ShoppingListRepo::items(&pool, owner, list.id, false)
            .await
            .unwrap(),
        ShoppingListRepo::items(&pool, owner, list.id, true)
            .await
            .unwrap(),
        ShoppingListRepo::get(&pool, owner, list.id).await.unwrap(),
    );

    // Refused: every item and recipe write, for the owner and the editor.
    let any = Uuid::new_v4();
    for who in [owner, editor] {
        let refused = [
            is_archived_conflict(
                &ShoppingListRepo::add_item(&pool, who, list.id, "Kahvi", None).await,
            ),
            is_archived_conflict(
                &ShoppingListRepo::add_owner_favorite_by_name(&pool, who, list.id, "Kahvi", None)
                    .await,
            ),
            is_archived_conflict(
                &ShoppingListRepo::set_item_quantity(&pool, who, list.id, item.id, Some("2 l"))
                    .await,
            ),
            is_archived_conflict(
                &ShoppingListRepo::update_item(&pool, who, list.id, item.id, None, Some("2 l"))
                    .await,
            ),
            is_archived_conflict(
                &ShoppingListRepo::set_checked(&pool, who, list.id, item.id, true).await,
            ),
            is_archived_conflict(
                &ShoppingListRepo::remove_item(&pool, who, list.id, item.id).await,
            ),
            is_archived_conflict(
                &ShoppingListRepo::restore_item(&pool, who, list.id, gone.id).await,
            ),
        ];
        assert_eq!(refused, [true; 7], "{who}");
    }
    // The owner-only item and recipe writes: refused for the owner; the
    // editor's role is checked first.
    let owner_only = [
        ShoppingListRepo::add_product(&pool, owner, list.id, any, None)
            .await
            .map(|_| ()),
        ShoppingListRepo::add_recipe(&pool, owner, list.id, any, 1.0)
            .await
            .map(|_| ()),
        ShoppingListRepo::assign_category(&pool, owner, list.id, item.id, any)
            .await
            .map(|_| ()),
        ShoppingListRepo::update_item(&pool, owner, list.id, item.id, Some(any), None)
            .await
            .map(|_| ()),
    ];
    for result in &owner_only {
        assert!(is_archived_conflict(result), "{result:?}");
    }
    assert!(matches!(
        ShoppingListRepo::add_product(&pool, editor, list.id, any, None).await,
        Err(CoreError::Forbidden)
    ));
    // A viewer and a stranger keep their usual answers.
    assert!(matches!(
        ShoppingListRepo::add_item(&pool, viewer, list.id, "Kahvi", None).await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        ShoppingListRepo::add_item(&pool, stranger, list.id, "Kahvi", None).await,
        Err(CoreError::NotFound)
    ));

    let after = (
        ShoppingListRepo::items(&pool, owner, list.id, false)
            .await
            .unwrap(),
        ShoppingListRepo::items(&pool, owner, list.id, true)
            .await
            .unwrap(),
        ShoppingListRepo::get(&pool, owner, list.id).await.unwrap(),
    );
    assert_eq!(
        format!("{after:?}"),
        format!("{before:?}"),
        "nothing changed"
    );

    // Allowed for the owner: rename, share, revoke; the list stays archived.
    let renamed = ShoppingListRepo::rename(&pool, owner, list.id, "Mökki 2025")
        .await
        .unwrap();
    assert!(renamed.archived_at.is_some());
    let share = ShoppingListRepo::share(
        &pool,
        owner,
        list.id,
        "stranger@example.com",
        ShareRole::View,
    )
    .await
    .unwrap()
    .expect("shared");
    ShoppingListRepo::revoke(&pool, owner, list.id, share.id)
        .await
        .unwrap();
    assert_eq!(
        archived_names(&pool, viewer).await,
        [("Mökki 2025".to_owned(), ListAccess::View)],
        "shares unchanged by archiving"
    );

    // Restored, the same writes work again.
    ShoppingListRepo::unarchive(&pool, owner, list.id)
        .await
        .unwrap();
    ShoppingListRepo::add_item(&pool, editor, list.id, "Kahvi", None)
        .await
        .unwrap();

    // Delete is allowed on an archived list.
    ShoppingListRepo::archive(&pool, owner, list.id)
        .await
        .unwrap();
    ShoppingListRepo::delete(&pool, owner, list.id)
        .await
        .unwrap();
    assert_eq!(archived_names(&pool, owner).await, []);
    assert_eq!(archived_names(&pool, viewer).await, []);
}
