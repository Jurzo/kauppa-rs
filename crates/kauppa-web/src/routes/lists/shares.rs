//! Sharing (MB-0008): the owner's `/lists/{id}/sharing` page, share and revoke.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::domain::{ListShare, ShareRole};
use kauppa_core::repo::ShoppingListRepo;

use crate::error::WebResult;
use crate::routes::request::{current_user, is_hx, wants_fragment};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct ShareForm {
    #[serde(default)]
    email: String,
    #[serde(default)]
    role: String,
}

#[derive(Debug, Deserialize)]
struct SharingQuery {
    /// Set by the 303 after a plain share, to show the generic notice.
    #[serde(default)]
    shared: Option<String>,
}

/// The sharing page (`lists/sharing`).
#[derive(Debug, Serialize)]
struct SharingView {
    id: Uuid,
    name: String,
    form: ShareFormView,
    grantees: GranteesView,
}

/// The share form (`lists/_share_form`). After a share it shows the same
/// notice whether or not the email matched an account, and nothing else that
/// depends on it (RR-0002 constraint 3).
#[derive(Debug, Default, Serialize)]
struct ShareFormView {
    id: Uuid,
    email: String,
    edit_selected: bool,
    notice: bool,
    error: Option<String>,
}

/// The people with access (`lists/_grantees`).
#[derive(Debug, Serialize)]
struct GranteesView {
    id: Uuid,
    shares: Vec<GranteeRow>,
    count: usize,
}

#[derive(Debug, Serialize)]
struct GranteeRow {
    /// The grant's id, which revoke takes.
    id: Uuid,
    name: String,
    email: String,
    can_edit: bool,
}

impl GranteesView {
    fn new(list_id: Uuid, shares: Vec<ListShare>) -> Self {
        let shares: Vec<GranteeRow> = shares
            .into_iter()
            .map(|s| GranteeRow {
                id: s.id,
                name: s.grantee_name,
                email: s.grantee_email,
                can_edit: s.role == ShareRole::Edit,
            })
            .collect();
        Self {
            id: list_id,
            count: shares.len(),
            shares,
        }
    }
}

/// Render the sharing page, or for htmx the share form fragment. Owner only:
/// `shares` runs the access predicate (404 for strangers, 403 for grantees).
async fn render_sharing(
    state: &AppState,
    user: Uuid,
    list_id: Uuid,
    fragment: bool,
    status: StatusCode,
    form: ShareFormView,
) -> WebResult<HttpResponse> {
    let shares = ShoppingListRepo::shares(&state.pool, user, list_id).await?;
    let body = if fragment {
        state.templates.render("lists/_share_form", &form)?
    } else {
        let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
        state.templates.render(
            "lists/sharing",
            &SharingView {
                id: list.id,
                name: list.name,
                form,
                grantees: GranteesView::new(list_id, shares),
            },
        )?
    };
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Who has access to a list, and the form to share it.
#[get("/{id}/sharing")]
pub async fn sharing(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    query: web::Query<SharingQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let form = ShareFormView {
        id: list_id,
        notice: query.shared.is_some(),
        ..ShareFormView::default()
    };
    // A boosted or plain request gets the page; only a fragment request the
    // form.
    render_sharing(
        &state,
        user,
        list_id,
        wants_fragment(&req),
        StatusCode::OK,
        form,
    )
    .await
}

/// Share the list with the account that uses `email`, as `view` or `edit`
/// (owner only). The answer is the same whether or not an account matched,
/// so it never tells the owner whether one exists (RR-0002 constraint 3):
/// the form comes back empty with a generic notice, or a 303 to the sharing
/// page.
#[post("/{id}/shares")]
pub async fn share(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<ShareForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let outcome = match ShareRole::parse(&form.role) {
        Ok(role) => ShoppingListRepo::share(&state.pool, user, list_id, &form.email, role)
            .await
            .map(|granted| (role, granted)),
        Err(e) => Err(e),
    };
    let error = match outcome {
        Ok((role, granted)) => {
            // Audit (RR-0002 constraint 5): grantor and list, never the
            // grantee's email. Whether an account matched is for the server
            // log only.
            if let Some(share) = granted {
                tracing::info!(
                    grantor_id = %user,
                    list_id = %list_id,
                    share_id = %share.id,
                    grantee_id = %share.grantee_id,
                    role = %role,
                    "list share granted"
                );
            } else {
                tracing::info!(
                    grantor_id = %user,
                    list_id = %list_id,
                    role = %role,
                    "list share requested; no single other account matched"
                );
            }
            None
        }
        Err(CoreError::Invalid(m)) => Some(m),
        Err(e) => return Err(e.into()),
    };

    let Some(error) = error else {
        return if is_hx(&req) {
            let form = ShareFormView {
                id: list_id,
                notice: true,
                ..ShareFormView::default()
            };
            render_sharing(&state, user, list_id, true, StatusCode::OK, form).await
        } else {
            Ok(HttpResponse::SeeOther()
                .append_header((
                    header::LOCATION,
                    format!("/lists/{list_id}/sharing?shared=1"),
                ))
                .finish())
        };
    };
    tracing::debug!(user_id = %user, list_id = %list_id, reason = %error, "share rejected");
    let form = form.into_inner();
    let view = ShareFormView {
        id: list_id,
        email: form.email,
        edit_selected: form.role.trim() == "edit",
        notice: false,
        error: Some(error),
    };
    let (fragment, status) = if is_hx(&req) {
        (true, StatusCode::OK)
    } else {
        (false, StatusCode::BAD_REQUEST)
    };
    render_sharing(&state, user, list_id, fragment, status, view).await
}

/// Revoke a grant (owner only). The grantee loses access on their next
/// request. htmx gets the `#grantees-{id}` fragment, a plain request a 303
/// back to the sharing page.
#[post("/{id}/shares/{share}/revoke")]
pub async fn revoke(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, share_id) = path.into_inner();
    let revoked = ShoppingListRepo::revoke(&state.pool, user, list_id, share_id).await?;
    tracing::info!(
        grantor_id = %user,
        list_id = %list_id,
        share_id = %revoked.id,
        grantee_id = %revoked.grantee_id,
        role = %revoked.role,
        "list share revoked"
    );

    if is_hx(&req) {
        let shares = ShoppingListRepo::shares(&state.pool, user, list_id).await?;
        let body = state
            .templates
            .render("lists/_grantees", &GranteesView::new(list_id, shares))?;
        Ok(HttpResponse::Ok().content_type("text/html").body(body))
    } else {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, format!("/lists/{list_id}/sharing")))
            .finish())
    }
}
