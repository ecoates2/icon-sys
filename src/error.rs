use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    IconError(#[from] crate::icon::IconError),

    #[cfg(feature = "folder-settings")]
    #[error(transparent)]
    FolderSettings(#[from] crate::folder_settings::error::FolderSettingsError),
}
