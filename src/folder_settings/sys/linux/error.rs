use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LinuxFolderSettingsError {
    /// Could not detect a supported desktop environment.
    #[error("could not detect a supported desktop environment")]
    UndetectedDesktop,

    /// The `gio` binary is not installed or not on PATH.
    #[error("the `gio` command is not available; GVFS metadata backend requires gio-cli")]
    GioNotFound,

    /// A `gio` subprocess failed.
    #[error("gio command failed for {path}: {key} = {value:?} — {detail}")]
    Gio {
        path: PathBuf,
        key: String,
        value: Option<String>,
        detail: String,
    },

    /// A `gsettings` query failed.
    #[error("gsettings query failed: {0}")]
    Gsettings(String),

    /// An icon operation on a path failed.
    #[error("{1}")]
    IconOperation(PathBuf, String),

    /// A `.directory` file exists but could not be parsed as valid INI.
    #[error("failed to parse .directory file at {0}: {1}")]
    DirectoryFileParse(PathBuf, String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    IconError(#[from] crate::icon::IconError),

    #[error("{0}")]
    Error(String),
}
