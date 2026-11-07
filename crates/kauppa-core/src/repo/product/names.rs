//! MB-0010 (P2, P3): product names are unique ignoring case and stored as typed.

use crate::CoreError;
use crate::repo::ProductRepo;

use super::{make_category, make_user, shape, test_pool};

/// MB-0010 P2: an owner can't have two products whose names differ only by
/// case, ASCII or not, on create or rename — across categories too. Other
/// owners are unaffected.
#[tokio::test]
async fn unique_ignoring_case() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let theirs = make_category(&pool, other, "Kaikki").await;

    for (first, second) in [("maito", "Maito"), ("äyräs", "ÄYRÄS"), ("Öljy", "öljy")] {
        ProductRepo::create(&pool, u, first, hevi).await.unwrap();
        assert!(
            matches!(
                ProductRepo::create(&pool, u, second, kylma).await,
                Err(CoreError::Conflict(_))
            ),
            "{second:?} collides with {first:?}"
        );
        // Trimming happens before matching.
        assert!(matches!(
            ProductRepo::create(&pool, u, &format!(" {second}  "), hevi).await,
            Err(CoreError::Conflict(_))
        ));
        ProductRepo::create(&pool, other, second, theirs)
            .await
            .unwrap();
    }

    // Rename into another row's name, in a different case.
    let omena = ProductRepo::create(&pool, u, "Omena", kylma).await.unwrap();
    assert!(matches!(
        ProductRepo::rename(&pool, u, omena.id, "MAITO").await,
        Err(CoreError::Conflict(_))
    ));
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [
            (Some("Hevi"), vec!["maito", "äyräs", "Öljy"]),
            (Some("Kylmä"), vec!["Omena"]),
        ]
    );
}

/// MB-0010 P3: names are stored and shown as typed (trimmed); only matching
/// ignores case. Renaming a product to another casing of its own name works.
#[tokio::test]
async fn case_only_rename() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;

    let maito = ProductRepo::create(&pool, u, " maito ", kylma)
        .await
        .unwrap();
    assert_eq!(maito.name, "maito");

    let renamed = ProductRepo::rename(&pool, u, maito.id, "Maito")
        .await
        .unwrap();
    assert_eq!(renamed.name, "Maito");
    ProductRepo::rename(&pool, u, maito.id, "täysmaito")
        .await
        .unwrap();
    let renamed = ProductRepo::rename(&pool, u, maito.id, "TÄYSMAITO")
        .await
        .unwrap();
    assert_eq!(renamed.name, "TÄYSMAITO");

    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Kylmä"), vec!["TÄYSMAITO"])]
    );
}
