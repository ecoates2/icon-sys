use crate::DefaultFolderIconProvider;

pub struct LinuxDefaultFolderIconProvider;

impl DefaultFolderIconProvider for LinuxDefaultFolderIconProvider {
    fn dump_default_folder_icon(
        &self,
    ) -> Result<crate::api::IconSet, crate::folder_settings::FolderSettingsError> {
        unimplemented!()
    }
}
