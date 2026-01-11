pub mod error;
pub use error::{FolderSettingsError, Result};
use std::path::Path;

use crate::api::IconSet;

/// Provides system folder icon settings operations
pub trait FolderSettingsProvider {
    fn new() -> Self;
    /// Set the icon for a folder
    fn set_icon_for_folder<P: AsRef<Path>>(&self, path: P, icon_set: &IconSet) -> Result<()>;
    /// Reset the icon for a folder
    fn reset_icon_for_folder<P: AsRef<Path>>(&self, path: P) -> Result<()>;
}

/// Provides default system folder icon operations
pub trait DefaultFolderIconProvider {
    /// Dump the default folder icon
    fn dump_default_folder_icon(&self) -> Result<IconSet>;
}

pub mod sys {
    #[cfg(target_os = "windows")]
    pub mod windows;

    #[cfg(target_os = "macos")]
    pub mod macos;

    #[cfg(target_os = "linux")]
    pub mod linux;
}

#[cfg(target_os = "windows")]
pub use sys::windows::{
    WindowsDefaultFolderIconProvider as PlatformDefaultFolderIconProvider,
    WindowsFolderSettingsProvider as PlatformFolderSettingsProvider,
};

#[cfg(target_os = "macos")]
pub use sys::macos::{
    MacOsDefaultFolderIconProvider as PlatformDefaultFolderIconProvider,
    MacOsFolderSettingsProvider as PlatformFolderSettingsProvider,
};

#[cfg(target_os = "linux")]
pub use sys::linux::{
    LinuxDefaultFolderIconProvider as PlatformDefaultFolderIconProvider,
    LinuxFolderSettingsProvider as PlatformFolderSettingsProvider,
};
