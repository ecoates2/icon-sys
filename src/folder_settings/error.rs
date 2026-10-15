use thiserror::Error;

pub type Result<T> = std::result::Result<T, FolderSettingsError>;

#[derive(Debug, Error)]
pub enum FolderSettingsError {
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    Windows(#[from] crate::folder_settings::sys::windows::WindowsFolderSettingsError),

    #[cfg(target_os = "linux")]
    #[error(transparent)]
    Linux(#[from] crate::folder_settings::sys::linux::LinuxFolderSettingsError),

    #[cfg(target_os = "macos")]
    #[error(transparent)]
    MacOs(#[from] crate::folder_settings::sys::macos::MacOsFolderSettingsError),

    #[error(transparent)]
    IconError(#[from] crate::icon::IconError),
}
