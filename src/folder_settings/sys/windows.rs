mod folder_settings_provider;
pub use folder_settings_provider::{
    WindowsFolderSettingsProvider, WindowsFolderSettingsProviderExt,
};
mod default_folder_icon_provider;
pub use default_folder_icon_provider::{
    WindowsDefaultFolderIconProvider, WindowsDefaultFolderIconProviderExt,
};

pub mod error;
pub use error::WindowsFolderSettingsError;
