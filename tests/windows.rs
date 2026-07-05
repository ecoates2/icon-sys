#![cfg(windows)]

//! Integration test suite for Windows.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use icon_sys::folder_settings::sys::windows::{
    WindowsDefaultFolderIconProvider, WindowsDefaultFolderIconProviderExt,
    WindowsFolderSettingsProvider, WindowsFolderSettingsProviderExt,
};
use icon_sys::icon::sys::windows::{WindowsIconImage, WindowsIconSet, WindowsIconSize};
use tempfile::tempdir;

/// Build a complete Windows icon set of blank images.
fn blank_icon_set() -> WindowsIconSet<'static> {
    let icons = WindowsIconSize::all().map(|size| {
        let dim = size.dimension();
        WindowsIconImage {
            size,
            image: Cow::Owned(image::DynamicImage::new_rgba8(dim, dim)),
        }
    });
    WindowsIconSet::from_icons(icons).expect("Failed to create WindowsIconSet")
}

/// Collect the generated `.ico` files in `folder` whose names start with `prefix`.
fn generated_icos(folder: &Path, prefix: &str) -> Vec<PathBuf> {
    std::fs::read_dir(folder)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()) == Some("ico")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(prefix))
        })
        .collect()
}

#[test]
fn test_default_folder_icon_provider() {
    let provider = WindowsDefaultFolderIconProvider;
    let icon_set = provider
        .dump_default_folder_icon_windows()
        .expect("Failed to get default folder icon");

    assert!(icon_set.is_complete(), "Icon set should contain all sizes");
}

#[test]
fn test_set_folder_icon() {
    let temp_dir = tempdir().expect("Failed to create temp dir");

    let provider = WindowsFolderSettingsProvider::new_windows(true, None);
    provider
        .set_icon_for_folder_windows(temp_dir.path(), &blank_icon_set())
        .expect("Failed to set folder icon");
}

#[test]
fn test_reset_folder_icon() {
    let temp_dir = tempdir().expect("Failed to create temp dir");

    let provider = WindowsFolderSettingsProvider::new_windows(true, None);
    provider
        .reset_icon_for_folder_windows(temp_dir.path())
        .expect("Failed to reset folder icon");
}

#[test]
fn test_set_icon_rejects_nonexistent_path() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let missing = temp_dir.path().join("does-not-exist");

    let provider = WindowsFolderSettingsProvider::new_windows(false, None);
    assert!(
        provider
            .set_icon_for_folder_windows(&missing, &blank_icon_set())
            .is_err(),
        "Setting an icon on a nonexistent path should fail"
    );
}

#[test]
fn test_set_icon_rejects_file_path() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let file = temp_dir.path().join("a_file.txt");
    std::fs::write(&file, b"not a directory").expect("Failed to write file");

    let provider = WindowsFolderSettingsProvider::new_windows(false, None);
    assert!(
        provider
            .set_icon_for_folder_windows(&file, &blank_icon_set())
            .is_err(),
        "Setting an icon on a file (not a directory) should fail"
    );
}

#[test]
fn test_set_then_reset_cleans_up_generated_icon() {
    const PREFIX: &str = "test-prefix";

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    let provider = WindowsFolderSettingsProvider::new_windows(true, Some(PREFIX));

    // Setting an icon should write exactly one generated .ico matching the prefix.
    provider
        .set_icon_for_folder_windows(folder_path, &blank_icon_set())
        .expect("Failed to set folder icon");
    assert_eq!(
        generated_icos(folder_path, PREFIX).len(),
        1,
        "Exactly one generated .ico should exist after set"
    );

    // Resetting should remove the generated .ico again.
    provider
        .reset_icon_for_folder_windows(folder_path)
        .expect("Failed to reset folder icon");
    let leftover = generated_icos(folder_path, PREFIX);
    assert!(
        leftover.is_empty(),
        "Generated .ico should be cleaned up after reset, found: {leftover:?}"
    );
}
