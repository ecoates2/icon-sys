use crate::FolderSettingsProvider;

use super::MacOsFolderSettingsError;

#[derive(Debug, Clone, Copy, Default)]
pub struct MacOsFolderSettingsProvider;

impl FolderSettingsProvider for MacOsFolderSettingsProvider {
    fn new() -> Self {
        Self
    }

    fn set_icon_for_folder<P: AsRef<std::path::Path>>(
        &self,
        _path: P,
        _icon_set: &crate::IconSet,
    ) -> crate::folder_settings::Result<()> {
        Err(MacOsFolderSettingsError::NotImplemented.into())
    }

    fn reset_icon_for_folder<P: AsRef<std::path::Path>>(
        &self,
        _path: P,
    ) -> crate::folder_settings::Result<()> {
        Err(MacOsFolderSettingsError::NotImplemented.into())
    }
}
