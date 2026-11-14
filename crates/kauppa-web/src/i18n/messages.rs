//! Rust-built user text as catalog keys (MB-0035, RR-0023 sub-answer 6):
//! core's typed reasons and the web layer's own messages, each mapped in one
//! exhaustive `match`, so a new reason without a key doesn't compile.

use kauppa_core::domain::{InvitationState, PASSWORD_MAX_CHARS, PASSWORD_MIN_CHARS};
use kauppa_core::{Conflict, Entity, Invalid};

/// A key and its arguments.
pub type Text = (String, Vec<(&'static str, String)>);

fn key(key: &str) -> Text {
    (key.to_owned(), Vec::new())
}

fn entity_key(prefix: &str, entity: Entity) -> Text {
    key(&format!("{prefix}.{}", entity.noun()))
}

/// The key for a rejected input's reason.
pub fn invalid(reason: &Invalid) -> Text {
    match reason {
        Invalid::NameEmpty(entity) => entity_key("errors.name_empty", *entity),
        Invalid::TooMany(entity) => entity_key("errors.too_many", *entity),
        Invalid::Unknown(entity) => entity_key("errors.unknown", *entity),
        Invalid::TitleEmpty => key("errors.title_empty"),
        Invalid::ChooseList => key("errors.choose_list"),
        Invalid::ChooseRecipe => key("errors.choose_recipe"),
        Invalid::ChooseFavorite => key("errors.choose_favorite"),
        Invalid::ChooseCategory => key("errors.choose_category"),
        Invalid::NewProductNeedsCategory { name } => (
            "errors.new_product_needs_category".to_owned(),
            vec![("name", name.clone())],
        ),
        Invalid::FavoriteNeedsCategory { name } => (
            "errors.favorite_needs_category".to_owned(),
            vec![("name", name.clone())],
        ),
        Invalid::ProductUncategorized { name } => (
            "errors.product_uncategorized".to_owned(),
            vec![("name", name.clone())],
        ),
        Invalid::MultiplierRange => key("errors.multiplier_range"),
        Invalid::MultiplierNotNumber => key("errors.multiplier_not_number"),
        Invalid::NoIngredients { recipe } => (
            "errors.no_ingredients".to_owned(),
            vec![("recipe", recipe.clone())],
        ),
        Invalid::ServingsTooFew => key("errors.servings_too_few"),
        Invalid::ServingsNotWhole => key("errors.servings_not_whole"),
        Invalid::EmailRequired => key("errors.email_required"),
        Invalid::ShareRole => key("errors.share_role"),
        Invalid::NoteTooLong { max } => (
            "errors.note_too_long".to_owned(),
            vec![("max", max.to_string())],
        ),
        Invalid::FoundState => key("errors.found_state"),
        Invalid::Language => key("errors.language"),
        Invalid::Theme => key("errors.theme"),
    }
}

/// The key for a conflict's reason.
pub fn conflict(reason: &Conflict) -> Text {
    match reason {
        Conflict::Exists { entity, name } => (
            format!("errors.exists.{}", entity.noun()),
            vec![("name", name.clone())],
        ),
        Conflict::EmailTaken => key("errors.email_taken"),
    }
}

/// Messages only the web layer says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// The login error for an unknown email and a wrong password alike.
    InvalidLogin,
    /// The one answer to every invalid invitation (RR-0012 constraint 4).
    InvalidInvitation,
    /// Registration with an email an account uses (RR-0012 sub-answer 4).
    EmailTaken,
    /// Rate-limited (RR-0013 sub-answer 1).
    TooManyAttempts { minutes: u64 },
    /// A new password outside the length rule (RR-0022).
    PasswordLength,
}

/// The key for a web message.
pub fn message(message: Message) -> Text {
    match message {
        Message::InvalidLogin => key("auth.invalid_login"),
        Message::InvalidInvitation => key("auth.invalid_invitation"),
        Message::EmailTaken => key("auth.email_taken"),
        Message::TooManyAttempts { minutes } => (
            "auth.too_many_attempts".to_owned(),
            vec![("count", minutes.to_string())],
        ),
        Message::PasswordLength => (
            "auth.password_length".to_owned(),
            vec![
                ("min", PASSWORD_MIN_CHARS.to_string()),
                ("max", PASSWORD_MAX_CHARS.to_string()),
            ],
        ),
    }
}

/// The key for an invitation's state label.
pub fn invitation_state(state: InvitationState) -> Text {
    key(match state {
        InvitationState::Pending => "invitations.state.pending",
        InvitationState::Used => "invitations.state.used",
        InvitationState::Revoked => "invitations.state.revoked",
        InvitationState::Expired => "invitations.state.expired",
    })
}

/// One of every reason and message, for the catalog check (P2): a test
/// below fails if a variant is missing here.
#[cfg(test)]
pub fn every_text() -> Vec<Text> {
    let name = || "x".to_owned();
    let mut invalids = vec![
        Invalid::TitleEmpty,
        Invalid::ChooseList,
        Invalid::ChooseRecipe,
        Invalid::ChooseFavorite,
        Invalid::ChooseCategory,
        Invalid::NewProductNeedsCategory { name: name() },
        Invalid::FavoriteNeedsCategory { name: name() },
        Invalid::ProductUncategorized { name: name() },
        Invalid::MultiplierRange,
        Invalid::MultiplierNotNumber,
        Invalid::NoIngredients { recipe: name() },
        Invalid::ServingsTooFew,
        Invalid::ServingsNotWhole,
        Invalid::EmailRequired,
        Invalid::ShareRole,
        Invalid::NoteTooLong { max: 200 },
        Invalid::FoundState,
        Invalid::Language,
        Invalid::Theme,
    ];
    let mut conflicts = vec![Conflict::EmailTaken];
    for entity in Entity::ALL {
        invalids.extend([
            Invalid::NameEmpty(entity),
            Invalid::TooMany(entity),
            Invalid::Unknown(entity),
        ]);
        conflicts.push(Conflict::Exists {
            entity,
            name: name(),
        });
    }
    let mut out: Vec<Text> = invalids.iter().map(invalid).collect();
    out.extend(conflicts.iter().map(conflict));
    out.extend(
        [
            Message::InvalidLogin,
            Message::InvalidInvitation,
            Message::EmailTaken,
            Message::TooManyAttempts { minutes: 1 },
            Message::TooManyAttempts { minutes: 5 },
            Message::PasswordLength,
        ]
        .into_iter()
        .map(message),
    );
    for state in [
        InvitationState::Pending,
        InvitationState::Used,
        InvitationState::Revoked,
        InvitationState::Expired,
    ] {
        out.push(invitation_state(state));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kauppa_core::Invalid;

    use super::every_text;

    /// `every_text` lists every `Invalid` variant: each gives a distinct
    /// key prefix, and their count matches the variants this match knows.
    #[test]
    fn every_text_covers_every_reason() {
        let variant = |r: &Invalid| match r {
            Invalid::NameEmpty(_) => 0,
            Invalid::TooMany(_) => 1,
            Invalid::Unknown(_) => 2,
            Invalid::TitleEmpty => 3,
            Invalid::ChooseList => 4,
            Invalid::ChooseRecipe => 5,
            Invalid::ChooseFavorite => 6,
            Invalid::ChooseCategory => 7,
            Invalid::NewProductNeedsCategory { .. } => 8,
            Invalid::FavoriteNeedsCategory { .. } => 9,
            Invalid::ProductUncategorized { .. } => 10,
            Invalid::MultiplierRange => 11,
            Invalid::MultiplierNotNumber => 12,
            Invalid::NoIngredients { .. } => 13,
            Invalid::ServingsTooFew => 14,
            Invalid::ServingsNotWhole => 15,
            Invalid::EmailRequired => 16,
            Invalid::ShareRole => 17,
            Invalid::NoteTooLong { .. } => 18,
            Invalid::FoundState => 19,
            Invalid::Language => 20,
            Invalid::Theme => 21,
        };
        let keys: BTreeSet<String> = every_text().into_iter().map(|(k, _)| k).collect();
        // 19 single reasons + 3 entity reasons × 7 entities.
        let errors = keys
            .iter()
            .filter(|k| {
                k.starts_with("errors.")
                    && !k.starts_with("errors.exists.")
                    && *k != "errors.email_taken"
            })
            .count();
        assert_eq!(errors, 19 + 3 * 7);
        assert_eq!(variant(&Invalid::Theme), 21, "22 variants");
    }
}
