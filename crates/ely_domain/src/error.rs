use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DomainError {
    #[error("{field} cannot be empty")]
    EmptyField { field: &'static str },

    #[error("invalid URL: {value}")]
    InvalidUrl { value: String },

    #[error("invalid command query")]
    InvalidCommand,
}
