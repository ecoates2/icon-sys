use crate::DefaultFolderIconProvider;

pub struct MacOsDefaultFolderIconProvider;

impl DefaultFolderIconProvider for MacOsDefaultFolderIconProvider {
    fn dump_default_folder_icon(
        &self,
    ) -> Result<crate::api::IconSet, crate::folder_settings::FolderSettingsError> {
        unimplemented!()
    }
}
