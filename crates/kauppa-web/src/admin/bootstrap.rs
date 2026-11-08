//! Host subcommands (MB-0023 P5, RR-0012 sub-answer 3): how the first
//! administrator is set up, from a shell on the host, with no web request
//! and no default credentials.
//!
//! - `kauppa-web admin invite [--note TEXT]` creates an ordinary invitation
//!   (`created_by` NULL) and prints its link once, to stdout.
//! - `kauppa-web admin grant <email>` makes that account an administrator.
//!   Running it again changes nothing.
//!
//! On a fresh install: `admin invite`, register with the link, then
//! `admin grant <that email>`. `main.rs` parses the arguments before it
//! does anything else; unknown ones exit non-zero and never start the
//! server.

use std::io::Write;

use chrono::{DateTime, Utc};

use kauppa_core::CoreResult;
use kauppa_core::repo::{AdminRepo, InvitationRepo, SqlitePool, UserRepo};

use crate::csrf::AppOrigin;

pub const USAGE: &str = "usage: kauppa-web                               start the server
       kauppa-web admin invite [--note TEXT]   create an invitation and print its link
       kauppa-web admin grant EMAIL            make that account an administrator";

/// What `grant` prints, the same whether no account uses the email or it
/// already is an administrator.
pub const GRANT_UNCHANGED: &str =
    "No change: no account uses that email, or it is already an administrator.";
pub const GRANT_DONE: &str = "Done: that account is now an administrator.";

/// A host subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Invite { note: Option<String> },
    Grant { email: String },
}

/// Arguments that are not a known subcommand. Never echoes them: one may be
/// an email.
#[derive(Debug, thiserror::Error)]
#[error("unknown command or arguments\n{USAGE}")]
pub struct UsageError;

/// The subcommand in `args` (without the program name); `None` for no
/// arguments at all, which starts the server.
pub fn parse<I>(args: I) -> Result<Option<Command>, UsageError>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => Ok(None),
        ["admin", "invite"] => Ok(Some(Command::Invite { note: None })),
        ["admin", "invite", "--note", note] if !note.starts_with("--") => {
            Ok(Some(Command::Invite {
                note: Some((*note).to_owned()),
            }))
        }
        ["admin", "grant", email] if !email.trim().is_empty() && !email.starts_with('-') => {
            Ok(Some(Command::Grant {
                email: (*email).to_owned(),
            }))
        }
        _ => Err(UsageError),
    }
}

/// Run `command` against `pool`, writing what the operator needs to `out`.
/// The invitation link is absolute with `public_origin`
/// (`KAUPPA_PUBLIC_ORIGIN`), else just the path.
pub async fn run(
    command: Command,
    pool: &SqlitePool,
    public_origin: Option<&AppOrigin>,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    match command {
        Command::Invite { note } => {
            let link = invite(pool, public_origin, note.as_deref(), Utc::now()).await?;
            writeln!(out, "Invitation link (shown once, valid for one account):")?;
            writeln!(out, "{link}")?;
        }
        Command::Grant { email } => {
            let message = if grant(pool, &email, Utc::now()).await? {
                GRANT_DONE
            } else {
                GRANT_UNCHANGED
            };
            writeln!(out, "{message}")?;
        }
    }
    Ok(())
}

/// Create an invitation from the host and return its link. Logs only the
/// invitation id (P7).
pub async fn invite(
    pool: &SqlitePool,
    public_origin: Option<&AppOrigin>,
    note: Option<&str>,
    now: DateTime<Utc>,
) -> CoreResult<String> {
    let created = InvitationRepo::create(pool, None, note, now).await?;
    tracing::info!(
        invitation_id = %created.invitation.id,
        created_by = "host",
        "invitation created"
    );
    let path = format!("/auth/register?invite={}", created.token.as_str());
    Ok(public_origin.map_or_else(|| path.clone(), |origin| format!("{origin}{path}")))
}

/// Make the account that uses `email` an administrator. `true` if it
/// became one now; `false` if no account matched or it already was one.
/// Logs the user id, never the email (P7).
pub async fn grant(pool: &SqlitePool, email: &str, now: DateTime<Utc>) -> CoreResult<bool> {
    let Some((user, _)) = UserRepo::find_by_email(pool, email).await? else {
        tracing::info!(granted_by = "host", "admin grant: no account matched");
        return Ok(false);
    };
    let granted = AdminRepo::grant(pool, user.id, None, now).await?;
    if granted {
        tracing::info!(user_id = %user.id, granted_by = "host", "admin granted");
    } else {
        tracing::info!(user_id = %user.id, granted_by = "host", "admin grant: already an admin");
    }
    Ok(granted)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::str::FromStr;

    use sqlx::sqlite::SqliteConnectOptions;

    use super::*;
    use crate::routes::test_support::{make_user, test_state_with_url};

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    async fn admin_rows(url: &str) -> Vec<(String, String, Option<String>)> {
        let raw = sqlx::SqlitePool::connect_with(SqliteConnectOptions::from_str(url).unwrap())
            .await
            .unwrap();
        sqlx::query_as("SELECT user_id, granted_at, granted_by FROM admins ORDER BY user_id")
            .fetch_all(&raw)
            .await
            .unwrap()
    }

    /// P5: there is no admin until the host grants one; granting the same
    /// user again changes nothing, and an unknown email prints the same as
    /// an existing admin.
    #[actix_web::test]
    async fn grants_admin_once() {
        let (state, url) = test_state_with_url().await;
        let pool = &state.pool;
        let user = make_user(&state, "owner@example.com").await;
        assert!(admin_rows(&url).await.is_empty(), "no default admin");
        assert!(!AdminRepo::is_admin(pool, user).await.unwrap());

        let run_grant = |email: &'static str| async move {
            let mut out = Vec::new();
            let command = parse(args(&format!("admin grant {email}")))
                .unwrap()
                .unwrap();
            run(command, pool, None, &mut out).await.unwrap();
            String::from_utf8(out).unwrap()
        };

        assert_eq!(
            run_grant("owner@example.com").await,
            format!("{GRANT_DONE}\n")
        );
        assert!(AdminRepo::is_admin(pool, user).await.unwrap());
        let first = admin_rows(&url).await;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].0, user.to_string());
        assert_eq!(first[0].2, None, "granted from the host");

        let again = run_grant("owner@example.com").await;
        assert_eq!(again, format!("{GRANT_UNCHANGED}\n"));
        assert_eq!(admin_rows(&url).await, first, "nothing changed");

        let unknown = run_grant("nobody@example.com").await;
        assert_eq!(unknown, again, "same message for an unknown email");
        assert_eq!(admin_rows(&url).await, first);
    }

    /// `admin invite` prints a link once; it registers like any other
    /// invitation, created by the host.
    #[actix_web::test]
    async fn invite_prints_a_link() {
        let (state, _) = test_state_with_url().await;
        let pool = &state.pool;
        let mut out = Vec::new();
        let command = parse(args("admin invite --note Mum")).unwrap().unwrap();
        run(command, pool, None, &mut out).await.unwrap();
        let out = String::from_utf8(out).unwrap();
        let link = out.lines().nth(1).expect("link line");
        let token = link
            .strip_prefix("/auth/register?invite=")
            .expect("path without an origin");
        assert!(kauppa_core::token::is_well_formed(token), "{out}");
        let listed = InvitationRepo::list(pool).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].created_by, None);
        assert_eq!(listed[0].note.as_deref(), Some("Mum"));

        let origin = AppOrigin::parse("https://kauppa.example").unwrap();
        let link = invite(pool, Some(&origin), None, Utc::now()).await.unwrap();
        assert!(
            link.starts_with("https://kauppa.example/auth/register?invite="),
            "{link}"
        );
    }

    /// No arguments start the server; anything unknown is an error, never
    /// the server.
    #[test]
    fn parse_arguments() {
        assert_eq!(parse(args("")).unwrap(), None);
        assert_eq!(
            parse(args("admin invite")).unwrap(),
            Some(Command::Invite { note: None })
        );
        assert_eq!(
            parse(vec![
                "admin".to_owned(),
                "invite".to_owned(),
                "--note".to_owned(),
                "for Mum".to_owned()
            ])
            .unwrap(),
            Some(Command::Invite {
                note: Some("for Mum".to_owned())
            })
        );
        assert_eq!(
            parse(args("admin grant pat@example.com")).unwrap(),
            Some(Command::Grant {
                email: "pat@example.com".to_owned()
            })
        );
        for bad in [
            "admin",
            "admin grant",
            "admin grant a@x.fi b@x.fi",
            "admin grant --help",
            "admin invite --note",
            "admin invite --note --x",
            "admin invite extra",
            "admin revoke x",
            "serve",
            "--help",
            "grant a@x.fi",
        ] {
            assert!(parse(args(bad)).is_err(), "{bad:?}");
        }
        let message = parse(args("admin grant a@x.fi b@x.fi"))
            .unwrap_err()
            .to_string();
        assert!(!message.contains("a@x.fi"), "never echoes: {message}");
        assert!(message.contains("usage:"), "{message}");
    }
}
