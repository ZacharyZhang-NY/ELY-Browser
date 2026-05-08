use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DomainError {
    #[error("{field} cannot be empty")]
    EmptyField { field: &'static str },

    #[error("invalid URL: {value}")]
    InvalidUrl { value: String },

    #[error("invalid command query")]
    InvalidCommand,

    #[error("cannot {action} download while state is {state}")]
    InvalidDownloadTransition { action: &'static str, state: &'static str },

    #[error("download progress {received_bytes} exceeds total {total_bytes}")]
    InvalidDownloadProgress { received_bytes: u64, total_bytes: u64 },
}
