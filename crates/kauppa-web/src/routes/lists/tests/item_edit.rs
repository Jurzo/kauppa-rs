//! MB-0052 (P1–P3, RR-0033): one form and one Save button for a list item's
//! category and quantity, `POST /lists/{id}/items/{item}`.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;

use crate::state::AppState;

use super::shared_list;

/// Every `<details class="product-edit">` section in `body`, from its
/// opening tag to its `</details>`.
fn edit_sections(body: &str) -> Vec<&str> {
    body.match_indices(r#"<details class="product-edit""#)
        .map(|(start, _)| {
            let rest = &body[start..];
            rest.find("</details>")
                .map_or(rest, |end| &rest[..end + "</details>".len()])
        })
        .collect()
}

/// Every `<form …>…</form>` in `section`, in order.
fn forms(section: &str) -> Vec<&str> {
    section
        .match_indices("<form")
        .map(|(start, _)| {
            let rest = &section[start..];
            rest.find("</form>")
                .map_or(rest, |end| &rest[..end + "</form>".len()])
        })
        .collect()
}

/// `(text, category name, quantity)` of a list's items, active then
/// removed, read as `owner`, plus the list's `updated_at`.
#[allow(clippy::expect_used)]
async fn items_state(state: &AppState, owner: Uuid, list: Uuid) -> String {
    let mut rows = Vec::new();
    for removed in [false, true] {
        rows.extend(
            ShoppingListRepo::items(&state.pool, owner, list, removed)
                .await
                .expect("items")
                .into_iter()
                .map(|i| (i.text, i.category.map(|c| c.name), i.quantity)),
        );
    }
    let updated = ShoppingListRepo::get(&state.pool, owner, list)
        .await
        .expect("list")
        .updated_at;
    format!("{updated:?} {rows:?}")
}

/// `(category name, quantity)` of one active item, read as `owner`.
#[allow(clippy::expect_used)]
async fn item_fields(
    state: &AppState,
    owner: Uuid,
    list: Uuid,
    item: Uuid,
) -> (Option<String>, Option<String>) {
    let found = ShoppingListRepo::items(&state.pool, owner, list, false)
        .await
        .expect("items")
        .into_iter()
        .find(|i| i.id == item)
        .expect("active item");
    (found.category.map(|c| c.name), found.quantity)
}

/// MB-0052 P1: an item's edit section has one form with one submit button,
/// Save, last in the form, posting to `/lists/{id}/items/{item}` (plain and
/// htmx); the only other form is Remove, below it. The owner with
/// categories gets the picker, not required, and the quantity; the owner
/// without categories, and an `edit` grantee, the quantity only. There is
/// no Set button and no `/category` or `/quantity` form. A `view` grantee
/// gets no edit section. Full page and fragment alike.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn one_save_button() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_list, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    // `shared_list`: the owner has a category; one item active, one removed.
    let s = shared_list(&state).await;
    ShoppingListRepo::assign_category(&state.pool, s.owner, s.list, s.bread, s.category)
        .await
        .expect("categorized");
    ShoppingListRepo::add_item(&state.pool, s.owner, s.list, "Kynttilät", Some("6"))
        .await
        .expect("uncategorized item");
    let without = make_user(&state, "without@example.com").await;
    let bare = make_list(&state, without, "Oma").await;
    ShoppingListRepo::add_item(&state.pool, without, bare, "Omena", None)
        .await
        .expect("item");

    // (who, list, its owner, items with an edit section, sees the picker)
    for (email, list, owner, sections, picker) in [
        ("owner@example.com", s.list, s.owner, 2, true),
        ("without@example.com", bare, without, 1, false),
        ("editor@example.com", s.list, s.owner, 2, false),
        ("viewer@example.com", s.list, s.owner, 0, false),
    ] {
        let session = session_cookie(&call_service(&app, login_request(email).to_request()).await);
        let base = format!("/lists/{list}");
        let active: Vec<Uuid> = ShoppingListRepo::items(&state.pool, owner, list, false)
            .await
            .expect("items")
            .into_iter()
            .map(|i| i.id)
            .collect();
        for htmx in [false, true] {
            let mut req = TestRequest::get().uri(&base).cookie(session.clone());
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::OK, "{email} htmx={htmx}");
            let body = body_string(resp).await;
            assert_eq!(body.contains("<html"), !htmx, "{email} htmx={htmx}");
            for gone in [r#"/category""#, r#"/quantity""#, ">Set<"] {
                assert!(!body.contains(gone), "{email} htmx={htmx}: no `{gone}`");
            }

            let found = edit_sections(&body);
            assert_eq!(found.len(), sections, "{email} htmx={htmx}: sections");
            for (n, section) in found.iter().enumerate() {
                let at = format!("{email} htmx={htmx} section {n}");
                let section_forms = forms(section);
                assert_eq!(section_forms.len(), 2, "{at}: Save's form and Remove");
                let (save, remove) = (section_forms[0], section_forms[1]);
                assert!(remove.contains(r#"/remove""#), "{at}: Remove below");
                let action = active
                    .iter()
                    .map(|item| format!("{base}/items/{item}"))
                    .find(|uri| save.contains(&format!(r#"action="{uri}""#)))
                    .expect("Save posts to the item");
                assert!(save.contains(&format!(r#"hx-post="{action}""#)), "{at}");
                assert!(
                    save.contains(&format!(r##"hx-target="#list-{list}""##)),
                    "{at}"
                );
                assert_eq!(save.matches("<button").count(), 1, "{at}: one button");
                assert_eq!(save.matches(r#"type="submit""#).count(), 1, "{at}");
                let button = &save[save.find("<button").expect("button")..];
                assert!(button.contains(">Save</button>"), "{at}: Save");
                assert!(
                    button
                        .trim_end_matches("</form>")
                        .trim_end()
                        .ends_with("</button>"),
                    "{at}: Save is last"
                );
                assert!(save.contains(r#"name="quantity""#), "{at}: quantity");
                assert_eq!(
                    save.contains(r#"<select name="category_id">"#),
                    picker,
                    "{at}: picker, not required"
                );
                assert!(!save.contains("required"), "{at}: nothing required");
                if picker {
                    assert!(
                        save.contains(r#"<option value="" disabled"#),
                        "{at}: the placeholder can't be submitted"
                    );
                    assert!(save.find("<select") < save.find(r#"name="quantity""#));
                }
            }
        }
    }
}

/// MB-0052 P2: one Save by the owner stores the category and the quantity
/// together, plain (303 back to the list) and over htmx (the fragment). A
/// category that isn't the owner's (unknown, another user's, malformed)
/// changes neither field and shows the message: the full page with 400, or
/// the fragment. No category (missing or blank) saves the quantity only; a
/// blank quantity clears it. A removed item is a 404.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn saves_category_and_quantity_together() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_list, make_user, session_cookie, test_app,
        test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let koti = make_category(&state, user, "Koti").await;
    let theirs = make_category(&state, other, "Muiden").await;
    let list = make_list(&state, user, "Viikko").await;
    let add = |text: &'static str| {
        let state = &state;
        async move {
            ShoppingListRepo::add_item(&state.pool, user, list, text, None)
                .await
                .expect("item")
                .id
        }
    };
    let bread = add("Leipä").await;
    let candles = add("Kynttilät").await;
    let sugar = add("Sokeri").await;
    ShoppingListRepo::remove_item(&state.pool, user, list, sugar)
        .await
        .expect("remove");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{list}");
    let fragment_id = format!(r#"id="list-{list}""#);
    let post = |item: Uuid, form: Vec<(&'static str, String)>, htmx: bool| {
        let mut req = TestRequest::post()
            .uri(&format!("{base}/items/{item}"))
            .cookie(session.clone())
            .set_form(form);
        if htmx {
            req = req.insert_header(("HX-Request", "true"));
        }
        req.to_request()
    };
    let both = |category: Uuid, quantity: &str| {
        vec![
            ("category_id", category.to_string()),
            ("quantity", quantity.to_owned()),
        ]
    };
    let fields = |item: Uuid| item_fields(&state, user, list, item);

    // Plain: one request, both fields, a 303 back to the list.
    let resp = call_service(&app, post(bread, both(kylma, "2"), false)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        base.as_str()
    );
    assert_eq!(
        fields(bread).await,
        (Some("Kylmä".to_owned()), Some("2".to_owned()))
    );
    let page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&base)
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(page.contains(r#"<span class="ingredient-quantity">2</span> Leipä"#));
    assert!(page.contains("· Kylmä"));

    // htmx: the fragment, with both.
    let resp = call_service(&app, post(bread, both(koti, " 3 kpl "), true)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(&fragment_id));
    assert!(body.contains(r#"<span class="ingredient-quantity">3 kpl</span> Leipä"#));
    assert!(body.contains("· Koti"));
    assert_eq!(
        fields(bread).await,
        (Some("Koti".to_owned()), Some("3 kpl".to_owned()))
    );

    // A rejected category changes neither field and shows the message.
    let before = items_state(&state, user, list).await;
    for category in [
        Uuid::new_v4().to_string(),
        theirs.to_string(),
        "nope".to_owned(),
    ] {
        let form = vec![
            ("category_id", category.clone()),
            ("quantity", "9".to_owned()),
        ];
        let resp = call_service(&app, post(bread, form.clone(), false)).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{category}");
        let body = body_string(resp).await;
        assert!(body.contains("<html"), "{category}: full page");
        assert!(body.contains("unknown category"), "{category}");
        let resp = call_service(&app, post(bread, form, true)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{category} htmx");
        let body = body_string(resp).await;
        assert!(!body.contains("<html") && body.contains(&fragment_id));
        assert!(body.contains("unknown category"), "{category} htmx");
        assert_eq!(
            items_state(&state, user, list).await,
            before,
            "{category}: nothing changed"
        );
    }

    // No category sent (the placeholder isn't submitted), or a blank one:
    // the quantity is saved, the category stays as it was.
    let resp = call_service(
        &app,
        post(candles, vec![("quantity", "6".to_owned())], false),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(fields(candles).await, (None, Some("6".to_owned())));
    let resp = call_service(
        &app,
        post(
            bread,
            vec![("category_id", String::new()), ("quantity", "4".to_owned())],
            true,
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        fields(bread).await,
        (Some("Koti".to_owned()), Some("4".to_owned()))
    );

    // A blank quantity clears it, with or without a category.
    let resp = call_service(&app, post(bread, both(kylma, "  "), false)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(fields(bread).await, (Some("Kylmä".to_owned()), None));
    let resp = call_service(&app, post(candles, vec![("quantity", String::new())], true)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(fields(candles).await, (None, None));

    // A removed or unknown item is a 404, and nothing changes.
    let before = items_state(&state, user, list).await;
    for item in [sugar, Uuid::new_v4()] {
        for htmx in [false, true] {
            for form in [both(kylma, "9"), vec![("quantity", "9".to_owned())]] {
                let resp = call_service(&app, post(item, form, htmx)).await;
                assert_eq!(resp.status(), StatusCode::NOT_FOUND, "htmx={htmx}");
            }
        }
    }
    assert_eq!(items_state(&state, user, list).await, before);
}

/// MB-0052 P3 (RR-0033 sub-answer 4): an `edit` grantee's Save changes the
/// quantity only (no category, or a blank one). With a category id — the
/// owner's or their own — they get 403 and nothing changes, the quantity
/// included; a malformed id is the usual "unknown category" rejection. A
/// `view` grantee gets 403 and a stranger 404. Plain and htmx.
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn grantee_saves_quantity_only() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let s = shared_list(&state).await;
    let editors_own = make_category(&state, s.editor, "Hevi").await;
    make_user(&state, "stranger@example.com").await;
    let login = |email: &'static str| {
        let app = &app;
        async move { session_cookie(&call_service(app, login_request(email).to_request()).await) }
    };
    let editor = login("editor@example.com").await;
    let viewer = login("viewer@example.com").await;
    let stranger = login("stranger@example.com").await;
    let uri = format!("/lists/{}/items/{}", s.list, s.bread);
    let fragment_id = format!(r#"id="list-{}""#, s.list);
    let post = |session: &actix_web::cookie::Cookie<'static>,
                form: Vec<(&'static str, String)>,
                htmx: bool| {
        let mut req = TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form(form);
        if htmx {
            req = req.insert_header(("HX-Request", "true"));
        }
        req.to_request()
    };
    let quantity = |q: &str| vec![("quantity", q.to_owned())];
    let with_category =
        |category: String, q: &str| vec![("category_id", category), ("quantity", q.to_owned())];
    let fields = || item_fields(&state, s.owner, s.list, s.bread);

    // The editor saves the quantity, plain and over htmx; a blank category
    // counts as none. The category stays as it was.
    let resp = call_service(&app, post(&editor, quantity("2"), false)).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(fields().await, (None, Some("2".to_owned())));
    let resp = call_service(&app, post(&editor, quantity("3 kpl"), true)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(&fragment_id) && body.contains("3 kpl"));
    assert_eq!(fields().await, (None, Some("3 kpl".to_owned())));
    let resp = call_service(
        &app,
        post(&editor, with_category(String::new(), "4"), false),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(fields().await, (None, Some("4".to_owned())));

    // With a category id, refused whole: 403, and neither field changes.
    let before = items_state(&state, s.owner, s.list).await;
    for (who, session, category, status) in [
        ("editor", &editor, s.category, StatusCode::FORBIDDEN),
        ("editor", &editor, editors_own, StatusCode::FORBIDDEN),
        ("editor", &editor, Uuid::new_v4(), StatusCode::FORBIDDEN),
        ("viewer", &viewer, s.category, StatusCode::FORBIDDEN),
        ("stranger", &stranger, s.category, StatusCode::NOT_FOUND),
    ] {
        for htmx in [false, true] {
            let resp = call_service(
                &app,
                post(session, with_category(category.to_string(), "9"), htmx),
            )
            .await;
            assert_eq!(resp.status(), status, "{who} {category} htmx={htmx}");
        }
    }
    // A viewer or a stranger without a category: refused too.
    for (who, session, status) in [
        ("viewer", &viewer, StatusCode::FORBIDDEN),
        ("stranger", &stranger, StatusCode::NOT_FOUND),
    ] {
        for htmx in [false, true] {
            for q in ["9", ""] {
                let resp = call_service(&app, post(session, quantity(q), htmx)).await;
                assert_eq!(resp.status(), status, "{who} {q:?} htmx={htmx}");
            }
        }
    }
    // A malformed id is the usual rejection, for an editor as for anyone.
    let resp = call_service(
        &app,
        post(&editor, with_category("nope".to_owned(), "9"), false),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(body_string(resp).await.contains("unknown category"));
    let resp = call_service(
        &app,
        post(&editor, with_category("nope".to_owned(), "9"), true),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(&fragment_id) && body.contains("unknown category"));
    assert_eq!(
        items_state(&state, s.owner, s.list).await,
        before,
        "nothing changed"
    );
    assert_eq!(fields().await, (None, Some("4".to_owned())));
}
