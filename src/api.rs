use image::DynamicImage;

use crate::error::Result;

use std::path::Path;

/// Individual size of a system icon
#[derive(Debug, Clone)]
pub struct IconImage {
    pub data: DynamicImage,
}

/// Platform-agnostic system icon image set
#[derive(Debug, Clone, Default)]
pub struct IconSet {
    pub images: Vec<IconImage>,
    /// Optional scalable SVG representation.
    pub svg: Option<String>,
}

impl From<IconImage> for IconSet {
    fn from(value: IconImage) -> Self {
        Self {
            images: vec![value],
            svg: None,
        }
    }
}

/// Platform-agnostic icon operations for arbitrary file paths.
///
/// Currently only implemented for directories via the
/// [`FolderSettingsProvider`](crate::folder_settings::FolderSettingsProvider)
/// trait. The intent is to generalize this trait to cover arbitrary file
/// types (e.g. setting icons for individual files on Linux via GVFS
/// `metadata::custom-icon` attributes) once the platform backends
/// support it.
///
/// Until then, use the `folder_settings` module for directory icon
/// operations. This trait is kept for future API design continuity.
#[doc(hidden)]
pub trait IconProvider {
    /// Set the icon for a file or directory.
    ///
    /// # Future
    ///
    /// On Linux this will eventually operate on any file path via GVFS
    /// metadata. On Windows, file-level icon setting requires a
    /// different mechanism than the directory-based `desktop.ini`
    /// approach used by `FolderSettingsProvider`.
    fn set_icon_for_path<P, I>(&self, path: P, icon_set: &I) -> Result<()>
    where
        P: AsRef<Path>,
        I: Into<IconSet>;
}
