use thiserror::Error;

#[derive(Debug, Error)]
pub enum MacOsFolderSettingsError {
    #[error("{0}")]
    Error(String),
}
