//! MB-0060 (P1–P6; RR-0047): exporting a list's items to favorites.

use actix_web::dev::ServiceResponse;
use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::{CategoryRepo, ProductRepo, ShoppingListRepo};

use crate::routes::test_support::make_recipe_with;
use crate::state::AppState;

/// `(category, [favorites])` of a user's favorites; `None` is uncategorized.
#[allow(clippy::expect_used)]
async fn favorites(state: &AppState, user: Uuid) -> Vec<(Option<String>, Vec<String>)> {
    ProductRepo::list_for_owner(&state.pool, user)
        .await
        .expect("favorites")
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.products.into_iter().map(|p| p.name).collect(),
            )
        })
        .collect()
}

/// Everything about a user's products, for "nothing changed" checks.
#[allow(clippy::expect_used)]
async fn catalog(state: &AppState, user: Uuid) -> String {
    let favorites = ProductRepo::list_for_owner(&state.pool, user)
        .await
        .expect("favorites");
    let hidden = ProductRepo::list_hidden_with_usage(&state.pool, user)
        .await
        .expect("hidden");
    format!("{favorites:?}\n{hidden:?}")
}

/// Add an item to `list` as its owner, in `category` if given.
#[allow(clippy::expect_used)]
async fn item(state: &AppState, owner: Uuid, list: Uuid, text: &str, category: Option<Uuid>) {
    let added = ShoppingListRepo::add_item(&state.pool, owner, list, text, Some("2"))
        .await
        .expect("item");
    if let Some(category) = category {
        ShoppingListRepo::assign_category(&state.pool, owner, list, added.id, category)
            .await
            .expect("category");
    }
}

fn location<B>(resp: &ServiceResponse<B>) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// The review page's fingerprint field.
fn fingerprint(body: &str) -> String {
    let marker = r#"name="fingerprint" value=""#;
    body.find(marker)
        .map(|at| &body[at + marker.len()..])
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_owned()
}

/// The legends of the review's rows, in order.
fn review_rows(body: &str) -> Vec<String> {
    body.split(r#"<fieldset class="export-row">"#)
        .skip(1)
        .filter_map(|part| part.split("<legend>").nth(1))
        .filter_map(|part| part.split("</legend>").next())
        .map(str::to_owned)
        .collect()
}

macro_rules! session {
    ($app:expr, $email:expr) => {{
        use crate::routes::test_support::{login_request, session_cookie};
        session_cookie(
            &actix_web::test::call_service(&$app, login_request($email).to_request()).await,
        )
    }};
}

/// P1: everyone who can open the list is offered "Add to favorites" —
/// owner, editor and viewer, on archived lists too; a stranger gets 404.
/// Every active item is covered; a removed one never is.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn offered_to_allowed_roles() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::domain::ShareRole;

    use crate::routes::test_support::{
        body_string, make_category, make_user, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let viewer = make_user(&state, "viewer@example.com").await;
    let editor = make_user(&state, "editor@example.com").await;
    make_user(&state, "stranger@example.com").await;
    let hevi = make_category(&state, owner, "Hevi").await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    item(&state, owner, list, "Omena", Some(hevi)).await;
    item(&state, owner, list, "Leipä", Some(hevi)).await;
    let leipa = ShoppingListRepo::items(&state.pool, owner, list, false)
        .await
        .expect("items")[1]
        .id;
    ShoppingListRepo::remove_item(&state.pool, owner, list, leipa)
        .await
        .expect("remove");
    for (email, role) in [
        ("viewer@example.com", ShareRole::View),
        ("editor@example.com", ShareRole::Edit),
    ] {
        ShoppingListRepo::share(&state.pool, owner, list, email, role)
            .await
            .expect("share")
            .expect("matched");
    }
    let sessions = [
        ("owner", session!(app, "owner@example.com")),
        ("viewer", session!(app, "viewer@example.com")),
        ("editor", session!(app, "editor@example.com")),
    ];
    let stranger = session!(app, "stranger@example.com");
    let base = format!("/lists/{list}");

    for (who, cookie) in &sessions {
        let body = body_string(
            call_service(
                &app,
                TestRequest::get()
                    .uri(&base)
                    .cookie(cookie.clone())
                    .to_request(),
            )
            .await,
        )
        .await;
        assert!(
            body.contains(&format!(r#"action="{base}/export""#))
                && body.contains("Add to favorites"),
            "{who}"
        );
    }
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/export"))
            .cookie(stranger.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "stranger");

    // The owner: saved at once (the removed Leipä isn't covered).
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/export"))
            .cookie(sessions[0].1.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        location(&resp),
        format!("{base}/export/done?added=1&already=0&favorite=0&moved=0&skipped=0")
    );
    assert_eq!(
        favorites(&state, owner).await,
        [(Some("Hevi".to_owned()), vec!["Omena".to_owned()])]
    );

    // Archived: still offered, and the editor exports into their own
    // favorites (their "hevi" matches by name).
    ShoppingListRepo::archive(&state.pool, owner, list)
        .await
        .expect("archive");
    make_category(&state, editor, "hevi").await;
    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&base)
                .cookie(sessions[2].1.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(
        body.contains(&format!(r#"action="{base}/export""#)),
        "offered when archived"
    );
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/export"))
            .cookie(sessions[2].1.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        favorites(&state, editor).await,
        [(Some("hevi".to_owned()), vec!["Omena".to_owned()])]
    );

    // The viewer has no categories: Omena needs a choice; Leipä isn't there.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/export"))
            .cookie(sessions[1].1.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert_eq!(review_rows(&body), ["Omena"]);
    assert!(
        body.contains("On the list: Hevi, which isn't one of your categories."),
        "{body}"
    );
    assert_eq!(favorites(&state, viewer).await, []);
}

/// P2: a new name becomes a favorite in the item's category; a favorite in
/// the same category changes nothing; a product from recipes in the same
/// category is made a favorite. Names match ignoring case, the list is
/// untouched, and the summary says what happened.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn adds_new_and_skips_identical() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, make_category, make_user, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let hevi = make_category(&state, owner, "Hevi").await;
    let kylma = make_category(&state, owner, "Kylmä").await;
    ProductRepo::create(&state.pool, owner, "Maito", kylma)
        .await
        .expect("favorite");
    make_recipe_with(&state, owner, "Salaatti", hevi, &[("Kurkku", None)]).await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    item(&state, owner, list, "maito", Some(kylma)).await;
    item(&state, owner, list, "Kurkku", Some(hevi)).await;
    item(&state, owner, list, "Banaani", Some(hevi)).await;
    item(&state, owner, list, "BANAANI", Some(hevi)).await;
    let before = ShoppingListRepo::items(&state.pool, owner, list, false)
        .await
        .expect("items");
    let cookie = session!(app, "owner@example.com");

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/export"))
            .cookie(cookie.clone())
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "htmx follows the 303");
    let to = location(&resp);
    assert_eq!(
        to,
        format!("/lists/{list}/export/done?added=1&already=1&favorite=1&moved=0&skipped=0")
    );
    assert_eq!(
        favorites(&state, owner).await,
        [
            (
                Some("Hevi".to_owned()),
                vec!["Kurkku".to_owned(), "Banaani".to_owned()]
            ),
            (Some("Kylmä".to_owned()), vec!["Maito".to_owned()]),
        ]
    );
    let after = ShoppingListRepo::items(&state.pool, owner, list, false)
        .await
        .expect("items");
    assert_eq!(
        format!("{after:?}"),
        format!("{before:?}"),
        "the list is untouched"
    );

    let body = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&to)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    for line in [
        "1 new favorite",
        "1 product from recipes made a favorite",
        "1 was already a favorite",
    ] {
        assert!(body.contains(line), "{line}");
    }
    assert!(!body.contains("1 skipped") && !body.contains("moved to another category"));
}

/// P3: a conflict saves nothing until reviewed. The review lists each
/// conflict with explicit choices, none preselected — a favorite: keep or
/// move; a product from recipes: favorite in either category, or skip — and
/// confirming applies every answer and every other item together.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn conflicts_need_review() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, make_category, make_user, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let hevi = make_category(&state, owner, "Hevi").await;
    let kylma = make_category(&state, owner, "Kylmä").await;
    for name in ["Omena", "Maito"] {
        ProductRepo::create(&state.pool, owner, name, kylma)
            .await
            .expect("favorite");
    }
    make_recipe_with(&state, owner, "Keitto", kylma, &[("Tomaatti", None)]).await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    item(&state, owner, list, "Omena", Some(hevi)).await;
    item(&state, owner, list, "Tomaatti", Some(hevi)).await;
    item(&state, owner, list, "Maito", Some(kylma)).await;
    item(&state, owner, list, "Banaani", Some(hevi)).await;
    let cookie = session!(app, "owner@example.com");
    let before = catalog(&state, owner).await;

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/export"))
            .cookie(cookie.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert_eq!(catalog(&state, owner).await, before, "nothing saved yet");
    assert_eq!(review_rows(&body), ["Omena", "Tomaatti"]);
    assert!(body.contains("1 other item will be added as it is."));
    let omena = &body[body.find("<legend>Omena").expect("omena")
        ..body.find("<legend>Tomaatti").expect("tomaatti")];
    assert!(omena.contains("On the list: Hevi.") && omena.contains("Your favorite: Kylmä."));
    assert!(
        omena.contains(&format!(r#"name="r0" value="{kylma}" required"#))
            && omena.contains("Keep in Kylmä")
    );
    assert!(
        omena.contains(&format!(r#"name="r0" value="{hevi}" required"#))
            && omena.contains("Move to Hevi")
    );
    assert!(!omena.contains("Skip"), "a favorite is kept, not skipped");
    let tomaatti = &body[body.find("<legend>Tomaatti").expect("tomaatti")..];
    assert!(tomaatti.contains("From your recipes: Kylmä."));
    assert!(
        tomaatti.contains("Make it a favorite in Kylmä")
            && tomaatti.contains("Make it a favorite in Hevi")
    );
    assert!(tomaatti.contains(r#"name="r1" value="skip" required"#));
    let form = &body[body.find(r#"<form class="export-review""#).expect("form")..];
    assert!(!form.contains("checked"), "nothing preselected");

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/export/confirm"))
            .cookie(cookie.clone())
            .set_form([
                ("fingerprint", fingerprint(&body)),
                ("r0", hevi.to_string()),
                ("r1", "skip".to_owned()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        location(&resp),
        format!("/lists/{list}/export/done?added=1&already=1&favorite=0&moved=1&skipped=1")
    );
    assert_eq!(
        favorites(&state, owner).await,
        [
            (
                Some("Hevi".to_owned()),
                vec!["Omena".to_owned(), "Banaani".to_owned()]
            ),
            (Some("Kylmä".to_owned()), vec!["Maito".to_owned()]),
        ]
    );
    let hidden = ProductRepo::list_hidden_with_usage(&state.pool, owner)
        .await
        .expect("hidden");
    assert_eq!(hidden.len(), 1, "Tomaatti stays a product from recipes");
}

/// P4: if the list or the catalog changed since the review, the
/// confirmation saves nothing and shows the review again with the current
/// rows; a missing answer is shown again too; a tampered answer (an index
/// that isn't a row, a category that isn't offered) is refused.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn stale_review_is_redone() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, make_category, make_user, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let theirs = make_category(&state, other, "Salaiset").await;
    let hevi = make_category(&state, owner, "Hevi").await;
    let kylma = make_category(&state, owner, "Kylmä").await;
    let omena = ProductRepo::create(&state.pool, owner, "Omena", kylma)
        .await
        .expect("favorite")
        .id;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    item(&state, owner, list, "Omena", Some(hevi)).await;
    let cookie = session!(app, "owner@example.com");
    let start = || {
        TestRequest::post()
            .uri(&format!("/lists/{list}/export"))
            .cookie(cookie.clone())
            .to_request()
    };
    let confirm = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&format!("/lists/{list}/export/confirm"))
            .cookie(cookie.clone())
            .set_form(form)
            .to_request()
    };
    let first = body_string(call_service(&app, start()).await).await;
    let old = fingerprint(&first);
    assert_eq!(old.len(), 64);

    // The catalog changes: Omena moved to Hevi elsewhere. The old answer
    // ("keep in Kylmä") no longer applies.
    ProductRepo::update(&state.pool, owner, omena, "Omena", Some(hevi))
        .await
        .expect("move");
    let before = catalog(&state, owner).await;
    let resp = call_service(
        &app,
        confirm(vec![
            ("fingerprint", old.clone()),
            ("r0", kylma.to_string()),
        ]),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "saves nothing; the review again"
    );
    let body = body_string(resp).await;
    assert!(body.contains("The list or your favorites changed since."));
    assert_eq!(review_rows(&body), Vec::<String>::new(), "no conflict now");
    assert_eq!(catalog(&state, owner).await, before, "Omena stays in Hevi");
    // Confirming the current (empty) review saves, without the stale answer.
    let resp = call_service(&app, confirm(vec![("fingerprint", fingerprint(&body))])).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert!(
        location(&resp).contains("already=1"),
        "not the stale answer"
    );
    assert_eq!(catalog(&state, owner).await, before, "Omena stays in Hevi");

    // The list changes between review and confirmation.
    ProductRepo::update(&state.pool, owner, omena, "Omena", Some(kylma))
        .await
        .expect("move back");
    let review = body_string(call_service(&app, start()).await).await;
    item(&state, owner, list, "Päärynä", Some(kylma)).await;
    let before = catalog(&state, owner).await;
    let resp = call_service(
        &app,
        confirm(vec![
            ("fingerprint", fingerprint(&review)),
            ("r0", hevi.to_string()),
        ]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("The list or your favorites changed since."));
    assert_eq!(review_rows(&body), ["Omena"]);
    assert_eq!(catalog(&state, owner).await, before, "nothing saved");

    // A missing answer: shown again, 400 without htmx, nothing saved.
    let current = fingerprint(&body);
    let resp = call_service(&app, confirm(vec![("fingerprint", current.clone())])).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(
        body_string(resp)
            .await
            .contains("Choose an option for every item.")
    );
    assert_eq!(catalog(&state, owner).await, before);

    // Tampered: another user's category, a row that isn't there, a skip
    // that isn't offered.
    for form in [
        vec![("fingerprint", current.clone()), ("r0", theirs.to_string())],
        vec![
            ("fingerprint", current.clone()),
            ("r0", hevi.to_string()),
            ("r7", "skip".to_owned()),
        ],
        vec![("fingerprint", current.clone()), ("r0", "skip".to_owned())],
    ] {
        let resp = call_service(&app, confirm(form.clone())).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{form:?}");
    }
    assert_eq!(catalog(&state, owner).await, before, "nothing saved");

    // The current review, answered: saved.
    let resp = call_service(
        &app,
        confirm(vec![("fingerprint", current), ("r0", hevi.to_string())]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        favorites(&state, owner).await,
        [
            (Some("Hevi".to_owned()), vec!["Omena".to_owned()]),
            (Some("Kylmä".to_owned()), vec!["Päärynä".to_owned()]),
        ]
    );
}

/// P5: an item without a category is listed with a picker and "Skip", and
/// needs a choice; the summary counts what happened; an empty list says
/// there was nothing to add.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn uncategorized_and_summary() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, make_category, make_user, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let hevi = make_category(&state, owner, "Hevi").await;
    let koti = make_category(&state, owner, "Koti").await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    item(&state, owner, list, "Kynttilät", None).await;
    item(&state, owner, list, "Paristot", None).await;
    item(&state, owner, list, "Omena", Some(hevi)).await;
    let cookie = session!(app, "owner@example.com");

    let body = body_string(
        call_service(
            &app,
            TestRequest::post()
                .uri(&format!("/lists/{list}/export"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert_eq!(review_rows(&body), ["Kynttilät", "Paristot"]);
    assert!(body.contains("On the list: no category."));
    assert!(
        body.contains(r#"<select name="r0" required>"#)
            && body.contains(r#"<option value="skip">Skip</option>"#)
    );
    assert!(body.contains(r#"<option value="" disabled selected>Choose a category</option>"#));

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/export/confirm"))
            .cookie(cookie.clone())
            .set_form([
                ("fingerprint", fingerprint(&body)),
                ("r0", koti.to_string()),
                ("r1", "skip".to_owned()),
            ])
            .to_request(),
    )
    .await;
    let to = location(&resp);
    assert_eq!(
        to,
        format!("/lists/{list}/export/done?added=2&already=0&favorite=0&moved=0&skipped=1")
    );
    assert_eq!(
        favorites(&state, owner).await,
        [
            (Some("Hevi".to_owned()), vec!["Omena".to_owned()]),
            (Some("Koti".to_owned()), vec!["Kynttilät".to_owned()]),
        ]
    );
    let done = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&to)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(done.contains("2 new favorites") && done.contains("1 skipped"));

    let empty = ShoppingListRepo::create(&state.pool, owner, "Tyhjä")
        .await
        .expect("list")
        .id;
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{empty}/export"))
            .cookie(cookie.clone())
            .to_request(),
    )
    .await;
    let done = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&location(&resp))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(done.contains("The list had nothing to add."));
}

/// P6: a grantee's export writes only their own catalog. The owner's
/// categories map by name to the grantee's own (none is created, none of the
/// owner's ids is linked); the owner's catalog is unchanged; an answer with
/// the owner's category id is refused.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn writes_only_own_catalog() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::domain::ShareRole;

    use crate::routes::test_support::{
        body_string, make_category, make_user, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let editor = make_user(&state, "editor@example.com").await;
    let hevi = make_category(&state, owner, "Hevi").await;
    let kylma = make_category(&state, owner, "Kylmä").await;
    ProductRepo::create(&state.pool, owner, "Omena", kylma)
        .await
        .expect("owner's favorite");
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    item(&state, owner, list, "Omena", Some(hevi)).await;
    item(&state, owner, list, "Maito", Some(kylma)).await;
    ShoppingListRepo::share(
        &state.pool,
        owner,
        list,
        "editor@example.com",
        ShareRole::Edit,
    )
    .await
    .expect("share")
    .expect("matched");
    let my_hevi = make_category(&state, editor, "HEVI").await;
    let my_maito = make_category(&state, editor, "Maitotuotteet").await;
    let owner_before = catalog(&state, owner).await;
    let cookie = session!(app, "editor@example.com");

    let body = body_string(
        call_service(
            &app,
            TestRequest::post()
                .uri(&format!("/lists/{list}/export"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    // Omena maps to the editor's HEVI; Maito's Kylmä isn't theirs.
    assert_eq!(review_rows(&body), ["Maito"]);
    assert!(body.contains("On the list: Kylmä, which isn't one of your categories."));
    assert!(
        !body.contains(&kylma.to_string()) && !body.contains(&hevi.to_string()),
        "no owner category id offered"
    );

    let fp = fingerprint(&body);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/export/confirm"))
            .cookie(cookie.clone())
            .set_form([("fingerprint", fp.clone()), ("r1", kylma.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "the owner's category is refused"
    );
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/export/confirm"))
            .cookie(cookie.clone())
            .set_form([("fingerprint", fp), ("r1", my_maito.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        favorites(&state, editor).await,
        [
            (Some("HEVI".to_owned()), vec!["Omena".to_owned()]),
            (Some("Maitotuotteet".to_owned()), vec!["Maito".to_owned()]),
        ]
    );
    let mine: Vec<Uuid> = CategoryRepo::list_for_owner(&state.pool, editor)
        .await
        .expect("categories")
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(mine, [my_hevi, my_maito], "no category created");
    for group in ProductRepo::list_for_owner(&state.pool, editor)
        .await
        .expect("favorites")
    {
        for product in group.products {
            assert!(product.category_id.is_some_and(|c| mine.contains(&c)));
            assert_eq!(product.owner_id, editor);
        }
    }
    assert_eq!(
        catalog(&state, owner).await,
        owner_before,
        "the owner's catalog is unchanged"
    );
}

/// MB-0065 P4 (RR-0052): a grantee's item whose list category they don't
/// have can be created on their review: "Create ‹name›" in a "New category"
/// group of the picker, sent as an index; rows choosing the same name share
/// one new category, last in their order, with none of the owner's ids. The
/// list owner's own export, whose categories all match, offers no creation.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn create_missing_category() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::domain::ShareRole;

    use crate::routes::test_support::{
        body_string, make_category, make_user, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let editor = make_user(&state, "editor@example.com").await;
    let hevi = make_category(&state, owner, "Hevi").await;
    let kylma = make_category(&state, owner, "Kylmä").await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    item(&state, owner, list, "Omena", Some(hevi)).await;
    item(&state, owner, list, "Maito", Some(kylma)).await;
    item(&state, owner, list, "Jogurtti", Some(kylma)).await;
    item(&state, owner, list, "Kynttilät", None).await;
    ShoppingListRepo::share(
        &state.pool,
        owner,
        list,
        "editor@example.com",
        ShareRole::Edit,
    )
    .await
    .expect("share")
    .expect("matched");
    let my_hevi = make_category(&state, editor, "HEVI").await;
    let owner_before = catalog(&state, owner).await;
    let export = |cookie| {
        TestRequest::post()
            .uri(&format!("/lists/{list}/export"))
            .cookie(cookie)
            .to_request()
    };

    // The owner: only Kynttilät (no category) is asked, with no creation.
    let owner_cookie = session!(app, "owner@example.com");
    let body = body_string(call_service(&app, export(owner_cookie)).await).await;
    assert_eq!(review_rows(&body), ["Kynttilät"]);
    assert!(!body.contains("New category") && !body.contains("new:"));

    // The editor: Maito and Jogurtti (Kylmä isn't theirs) offer "Create
    // Kylmä"; Kynttilät has no source category, so nothing to create.
    let cookie = session!(app, "editor@example.com");
    let body = body_string(call_service(&app, export(cookie.clone())).await).await;
    assert_eq!(review_rows(&body), ["Maito", "Jogurtti", "Kynttilät"]);
    let rows: Vec<&str> = body
        .split(r#"<fieldset class="export-row">"#)
        .skip(1)
        .collect();
    for row in &rows[..2] {
        assert!(row.contains(r#"<optgroup label="New category">"#), "{row}");
        assert!(row.contains(r#"<option value="new:0">Create Kylmä</option>"#));
    }
    assert!(!rows[2].contains("New category"));
    let form = &body[body.find(r#"class="export-review""#).expect("form")..];
    assert!(!form.contains("checked"), "nothing preselected");
    assert!(
        !body.contains(&kylma.to_string()) && !body.contains(&hevi.to_string()),
        "no owner category id offered"
    );

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/export/confirm"))
            .cookie(cookie.clone())
            .set_form([
                ("fingerprint", fingerprint(&body)),
                ("r1", "new:0".to_owned()),
                ("r2", "new:0".to_owned()),
                ("r3", "skip".to_owned()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let done = location(&resp);
    assert_eq!(
        done,
        format!(
            "/lists/{list}/export/done?added=3&already=0&favorite=0&moved=0&skipped=1&categories=1"
        )
    );
    let summary = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&done)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(summary.contains("1 new category") && summary.contains("3 new favorites"));

    let mine = CategoryRepo::list_for_owner(&state.pool, editor)
        .await
        .expect("categories");
    let names: Vec<&str> = mine.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["HEVI", "Kylmä"], "created once, last");
    assert_eq!(mine[0].id, my_hevi);
    assert_ne!(mine[1].id, kylma, "a new category, not the owner's");
    assert_eq!(mine[1].owner_id, editor);
    assert_eq!(
        favorites(&state, editor).await,
        [
            (Some("HEVI".to_owned()), vec!["Omena".to_owned()]),
            (
                Some("Kylmä".to_owned()),
                vec!["Maito".to_owned(), "Jogurtti".to_owned()]
            ),
        ]
    );
    assert_eq!(
        catalog(&state, owner).await,
        owner_before,
        "the owner's catalog is untouched"
    );
}
