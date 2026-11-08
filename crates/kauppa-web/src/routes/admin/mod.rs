//! The admin portal (MB-0023, RR-0012): invitations, for administrators
//! only.
//!
//! Every handler takes [`RequireAdmin`] first: no identity → 303 to the
//! login page; signed in but not in `admins` → the plain 404, as for a page
//! that doesn't exist. Admin status is read from the database on every
//! request, never cached in the session (RR-0012 constraint 5).
//!
//! Plain forms and 303s throughout, so the portal works without htmx; with
//! `hx-boost` the same responses swap the whole page.

use std::future::Future;
use std::pin::Pin;

use actix_identity::Identity;
use actix_web::dev::Payload;
use actix_web::error::InternalError;
use actix_web::http::StatusCode;
use actix_web::http::header::{self, HeaderValue};
use actix_web::{FromRequest, HttpRequest, HttpResponse, get, post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::domain::{Invitation, InvitationState, InvitationUser};
use kauppa_core::repo::{AdminRepo, InvitationRepo};

use crate::error::{WebError, WebResult};
use crate::state::AppState;

#[cfg(test)]
mod tests;

/// The signed-in administrator's user id. Extracting it is the access
/// check: anonymous → 303 to `/auth/login`, non-admin → 404.
#[derive(Debug, Clone, Copy)]
pub struct RequireAdmin(pub Uuid);

impl FromRequest for RequireAdmin {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let user = Identity::from_request(req, payload)
            .into_inner()
            .ok()
            .and_then(|id| id.id().ok())
            .and_then(|raw| Uuid::parse_str(&raw).ok());
        let state = req.app_data::<web::Data<AppState>>().cloned();
        Box::pin(async move {
            let Some(user) = user else {
                let to_login = HttpResponse::SeeOther()
                    .insert_header((header::LOCATION, "/auth/login"))
                    .finish();
                return Err(InternalError::from_response("sign-in required", to_login).into());
            };
            let state = state.ok_or_else(|| WebError::Internal("no app state".to_owned()))?;
            if AdminRepo::is_admin(&state.pool, user)
                .await
                .map_err(WebError::from)?
            {
                Ok(Self(user))
            } else {
                tracing::debug!(user_id = %user, "admin portal: not an admin");
                Err(WebError::Core(CoreError::NotFound).into())
            }
        })
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct NoteForm {
    #[serde(default)]
    note: String,
}

/// The invitations page (`admin/invitations`).
#[derive(Debug, Default, Serialize)]
struct InvitationsView {
    /// The invitation just created, with its link: shown this once.
    created: Option<CreatedView>,
    /// The create form's note and error, after a rejected create.
    note: String,
    error: Option<String>,
    invitations: Vec<InvitationRow>,
    count: usize,
}

#[derive(Debug, Serialize)]
struct CreatedView {
    link: String,
    note: Option<String>,
    expires: String,
}

#[derive(Debug, Serialize)]
struct InvitationRow {
    id: Uuid,
    note: Option<String>,
    state: &'static str,
    pending: bool,
    created: String,
    expires: String,
    used: Option<String>,
    used_by: Option<InvitationUser>,
}

impl InvitationRow {
    fn new(invitation: Invitation, now: DateTime<Utc>) -> Self {
        let state = invitation.state(now);
        Self {
            id: invitation.id,
            state: state.as_str(),
            pending: state == InvitationState::Pending,
            created: when(invitation.created_at),
            expires: when(invitation.expires_at),
            used: invitation.used_at.map(when),
            note: invitation.note,
            used_by: invitation.used_by,
        }
    }
}

fn when(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M UTC").to_string()
}

async fn render_invitations(
    state: &AppState,
    status: StatusCode,
    mut view: InvitationsView,
) -> WebResult<HttpResponse> {
    let now = Utc::now();
    view.invitations = InvitationRepo::list(&state.pool)
        .await?
        .into_iter()
        .map(|i| InvitationRow::new(i, now))
        .collect();
    view.count = view.invitations.len();
    let body = state.templates.render("admin/invitations", &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// `/admin` is the invitations page.
#[get("")]
pub async fn index(_admin: RequireAdmin) -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/admin/invitations"))
        .finish()
}

/// Every invitation, newest first, and the form to create one.
#[get("/invitations")]
pub async fn invitations(
    _admin: RequireAdmin,
    state: web::Data<AppState>,
) -> WebResult<HttpResponse> {
    render_invitations(&state, StatusCode::OK, InvitationsView::default()).await
}

/// Create an invitation and show its link, once. The answer is the page
/// itself rather than a 303: the token can't go in a redirect URL, and is
/// never stored. It is kept out of caches and referrers.
#[post("/invitations")]
pub async fn create_invitation(
    admin: RequireAdmin,
    req: HttpRequest,
    state: web::Data<AppState>,
    form: Result<web::Form<NoteForm>, actix_web::Error>,
) -> WebResult<HttpResponse> {
    let note = form.map(|f| f.into_inner().note).unwrap_or_default();
    let created =
        match InvitationRepo::create(&state.pool, Some(admin.0), Some(&note), Utc::now()).await {
            Ok(created) => created,
            Err(CoreError::Invalid(message)) => {
                let view = InvitationsView {
                    note,
                    error: Some(message),
                    ..InvitationsView::default()
                };
                return render_invitations(&state, StatusCode::BAD_REQUEST, view).await;
            }
            Err(e) => return Err(e.into()),
        };
    // Audit (P7): the admin and the invitation; never the token or the note.
    tracing::info!(
        admin_id = %admin.0,
        invitation_id = %created.invitation.id,
        "invitation created"
    );

    let path = format!("/auth/register?invite={}", created.token.as_str());
    let link = state
        .origin
        .app_origin(req.headers(), req.uri())
        .map_or_else(|| path.clone(), |origin| format!("{origin}{path}"));
    let view = InvitationsView {
        created: Some(CreatedView {
            link,
            note: created.invitation.note,
            expires: when(created.invitation.expires_at),
        }),
        ..InvitationsView::default()
    };
    let mut resp = render_invitations(&state, StatusCode::OK, view).await?;
    let headers = resp.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    Ok(resp)
}

/// Revoke a pending invitation. Anything else (unknown, used, revoked or
/// expired) is 404.
#[post("/invitations/{id}/revoke")]
pub async fn revoke_invitation(
    admin: RequireAdmin,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> WebResult<HttpResponse> {
    // Parsed here, after the admin check, so a bad id answers anonymous
    // users and non-admins like any other admin URL.
    let id = Uuid::parse_str(&path).map_err(|_| CoreError::NotFound)?;
    let revoked = InvitationRepo::revoke(&state.pool, id, Utc::now()).await?;
    tracing::info!(
        admin_id = %admin.0,
        invitation_id = %revoked.id,
        "invitation revoked"
    );
    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/admin/invitations"))
        .finish())
}
