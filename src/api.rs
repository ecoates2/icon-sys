use image::DynamicImage;

use crate::error::Result;

use std::path::Path;

/// Individual size of a system icon
pub struct IconImage {
    pub data: DynamicImage,
}

/// Platform-agnostic system icon image set
pub struct IconSet {
    pub images: Vec<IconImage>,
}

impl From<IconImage> for IconSet {
    fn from(value: IconImage) -> Self {
        Self {
            images: vec![value],
        }
    }
}

#[doc(hidden)]
/// Platform-agnostic icon operations
pub trait IconProvider {
    /// Set the icon for a file/directory
    fn set_icon_for_path<P, I>(&self, path: P, icon_set: &I) -> Result<()>
    where
        P: AsRef<Path>,
        I: Into<IconSet>;
}
