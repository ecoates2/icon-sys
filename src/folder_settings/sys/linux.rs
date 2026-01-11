mod folder_settings_provider;
pub use folder_settings_provider::LinuxFolderSettingsProvider;
mod default_folder_icon_provider;
pub use default_folder_icon_provider::LinuxDefaultFolderIconProvider;

pub mod error;
pub use error::LinuxFolderSettingsError;
