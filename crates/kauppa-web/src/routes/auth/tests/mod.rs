//! Tests for `routes::auth`, by area (RR-0009). Fixtures shared by the
//! areas live here.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod register_requires_invite;

use actix_web::test::TestRequest;
use chrono::{DateTime, Utc};

use kauppa_core::repo::InvitationRepo;

use crate::routes::test_support::PASSWORD;
use crate::state::AppState;

/// The one answer to every invalid invitation (RR-0012 sub-answer 7).
pub const INVALID_INVITATION: &str = "This invitation link isn&#x27;t valid. Ask for a new one.";

/// A new invitation created at `now` by the host command; returns its
/// plaintext token.
pub async fn invitation_at(state: &AppState, now: DateTime<Utc>) -> (uuid::Uuid, String) {
    let created = InvitationRepo::create(&state.pool, None, None, now)
        .await
        .expect("create invitation");
    (created.invitation.id, created.token.as_str().to_owned())
}

/// A pending invitation's plaintext token.
pub async fn invitation(state: &AppState) -> String {
    invitation_at(state, Utc::now()).await.1
}

/// The registration form as the register page posts it, with `invite` as
/// its hidden field (`None`: no such field at all).
pub fn register_request(invite: Option<&str>, email: &str) -> TestRequest {
    let mut form = vec![
        ("display_name", "Pat"),
        ("email", email),
        ("password", PASSWORD),
    ];
    if let Some(invite) = invite {
        form.push(("invite", invite));
    }
    TestRequest::post().uri("/auth/register").set_form(form)
}

/// The value of the hidden `invite` field in a rendered register page.
pub fn hidden_invite(page: &str) -> Option<String> {
    let field = page
        .split("<input")
        .find(|input| input.contains("type=\"hidden\"") && input.contains("name=\"invite\""))?;
    Some(
        field
            .split("value=\"")
            .nth(1)?
            .split('"')
            .next()?
            .to_owned(),
    )
}
