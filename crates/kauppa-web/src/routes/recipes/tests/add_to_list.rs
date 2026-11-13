//! MB-0015 (P1–P3): adding a recipe to a list from the recipe page.

use actix_web::http::{StatusCode, header};
use uuid::Uuid;

use kauppa_core::repo::{RecipeRepo, ShoppingListRepo};

#[cfg(test)]
use crate::routes::test_support::{make_list, make_recipe, make_recipe_with};
use crate::state::AppState;

/// A list's active items as `(text, quantity, product, category)`.
#[allow(clippy::expect_used)]
async fn list_shape(
    state: &AppState,
    owner: Uuid,
    list: Uuid,
) -> Vec<(String, Option<String>, Option<Uuid>, Option<Uuid>)> {
    ShoppingListRepo::items(&state.pool, owner, list, false)
        .await
        .expect("list items")
        .into_iter()
        .map(|i| (i.text, i.quantity, i.product_id, i.category.map(|c| c.id)))
        .collect()
}

/// MB-0015 P1: the recipe page offers the user's own lists, most recently
/// updated first, with a "Times" field defaulting to 1; another user's lists
/// never appear. Without lists, it links to `/lists` instead.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_to_list_form() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let fresh = make_user(&state, "fresh@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let recipe = make_recipe_with(&state, user, "Puuro", kylma, &[("Maito", Some("5 dl"))]).await;
    let fresh_category = make_category(&state, fresh, "Kuivat").await;
    let fresh_recipe = make_recipe_with(
        &state,
        fresh,
        "Leipä",
        fresh_category,
        &[("Jauho", Some("1 kg"))],
    )
    .await;
    let weekend = make_list(&state, user, "Viikonloppu").await;
    let weekday = make_list(&state, user, "Arki").await;
    let theirs = make_list(&state, other, "Heidän lista").await;
    // A rename counts as an update: the older list moves to the top.
    ShoppingListRepo::rename(&state.pool, user, weekend, "Viikonloppu")
        .await
        .expect("rename");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let fresh_session =
        session_cookie(&call_service(&app, login_request("fresh@example.com").to_request()).await);

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{recipe}"))
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        body.contains("<title>Puuro — kauppalista.app</title>"),
        "full page"
    );
    assert!(body.contains(&format!(r#"action="/recipes/{recipe}/add-to-list""#)));
    assert!(body.contains(r#"<option value="" disabled selected>Choose a list</option>"#));
    let option = |id: Uuid, name: &str| format!(r#"<option value="{id}">{name}</option>"#);
    let first = body
        .find(&option(weekend, "Viikonloppu"))
        .expect("most recently updated list offered");
    let second = body
        .find(&option(weekday, "Arki"))
        .expect("other own list offered");
    assert!(first < second, "most recently updated first");
    assert!(
        body.contains(r#"<input name="multiplier" type="text" inputmode="decimal""#)
            && body.contains(r#"value="1" />"#),
        "Times defaults to 1"
    );
    assert!(!body.contains("Heidän lista") && !body.contains(&theirs.to_string()));

    // Without lists: a link to /lists, no form.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{fresh_recipe}"))
            .cookie(fresh_session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        body.contains("<title>Leipä — kauppalista.app</title>"),
        "full page"
    );
    assert!(body.contains(r#"<a href="/lists">create a list</a>"#));
    assert!(!body.contains(r#"name="list_id""#));
    assert!(!body.contains("Viikonloppu") && !body.contains("Heidän lista"));
}

/// MB-0015 P2: submitting adds the recipe's ingredients to the chosen list
/// exactly as `POST /lists/{id}/recipes` does (snapshot, multiplier). Over
/// htmx the answer is a confirmation naming the list and the item count, with
/// a link to it; without htmx (or boosted), a 303 to the list.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_to_list() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let recipe = make_recipe_with(
        &state,
        user,
        "Pannukakku",
        kylma,
        &[
            ("Maito", Some("5 dl")),
            ("Vehnäjauho", Some("2½ dl")),
            ("Kananmuna", None),
        ],
    )
    .await;
    let single = make_recipe_with(&state, user, "Kahvi", kylma, &[("Kerma", Some("1 dl"))]).await;
    let weekend = make_list(&state, user, "Viikonloppu").await;
    let weekday = make_list(&state, user, "Arki").await;
    let reference = make_list(&state, user, "Vertailu").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let submit = |recipe: Uuid, list: Uuid, multiplier: &str| {
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/add-to-list"))
            .cookie(session.clone())
            .set_form([
                ("list_id", list.to_string()),
                ("multiplier", multiplier.to_owned()),
            ])
    };

    // Over htmx: the section with a confirmation, the list still chosen.
    let resp = call_service(
        &app,
        submit(recipe, weekend, "1,5")
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html"), "fragment only");
    assert!(body.contains(r#"<div id="add-to-list""#));
    assert!(body.contains(&format!(
        r#"Added 3 items to Viikonloppu — <a href="/lists/{weekend}">open list</a>"#
    )));
    assert!(body.contains(&format!(
        r#"<option value="{weekend}" selected>Viikonloppu</option>"#
    )));

    // The same as adding it from the list page.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{reference}/recipes"))
            .cookie(session.clone())
            .set_form([
                ("recipe_id", recipe.to_string()),
                ("multiplier", "1,5".to_owned()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let added = list_shape(&state, user, weekend).await;
    assert_eq!(added, list_shape(&state, user, reference).await);
    let quantities: Vec<_> = added.iter().map(|i| i.1.as_deref()).collect();
    assert_eq!(quantities, [Some("7.5 dl"), Some("3.75 dl"), None]);
    let detail = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe");
    assert_eq!(added.len(), detail.ingredients.len());
    for (item, ingredient) in added.iter().zip(&detail.ingredients) {
        assert_eq!(item.0, ingredient.text);
        assert_eq!(item.2, ingredient.product_id);
        assert_eq!(item.3, Some(kylma));
    }

    // One ingredient: "1 item".
    let resp = call_service(
        &app,
        submit(single, weekday, "")
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert!(body.contains("Added 1 item to Arki"));

    // Without htmx, and boosted: a 303 to the list; blank means 1.
    for boosted in [false, true] {
        let mut req = submit(recipe, weekday, "");
        if boosted {
            req = req
                .insert_header(("HX-Request", "true"))
                .insert_header(("HX-Boosted", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "boosted: {boosted}");
        assert_eq!(
            resp.headers()
                .get(header::LOCATION)
                .expect("location")
                .to_str()
                .expect("ascii"),
            format!("/lists/{weekday}")
        );
    }
    let texts: Vec<_> = list_shape(&state, user, weekday)
        .await
        .into_iter()
        .map(|i| (i.0, i.1))
        .collect();
    let verbatim = [
        ("Kerma", Some("1 dl")),
        ("Maito", Some("5 dl")),
        ("Vehnäjauho", Some("2½ dl")),
        ("Kananmuna", None),
        ("Maito", Some("5 dl")),
        ("Vehnäjauho", Some("2½ dl")),
        ("Kananmuna", None),
    ]
    .map(|(t, q)| (t.to_owned(), q.map(str::to_owned)));
    assert_eq!(texts, verbatim);

    // Signed out: rejected, nothing added.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/add-to-list"))
            .set_form([("list_id", weekend.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(list_shape(&state, user, weekend).await.len(), 3);
}

/// MB-0015 P3: another user's list or recipe is a 404 and adds nothing; a
/// missing list choice, a bad multiplier, or a recipe without ingredients is
/// a message — in the fragment over htmx, on the full page without it.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_to_list_rejections() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let mine = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "Kylmä").await;
    let my_recipe = make_recipe_with(&state, user, "Puuro", mine, &[("Maito", Some("5 dl"))]).await;
    let empty = make_recipe(&state, user, "Tyhjä").await;
    let their_recipe = make_recipe_with(
        &state,
        other,
        "Salaatti",
        theirs,
        &[("Feta", Some("1 pkt"))],
    )
    .await;
    let my_list = make_list(&state, user, "Oma").await;
    let their_list = make_list(&state, other, "Heidän").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let submit = |recipe: Uuid, form: Vec<(&'static str, String)>, hx: bool| {
        let mut req = TestRequest::post()
            .uri(&format!("/recipes/{recipe}/add-to-list"))
            .cookie(session.clone())
            .set_form(form);
        if hx {
            req = req.insert_header(("HX-Request", "true"));
        }
        req.to_request()
    };
    let list = |id: Uuid| ("list_id", id.to_string());
    let times = |m: &str| ("multiplier", m.to_owned());

    // Another user's list or recipe (or one that doesn't exist): 404, even
    // with otherwise bad input.
    let foreign = [
        (my_recipe, vec![list(their_list), times("2")]),
        (their_recipe, vec![list(my_list), times("2")]),
        (their_recipe, vec![list(their_list), times("2")]),
        (their_recipe, vec![times("kaksi")]),
        (my_recipe, vec![list(Uuid::new_v4()), times("2")]),
        (Uuid::new_v4(), vec![list(my_list), times("2")]),
    ];
    for (recipe, form) in foreign {
        for hx in [false, true] {
            let resp = call_service(&app, submit(recipe, form.clone(), hx)).await;
            assert_eq!(
                resp.status(),
                StatusCode::NOT_FOUND,
                "{form:?} (htmx: {hx})"
            );
        }
    }

    // Rejected input: the message, keeping the typed multiplier and list.
    let rejected = [
        (my_recipe, vec![times("2")], "choose a list to add to"),
        (
            my_recipe,
            vec![("list_id", String::new())],
            "choose a list to add to",
        ),
        (
            my_recipe,
            vec![("list_id", "abc".to_owned())],
            "unknown list",
        ),
        (
            my_recipe,
            vec![list(my_list), times("kaksi")],
            "the multiplier must be a number",
        ),
        (
            my_recipe,
            vec![list(my_list), times("0")],
            "the multiplier must be more than 0 and at most 100",
        ),
        (
            my_recipe,
            vec![list(my_list), times("101")],
            "the multiplier must be more than 0 and at most 100",
        ),
        (
            empty,
            vec![list(my_list), times("2")],
            "Tyhjä has no ingredients",
        ),
    ];
    for (recipe, form, message) in rejected {
        let resp = call_service(&app, submit(recipe, form.clone(), true)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{form:?} over htmx");
        let body = body_string(resp).await;
        assert!(!body.contains("<html"), "fragment only");
        assert!(body.contains(r#"<div id="add-to-list""#));
        assert!(body.contains(message), "{form:?} should say {message:?}");
        if let Some((_, typed)) = form.iter().find(|(k, _)| *k == "multiplier") {
            assert!(
                body.contains(&format!(r#"value="{typed}" />"#)),
                "keeps the typed multiplier"
            );
        }
        if form.contains(&list(my_list)) {
            let chosen = format!(r#"<option value="{my_list}" selected>Oma</option>"#);
            assert!(body.contains(&chosen), "keeps the chosen list");
        }

        let resp = call_service(&app, submit(recipe, form.clone(), false)).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{form:?}");
        let body = body_string(resp).await;
        assert!(body.contains("<html") && body.contains("<h2>Add to a list</h2>"));
        assert!(
            body.contains(message),
            "{form:?} should say {message:?} on the page"
        );
    }

    // Nothing was added anywhere.
    assert_eq!(list_shape(&state, user, my_list).await, Vec::new());
    assert_eq!(list_shape(&state, other, their_list).await, Vec::new());
}
