use std::{
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

use super::WindowsFolderSettingsError;
use crate::folder_settings::error::Result;
use crate::{
    folder_settings::FolderSettingsProvider,
    icon::sys::windows::{WindowsIconSet, WindowsIconSize},
};

use image::codecs::ico::{IcoEncoder, IcoFrame};
use uuid::Uuid;
use windows::Win32::{
    Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
    System::Com::{CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoInitializeEx},
    UI::Shell::{
        FCS_FORCEWRITE, FCSM_ICONFILE, FFFP_EXACTMATCH, IKnownFolderManager, KnownFolderManager,
        SHFOLDERCUSTOMSETTINGS, SHGetSetFolderCustomSettings,
    },
};
use windows::core::{HSTRING, PWSTR};

use windows::Win32::System::Com::CoCreateInstance;

use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM, SetFileAttributesW,
};

const DEFAULT_GENERATED_ICON_PREFIX: &str = env!("CARGO_PKG_NAME");

/// Initializes COM on the currently executing thread with apartment threading.
///
/// Returns `Ok(())` on success. `RPC_E_CHANGED_MODE` (already initialized
/// with a different concurrency model) is treated as benign — the caller may
/// still proceed without the known-folder guard.
fn ensure_com_initialized() -> std::result::Result<(), windows::core::Error> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    Ok(())
}

/// Helper to create a `WindowsFolderSettingsError::IconOperation` for a given path and message.
fn icon_op_error<P: AsRef<Path>>(path: P, msg: &str) -> WindowsFolderSettingsError {
    WindowsFolderSettingsError::IconOperation(path.as_ref().to_path_buf(), msg.to_string())
}

/// Provides Windows folder icon settings operations
pub trait WindowsFolderSettingsProviderExt {
    /// Set the icon for a folder
    fn set_icon_for_folder_windows<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &WindowsIconSet,
    ) -> Result<()>;

    /// Reset the icon for a folder
    fn reset_icon_for_folder_windows<P: AsRef<Path>>(&self, path: P) -> Result<()>;

    /// Constructor with Windows-specific options.
    fn new_windows(block_known_folders: bool, generated_icon_prefix: Option<&str>) -> Self;
}

/// Provides Windows folder icon settings operations
#[derive(Debug, Clone)]
pub struct WindowsFolderSettingsProvider {
    // COM interface for managing known folders (ex. Desktop, Downloads).
    // It's only used here to check if a folder is NOT a known folder before proceeding with writes/resets.
    // This prevents a lot of bad use cases where Windows factory settings are overwritten.
    com_known_folder_manager: Option<IKnownFolderManager>,
    generated_icon_prefix: String,
}

impl FolderSettingsProvider for WindowsFolderSettingsProvider {
    fn set_icon_for_folder<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &crate::api::IconSet,
    ) -> Result<()> {
        let windows_icon_set = WindowsIconSet::try_from(icon_set)?;
        self.set_icon_for_folder_windows(&path, &windows_icon_set)
    }

    fn reset_icon_for_folder<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.reset_icon_for_folder_windows(&path)
    }

    fn new() -> Self {
        WindowsFolderSettingsProvider::new_windows(true, None)
    }
}

impl WindowsFolderSettingsProviderExt for WindowsFolderSettingsProvider {
    fn new_windows(block_known_folders: bool, generated_icon_prefix: Option<&str>) -> Self {
        let com_known_folder_manager = if block_known_folders {
            // If COM init or CoCreateInstance fails, fall back to None.
            // RPC_E_CHANGED_MODE (thread already has a different COM concurrency model)
            // is benign — the caller may still proceed without the known-folder guard.
            ensure_com_initialized().ok().and_then(|_| unsafe {
                CoCreateInstance(&KnownFolderManager, None, CLSCTX_ALL).ok()
            })
        } else {
            None
        };

        let generated_icon_prefix = if let Some(p) = generated_icon_prefix {
            p.to_owned()
        } else {
            DEFAULT_GENERATED_ICON_PREFIX.to_owned()
        };

        Self {
            com_known_folder_manager,
            generated_icon_prefix,
        }
    }

    fn set_icon_for_folder_windows<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &WindowsIconSet,
    ) -> Result<()> {
        // Perform all necessary checks on the directory before proceeding.
        self.validate_folder(&path)?;

        self.remove_existing_generated_ico(&path).map_err(|e| {
            WindowsFolderSettingsError::IconOperation(path.as_ref().to_path_buf(), e.to_string())
        })?;

        let generated_ico_name = self.generate_unique_ico_file_name();

        let new_icon_path = PathBuf::from(path.as_ref()).join(&generated_ico_name);

        // Write to a .ico
        if let Err(e) = encode_to_system(icon_set, &new_icon_path) {
            // If encoding fails, clean up the orphaned file
            let _ = std::fs::remove_file(&new_icon_path);
            return Err(e);
        }

        // Instruct Windows to use the new icon for the folder.
        if let Err(e) = set_folder_icon_settings(path.as_ref(), &generated_ico_name) {
            // If setting the folder icon fails, clean up the orphaned .ico file
            let _ = std::fs::remove_file(&new_icon_path);
            return Err(e);
        }

        Ok(())
    }

    fn reset_icon_for_folder_windows<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Perform all necessary checks on the directory before proceeding.
        self.validate_folder(&path)?;

        if let Err(e) = clear_folder_icon_settings(path.as_ref()) {
            // If clearing folder settings fails, attempt to clean up orphaned .ico files
            let _ = self.remove_existing_generated_ico(&path);
            return Err(e);
        }

        self.remove_existing_generated_ico(&path).map_err(|e| {
            WindowsFolderSettingsError::IconOperation(path.as_ref().to_path_buf(), e.to_string())
        })?;

        Ok(())
    }
}

impl WindowsFolderSettingsProvider {
    /// Validate that a folder's icon can be modified
    fn validate_folder<P: AsRef<Path>>(&self, directory: P) -> Result<()> {
        // Check that it exists.
        if !directory.as_ref().exists() {
            return Err(icon_op_error(&directory, "Directory does not exist on filesystem").into());
        }

        // Check that it's a directory.
        if !directory.as_ref().is_dir() {
            return Err(icon_op_error(&directory, "Path is not a directory").into());
        }

        if let Some(com_known_folder_manager) = &self.com_known_folder_manager {
            // Check that it's not a known folder. (ex. C:\Users\username\Documents)
            // TODO: Parse the error and make sure it's a "known folder not found" error and not an "api did something bad" error.
            unsafe {
                com_known_folder_manager
                    .FindFolderFromPath(&HSTRING::from(directory.as_ref()), FFFP_EXACTMATCH)
            }
            .is_err()
            .then_some(())
            .ok_or_else(|| icon_op_error(&directory, "Folder is a known folder"))?;
        }

        Ok(())
    }

    /// Find and remove ALL existing generated .ico files in the provided directory.
    fn remove_existing_generated_ico<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> core::result::Result<(), std::io::Error> {
        let existing_ico_files = self.find_existing_icos(directory)?;

        for existing_ico_file in existing_ico_files {
            std::fs::remove_file(&existing_ico_file)?;
        }

        Ok(())
    }

    /// Returns all paths to generated .ico files in the provided directory.
    fn find_existing_icos<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> core::result::Result<Vec<PathBuf>, std::io::Error> {
        let mut found = Vec::new();
        for entry in fs::read_dir(directory.as_ref())? {
            let entry = entry?;
            let path = entry.path();

            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| is_generated_icon(name, &self.generated_icon_prefix))
            {
                found.push(path);
            }
        }

        Ok(found)
    }

    /// Generates a unique icon file name for a newly generated icon.
    /// Having a unique name each time is necessary to refresh the icon cache.
    fn generate_unique_ico_file_name(&self) -> String {
        format!("{}-{}.ico", self.generated_icon_prefix, Uuid::new_v4())
    }
}

/// Returns whether `file_name` names one of this crate's generated icon files:
/// a `.ico` whose name matches the pattern `{prefix}-{uuid}.ico` where uuid
/// is a standard v4 UUID (8-4-4-4-12 hex digits).
///
/// This strict format prevents accidental deletion of user-created icon files
/// that happen to share the prefix but don't follow the generated naming convention.
fn is_generated_icon(file_name: &str, prefix: &str) -> bool {
    // Check it's an .ico file
    if Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        != Some("ico")
    {
        return false;
    }

    // Check it starts with prefix-{uuid}.ico
    if !file_name.starts_with(prefix) {
        return false;
    }

    // Extract the UUID portion: everything between the dash after the prefix and ".ico"
    let rest = &file_name[prefix.len()..];
    rest.find(".ico")
        .map(|i| &rest[1..i])
        .map(|uuid_str| Uuid::parse_str(uuid_str).is_ok())
        .unwrap_or(false)
}

/// 1. Encode and write the provided icon set to a .ico file at the provided path.
/// 2. Write shell attributes hiding the .ico in Explorer
fn encode_to_system<P: AsRef<Path>>(icon_set: &WindowsIconSet, ico_path: P) -> Result<()> {
    // Encode to .ico
    let ico_frames = to_ico_frames(icon_set).map_err(|e| {
        WindowsFolderSettingsError::IconOperation(ico_path.as_ref().to_path_buf(), e.to_string())
    })?;

    // Write the file
    encode_and_write_ico(ico_frames, &ico_path)?;

    // Make the resulting icon file have the hidden and system attributes.
    // We just created this file, so we know it exists—no need to check attributes first.
    let ico_path_hstr = HSTRING::from(ico_path.as_ref());
    let new_icon_attribs = FILE_ATTRIBUTE_HIDDEN.0 | FILE_ATTRIBUTE_SYSTEM.0;

    unsafe { SetFileAttributesW(&ico_path_hstr, FILE_FLAGS_AND_ATTRIBUTES(new_icon_attribs)) }
        .map_err(|e| {
            WindowsFolderSettingsError::IconOperation(
                ico_path.as_ref().to_path_buf(),
                format!(
                    "Failed to set file attributes for generated icon: {}",
                    e.message()
                ),
            )
        })?;

    Ok(())
}

/// Convert RGBA bitmaps to individual .ico sizes
fn to_ico_frames<'a>(
    windows_icon_set: &'a WindowsIconSet<'a>,
) -> core::result::Result<Vec<IcoFrame<'a>>, image::error::ImageError> {
    windows_icon_set.iter().try_fold(
        Vec::with_capacity(WindowsIconSize::NUM_SIZES),
        |mut ico_frames, (res, img)| {
            let dim = res.dimension();
            let ico_frame = IcoFrame::as_png(
                img.image.as_bytes(),
                dim,
                dim,
                image::ExtendedColorType::Rgba8,
            )?;

            ico_frames.push(ico_frame);
            Ok(ico_frames)
        },
    )
}

// Write IcoFrames to an .ico file
fn encode_and_write_ico<P: AsRef<Path>>(ico_frames: Vec<IcoFrame>, path: P) -> Result<()> {
    let file = std::fs::File::create(path)
        .map_err(|e| WindowsFolderSettingsError::Error(e.to_string()))?;

    let writer = BufWriter::new(file);
    let encoder = IcoEncoder::new(writer);

    encoder
        .encode_images(&ico_frames)
        .map_err(|e| WindowsFolderSettingsError::Error(e.to_string()))?;

    Ok(())
}

/// Instructs the Windows shell to use the provided icon file for the provided directory.
/// Writes to desktop.ini in the directory, appending to any existing settings.
fn set_folder_icon_settings(
    directory: impl AsRef<Path>,
    icon_path: impl AsRef<Path>,
) -> Result<()> {
    // Note for the future: HSTRING cannot be created inside of the constructor for PWSTR, as it will be dropped before the PWSTR is used.
    // this leads to some confusing UB.
    let icon_path_hstr = HSTRING::from(icon_path.as_ref());

    set_folder_icon_internal(directory, Some(&icon_path_hstr))
}

/// Wipe windows shell settings for folder icon
fn clear_folder_icon_settings<P: AsRef<Path>>(directory: P) -> Result<()> {
    // Set the folder icon to a null string; this instructs Windows to remove the setting from desktop.ini and display the default
    // icon again.
    // This will also remove desktop.ini if it's empty post-mutation.
    set_folder_icon_internal(directory, None)
}

/// Internal helper for setting or clearing folder icon settings.
/// If `icon_path_hstr` is Some, sets the icon to that path (relative).
/// If `icon_path_hstr` is None, clears the icon setting.
fn set_folder_icon_internal<P: AsRef<Path>>(
    directory: P,
    icon_path_hstr: Option<&HSTRING>,
) -> Result<()> {
    let psz_icon_ptr = icon_path_hstr.map_or(std::ptr::null_mut(), |h| h.as_ptr() as *mut _);

    let mut fcs = SHFOLDERCUSTOMSETTINGS {
        dwSize: std::mem::size_of::<SHFOLDERCUSTOMSETTINGS>() as u32,
        dwMask: FCSM_ICONFILE,
        pszIconFile: PWSTR(psz_icon_ptr),
        ..SHFOLDERCUSTOMSETTINGS::default()
    };

    unsafe {
        SHGetSetFolderCustomSettings(&mut fcs, &HSTRING::from(directory.as_ref()), FCS_FORCEWRITE)
    }
    .map_err(|e| {
        WindowsFolderSettingsError::IconOperation(
            directory.as_ref().to_path_buf(),
            format!("Failed to set folder custom settings: {}", e.message()),
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::sys::windows::{WindowsIconImage, WindowsIconSize};
    use std::borrow::Cow;

    /// Builds a provider that does not initialize COM, keeping unit tests free
    /// of side effects. With `block_known_folders` disabled no COM apartment is
    /// created, so construction performs no I/O.
    fn provider_with_prefix(prefix: &str) -> WindowsFolderSettingsProvider {
        WindowsFolderSettingsProvider::new_windows(false, Some(prefix))
    }

    fn sample_icon_set() -> WindowsIconSet<'static> {
        let icons = WindowsIconSize::all().map(|size| {
            let dim = size.dimension();
            WindowsIconImage {
                size,
                image: Cow::Owned(image::DynamicImage::new_rgba8(dim, dim)),
            }
        });
        WindowsIconSet::from_icons(icons).unwrap()
    }

    #[test]
    fn default_prefix_is_crate_name() {
        let p = WindowsFolderSettingsProvider::new_windows(false, None);
        assert_eq!(p.generated_icon_prefix, DEFAULT_GENERATED_ICON_PREFIX);
    }

    #[test]
    fn generated_ico_name_uses_prefix_and_extension() {
        let p = provider_with_prefix("myprefix");
        let name = p.generate_unique_ico_file_name();
        assert!(name.starts_with("myprefix-"));
        assert!(name.ends_with(".ico"));
    }

    #[test]
    fn generated_ico_names_are_unique() {
        let p = provider_with_prefix("gen");
        assert_ne!(
            p.generate_unique_ico_file_name(),
            p.generate_unique_ico_file_name()
        );
    }

    #[test]
    fn generated_ico_name_is_recognized_as_generated() {
        // The name we produce must be matched by the predicate we clean up with.
        let p = provider_with_prefix("gen");
        let name = p.generate_unique_ico_file_name();
        assert!(is_generated_icon(&name, "gen"));
    }

    #[test]
    fn is_generated_icon_matches_prefixed_ico() {
        assert!(is_generated_icon(
            "icon-sys-550e8400-e29b-41d4-a716-446655440000.ico",
            "icon-sys"
        ));
    }

    #[test]
    fn is_generated_icon_rejects_wrong_extension() {
        assert!(!is_generated_icon(
            "icon-sys-550e8400-e29b-41d4-a716-446655440000.png",
            "icon-sys"
        ));
    }

    #[test]
    fn is_generated_icon_rejects_missing_extension() {
        assert!(!is_generated_icon(
            "icon-sys-550e8400-e29b-41d4-a716-446655440000",
            "icon-sys"
        ));
    }

    #[test]
    fn is_generated_icon_rejects_wrong_prefix() {
        assert!(!is_generated_icon(
            "other-550e8400-e29b-41d4-a716-446655440000.ico",
            "icon-sys"
        ));
    }

    #[test]
    fn is_generated_icon_rejects_non_uuid_after_prefix() {
        // "1234" is not a valid UUID — tests that we extract the UUID portion correctly
        assert!(!is_generated_icon("icon-sys-1234.ico", "icon-sys"));
    }

    #[test]
    fn is_generated_icon_rejects_missing_dash_before_uuid() {
        // No dash between prefix and UUID
        assert!(!is_generated_icon(
            "icon-sys550e8400-e29b-41d4-a716-446655440000.ico",
            "icon-sys"
        ));
    }

    #[test]
    fn is_generated_icon_rejects_short_prefix_match() {
        // Prefix "icon" should not match "myicon-..." because the dash check fails
        assert!(!is_generated_icon(
            "myicon-550e8400-e29b-41d4-a716-446655440000.ico",
            "icon"
        ));
    }

    #[test]
    fn to_ico_frames_produces_one_frame_per_size() {
        let set = sample_icon_set();
        let frames = to_ico_frames(&set).unwrap();
        assert_eq!(frames.len(), WindowsIconSize::NUM_SIZES);
    }
}
