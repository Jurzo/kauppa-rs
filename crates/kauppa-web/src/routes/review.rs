//! The review step shared by list export (MB-0060, RR-0047) and favorites
//! import (MB-0061, RR-0048, constraint 5): the review page, its answers, and
//! the summary page.
//!
//! The page posts the answers (`r<index>`), the plan's fingerprint and any
//! fields the caller carries (`hidden`, e.g. the chosen favorites) to
//! `action`; saving answers 303 to `<base>/done` with the counts, which the
//! summary only displays.

use std::collections::BTreeMap;

use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::Category;
use kauppa_core::export::{
    Action, ExportOutcome, ExportPlan, ExportSummary, PlanRow, ReviewAnswers, ReviewReason, SKIP,
};

use crate::error::WebResult;
use crate::i18n::{Ui, VARY};
use crate::routes::request::is_hx;
use crate::state::AppState;

/// Where the items come from, for the pages' wording.
#[derive(Debug, Clone)]
pub enum Source {
    /// A shopping list, by name (MB-0060).
    List(String),
    /// Another user's shared favorites, by their display name (MB-0061).
    Shared(String),
    /// Another user's shared recipe: their display name and its title
    /// (MB-0061).
    SharedRecipe { name: String, title: String },
}

/// What a review or summary page needs besides the plan.
#[derive(Debug, Clone)]
pub struct Page {
    /// Where the review form posts.
    pub action: String,
    /// The summary's path prefix: saving redirects to `<base>/done`.
    pub base: String,
    /// The back link.
    pub back: String,
    pub source: Source,
    /// Fields the review form carries unchanged.
    pub hidden: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
struct HiddenView {
    name: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct SourceView {
    from_list: bool,
    from_recipe: bool,
    name: String,
    title: String,
}

impl From<&Source> for SourceView {
    fn from(source: &Source) -> Self {
        match source {
            Source::List(name) => Self {
                from_list: true,
                from_recipe: false,
                name: name.clone(),
                title: String::new(),
            },
            Source::Shared(name) => Self {
                from_list: false,
                from_recipe: false,
                name: name.clone(),
                title: String::new(),
            },
            Source::SharedRecipe { name, title } => Self {
                from_list: false,
                from_recipe: true,
                name: name.clone(),
                title: title.clone(),
            },
        }
    }
}

/// The review form's field holding the fingerprint; answers are `r<index>`.
const FINGERPRINT: &str = "fingerprint";

/// The review page (`review/page`).
#[derive(Debug, Serialize)]
struct ReviewView {
    action: String,
    back: String,
    source: SourceView,
    hidden: Vec<HiddenView>,
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
    /// A category picker, for an item without a category.
    picker: Option<PickerView>,
}

#[derive(Debug, Serialize)]
struct PickerView {
    /// Whether "Skip" is offered (an ingredient needs a category).
    skippable: bool,
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

/// The summary page (`review/done`).
#[derive(Debug, Serialize)]
struct DoneView {
    back: String,
    source: SourceView,
    summary: ExportSummary,
    nothing: bool,
}

/// `GET <base>/done`: the counts the save redirected with. They are only
/// shown, never trusted for anything else.
#[derive(Debug, Default, Clone, Copy, Deserialize)]
pub struct DoneQuery {
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
    let mut any = false;
    let mut skippable = false;
    if let Some(offered) = row.choices() {
        skippable = offered.skip;
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
        any = offered.any;
        if offered.skip && !any {
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
        picker: any.then(|| PickerView {
            skippable,
            options: categories
                .iter()
                .map(|c| OptionView {
                    id: c.id,
                    name: c.name.clone(),
                })
                .collect(),
        }),
    })
}

fn review_view(
    ui: &Ui,
    page: &Page,
    plan: &ExportPlan,
    categories: &[Category],
    reason: ReviewReason,
) -> WebResult<ReviewView> {
    Ok(ReviewView {
        action: page.action.clone(),
        back: page.back.clone(),
        source: SourceView::from(&page.source),
        hidden: page
            .hidden
            .iter()
            .map(|(name, value)| HiddenView {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
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

/// Split a review form into its answers and the other fields.
pub fn answers(form: Vec<(String, String)>) -> (ReviewAnswers, Vec<(String, String)>) {
    let mut answers = ReviewAnswers {
        fingerprint: String::new(),
        answers: BTreeMap::new(),
    };
    let mut others = Vec::new();
    for (name, value) in form {
        if name == FINGERPRINT {
            answers.fingerprint = value;
        } else if let Some(index) = name.strip_prefix('r').and_then(|i| i.parse().ok()) {
            answers.answers.insert(index, value);
        } else {
            others.push((name, value));
        }
    }
    (answers, others)
}

/// Answer an export or import: the summary's 303 once saved, else the review
/// page — 200, or 400 for a missing answer without htmx.
pub fn respond(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    page: &Page,
    outcome: ExportOutcome,
) -> WebResult<HttpResponse> {
    match outcome {
        ExportOutcome::Saved(s) => {
            let location = format!(
                "{}/done?added={}&already={}&favorite={}&moved={}&skipped={}",
                page.base, s.added, s.already, s.made_favorite, s.recategorized, s.skipped
            );
            Ok(HttpResponse::SeeOther()
                .append_header((header::LOCATION, location))
                .finish())
        }
        ExportOutcome::Review {
            plan,
            categories,
            reason,
        } => review_page(state, ui, req, page, &plan, &categories, reason),
    }
}

/// The review page for `plan` — 200, or 400 for a missing answer without
/// htmx. Also used by the recipe import (MB-0061).
pub fn review_page(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    page: &Page,
    plan: &ExportPlan,
    categories: &[Category],
    reason: ReviewReason,
) -> WebResult<HttpResponse> {
    let view = review_view(ui, page, plan, categories, reason)?;
    let status = if reason == ReviewReason::Unanswered && !is_hx(req) {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::OK
    };
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .insert_header(VARY)
        .body(ui.render(state, "review/page", &view)?))
}

/// The summary page after a save.
pub fn done(
    state: &AppState,
    ui: &Ui,
    back: String,
    source: &Source,
    query: DoneQuery,
) -> WebResult<HttpResponse> {
    let summary = ExportSummary {
        added: query.added,
        already: query.already,
        made_favorite: query.favorite,
        recategorized: query.moved,
        skipped: query.skipped,
    };
    let view = DoneView {
        back,
        source: SourceView::from(source),
        nothing: summary == ExportSummary::default(),
        summary,
    };
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(VARY)
        .body(ui.render(state, "review/done", &view)?))
}
