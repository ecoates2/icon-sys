#![cfg(target_os = "linux")]

// Integration test suite for Linux

#[test]
fn test_default_folder_icon_provider() {
    use icon_sys::folder_settings::sys::linux::{
        LinuxDefaultFolderIconProvider, LinuxDefaultFolderIconProviderExt,
    };

    let provider = LinuxDefaultFolderIconProvider;
    let result = provider.dump_default_folder_icon_linux();

    // Themes vary by environment, but a folder icon (raster and/or SVG)
    // should be resolvable on any desktop system.
    assert!(
        result.is_ok(),
        "Failed to get default folder icon: {:?}",
        result.err()
    );
    let icon_set = result.unwrap();
    assert!(!icon_set.is_empty(), "Icon set should not be empty");
}

#[test]
fn test_set_folder_icon() {
    use icon_sys::folder_settings::sys::linux::{
        LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
    };
    use icon_sys::icon::sys::linux::{LinuxIconImage, LinuxIconSet};
    use std::borrow::Cow;
    use tempfile::tempdir;

    let img = image::DynamicImage::new_rgba8(256, 256);
    let icon_set = LinuxIconSet::from_icons([LinuxIconImage {
        size: 256,
        image: Cow::Owned(img),
    }])
    .expect("Failed to create LinuxIconSet");

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    // Use the .directory backend so the test is deterministic without gio.
    let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::DirectoryFile, None, false);
    let result = provider.set_icon_for_folder_linux(folder_path, &icon_set);
    assert!(
        result.is_ok(),
        "Failed to set folder icon: {:?}",
        result.err()
    );

    let directory_file = folder_path.join(".directory");
    assert!(directory_file.exists(), ".directory file should be created");
}

#[test]
fn test_reset_folder_icon() {
    use icon_sys::folder_settings::sys::linux::{
        LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
    };
    use icon_sys::icon::sys::linux::{LinuxIconImage, LinuxIconSet};
    use std::borrow::Cow;
    use tempfile::tempdir;

    let img = image::DynamicImage::new_rgba8(256, 256);
    let icon_set = LinuxIconSet::from_icons([LinuxIconImage {
        size: 256,
        image: Cow::Owned(img),
    }])
    .expect("Failed to create LinuxIconSet");

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::DirectoryFile, None, false);
    provider
        .set_icon_for_folder_linux(folder_path, &icon_set)
        .expect("Failed to set folder icon");

    let result = provider.reset_icon_for_folder_linux(folder_path);
    assert!(
        result.is_ok(),
        "Failed to reset folder icon: {:?}",
        result.err()
    );

    // The .directory file held only the generated Icon key, so it should be gone.
    assert!(
        !folder_path.join(".directory").exists(),
        ".directory file should be removed on reset"
    );

    // No generated PNGs should remain.
    let leftover_png = std::fs::read_dir(folder_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("png"));
    assert!(!leftover_png, "Generated PNG should be cleaned up");
}
