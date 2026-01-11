#![cfg(windows)]

// Integration test suite for Windows

#[test]
fn test_default_folder_icon_provider() {
    use icon_sys::folder_settings::sys::windows::{
        WindowsDefaultFolderIconProvider, WindowsDefaultFolderIconProviderExt,
    };

    let provider = WindowsDefaultFolderIconProvider;
    let result = provider.dump_default_folder_icon_windows();

    assert!(
        result.is_ok(),
        "Failed to get default folder icon: {:?}",
        result.err()
    );
    let icon_set = result.unwrap();

    assert!(icon_set.is_complete(), "Icon set should contain all sizes");
}

#[test]
fn test_set_folder_icon() {
    use icon_sys::folder_settings::sys::windows::{
        WindowsFolderSettingsProvider, WindowsFolderSettingsProviderExt,
    };
    use icon_sys::icon::sys::windows::{WindowsIconImage, WindowsIconSet, WindowsIconSize};
    use std::borrow::Cow;
    use tempfile::tempdir;

    let mut icons = Vec::new();
    for size in WindowsIconSize::all() {
        let dim = size.dimension();
        let img = image::DynamicImage::new_rgba8(dim, dim);
        icons.push(WindowsIconImage {
            size,
            image: Cow::Owned(img),
        });
    }

    let icon_set = WindowsIconSet::from_icons(icons).expect("Failed to create WindowsIconSet");

    // Create a temporary directory to act as the folder
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    let folder_settings_provider = WindowsFolderSettingsProvider::new_windows(true, None);
    let result = folder_settings_provider.set_icon_for_folder_windows(&folder_path, &icon_set);
    assert!(
        result.is_ok(),
        "Failed to set folder icon: {:?}",
        result.err()
    );
}

#[test]
fn test_reset_folder_icon() {
    use icon_sys::folder_settings::sys::windows::{
        WindowsFolderSettingsProvider, WindowsFolderSettingsProviderExt,
    };
    use tempfile::tempdir;

    // Create a temporary directory to act as the folder
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    let folder_settings_provider = WindowsFolderSettingsProvider::new_windows(true, None);
    let result = folder_settings_provider.reset_icon_for_folder_windows(&folder_path);
    assert!(
        result.is_ok(),
        "Failed to reset folder icon: {:?}",
        result.err()
    );
}

// TODO: More assertions; test reset function on a folder with an icon and make sure files get cleaned up properly.
