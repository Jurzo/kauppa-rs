use std::fmt;

use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(Conflict),

    #[error("invalid input: {0}")]
    Invalid(Invalid),

    #[error("unauthorized")]
    Unauthorized,

    /// Signed in and allowed to see the resource, but not to do this to it
    /// (e.g. a `view` grantee adding an item to a shared list, MB-0008).
    #[error("forbidden")]
    Forbidden,

    #[error("database error")]
    Database(#[from] sqlx::Error),

    /// The database can't be opened or migrated, or holds a value the app
    /// never writes (MB-0035: these were `Invalid` before, but they are
    /// never the user's input).
    #[error("storage error: {0}")]
    Storage(String),

    #[error("password hashing error")]
    PasswordHash(String),

    /// The OS random number generator failed (invitation tokens).
    #[error("random number generator error")]
    Random(String),
}

/// What a rejected name or reference belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Category,
    Favorite,
    Ingredient,
    Item,
    List,
    Product,
    Recipe,
}

impl Entity {
    /// Every entity.
    pub const ALL: [Self; 7] = [
        Self::Category,
        Self::Favorite,
        Self::Ingredient,
        Self::Item,
        Self::List,
        Self::Product,
        Self::Recipe,
    ];

    /// The noun, singular.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::Favorite => "favorite",
            Self::Ingredient => "ingredient",
            Self::Item => "item",
            Self::List => "list",
            Self::Product => "product",
            Self::Recipe => "recipe",
        }
    }

    /// The noun, plural.
    #[must_use]
    pub const fn plural(self) -> &'static str {
        match self {
            Self::Category => "categories",
            Self::Favorite => "favorites",
            Self::Ingredient => "ingredients",
            Self::Item => "items",
            Self::List => "lists",
            Self::Product => "products",
            Self::Recipe => "recipes",
        }
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.noun())
    }
}

/// Why input was rejected (MB-0035, RR-0023 sub-answer 6).
///
/// Core stays language-free: `Display` is the English the app showed before
/// (logs and plain error bodies keep it), and `kauppa-web` maps each reason
/// to a translation key. Add a reason here rather than a free-text variant
/// (RR-0023 constraint 4).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Invalid {
    #[error("{0} name must not be empty")]
    NameEmpty(Entity),
    #[error("recipe title must not be empty")]
    TitleEmpty,
    #[error("too many {}", .0.plural())]
    TooMany(Entity),
    #[error("unknown {0}")]
    Unknown(Entity),
    #[error("choose a list to add to")]
    ChooseList,
    #[error("choose a recipe to add")]
    ChooseRecipe,
    #[error("choose a favorite to add")]
    ChooseFavorite,
    #[error("choose a category for the product")]
    ChooseCategory,
    #[error("choose a category for the new product {name}")]
    NewProductNeedsCategory { name: String },
    #[error("choose a category for {name} before making it a favorite")]
    FavoriteNeedsCategory { name: String },
    #[error("{name} has no category — choose one for it on Favorites first")]
    ProductUncategorized { name: String },
    #[error("the multiplier must be more than 0 and at most 100")]
    MultiplierRange,
    #[error("the multiplier must be a number, e.g. 2 or 1,5")]
    MultiplierNotNumber,
    #[error("{recipe} has no ingredients")]
    NoIngredients { recipe: String },
    #[error("servings must be at least 1")]
    ServingsTooFew,
    #[error("servings must be a whole number")]
    ServingsNotWhole,
    #[error("enter an email address")]
    EmailRequired,
    #[error("choose view or edit")]
    ShareRole,
    #[error("a note can be at most {max} characters")]
    NoteTooLong { max: usize },
    #[error("unknown found state")]
    FoundState,
    #[error("unsupported language")]
    Language,
    /// A theme other than System, Light or Dark (MB-0055, RR-0035).
    #[error("unsupported theme")]
    Theme,
}

/// Why a write conflicted with existing data (MB-0035); `Display` as
/// [`Invalid`]'s.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Conflict {
    #[error("{entity} already exists: {name}")]
    Exists { entity: Entity, name: String },
    /// Never names the email (RR-0012 sub-answer 4).
    #[error("email already registered")]
    EmailTaken,
}

impl From<Invalid> for CoreError {
    fn from(reason: Invalid) -> Self {
        Self::Invalid(reason)
    }
}

impl From<Conflict> for CoreError {
    fn from(reason: Conflict) -> Self {
        Self::Conflict(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::{Conflict, CoreError, Entity, Invalid};

    /// The English the app showed before MB-0035, byte for byte.
    #[test]
    fn display_unchanged() {
        for (err, text) in [
            (
                Invalid::NameEmpty(Entity::Category).into(),
                "invalid input: category name must not be empty",
            ),
            (
                Invalid::TooMany(Entity::Ingredient).into(),
                "invalid input: too many ingredients",
            ),
            (
                Invalid::Unknown(Entity::Product).into(),
                "invalid input: unknown product",
            ),
            (
                Invalid::NoIngredients {
                    recipe: "Puuro".to_owned(),
                }
                .into(),
                "invalid input: Puuro has no ingredients",
            ),
            (
                Invalid::NoteTooLong { max: 200 }.into(),
                "invalid input: a note can be at most 200 characters",
            ),
            (
                Conflict::Exists {
                    entity: Entity::Category,
                    name: "Hevi".to_owned(),
                }
                .into(),
                "conflict: category already exists: Hevi",
            ),
            (
                Conflict::EmailTaken.into(),
                "conflict: email already registered",
            ),
        ] {
            let err: CoreError = err;
            assert_eq!(err.to_string(), text);
        }
    }
}
