//! Export a list's items to the user's favorites (MB-0060, RR-0047).
//!
//! Anyone who can open the list exports into their own catalog. The list is
//! read through `ShoppingListRepo` (the access predicate, at `view`), and
//! `ProductRepo::apply_export` plans and writes in one transaction on the
//! user's products.
//!
//! - `POST /lists/{id}/export` saves at once when nothing needs an answer,
//!   else shows the review page.
//! - `POST /lists/{id}/export/confirm` carries the review's answers and its
//!   fingerprint. A changed list or catalog, or a missing answer, shows the
//!   review again and saves nothing; a tampered answer is a 400.
//! - Saving answers 303 to `GET /lists/{id}/export/done`, the summary.

use std::collections::BTreeMap;

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Category, ShoppingList};
use kauppa_core::export::{
    Action, ExportOutcome, ExportPlan, ExportSummary, PlanRow, ReviewAnswers, ReviewReason, SKIP,
};
use kauppa_core::repo::{ProductRepo, ShoppingListRepo};

use crate::error::WebResult;
use crate::i18n::{Ui, VARY};
use crate::routes::request::{current_user, is_hx};
use crate::state::AppState;

/// The review form's field holding the fingerprint; answers are `r<index>`.
const FINGERPRINT: &str = "fingerprint";

/// The review page (`lists/export`).
#[derive(Debug, Serialize)]
struct ReviewView {
    list_id: Uuid,
    list_name: String,
    fingerprint: String,
    changed: bool,
    unanswered: bool,
    /// Items saved as they are once the review is confirmed (new favorites
    /// and products from recipes in the list's category).
    auto_count: usize,
    rows: Vec<RowView>,
}

/// One row to answer.
#[derive(Debug, Serialize)]
struct RowView {
    field: String,
    name: String,
    /// The list's categories, as names, joined; empty when none.
    on_list: String,
    /// The list's category names the user doesn't have (a grantee).
    unmatched: String,
    /// The user's product of this name: a favorite, or from recipes, and its
    /// category's name (or "Uncategorized").
    is_favorite: bool,
    from_recipes: bool,
    mine: String,
    choices: Vec<ChoiceView>,
    /// A category picker (an item without a category).
    picker: bool,
    options: Vec<OptionView>,
}

/// A radio choice.
#[derive(Debug, Serialize)]
struct ChoiceView {
    value: String,
    dom_id: String,
    category: String,
    label: Label,
}

/// Which wording a choice gets. Serialized as a map of flags, one true
/// (`{"keep": true, "move_to": false, …}`), so the template tests
/// `this.label.keep`.
#[derive(Debug, Clone, Copy)]
enum Label {
    /// "Keep in ‹category›": the favorite's own category.
    Keep,
    /// "Move to ‹category›": another category for a favorite.
    Move,
    /// "Make it a favorite in ‹category›": a product from recipes.
    FavoriteIn,
    /// "Add in ‹category›": a new favorite.
    AddIn,
    Skip,
}

impl Serialize for Label {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let key = match self {
            Self::Keep => "keep",
            Self::Move => "move_to",
            Self::FavoriteIn => "favorite_in",
            Self::AddIn => "add_in",
            Self::Skip => "skip",
        };
        // Every key, so the template's strict mode finds each one.
        let keys = ["keep", "move_to", "favorite_in", "add_in", "skip"];
        let mut map = serializer.serialize_map(Some(keys.len()))?;
        for k in keys {
            map.serialize_entry(k, &(k == key))?;
        }
        map.end()
    }
}

#[derive(Debug, Serialize)]
struct OptionView {
    id: Uuid,
    name: String,
}

/// The summary page (`lists/export_done`).
#[derive(Debug, Serialize)]
struct DoneView {
    list_id: Uuid,
    list_name: String,
    summary: ExportSummary,
    nothing: bool,
}

/// `GET /lists/{id}/export/done`: the counts the save redirected with. They
/// are only shown, never trusted for anything else.
#[derive(Debug, Default, Deserialize)]
struct DoneQuery {
    #[serde(default)]
    added: usize,
    #[serde(default)]
    already: usize,
    #[serde(default)]
    favorite: usize,
    #[serde(default)]
    moved: usize,
    #[serde(default)]
    skipped: usize,
}

fn category_name(categories: &[Category], id: Uuid) -> String {
    categories
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name.clone())
        .unwrap_or_default()
}

fn choice(index: usize, value: String, category: String, label: Label) -> ChoiceView {
    ChoiceView {
        dom_id: format!("r{index}-{value}"),
        value,
        category,
        label,
    }
}

fn row_view(ui: &Ui, categories: &[Category], index: usize, row: &PlanRow) -> WebResult<RowView> {
    let names = |ids: &[Uuid]| -> Vec<String> {
        ids.iter()
            .map(|id| category_name(categories, *id))
            .collect()
    };
    let existing = row.existing.as_ref();
    let is_favorite = existing.is_some_and(|e| e.is_favorite);
    let mine = match existing.and_then(|e| e.category_id) {
        Some(id) => category_name(categories, id),
        None if existing.is_some() => ui.text("common.uncategorized", &[])?,
        None => String::new(),
    };
    let mut choices = Vec::new();
    let mut picker = false;
    if let Some(offered) = row.choices() {
        let label = |is_keep: bool| match existing {
            Some(e) if e.is_favorite && is_keep => Label::Keep,
            Some(e) if e.is_favorite => Label::Move,
            Some(_) => Label::FavoriteIn,
            None => Label::AddIn,
        };
        if let Some(keep) = offered.keep {
            let name = category_name(categories, keep);
            choices.push(choice(index, keep.to_string(), name, label(true)));
        }
        for id in offered.list {
            let is_keep = existing.and_then(|e| e.category_id) == Some(id);
            let name = category_name(categories, id);
            choices.push(choice(index, id.to_string(), name, label(is_keep)));
        }
        picker = offered.any;
        if offered.skip && !picker {
            choices.push(choice(index, SKIP.to_owned(), String::new(), Label::Skip));
        }
    }
    Ok(RowView {
        field: format!("r{index}"),
        name: row.name.clone(),
        on_list: names(&row.list_categories).join(", "),
        unmatched: row.unmatched.join(", "),
        is_favorite,
        from_recipes: existing.is_some() && !is_favorite,
        mine,
        choices,
        picker,
        options: categories
            .iter()
            .map(|c| OptionView {
                id: c.id,
                name: c.name.clone(),
            })
            .collect(),
    })
}

fn review_view(
    ui: &Ui,
    list: &ShoppingList,
    plan: &ExportPlan,
    categories: &[Category],
    reason: ReviewReason,
) -> WebResult<ReviewView> {
    Ok(ReviewView {
        list_id: list.id,
        list_name: list.name.clone(),
        fingerprint: plan.fingerprint.clone(),
        changed: reason == ReviewReason::Changed,
        unanswered: reason == ReviewReason::Unanswered,
        auto_count: plan
            .rows
            .iter()
            .filter(|r| matches!(r.action, Action::Add(_) | Action::Promote(_)))
            .count(),
        rows: plan
            .review_rows()
            .map(|(i, row)| row_view(ui, categories, i, row))
            .collect::<WebResult<_>>()?,
    })
}

/// Answer an export: the summary's 303 once saved, else the review page —
/// 200, or 400 for a missing answer without htmx.
fn answer(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    list: &ShoppingList,
    outcome: ExportOutcome,
) -> WebResult<HttpResponse> {
    match outcome {
        ExportOutcome::Saved(s) => {
            let location = format!(
                "/lists/{}/export/done?added={}&already={}&favorite={}&moved={}&skipped={}",
                list.id, s.added, s.already, s.made_favorite, s.recategorized, s.skipped
            );
            Ok(HttpResponse::SeeOther()
                .append_header((header::LOCATION, location))
                .finish())
        }
        ExportOutcome::Review {
            plan,
            categories,
            reason,
        } => {
            let view = review_view(ui, list, &plan, &categories, reason)?;
            let status = if reason == ReviewReason::Unanswered && !is_hx(req) {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::OK
            };
            Ok(HttpResponse::build(status)
                .content_type("text/html")
                .insert_header(VARY)
                .body(ui.render(state, "lists/export", &view)?))
        }
    }
}

/// Start an export: save, or show the review.
#[post("/{id}/export")]
pub async fn export(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let items = ShoppingListRepo::items(&state.pool, user, list_id, false).await?;
    let outcome = ProductRepo::apply_export(&state.pool, user, &items, None).await?;
    if let ExportOutcome::Saved(s) = &outcome {
        tracing::info!(user_id = %user, list_id = %list_id, added = s.added, "list exported to favorites");
    }
    answer(&state, &ui, &req, &list, outcome)
}

/// Confirm a review: the answers and the fingerprint they were made against.
#[post("/{id}/export/confirm")]
pub async fn export_confirm(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<Vec<(String, String)>>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let mut answers = ReviewAnswers {
        fingerprint: String::new(),
        answers: BTreeMap::new(),
    };
    for (name, value) in form.into_inner() {
        if name == FINGERPRINT {
            answers.fingerprint = value;
        } else if let Some(index) = name.strip_prefix('r').and_then(|i| i.parse().ok()) {
            answers.answers.insert(index, value);
        }
    }
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let items = ShoppingListRepo::items(&state.pool, user, list_id, false).await?;
    let outcome = ProductRepo::apply_export(&state.pool, user, &items, Some(&answers)).await?;
    if let ExportOutcome::Saved(s) = &outcome {
        tracing::info!(user_id = %user, list_id = %list_id, added = s.added, "list exported to favorites");
    }
    answer(&state, &ui, &req, &list, outcome)
}

/// The summary after a save.
#[get("/{id}/export/done")]
pub async fn export_done(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    query: web::Query<DoneQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list = ShoppingListRepo::get(&state.pool, user, path.into_inner()).await?;
    let q = query.into_inner();
    let summary = ExportSummary {
        added: q.added,
        already: q.already,
        made_favorite: q.favorite,
        recategorized: q.moved,
        skipped: q.skipped,
    };
    let view = DoneView {
        list_id: list.id,
        list_name: list.name,
        nothing: summary == ExportSummary::default(),
        summary,
    };
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(VARY)
        .body(ui.render(&state, "lists/export_done", &view)?))
}
