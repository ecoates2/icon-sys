use thiserror::Error;

#[derive(Debug, Error)]
pub enum LinuxFolderSettingsError {
    #[error("{0}")]
    Error(String),
}
