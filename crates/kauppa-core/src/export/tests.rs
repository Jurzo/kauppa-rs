//! MB-0060 plan step 2: the export plan, the fingerprint and the review's
//! answers (RR-0047), without a database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::Utc;
use uuid::Uuid;

use super::{Action, ReviewAnswers, ReviewKind, ReviewReason, SKIP, Write, decide, plan, writes};
use crate::CoreError;
use crate::domain::{Category, ListItem, Product};

fn category(owner: Uuid, name: &str) -> Category {
    Category {
        id: Uuid::new_v4(),
        owner_id: owner,
        name: name.to_owned(),
        position: 0,
        created_at: Utc::now(),
    }
}

fn item(text: &str, category: Option<&Category>) -> ListItem {
    ListItem {
        id: Uuid::new_v4(),
        list_id: Uuid::nil(),
        product_id: None,
        text: text.to_owned(),
        quantity: Some("2".to_owned()),
        checked: false,
        position: 0,
        category: category.cloned(),
        removed_at: None,
    }
}

fn product(owner: Uuid, name: &str, category: Option<&Category>, favorite: bool) -> Product {
    Product {
        id: Uuid::new_v4(),
        owner_id: owner,
        name: name.to_owned(),
        category_id: category.map(|c| c.id),
        position: 0,
        is_favorite: favorite,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// The owner's own export: new, already there, promote, conflict,
/// uncategorized, and copies, each classified; names match ignoring case and
/// one name is one row.
#[test]
fn classifies_rows() {
    let me = Uuid::new_v4();
    let (hevi, kylma) = (category(me, "Hevi"), category(me, "Kylmä"));
    let cats = [hevi.clone(), kylma.clone()];
    let products = [
        product(me, "Maito", Some(&kylma), true),
        product(me, "Kurkku", Some(&hevi), false),
        product(me, "Omena", Some(&kylma), true),
        product(me, "Tomaatti", Some(&kylma), false),
    ];
    let items = [
        item("Banaani", Some(&hevi)),
        item("maito", Some(&kylma)),
        item("Kurkku", Some(&hevi)),
        item("Omena", Some(&hevi)),
        item("Tomaatti", Some(&hevi)),
        item("Kynttilät", None),
        item("Leipä", Some(&hevi)),
        item("LEIPÄ", Some(&kylma)),
        item("Banaani", Some(&hevi)),
    ];
    let plan = plan(me, &items, &cats, &products);
    let actions: Vec<(&str, Action)> = plan
        .rows
        .iter()
        .map(|r| (r.name.as_str(), r.action))
        .collect();
    assert_eq!(
        actions,
        [
            ("Banaani", Action::Add(hevi.id)),
            ("maito", Action::AlreadyThere),
            ("Kurkku", Action::Promote(hevi.id)),
            ("Omena", Action::Review(ReviewKind::Conflict)),
            ("Tomaatti", Action::Review(ReviewKind::Conflict)),
            ("Kynttilät", Action::Review(ReviewKind::Uncategorized)),
            ("Leipä", Action::Review(ReviewKind::Copies)),
        ]
    );
    // Choices: a favorite in conflict keeps or moves, no skip; a product
    // from recipes may be skipped; copies offer each category.
    let row = |name: &str| plan.rows.iter().find(|r| r.name == name).unwrap();
    let omena = row("Omena").choices().unwrap();
    assert_eq!(
        (omena.keep, omena.list.clone(), omena.skip),
        (Some(kylma.id), vec![hevi.id], false)
    );
    let tomaatti = row("Tomaatti").choices().unwrap();
    assert_eq!((tomaatti.keep, tomaatti.skip), (Some(kylma.id), true));
    let leipa = row("Leipä").choices().unwrap();
    assert_eq!(
        (leipa.keep, leipa.list.clone(), leipa.skip),
        (None, vec![hevi.id, kylma.id], true)
    );
    let kynttilat = row("Kynttilät").choices().unwrap();
    assert!(kynttilat.any && kynttilat.skip);
    assert!(row("Banaani").choices().is_none());
}

/// A grantee's export: the list owner's categories map by name to the
/// grantee's own, ignoring case; an unmatched one counts as uncategorized
/// and its name is kept for display. No owner category id ever appears.
#[test]
fn grantee_maps_categories_by_name() {
    let owner = Uuid::new_v4();
    let me = Uuid::new_v4();
    let (their_hevi, their_pakaste) = (category(owner, "Hevi"), category(owner, "Pakasteet"));
    let my_hevi = category(me, "HEVI");
    let items = [
        item("Omena", Some(&their_hevi)),
        item("Jäätelö", Some(&their_pakaste)),
    ];
    let plan = plan(me, &items, std::slice::from_ref(&my_hevi), &[]);
    assert_eq!(plan.rows[0].action, Action::Add(my_hevi.id));
    assert_eq!(
        plan.rows[1].action,
        Action::Review(ReviewKind::Uncategorized)
    );
    assert_eq!(plan.rows[1].unmatched, ["Pakasteet"]);
    for row in &plan.rows {
        assert!(!row.list_categories.contains(&their_hevi.id));
    }
}

/// The fingerprint changes with the list and with the catalog, and not
/// otherwise.
#[test]
fn fingerprint_tracks_inputs() {
    let me = Uuid::new_v4();
    let hevi = category(me, "Hevi");
    let cats = [hevi.clone()];
    let items = vec![item("Omena", Some(&hevi))];
    let base = plan(me, &items, &cats, &[]).fingerprint;
    assert_eq!(
        plan(me, &items, &cats, &[]).fingerprint,
        base,
        "deterministic"
    );
    let mut more = items.clone();
    more.push(item("Päärynä", None));
    assert_ne!(
        plan(me, &more, &cats, &[]).fingerprint,
        base,
        "list changed"
    );
    let omena = product(me, "Omena", None, false);
    assert_ne!(
        plan(me, &items, &cats, std::slice::from_ref(&omena)).fingerprint,
        base,
        "catalog changed"
    );
    let renamed = Category {
        name: "Vihannekset".to_owned(),
        ..hevi
    };
    assert_ne!(
        plan(me, &items, &[renamed], &[]).fingerprint,
        base,
        "category renamed"
    );
}

/// Answers: a changed fingerprint or a missing answer shows the review
/// again; an index that isn't a review row, a skip where none is offered, or
/// a category that isn't a choice is refused; good answers become writes and
/// a summary.
#[test]
fn answers_are_checked() {
    let me = Uuid::new_v4();
    let (hevi, kylma, leipa) = (
        category(me, "Hevi"),
        category(me, "Kylmä"),
        category(me, "Leipä"),
    );
    let cats = [hevi.clone(), kylma.clone(), leipa.clone()];
    let omena = product(me, "Omena", Some(&kylma), true);
    let tomaatti = product(me, "Tomaatti", Some(&kylma), false);
    let items = [
        item("Banaani", Some(&hevi)),
        item("Omena", Some(&hevi)),
        item("Tomaatti", Some(&hevi)),
        item("Kynttilät", None),
    ];
    let plan = plan(me, &items, &cats, &[omena.clone(), tomaatti.clone()]);
    let answers = |pairs: &[(usize, &str)]| ReviewAnswers {
        fingerprint: plan.fingerprint.clone(),
        answers: pairs.iter().map(|(i, a)| (*i, (*a).to_owned())).collect(),
    };
    let (h, k, l) = (
        hevi.id.to_string(),
        kylma.id.to_string(),
        leipa.id.to_string(),
    );

    let stale = ReviewAnswers {
        fingerprint: "0".repeat(64),
        ..answers(&[])
    };
    assert_eq!(
        decide(&plan, &cats, &stale).unwrap(),
        Err(ReviewReason::Changed)
    );
    assert_eq!(
        decide(&plan, &cats, &answers(&[(1, &h), (2, SKIP)])).unwrap(),
        Err(ReviewReason::Unanswered)
    );
    for bad in [
        answers(&[(0, &h), (1, &h), (2, SKIP), (3, &l)]),
        answers(&[(1, SKIP), (2, SKIP), (3, &l)]),
        answers(&[(1, &l), (2, SKIP), (3, &l)]),
        answers(&[(1, &h), (2, SKIP), (3, &Uuid::new_v4().to_string())]),
        answers(&[(1, "nonsense"), (2, SKIP), (3, &l)]),
    ] {
        assert!(
            matches!(decide(&plan, &cats, &bad), Err(CoreError::Invalid(_))),
            "{bad:?}"
        );
    }

    let decisions = decide(&plan, &cats, &answers(&[(1, &h), (2, &k), (3, SKIP)]))
        .unwrap()
        .unwrap();
    let (writes, summary) = writes(&plan, &decisions);
    assert_eq!(
        writes,
        [
            Write::Insert {
                name: "Banaani".to_owned(),
                category: hevi.id
            },
            Write::Favorite {
                id: omena.id,
                category: hevi.id
            },
            Write::Favorite {
                id: tomaatti.id,
                category: kylma.id
            },
        ]
    );
    assert_eq!(
        (
            summary.added,
            summary.recategorized,
            summary.made_favorite,
            summary.skipped,
            summary.already
        ),
        (1, 1, 1, 1, 0)
    );
}
