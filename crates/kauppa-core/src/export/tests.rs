//! MB-0060 plan step 2: the export plan, the fingerprint and the review's
//! answers (RR-0047), without a database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::Utc;
use uuid::Uuid;

use super::{
    Action, Candidate, ReviewAnswers, ReviewKind, ReviewReason, SKIP, Write, decide, plan, writes,
};
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

fn item(text: &str, category: Option<&Category>) -> Candidate {
    Candidate::from(&ListItem {
        id: Uuid::new_v4(),
        list_id: Uuid::nil(),
        product_id: None,
        text: text.to_owned(),
        quantity: Some("2".to_owned()),
        checked: false,
        position: 0,
        category: category.cloned(),
        removed_at: None,
    })
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

/// MB-0061 (RR-0048 sub-answer 6): a recipe's ingredients. An existing
/// product with a category is linked as it is (its category and favorite
/// flag untouched, no conflict); one without a category, or a new one whose
/// category doesn't map, needs a category (no skip); a new one whose
/// category maps is created hidden there.
#[test]
fn ingredients_link_or_create() {
    use super::{IngredientProduct, ingredient_products, plan_ingredients};

    let sharer = Uuid::new_v4();
    let me = Uuid::new_v4();
    let (their_hevi, their_mauste) = (category(sharer, "Hevi"), category(sharer, "Mausteet"));
    let my_hevi = category(me, "hevi");
    let my_kylma = category(me, "Kylmä");
    let cats = [my_hevi.clone(), my_kylma.clone()];
    let maito = product(me, "Maito", Some(&my_kylma), true);
    let suola = product(me, "Suola", None, false);
    let items = [
        item("maito", Some(&their_hevi)),
        item("Suola", Some(&their_mauste)),
        item("Kurkku", Some(&their_hevi)),
        item("Pippuri", Some(&their_mauste)),
    ];
    let plan = plan_ingredients(me, &items, &cats, &[maito.clone(), suola.clone()]);
    let actions: Vec<Action> = plan.rows.iter().map(|r| r.action).collect();
    assert_eq!(
        actions,
        [
            Action::AlreadyThere,
            Action::Review(ReviewKind::Uncategorized),
            Action::Add(my_hevi.id),
            Action::Review(ReviewKind::Uncategorized),
        ]
    );
    for index in [1, 3] {
        let choices = plan.rows[index].choices().unwrap();
        assert!(choices.any && !choices.skip, "a category, never skip");
    }
    assert_ne!(
        plan.fingerprint,
        plan_ingredients(me, &items[..3], &cats, &[maito.clone(), suola.clone()]).fingerprint
    );

    let answers = ReviewAnswers {
        fingerprint: plan.fingerprint.clone(),
        answers: [(1, my_kylma.id.to_string()), (3, SKIP.to_owned())].into(),
    };
    assert!(
        matches!(decide(&plan, &cats, &answers), Err(CoreError::Invalid(_))),
        "no skip for an ingredient"
    );
    let answers = ReviewAnswers {
        fingerprint: plan.fingerprint.clone(),
        answers: [(1, my_kylma.id.to_string()), (3, my_hevi.id.to_string())].into(),
    };
    let decisions = decide(&plan, &cats, &answers).unwrap().unwrap();
    assert_eq!(
        ingredient_products(&plan, &decisions).unwrap(),
        [
            IngredientProduct::Link(maito.id),
            IngredientProduct::Categorize {
                id: suola.id,
                category: my_kylma.id
            },
            IngredientProduct::Create {
                name: "Kurkku".to_owned(),
                category: my_hevi.id
            },
            IngredientProduct::Create {
                name: "Pippuri".to_owned(),
                category: my_hevi.id
            },
        ]
    );
}

/// MB-0065 (RR-0052): a row with source category names the user lacks
/// offers them; `new:<n>` picks one by index; the names to create are
/// distinct by name key, in review order; resolving maps every such row to
/// the created id, and a name that wasn't created is `Invalid`.
#[test]
fn create_choices_and_resolve() {
    use std::collections::BTreeMap;

    use super::{CREATE, Decisions};

    let me = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let hevi = category(me, "Hevi");
    let (leipa, mausteet) = (category(owner, "Leipä"), category(owner, "Mausteet"));
    let cats = [hevi.clone()];
    let items = [
        item("Ruisleipä", Some(&leipa)),
        item("Suola", Some(&mausteet)),
        item("Kaura", Some(&category(owner, " LEIPÄ"))),
        item("Omena", Some(&category(owner, "hevi"))),
    ];
    let plan = plan(me, &items, &cats, &[]);
    assert_eq!(plan.rows[3].action, Action::Add(hevi.id));
    assert_eq!(plan.rows[0].choices().unwrap().create, ["Leipä"]);
    assert_eq!(plan.rows[2].choices().unwrap().create, [" LEIPÄ"]);

    let answers = |pairs: &[(usize, &str)]| ReviewAnswers {
        fingerprint: plan.fingerprint.clone(),
        answers: pairs.iter().map(|(i, a)| (*i, (*a).to_owned())).collect(),
    };
    for bad in ["new:1", "new:", "new:x", "new:-1", "new: 0"] {
        let bad = answers(&[(0, bad), (1, SKIP), (2, SKIP)]);
        assert!(
            matches!(decide(&plan, &cats, &bad), Err(CoreError::Invalid(_))),
            "{bad:?}"
        );
    }
    let new0 = format!("{CREATE}0");
    let decisions = decide(
        &plan,
        &cats,
        &answers(&[(0, &new0), (1, &new0), (2, &new0)]),
    )
    .unwrap()
    .unwrap();
    assert_eq!(decisions.new_categories(), ["Leipä", "Mausteet"]);
    assert_eq!(Decisions::new().new_categories(), Vec::<String>::new());

    let (l, m) = (Uuid::new_v4(), Uuid::new_v4());
    assert!(matches!(
        decisions
            .clone()
            .resolve(&BTreeMap::from([("leipä".to_owned(), l)])),
        Err(CoreError::Invalid(_))
    ));
    let resolved = decisions
        .resolve(&BTreeMap::from([
            ("leipä".to_owned(), l),
            ("mausteet".to_owned(), m),
        ]))
        .unwrap();
    assert_eq!(resolved.create.len(), 0);
    let (writes, summary) = writes(&plan, &resolved);
    assert_eq!(
        writes,
        [
            Write::Insert {
                name: "Ruisleipä".to_owned(),
                category: l
            },
            Write::Insert {
                name: "Suola".to_owned(),
                category: m
            },
            Write::Insert {
                name: "Kaura".to_owned(),
                category: l
            },
            Write::Insert {
                name: "Omena".to_owned(),
                category: hevi.id
            },
        ]
    );
    assert_eq!(summary.added, 4);
}
