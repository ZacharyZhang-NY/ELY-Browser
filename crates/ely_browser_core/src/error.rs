use ely_domain::{DomainError, ProfileId, SpaceId, TabId};
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CoreError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("tab not found: {id}")]
    TabNotFound { id: TabId },

    #[error("space not found: {id}")]
    SpaceNotFound { id: SpaceId },

    #[error("profile not found: {id}")]
    ProfileNotFound { id: ProfileId },

    #[error("favorite limit reached: {limit}")]
    FavoriteLimitReached { limit: usize },

    #[error("browser state has no archived tabs")]
    NoArchivedTabs,

    #[error("browser state has no active tab")]
    MissingActiveTab,
}
