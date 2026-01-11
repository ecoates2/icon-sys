mod folder_settings_provider;
pub use folder_settings_provider::MacOsFolderSettingsProvider;
mod default_folder_icon_provider;
pub use default_folder_icon_provider::MacOsDefaultFolderIconProvider;

pub mod error;
pub use error::MacOsFolderSettingsError;
