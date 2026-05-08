use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DomainError {
    #[error("{field} cannot be empty")]
    EmptyField { field: &'static str },

    #[error("invalid URL: {value}")]
    InvalidUrl { value: String },

    #[error("invalid file name: {value}")]
    InvalidFileName { value: String },

    #[error("invalid {algorithm} download checksum: {value}")]
    InvalidDownloadChecksum { algorithm: &'static str, value: String },

    #[error("invalid download directory: {path}")]
    InvalidDownloadDirectory { path: String },

    #[error("invalid command query")]
    InvalidCommand,

    #[error("cannot {action} download while state is {state}")]
    InvalidDownloadTransition { action: &'static str, state: &'static str },

    #[error("download progress {received_bytes} exceeds total {total_bytes}")]
    InvalidDownloadProgress { received_bytes: u64, total_bytes: u64 },

    #[error("invalid plugin manifest: {reason}")]
    InvalidPluginManifest { reason: String },

    #[error("invalid plugin id: {value}")]
    InvalidPluginId { value: String },

    #[error("invalid plugin homepage: {value}")]
    InvalidPluginHomepage { value: String },

    #[error("invalid plugin permission: {value}")]
    InvalidPluginPermission { value: String },

    #[error("duplicate plugin permission: {value}")]
    DuplicatePluginPermission { value: String },

    #[error("invalid plugin contribution: {value}")]
    InvalidPluginContribution { value: String },

    #[error("duplicate plugin contribution: {value}")]
    DuplicatePluginContribution { value: String },

    #[error("invalid plugin build requirement: {value}")]
    InvalidPluginBuildRequirement { value: String },

    #[error("invalid plugin checksum: {value}")]
    InvalidPluginChecksum { value: String },

    #[error("invalid plugin signature algorithm: {value}")]
    InvalidPluginSignatureAlgorithm { value: String },

    #[error("invalid plugin signature: {value}")]
    InvalidPluginSignature { value: String },
}
