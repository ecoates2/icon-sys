mod folder_settings_provider;
pub use folder_settings_provider::{
    LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
};
mod default_folder_icon_provider;
pub use default_folder_icon_provider::{
    LinuxDefaultFolderIconProvider, LinuxDefaultFolderIconProviderExt,
};

pub mod error;
pub use error::LinuxFolderSettingsError;
