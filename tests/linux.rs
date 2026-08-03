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
fn test_set_folder_icon_svg() {
    use icon_sys::folder_settings::sys::linux::{
        LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
    };
    use icon_sys::icon::sys::linux::LinuxIconSet;
    use tempfile::tempdir;

    // Use a minimal SVG; the set path should prefer it over raster.
    let mut icon_set = LinuxIconSet::new();
    icon_set
        .set_svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>")
        .expect("Failed to build SVG icon set");

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::DirectoryFile, None, false);
    provider
        .set_icon_for_folder_linux(folder_path, &icon_set)
        .expect("Failed to set SVG folder icon");

    // An .svg file should be generated, and no .png.
    let entries: Vec<_> = std::fs::read_dir(folder_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    let svg = entries
        .iter()
        .find(|p| p.extension().and_then(|x| x.to_str()) == Some("svg"))
        .expect("a generated .svg file should exist");
    assert!(
        !entries
            .iter()
            .any(|p| p.extension().and_then(|x| x.to_str()) == Some("png")),
        "no .png should be generated when an SVG is present"
    );

    // The .directory Icon key should reference that file by an absolute path.
    let conf = ini::Ini::load_from_file(folder_path.join(".directory")).unwrap();
    let icon = conf
        .section(Some("Desktop Entry"))
        .and_then(|s| s.get("Icon"))
        .expect("Icon key should be set");
    assert!(
        std::path::Path::new(icon).is_absolute(),
        "Icon path should be absolute, got: {icon}"
    );
    assert_eq!(
        std::path::Path::new(icon),
        svg.as_path(),
        "Icon key should point at the generated .svg"
    );
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

    // No generated PNG files should remain.
    let leftover_png = std::fs::read_dir(folder_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("png"));
    assert!(!leftover_png, "Generated PNG should be cleaned up");
}

#[test]
fn test_gio_metadata_backend_returns_error_when_gio_unavailable() {
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

    // GioMetadata backend should fail if gio is not available.
    let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::GioMetadata, None, false);
    let result = provider.set_icon_for_folder_linux(folder_path, &icon_set);

    // If gio is available, the test should pass; otherwise it should
    // fail with a GioNotFound error.
    let has_gio = std::process::Command::new("gio")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_gio {
        assert!(
            result.is_ok(),
            "GioMetadata backend should work when gio is available: {:?}",
            result.err()
        );
        // Clean up
        provider
            .reset_icon_for_folder_linux(folder_path)
            .expect("Failed to reset folder icon");
    } else {
        assert!(
            result.is_err(),
            "GioMetadata backend should fail when gio is not available"
        );
    }
}

#[test]
fn test_remove_generated_icons_does_not_delete_similar_files() {
    use icon_sys::folder_settings::sys::linux::{
        LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
    };
    use icon_sys::icon::sys::linux::LinuxIconSet;
    use tempfile::tempdir;

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    // Create files that look like they could be generated but aren't.
    // Use .png/.svg extensions but with names that don't match the icon-sys prefix.
    std::fs::write(folder_path.join("icon-sys-notes.txt"), "not an icon").unwrap();
    std::fs::write(folder_path.join("my-folder.svg"), "<xml></xml>").unwrap();
    std::fs::write(folder_path.join("background.png"), "not an icon").unwrap();

    // Setting an icon will call remove_generated_icons internally,
    // then create the actual generated icon file.
    let mut icon_set = LinuxIconSet::new();
    icon_set
        .set_svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>")
        .expect("Failed to build SVG icon set");

    let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::DirectoryFile, None, false);
    provider
        .set_icon_for_folder_linux(folder_path, &icon_set)
        .expect("set_icon_for_folder_linux should succeed");

    // These files should still exist — the cleanup should not delete them.
    assert!(
        folder_path.join("icon-sys-notes.txt").exists(),
        "Non-generated .txt file should not be deleted"
    );
    assert!(
        folder_path.join("my-folder.svg").exists(),
        "Non-generated .svg file should not be deleted"
    );
    assert!(
        folder_path.join("background.png").exists(),
        "Non-generated .png file should not be deleted"
    );

    // The actual generated SVG should also exist.
    let entries: Vec<_> = std::fs::read_dir(folder_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()) == Some("svg")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("icon-sys-"))
        })
        .collect();
    assert!(
        !entries.is_empty(),
        "A generated icon-sys-*.svg file should exist"
    );
}

#[test]
fn test_directory_file_parse_error_on_malformed_file() {
    use icon_sys::folder_settings::sys::linux::{
        LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
    };
    use icon_sys::icon::sys::linux::LinuxIconSet;
    use tempfile::tempdir;

    let mut icon_set = LinuxIconSet::new();
    icon_set
        .set_svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>")
        .expect("Failed to build SVG icon set");

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let folder_path = temp_dir.path();

    // Create a malformed .directory file.
    std::fs::write(
        folder_path.join(".directory"),
        b"\x00\x01\x02\x03", // Binary garbage
    )
    .unwrap();

    let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::DirectoryFile, None, false);
    let result = provider.set_icon_for_folder_linux(folder_path, &icon_set);

    // Should fail with a parse error, not silently discard the file.
    assert!(
        result.is_err(),
        "Setting icon on a malformed .directory file should fail"
    );
}

#[test]
fn test_reset_preserves_other_settings_in_directory_file() {
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

    // Create a .directory file with existing settings.
    std::fs::write(
        folder_path.join(".directory"),
        "[Desktop Entry]\n\
         Icon=old-icon\n\
         Name=My Folder\n\
         IconNonDefault=custom.png\n\
         [Custom]\n\
         Key=Value\n",
    )
    .unwrap();

    let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::DirectoryFile, None, false);
    provider
        .set_icon_for_folder_linux(folder_path, &icon_set)
        .expect("Failed to set folder icon");

    // Reset should remove the Icon key but preserve other settings.
    let result = provider.reset_icon_for_folder_linux(folder_path);
    assert!(
        result.is_ok(),
        "Failed to reset folder icon: {:?}",
        result.err()
    );

    // The file should still exist with other settings.
    let directory_file = folder_path.join(".directory");
    assert!(
        directory_file.exists(),
        ".directory file should be preserved"
    );

    let conf = ini::Ini::load_from_file(&directory_file).unwrap();
    let section = conf
        .section(Some("Desktop Entry"))
        .expect("Desktop Entry section should exist");
    assert_eq!(
        section.get("Name"),
        Some("My Folder"),
        "Name key should be preserved"
    );
    assert_eq!(
        section.get("IconNonDefault"),
        Some("custom.png"),
        "IconNonDefault key should be preserved"
    );
    assert!(section.get("Icon").is_none(), "Icon key should be removed");

    // Custom section should also be preserved.
    assert!(
        conf.section(Some("Custom")).is_some(),
        "Custom section should be preserved"
    );
}
