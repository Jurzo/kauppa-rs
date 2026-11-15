//! Exporting a list's items to the user's favorites (MB-0060, RR-0047).
//!
//! The plan, its fingerprint, and checking the user's answers, in plain Rust
//! with no I/O: `ProductRepo::apply_export` loads the catalog inside its
//! write transaction, plans with [`plan`], checks the review with
//! [`decide`], and writes.
//!
//! - **Candidates.** Every active list item, by name (`name_key`, RR-0003):
//!   the same name more than once is one row. An item's category is the list
//!   owner's; it counts for the exporting user only as their own category,
//!   matched by id when they own the list and by name otherwise (RR-0047
//!   sub-answer 2). Unmatched names count as uncategorized and are kept for
//!   display.
//! - **Rows.** Against the user's product of that name, if any: a new
//!   favorite, nothing to do, a product from recipes to promote, or a row to
//!   review — a category conflict, copies with different categories, or no
//!   category (sub-answers 3–5).
//! - **Review.** Every review row needs an explicit answer, none preselected;
//!   answers are accepted only from the row's choices (sub-answer 5).
//! - **Fingerprint.** SHA-256 over everything the plan was computed from, so
//!   a confirmation against a changed list or catalog is caught
//!   (sub-answer 6).

use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::{Category, ListItem, Product};

/// One thing to bring into the user's catalog.
///
/// A name and, if it has one, a category of whoever it comes from — a list
/// item (MB-0060) or a shared favorite (MB-0061, RR-0048). A category the
/// user owns is used by id; anyone else's is matched by name.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub category: Option<Category>,
}

impl From<&ListItem> for Candidate {
    fn from(item: &ListItem) -> Self {
        Self {
            name: item.text.clone(),
            category: item.category.clone(),
        }
    }
}
use crate::repo::name_key;
use crate::{CoreError, CoreResult, Entity, Invalid};

/// The user's existing product of a row's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Existing {
    pub id: Uuid,
    pub name: String,
    /// Its category, if it is one of the user's.
    pub category_id: Option<Uuid>,
    pub is_favorite: bool,
}

/// Why a row needs the user's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewKind {
    /// One list category, and the user's product of that name is in another
    /// (or none).
    Conflict,
    /// The list has the name in more than one category.
    Copies,
    /// No list category the user has.
    Uncategorized,
}

/// What a row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A new favorite in this category.
    Add(Uuid),
    /// Already a favorite in this category: nothing to do.
    AlreadyThere,
    /// A product from recipes in this category: make it a favorite.
    Promote(Uuid),
    /// Needs the user's answer.
    Review(ReviewKind),
}

/// One distinct name on the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRow {
    /// The name as the list's first copy has it.
    pub name: String,
    pub key: String,
    /// The list's categories as the user's own, distinct, in list order.
    pub list_categories: Vec<Uuid>,
    /// List category names the user has no category for (a grantee's
    /// export), distinct, in list order.
    pub unmatched: Vec<String>,
    pub existing: Option<Existing>,
    pub action: Action,
}

/// What a review row offers. `keep` and `list` are category ids; `any` means
/// any of the user's categories (a picker); `skip` whether skipping is
/// offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choices {
    pub keep: Option<Uuid>,
    pub list: Vec<Uuid>,
    pub any: bool,
    pub skip: bool,
}

impl PlanRow {
    /// The row's choices, if it needs review (RR-0047 sub-answer 5).
    #[must_use]
    pub fn choices(&self) -> Option<Choices> {
        let Action::Review(kind) = self.action else {
            return None;
        };
        let existing_category = self.existing.as_ref().and_then(|e| e.category_id);
        let favorite_in_place = self
            .existing
            .as_ref()
            .is_some_and(|e| e.is_favorite && e.category_id.is_some());
        Some(match kind {
            ReviewKind::Conflict | ReviewKind::Copies => Choices {
                keep: existing_category.filter(|c| !self.list_categories.contains(c)),
                list: self.list_categories.clone(),
                any: false,
                // A favorite with a category is kept by choosing it; anything
                // else can be left as it is.
                skip: !favorite_in_place,
            },
            ReviewKind::Uncategorized => Choices {
                keep: None,
                list: Vec::new(),
                any: true,
                skip: true,
            },
        })
    }
}

/// The plan for one export, in list order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlan {
    pub rows: Vec<PlanRow>,
    /// SHA-256 (hex) of the plan's inputs; see [`plan`].
    pub fingerprint: String,
}

impl ExportPlan {
    /// The rows that need the user's answer, with their index in `rows`.
    pub fn review_rows(&self) -> impl Iterator<Item = (usize, &PlanRow)> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.action, Action::Review(_)))
    }

    #[must_use]
    pub fn needs_review(&self) -> bool {
        self.review_rows().next().is_some()
    }
}

/// Plan bringing `items` (a list's active items, or shared favorites) into
/// `user`'s catalog: their `categories` and all their `products`, favorites
/// and from recipes.
#[must_use]
pub fn plan(
    user: Uuid,
    items: &[Candidate],
    categories: &[Category],
    products: &[Product],
) -> ExportPlan {
    let own: Vec<Uuid> = categories.iter().map(|c| c.id).collect();
    let by_name: BTreeMap<String, Uuid> = categories
        .iter()
        .map(|c| (name_key(&c.name), c.id))
        .collect();
    let products: BTreeMap<String, &Product> =
        products.iter().map(|p| (name_key(&p.name), p)).collect();

    let mut rows: Vec<PlanRow> = Vec::new();
    for item in items {
        let key = name_key(&item.name);
        if key.is_empty() {
            continue;
        }
        let (mapped, unmatched) = match &item.category {
            None => (None, None),
            Some(c) if c.owner_id == user => (own.contains(&c.id).then_some(c.id), None),
            Some(c) => by_name
                .get(&name_key(&c.name))
                .map_or_else(|| (None, Some(c.name.clone())), |id| (Some(*id), None)),
        };
        let index = rows.iter().position(|r| r.key == key).unwrap_or_else(|| {
            rows.push(PlanRow {
                name: item.name.trim().to_owned(),
                key: key.clone(),
                list_categories: Vec::new(),
                unmatched: Vec::new(),
                existing: products.get(&key).map(|p| Existing {
                    id: p.id,
                    name: p.name.clone(),
                    category_id: p.category_id.filter(|c| own.contains(c)),
                    is_favorite: p.is_favorite,
                }),
                action: Action::AlreadyThere,
            });
            rows.len() - 1
        });
        let Some(row) = rows.get_mut(index) else {
            continue;
        };
        if let Some(id) = mapped
            && !row.list_categories.contains(&id)
        {
            row.list_categories.push(id);
        }
        if let Some(name) = unmatched
            && !row.unmatched.contains(&name)
        {
            row.unmatched.push(name);
        }
    }
    for row in &mut rows {
        row.action = classify(row);
    }
    let fingerprint = fingerprint(categories, &rows);
    ExportPlan { rows, fingerprint }
}

fn classify(row: &PlanRow) -> Action {
    let existing = row.existing.as_ref();
    match row.list_categories.as_slice() {
        [] => match existing {
            Some(e) if e.is_favorite => Action::AlreadyThere,
            _ => Action::Review(ReviewKind::Uncategorized),
        },
        [c] => match existing {
            None => Action::Add(*c),
            Some(e) if e.category_id == Some(*c) && e.is_favorite => Action::AlreadyThere,
            Some(e) if e.category_id == Some(*c) => Action::Promote(*c),
            Some(_) => Action::Review(ReviewKind::Conflict),
        },
        _ => Action::Review(ReviewKind::Copies),
    }
}

/// SHA-256 (hex) over the user's categories and every row's inputs: name,
/// list categories, unmatched names and the existing product. Any change to
/// the list's items or the user's catalog that could change the review
/// changes it.
fn fingerprint(categories: &[Category], rows: &[PlanRow]) -> String {
    let mut hash = Sha256::new();
    let mut line = |parts: &[&str]| {
        for part in parts {
            hash.update(part.len().to_le_bytes());
            hash.update(part.as_bytes());
        }
        hash.update(b"\n");
    };
    for c in categories {
        line(&["c", &c.id.to_string(), &c.name]);
    }
    for row in rows {
        let list: Vec<String> = row.list_categories.iter().map(Uuid::to_string).collect();
        line(&[
            "r",
            &row.key,
            &row.name,
            &list.join(","),
            &row.unmatched.join("\u{1f}"),
        ]);
        if let Some(e) = &row.existing {
            line(&[
                "p",
                &e.id.to_string(),
                &e.name,
                &e.category_id.map(|c| c.to_string()).unwrap_or_default(),
                if e.is_favorite { "1" } else { "0" },
            ]);
        }
    }
    hex(&hash.finalize())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// The answer that skips a row.
pub const SKIP: &str = "skip";

/// The review form's answers: the fingerprint it was computed from, and the
/// raw answer per review row index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewAnswers {
    pub fingerprint: String,
    pub answers: BTreeMap<usize, String>,
}

/// Why the review is shown (again).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReason {
    /// The first time: something needs an answer.
    Needed,
    /// The list or the catalog changed since the review was shown.
    Changed,
    /// A row had no answer.
    Unanswered,
}

/// The checked answers: for each review row index, the category to use, or
/// `None` to skip.
pub type Decisions = BTreeMap<usize, Option<Uuid>>;

/// Check `answers` against `plan`.
///
/// `Err(reason)` means show the review again (stale or unanswered); a
/// tampered answer — an index that isn't a review row, or a value that isn't
/// one of the row's choices — is `Invalid`.
pub fn decide(
    plan: &ExportPlan,
    categories: &[Category],
    answers: &ReviewAnswers,
) -> CoreResult<Result<Decisions, ReviewReason>> {
    if answers.fingerprint != plan.fingerprint {
        return Ok(Err(ReviewReason::Changed));
    }
    let review: BTreeMap<usize, &PlanRow> = plan.review_rows().collect();
    if answers.answers.keys().any(|i| !review.contains_key(i)) {
        return Err(tampered());
    }
    let mut decisions = Decisions::new();
    let mut unanswered = false;
    for (index, row) in review {
        let choices = row.choices().ok_or_else(tampered)?;
        match answers.answers.get(&index).map(|a| a.trim()) {
            None | Some("") => unanswered = true,
            Some(SKIP) if choices.skip => {
                decisions.insert(index, None);
            }
            Some(raw) => {
                let id = Uuid::parse_str(raw).map_err(|_| tampered())?;
                let offered = choices.keep == Some(id)
                    || choices.list.contains(&id)
                    || (choices.any && categories.iter().any(|c| c.id == id));
                if !offered {
                    return Err(tampered());
                }
                decisions.insert(index, Some(id));
            }
        }
    }
    Ok(if unanswered {
        Err(ReviewReason::Unanswered)
    } else {
        Ok(decisions)
    })
}

const fn tampered() -> CoreError {
    CoreError::Invalid(Invalid::Unknown(Entity::Category))
}

/// What an export did (MB-0060 P5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ExportSummary {
    /// New favorites.
    pub added: usize,
    /// Already favorites, left as they were.
    pub already: usize,
    /// Products from recipes made favorites.
    pub made_favorite: usize,
    /// Favorites moved to another category.
    pub recategorized: usize,
    /// Rows the user skipped.
    pub skipped: usize,
}

/// The result of `ProductRepo::apply_export`.
#[derive(Debug, Clone)]
pub enum ExportOutcome {
    /// Everything was saved, in one transaction.
    Saved(ExportSummary),
    /// Nothing was saved: the review to show, with the user's categories it
    /// was planned against.
    Review {
        plan: ExportPlan,
        categories: Vec<Category>,
        reason: ReviewReason,
    },
}

/// One write the export makes, from a plan row and its decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    /// Insert a favorite named `name` in `category`.
    Insert { name: String, category: Uuid },
    /// Make product `id` a favorite in `category` (moving it there if it is
    /// elsewhere).
    Favorite { id: Uuid, category: Uuid },
}

/// The writes for `plan` with `decisions`, and the summary they add up to.
#[must_use]
pub fn writes(plan: &ExportPlan, decisions: &Decisions) -> (Vec<Write>, ExportSummary) {
    let mut out = Vec::new();
    let mut summary = ExportSummary::default();
    for (index, row) in plan.rows.iter().enumerate() {
        let target = match row.action {
            Action::AlreadyThere => {
                summary.already += 1;
                continue;
            }
            Action::Add(c) | Action::Promote(c) => Some(c),
            Action::Review(_) => decisions.get(&index).copied().flatten(),
        };
        let Some(category) = target else {
            summary.skipped += 1;
            continue;
        };
        match &row.existing {
            None => {
                summary.added += 1;
                out.push(Write::Insert {
                    name: row.name.clone(),
                    category,
                });
            }
            Some(e) if e.is_favorite && e.category_id == Some(category) => summary.already += 1,
            Some(e) => {
                if e.is_favorite {
                    summary.recategorized += 1;
                } else {
                    summary.made_favorite += 1;
                }
                out.push(Write::Favorite { id: e.id, category });
            }
        }
    }
    (out, summary)
}

#[cfg(test)]
mod tests;
