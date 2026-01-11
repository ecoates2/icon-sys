use thiserror::Error;

pub type Result<T> = std::result::Result<T, FolderSettingsError>;

#[derive(Debug, Error)]
pub enum FolderSettingsError {
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    Windows(#[from] crate::folder_settings::sys::windows::WindowsFolderSettingsError),

    #[error(transparent)]
    IconError(#[from] crate::icon::IconError),
}
