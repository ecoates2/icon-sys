use crate::FolderSettingsProvider;

#[derive(Debug, Clone, Copy, Default)]
pub struct MacOsFolderSettingsProvider;

impl FolderSettingsProvider for MacOsFolderSettingsProvider {
    fn new() -> Self {
        unimplemented!()
    }

    fn set_icon_for_folder<P: AsRef<std::path::Path>>(
        &self,
        _path: P,
        _icon_sett: &crate::IconSet,
    ) -> crate::folder_settings::Result<()> {
        unimplemented!()
    }

    fn reset_icon_for_folder<P: AsRef<std::path::Path>>(
        &self,
        _path: P,
    ) -> crate::folder_settings::Result<()> {
        unimplemented!()
    }
}
