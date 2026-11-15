//! MB-0061 (P7, P2; RR-0048): `catalog_shares` (migration 0012), the share
//! methods, and the predicate for reading another user's favorites.

use uuid::Uuid;

use crate::error::CoreError;
use crate::repo::{CatalogShareRepo, CategoryRepo, ProductRepo, SqlitePool, UserRepo};

async fn pool_and_raw() -> (SqlitePool, sqlx::SqlitePool) {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    let url = format!(
        "sqlite:file:catalog-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect");
    pool.run_migrations().await.expect("migrate");
    let raw = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::from_str(&url)
                .expect("url")
                .foreign_keys(true),
        )
        .await
        .expect("raw");
    (pool, raw)
}

async fn user(pool: &SqlitePool, email: &str, name: &str) -> Uuid {
    UserRepo::create(pool, email, name, "argon2-hash")
        .await
        .expect("user")
        .id
}

/// Sharing by email matches one other account ignoring ASCII case, sharing
/// again changes the switches (same id), both off or revoking deletes the
/// share, and an unknown or own email shares nothing — all answered alike.
#[tokio::test]
async fn share_change_and_revoke() {
    let (pool, _raw) = pool_and_raw().await;
    let me = user(&pool, "me@example.com", "Minä").await;
    let friend = user(&pool, "Friend@Example.com", "Ystävä").await;

    for email in ["nobody@example.com", "ME@example.com"] {
        assert_eq!(
            CatalogShareRepo::share_catalog(&pool, me, email, true, true)
                .await
                .unwrap(),
            None
        );
    }
    assert!(matches!(
        CatalogShareRepo::share_catalog(&pool, me, "  ", true, false).await,
        Err(CoreError::Invalid(_))
    ));
    let share = CatalogShareRepo::share_catalog(&pool, me, " friend@EXAMPLE.com ", true, false)
        .await
        .unwrap()
        .expect("matched");
    assert_eq!(
        (share.grantee_id, share.favorites, share.recipes),
        (friend, true, false)
    );
    assert_eq!(share.grantee_name, "Ystävä");

    let again = CatalogShareRepo::share_catalog(&pool, me, "friend@example.com", true, true)
        .await
        .unwrap()
        .expect("matched");
    assert_eq!(
        (again.id, again.recipes),
        (share.id, true),
        "same share, switches changed"
    );
    let changed = CatalogShareRepo::set_catalog_share(&pool, me, share.id, false, true)
        .await
        .unwrap()
        .expect("kept");
    assert_eq!((changed.favorites, changed.recipes), (false, true));
    assert_eq!(
        CatalogShareRepo::catalog_shares(&pool, me)
            .await
            .unwrap()
            .len(),
        1
    );

    // Only the owner can change or revoke it.
    assert!(matches!(
        CatalogShareRepo::set_catalog_share(&pool, friend, share.id, true, true).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        CatalogShareRepo::revoke_catalog_share(&pool, friend, share.id).await,
        Err(CoreError::NotFound)
    ));

    // Both off deletes it.
    assert_eq!(
        CatalogShareRepo::set_catalog_share(&pool, me, share.id, false, false)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        CatalogShareRepo::catalog_shares(&pool, me).await.unwrap(),
        []
    );

    let share = CatalogShareRepo::share_catalog(&pool, me, "friend@example.com", true, true)
        .await
        .unwrap()
        .expect("matched");
    CatalogShareRepo::revoke_catalog_share(&pool, me, share.id)
        .await
        .unwrap();
    assert_eq!(
        CatalogShareRepo::shared_with(&pool, friend).await.unwrap(),
        []
    );
}

/// P2: the recipient reads the sharer's favorites only through a share with
/// the favorites switch on; otherwise, and for anyone else, `NotFound`. The
/// sharer is shown by display name; hidden products aren't shared.
#[tokio::test]
async fn favorites_only_through_the_predicate() {
    let (pool, _raw) = pool_and_raw().await;
    let me = user(&pool, "me@example.com", "Minä").await;
    let friend = user(&pool, "friend@example.com", "Ystävä").await;
    let stranger = user(&pool, "stranger@example.com", "Vieras").await;
    let hevi = CategoryRepo::create(&pool, me, "Hevi").await.unwrap().id;
    let kylma = CategoryRepo::create(&pool, me, "Kylmä").await.unwrap().id;
    for (name, category) in [("Omena", hevi), ("Maito", kylma), ("Banaani", hevi)] {
        ProductRepo::create(&pool, me, name, category)
            .await
            .unwrap();
    }

    let read = |who: Uuid| CatalogShareRepo::shared_favorites(&pool, who, me);
    assert!(
        matches!(read(friend).await, Err(CoreError::NotFound)),
        "no share"
    );
    let share = CatalogShareRepo::share_catalog(&pool, me, "friend@example.com", false, true)
        .await
        .unwrap()
        .expect("matched");
    assert!(
        matches!(read(friend).await, Err(CoreError::NotFound)),
        "switch off"
    );

    CatalogShareRepo::set_catalog_share(&pool, me, share.id, true, true)
        .await
        .unwrap();
    let shared = read(friend).await.unwrap();
    assert_eq!(shared.owner_name, "Minä");
    let groups: Vec<(String, Vec<String>)> = shared
        .groups
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name).unwrap_or_default(),
                g.products.into_iter().map(|p| p.name).collect(),
            )
        })
        .collect();
    assert_eq!(
        groups,
        [
            (
                "Hevi".to_owned(),
                vec!["Omena".to_owned(), "Banaani".to_owned()]
            ),
            ("Kylmä".to_owned(), vec!["Maito".to_owned()]),
        ]
    );
    assert!(
        matches!(read(stranger).await, Err(CoreError::NotFound)),
        "not shared with them"
    );
    assert!(
        matches!(
            CatalogShareRepo::shared_favorites(&pool, me, friend).await,
            Err(CoreError::NotFound)
        ),
        "a share goes one way"
    );
    let with_friend = CatalogShareRepo::shared_with(&pool, friend).await.unwrap();
    assert_eq!(with_friend.len(), 1);
    assert_eq!(
        (
            with_friend[0].owner_id,
            with_friend[0].owner_name.as_str(),
            with_friend[0].favorites
        ),
        (me, "Minä", true)
    );

    CatalogShareRepo::revoke_catalog_share(&pool, me, share.id)
        .await
        .unwrap();
    assert!(
        matches!(read(friend).await, Err(CoreError::NotFound)),
        "revoked"
    );
}

/// P7: migration 0012's table and column; deleting either user deletes the
/// share; deleting a shared favorite only stops showing it; a share can't
/// point at its own owner; deleting an original recipe leaves the copy with
/// `imported_from` NULL.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn cascades() {
    let (pool, raw) = pool_and_raw().await;
    let me = user(&pool, "me@example.com", "Minä").await;
    let friend = user(&pool, "friend@example.com", "Ystävä").await;
    let hevi = CategoryRepo::create(&pool, me, "Hevi").await.unwrap().id;
    let omena = ProductRepo::create(&pool, me, "Omena", hevi)
        .await
        .unwrap()
        .id;
    CatalogShareRepo::share_catalog(&pool, me, "friend@example.com", true, true)
        .await
        .unwrap()
        .expect("matched");

    // No share with oneself, even written directly.
    let own = sqlx::query(
        "INSERT INTO catalog_shares (id, owner_id, grantee_id, favorites, recipes, created_at)
         VALUES (?1, ?2, ?2, 1, 1, '2026-11-15T00:00:00Z')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(me.to_string())
    .execute(&raw)
    .await;
    assert!(own.is_err(), "CHECK (owner_id <> grantee_id)");

    ProductRepo::delete(&pool, me, omena).await.unwrap();
    let shared = CatalogShareRepo::shared_favorites(&pool, friend, me)
        .await
        .unwrap();
    assert!(shared.groups.is_empty(), "the deleted favorite isn't shown");
    assert_eq!(
        CatalogShareRepo::catalog_shares(&pool, me)
            .await
            .unwrap()
            .len(),
        1,
        "the share stays"
    );

    // `recipes.imported_from`: a copy outlives its original.
    let now = "2026-11-15T00:00:00Z";
    let (original, copy) = (Uuid::new_v4().to_string(), Uuid::new_v4().to_string());
    for (id, owner, from) in [
        (&original, me, None),
        (&copy, friend, Some(original.clone())),
    ] {
        sqlx::query(
            "INSERT INTO recipes (id, owner_id, title, created_at, updated_at, imported_from)
             VALUES (?1, ?2, 'Puuro', ?3, ?3, ?4)",
        )
        .bind(id)
        .bind(owner.to_string())
        .bind(now)
        .bind(from)
        .execute(&raw)
        .await
        .unwrap();
    }
    sqlx::query("DELETE FROM recipes WHERE id = ?1")
        .bind(&original)
        .execute(&raw)
        .await
        .unwrap();
    let from: Option<String> =
        sqlx::query_scalar("SELECT imported_from FROM recipes WHERE id = ?1")
            .bind(&copy)
            .fetch_one(&raw)
            .await
            .unwrap();
    assert_eq!(from, None, "the copy stays, unlinked");

    // Deleting either user deletes the share: the grantee, then the owner.
    let count = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM catalog_shares")
            .fetch_one(&raw)
            .await
            .unwrap()
    };
    let delete_user = |id: Uuid| {
        sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(id.to_string())
            .execute(&raw)
    };
    assert_eq!(count().await, 1);
    delete_user(friend).await.unwrap();
    assert_eq!(count().await, 0, "the grantee's deletion removed the share");
    let other = user(&pool, "other@example.com", "Muu").await;
    CatalogShareRepo::share_catalog(&pool, me, "other@example.com", true, false)
        .await
        .unwrap()
        .expect("matched");
    assert_eq!(count().await, 1);
    delete_user(me).await.unwrap();
    assert_eq!(count().await, 0, "the owner's deletion removed the share");
    assert_eq!(
        CatalogShareRepo::shared_with(&pool, other).await.unwrap(),
        []
    );
}
