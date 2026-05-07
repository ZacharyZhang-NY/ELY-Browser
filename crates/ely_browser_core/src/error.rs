use ely_domain::{DomainError, TabId};
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CoreError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("tab not found: {id}")]
    TabNotFound { id: TabId },

    #[error("browser state has no active tab")]
    MissingActiveTab,
}
