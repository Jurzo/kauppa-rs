//! MB-0013 (P1): ingredient name suggestions.

use crate::repo::{CategoryRepo, ProductRepo};

use super::{make_category, make_hidden, make_user, test_pool};

/// MB-0013 P1: suggestion names are all of the owner's products — favorites
/// and recipe-created ones alike, categorized or not — as stored, sorted by
/// name ignoring case, and never another user's. The order is `name_key`
/// order, so Å/Ä/Ö fold like ASCII and sort after z by code point.
#[tokio::test]
async fn suggestion_names() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let theirs = make_category(&pool, other, "Hevi").await;

    for name in ["Omena", "äyräs", "banaani", "Åkerbär"] {
        ProductRepo::create(&pool, u, name, hevi).await.unwrap();
    }
    ProductRepo::create(&pool, u, "MAITO", kylma).await.unwrap();
    make_hidden(&pool, u, "Ölut", kylma).await;
    make_hidden(&pool, u, "kaura", hevi).await;
    let gone = make_category(&pool, u, "Poistuva").await;
    ProductRepo::create(&pool, u, "Lanttu", gone).await.unwrap();
    CategoryRepo::delete(&pool, u, gone).await.unwrap();

    for name in ["Aprikoosi", "Salaatti", "maito"] {
        ProductRepo::create(&pool, other, name, theirs)
            .await
            .unwrap();
    }
    make_hidden(&pool, other, "Zucchini", theirs).await;

    assert_eq!(
        ProductRepo::suggestion_names(&pool, u).await.unwrap(),
        [
            "banaani",
            "kaura",
            "Lanttu",
            "MAITO",
            "Omena",
            "äyräs",
            "Åkerbär",
            "Ölut"
        ]
    );
    assert_eq!(
        ProductRepo::suggestion_names(&pool, other).await.unwrap(),
        ["Aprikoosi", "maito", "Salaatti", "Zucchini"]
    );
    let nobody = make_user(&pool, "nobody@example.com").await;
    assert_eq!(
        ProductRepo::suggestion_names(&pool, nobody).await.unwrap(),
        Vec::<String>::new()
    );
}
