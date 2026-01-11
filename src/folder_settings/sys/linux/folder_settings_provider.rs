use crate::FolderSettingsProvider;

pub struct LinuxFolderSettingsProvider;

impl FolderSettingsProvider for LinuxFolderSettingsProvider {
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
