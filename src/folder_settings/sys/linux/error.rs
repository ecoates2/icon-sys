use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LinuxFolderSettingsError {
    /// The detected/selected backend does not support the requested operation.
    #[error("unsupported desktop backend: {0}")]
    UnsupportedBackend(String),

    /// Could not detect a supported desktop environment.
    #[error("could not detect a supported desktop environment")]
    UndetectedDesktop,

    /// A `gio` subprocess failed.
    #[error("gio command failed: {0}")]
    Gio(String),

    /// An icon operation on a path failed.
    #[error("{1}")]
    IconOperation(PathBuf, String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    IconError(#[from] crate::icon::IconError),

    #[error("{0}")]
    Error(String),
}
