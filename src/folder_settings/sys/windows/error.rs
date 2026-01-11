use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowsFolderSettingsError {
    #[error("Win32 error: {0}")]
    Win32(#[from] windows::core::Error),

    #[error("Error loading resource: {0}")]
    ProviderError(String),

    #[error("{1}")]
    IconOperation(PathBuf, String),

    #[error("{0}")]
    Error(String),

    #[error(transparent)]
    IconError(#[from] crate::icon::IconError),
}
