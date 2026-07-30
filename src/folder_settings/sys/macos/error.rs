use thiserror::Error;

#[derive(Debug, Error)]
pub enum MacOsFolderSettingsError {
    /// macOS support has not been implemented yet.
    #[error("macOS support is not yet implemented")]
    NotImplemented,

    #[error("{0}")]
    Error(String),
}
