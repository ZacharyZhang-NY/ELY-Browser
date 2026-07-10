use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FavoriteLimit {
    Six,
    #[default]
    Twelve,
    TwentyFour,
}

impl FavoriteLimit {
    pub const ALL: &[Self] = &[Self::Six, Self::Twelve, Self::TwentyFour];

    #[must_use]
    pub fn value(self) -> usize {
        match self {
            Self::Six => 6,
            Self::Twelve => 12,
            Self::TwentyFour => 24,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Six => "6 Favorites",
            Self::Twelve => "12 Favorites",
            Self::TwentyFour => "24 Favorites",
        }
    }

    #[must_use]
    pub fn detail(self) -> &'static str {
        match self {
            Self::Six => "Keep the Favorites shelf compact.",
            Self::Twelve => "Default Favorites shelf capacity.",
            Self::TwentyFour => "Allow a larger cross-space Favorites shelf.",
        }
    }
}
