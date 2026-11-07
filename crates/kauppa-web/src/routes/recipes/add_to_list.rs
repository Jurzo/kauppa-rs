//! "Add to a list" on the recipe page (MB-0015).

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::routes::lists::parse_multiplier;
use crate::routes::request::{current_user, wants_fragment};
use crate::state::AppState;

use super::{Draft, Feedback, Section, render_show};

#[derive(Debug, Deserialize)]
struct AddToListForm {
    // Missing and empty both mean "no list chosen", which becomes a message.
    #[serde(default)]
    list_id: String,
    #[serde(default)]
    multiplier: String,
}

/// The list chosen in the "Add to a list" form. Whether it is the user's is
/// checked by the repo.
fn parse_list(raw: &str) -> CoreResult<Uuid> {
    match raw.trim() {
        "" => Err(CoreError::Invalid("choose a list to add to".to_owned())),
        raw => Uuid::parse_str(raw).map_err(|_| CoreError::Invalid("unknown list".to_owned())),
    }
}

/// Add the recipe's ingredients to one of the user's lists (MB-0015), as
/// `POST /lists/{id}/recipes` does: the same snapshot and multiplier rules,
/// through `ShoppingListRepo::add_recipe`. htmx gets the section back with a
/// confirmation, plain requests a 303 to the list. A missing list, a bad
/// multiplier, or a recipe without ingredients is shown in the section (200
/// over htmx, else the full page with 400); another user's list or recipe is
/// a 404.
#[post("/{id}/add-to-list")]
pub async fn add_recipe_to_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<AddToListForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let recipe_id = path.into_inner();
    let form = form.into_inner();
    let outcome = match (
        parse_list(&form.list_id),
        parse_multiplier(&form.multiplier),
    ) {
        (Ok(list_id), Ok(multiplier)) => {
            ShoppingListRepo::add_recipe(&state.pool, user, list_id, recipe_id, multiplier)
                .await
                .map(|items| (list_id, items.len()))
        }
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    let fragment = wants_fragment(&req);
    let (status, feedback) = match outcome {
        Ok((list_id, _)) if !fragment => {
            return Ok(HttpResponse::SeeOther()
                .append_header((header::LOCATION, format!("/lists/{list_id}")))
                .finish());
        }
        Ok((list_id, count)) => (
            StatusCode::OK,
            Feedback {
                draft: Draft::AddToList {
                    list_id: Some(list_id),
                    multiplier: String::new(),
                },
                added: Some((list_id, count)),
                ..Feedback::default()
            },
        ),
        Err(CoreError::Invalid(reason)) => {
            tracing::debug!(user_id = %user, recipe_id = %recipe_id, %reason, "add to list rejected");
            (
                StatusCode::BAD_REQUEST,
                Feedback {
                    draft: Draft::AddToList {
                        list_id: Uuid::parse_str(form.list_id.trim()).ok(),
                        multiplier: form.multiplier,
                    },
                    error: Some((Section::AddToList, reason)),
                    ..Feedback::default()
                },
            )
        }
        Err(e) => return Err(e.into()),
    };
    if fragment {
        let section = Some(Section::AddToList);
        render_show(&state, user, recipe_id, section, StatusCode::OK, feedback).await
    } else {
        render_show(&state, user, recipe_id, None, status, feedback).await
    }
}

// MB-0003 acceptance tests (P2, P6). They sit at module level rather than in a
// `tests` submodule so the brief's `cargo test -p kauppa-web
// routes::recipes::<name>` checks select them.
