//! MB-0071 P3: dragging a favorite into another group saves its category and
//! the group's new order in one transaction (`move_favorite_into_group`), or
//! changes nothing.

use uuid::Uuid;

use crate::CoreError;
use crate::repo::ProductRepo;

use super::{make_category, make_hidden, make_user, shape, test_pool};
use crate::domain::Product;
use crate::repo::SqlitePool;

/// `owner`'s product called `name`.
async fn product(pool: &SqlitePool, owner: Uuid, name: &str) -> Product {
    ProductRepo::find_by_name(pool, owner, name)
        .await
        .unwrap()
        .expect(name)
}

/// A favorite dropped between two of another group's favorites takes that
/// group's category and that place; the group's hidden product keeps its
/// slot; the group it left keeps its order.
#[tokio::test]
async fn saves_category_and_order_together() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let omena = ProductRepo::create(&pool, u, "Omena", hevi)
        .await
        .unwrap()
        .id;
    make_hidden(&pool, u, "Tilli", hevi).await;
    let kurkku = ProductRepo::create(&pool, u, "Kurkku", hevi)
        .await
        .unwrap()
        .id;
    let maito = ProductRepo::create(&pool, u, "Maito", kylma)
        .await
        .unwrap()
        .id;
    let voi = ProductRepo::create(&pool, u, "Voi", kylma)
        .await
        .unwrap()
        .id;

    // Maito into Hevi, between Omena and Kurkku.
    assert!(
        ProductRepo::move_favorite_into_group(&pool, u, maito, Some(hevi), &[omena, maito, kurkku])
            .await
            .unwrap()
    );
    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        vec![
            (Some("Hevi"), vec!["Omena", "Maito", "Kurkku"]),
            (Some("Kylmä"), vec!["Voi"]),
        ]
    );
    assert_eq!(product(&pool, u, "Maito").await.category_id, Some(hevi));

    // Tilli (hidden) kept its slot between the favorites: second of four.
    let mut hevi_rows = Vec::new();
    for name in ["Omena", "Tilli", "Maito", "Kurkku"] {
        hevi_rows.push((product(&pool, u, name).await.position, name));
    }
    hevi_rows.sort_unstable();
    assert_eq!(
        hevi_rows.iter().map(|(_, n)| *n).collect::<Vec<_>>(),
        ["Omena", "Tilli", "Maito", "Kurkku"]
    );

    // Into Uncategorized: the category is cleared (RR-0046).
    assert!(
        ProductRepo::move_favorite_into_group(&pool, u, voi, None, &[voi])
            .await
            .unwrap()
    );
    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        vec![
            (Some("Hevi"), vec!["Omena", "Maito", "Kurkku"]),
            (None, vec!["Voi"]),
        ]
    );
    assert_eq!(product(&pool, u, "Voi").await.category_id, None);

    // And out of it again, to the front of Kylmä's (now empty) group.
    assert!(
        ProductRepo::move_favorite_into_group(&pool, u, voi, Some(kylma), &[voi])
            .await
            .unwrap()
    );
    assert_eq!(product(&pool, u, "Voi").await.category_id, Some(kylma));
}

/// Anything but "the group's favorites plus the moved one" changes nothing:
/// neither the category nor either group's order.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn rejected_move_changes_nothing() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let theirs = make_category(&pool, other, "X").await;
    let omena = ProductRepo::create(&pool, u, "Omena", hevi)
        .await
        .unwrap()
        .id;
    let kurkku = ProductRepo::create(&pool, u, "Kurkku", hevi)
        .await
        .unwrap()
        .id;
    let maito = ProductRepo::create(&pool, u, "Maito", kylma)
        .await
        .unwrap()
        .id;
    let voi = ProductRepo::create(&pool, u, "Voi", kylma)
        .await
        .unwrap()
        .id;
    let tilli = make_hidden(&pool, u, "Tilli", kylma).await;
    let foreign = ProductRepo::create(&pool, other, "Leipä", theirs)
        .await
        .unwrap()
        .id;
    let before = shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap())
        .into_iter()
        .map(|(g, p)| {
            (
                g.map(str::to_owned),
                p.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let unchanged = || async {
        let after = shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap())
            .into_iter()
            .map(|(g, p)| {
                (
                    g.map(str::to_owned),
                    p.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(after, before);
        assert_eq!(product(&pool, u, "Maito").await.category_id, Some(kylma));
    };

    // Not exactly Hevi's favorites plus Maito.
    for order in [
        vec![omena, maito],                // Kurkku missing
        vec![omena, kurkku],               // Maito missing
        vec![omena, maito, kurkku, maito], // a repeat
        vec![omena, maito, kurkku, voi],   // another from elsewhere
        vec![omena, maito, kurkku, Uuid::new_v4()],
    ] {
        assert!(
            !ProductRepo::move_favorite_into_group(&pool, u, maito, Some(hevi), &order)
                .await
                .unwrap(),
            "{order:?}"
        );
        unchanged().await;
    }
    // Already in the group: a move into its own group isn't one.
    assert!(
        !ProductRepo::move_favorite_into_group(&pool, u, maito, Some(kylma), &[maito, voi])
            .await
            .unwrap()
    );
    unchanged().await;
    // A recipe-created product doesn't move into a group (MB-0071 P5).
    assert!(
        !ProductRepo::move_favorite_into_group(
            &pool,
            u,
            tilli,
            Some(hevi),
            &[omena, tilli, kurkku]
        )
        .await
        .unwrap()
    );
    unchanged().await;

    // Another user's category or product: NotFound, nothing changed.
    let err = ProductRepo::move_favorite_into_group(&pool, u, maito, Some(theirs), &[maito])
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound), "{err:?}");
    let err = ProductRepo::move_favorite_into_group(
        &pool,
        u,
        foreign,
        Some(hevi),
        &[omena, foreign, kurkku],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, CoreError::NotFound), "{err:?}");
    unchanged().await;
    assert_eq!(
        product(&pool, other, "Leipä").await.category_id,
        Some(theirs)
    );
}
