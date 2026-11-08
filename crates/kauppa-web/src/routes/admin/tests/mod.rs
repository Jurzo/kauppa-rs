//! Tests for `routes::admin` (MB-0023), by area (RR-0009). Fixtures shared
//! by the areas live here.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod access;
mod invites;

use actix_web::cookie::Cookie;
use actix_web::test::TestRequest;
use chrono::Utc;
use uuid::Uuid;

use kauppa_core::repo::AdminRepo;

use crate::routes::test_support::make_user;
use crate::state::AppState;

pub const ADMIN: &str = "admin@example.com";
pub const MEMBER: &str = "member@example.com";

/// A user who is an admin, granted from the host.
pub async fn make_admin(state: &AppState, email: &str) -> Uuid {
    let id = make_user(state, email).await;
    assert!(
        AdminRepo::grant(&state.pool, id, None, Utc::now())
            .await
            .expect("grant")
    );
    id
}

/// Every `/admin` route, as a request a signed-in browser would send.
pub fn admin_requests(invitation: Uuid, session: Option<&Cookie<'static>>) -> Vec<TestRequest> {
    let mut reqs = vec![
        TestRequest::get().uri("/admin"),
        TestRequest::get().uri("/admin/invitations"),
        TestRequest::post()
            .uri("/admin/invitations")
            .set_form([("note", "Hijacked")]),
        TestRequest::post().uri(&format!("/admin/invitations/{invitation}/revoke")),
        TestRequest::post().uri("/admin/invitations/not-a-uuid/revoke"),
    ];
    if let Some(session) = session {
        reqs = reqs
            .into_iter()
            .map(|r| r.cookie(session.clone()))
            .collect();
    }
    reqs
}

/// The invitation link on a page that shows one, as a browser reads the
/// attribute (handlebars escapes `=`), and its token.
pub fn invitation_link(page: &str) -> Option<(String, String)> {
    let start = page.find("value=\"http")?;
    let link = page[start + 7..].split('"').next()?.replace("&#x3D;", "=");
    let token = link.split("?invite=").nth(1)?.to_owned();
    Some((link, token))
}
