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
    Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, INVALID_FILE_ATTRIBUTES},
    System::Com::{CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoInitializeEx},
    UI::Shell::{
        FCS_FORCEWRITE, FCSM_ICONFILE, FFFP_EXACTMATCH, IKnownFolderManager, KnownFolderManager,
        SHFOLDERCUSTOMSETTINGS, SHGetSetFolderCustomSettings,
    },
};
use windows::core::{HSTRING, PWSTR};

use windows::Win32::System::Com::CoCreateInstance;

use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM, GetFileAttributesW, SetFileAttributesW,
};

const DEFAULT_GENERATED_ICON_PREFIX: &str = env!("CARGO_PKG_NAME");

/// Initializes COM on the currently executing thread.
fn ensure_com_initialized() {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.unwrap();
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
        let com_known_folder_manager = block_known_folders.then(|| {
            ensure_com_initialized();
            unsafe { CoCreateInstance(&KnownFolderManager, None, CLSCTX_ALL) }.unwrap()
        });

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
        encode_to_system(icon_set, &new_icon_path)?;

        // Instruct Windows to use the new icon for the folder.
        set_folder_icon_settings(path, &generated_ico_name)?;

        Ok(())
    }

    fn reset_icon_for_folder_windows<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Perform all necessary checks on the directory before proceeding.
        self.validate_folder(&path)?;

        clear_folder_icon_settings(&path)?;

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
        directory.as_ref().exists().then_some(()).ok_or_else(|| {
            WindowsFolderSettingsError::IconOperation(
                directory.as_ref().to_path_buf(),
                "Directory does not exist on filesystem".to_string(),
            )
        })?;

        // Check that it's a directory.
        directory.as_ref().is_dir().then_some(()).ok_or_else(|| {
            WindowsFolderSettingsError::IconOperation(
                directory.as_ref().to_path_buf(),
                "Path is not a directory".to_string(),
            )
        })?;

        if let Some(com_known_folder_manager) = &self.com_known_folder_manager {
            // Check that it's not a known folder. (ex. C:\Users\username\Documents)
            // TODO: Parse the error and make sure it's a "known folder not found" error and not an "api did something bad" error.
            unsafe {
                com_known_folder_manager
                    .FindFolderFromPath(&HSTRING::from(directory.as_ref()), FFFP_EXACTMATCH)
            }
            .is_err()
            .then_some(())
            .ok_or_else(|| {
                WindowsFolderSettingsError::IconOperation(
                    directory.as_ref().to_path_buf(),
                    "Folder is a known folder".to_string(),
                )
            })?;
        }

        Ok(())
    }

    /// Find and remove any existing generated .ico files in the provided directory.
    fn remove_existing_generated_ico<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> core::result::Result<(), std::io::Error> {
        let existing_ico_file = self.find_existing_ico(directory)?;

        if let Some(existing_ico_file) = existing_ico_file {
            std::fs::remove_file(existing_ico_file)?;
        }

        Ok(())
    }

    /// Returns the path for the first existing generated .ico file in the provided directory, if any.
    fn find_existing_ico<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> core::result::Result<Option<PathBuf>, std::io::Error> {
        for entry in fs::read_dir(directory.as_ref())? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|ext| ext.to_str()) == Some("ico")
                && let Some(file_name) = path.file_name().and_then(|name| name.to_str())
                && file_name.starts_with(&self.generated_icon_prefix)
            {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    /// Generates a unique icon file name for a newly generated icon.
    /// Having a unique name each time is necessary to refresh the icon cache.
    fn generate_unique_ico_file_name(&self) -> String {
        format!("{}-{}.ico", &self.generated_icon_prefix, Uuid::new_v4())
    }
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

    let mut new_icon_attribs = FILE_FLAGS_AND_ATTRIBUTES({
        let mut new_icon_attribs = unsafe { GetFileAttributesW(&HSTRING::from(ico_path.as_ref())) };
        new_icon_attribs = (new_icon_attribs != INVALID_FILE_ATTRIBUTES)
            .then_some(new_icon_attribs)
            .ok_or_else(windows::core::Error::from_win32)
            .map_err(|e| {
                WindowsFolderSettingsError::IconOperation(
                    ico_path.as_ref().to_path_buf(),
                    format!(
                        "Failed to get file attributes for generated icon: {}",
                        e.message()
                    ),
                )
            })?;
        new_icon_attribs
    });

    new_icon_attribs |= FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;

    unsafe { SetFileAttributesW(&HSTRING::from(ico_path.as_ref()), new_icon_attribs) }.map_err(
        |e| {
            WindowsFolderSettingsError::IconOperation(
                ico_path.as_ref().to_path_buf(),
                format!(
                    "Failed to set file attributes for generated icon: {}",
                    e.message()
                ),
            )
        },
    )?;

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

    // Using a relative path for the icon file so that the icon is still displayed even if the folder is moved.
    let mut fcs = SHFOLDERCUSTOMSETTINGS {
        dwSize: std::mem::size_of::<SHFOLDERCUSTOMSETTINGS>() as u32,
        dwMask: FCSM_ICONFILE,
        pszIconFile: PWSTR(icon_path_hstr.as_wide().as_ptr() as *mut _),
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

/// Wipe windows shell settings for folder icon
fn clear_folder_icon_settings<P: AsRef<Path>>(directory: P) -> Result<()> {
    // Set the folder icon to a null string; this instructs Windows to remove the setting from desktop.ini and display the default
    // icon again.
    let mut fcs = SHFOLDERCUSTOMSETTINGS {
        dwSize: std::mem::size_of::<SHFOLDERCUSTOMSETTINGS>() as u32,
        dwMask: FCSM_ICONFILE,
        pszIconFile: PWSTR(std::ptr::null_mut()),
        ..SHFOLDERCUSTOMSETTINGS::default()
    };

    // This will also remove desktop.ini if it's empty post-mutation.
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
