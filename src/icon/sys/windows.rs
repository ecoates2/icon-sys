use image::DynamicImage;
use std::{borrow::Cow, collections::BTreeMap};

use crate::icon::IconError;

/// Compatible image sizes for Windows icons (in pixels)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WindowsIconSize {
    Px16,
    Px20,
    Px24,
    Px32,
    Px40,
    Px48,
    Px64,
    Px256,
}

impl WindowsIconSize {
    pub const NUM_SIZES: usize = 8;

    /// Returns the pixel dimension for this icon size.
    pub fn dimension(&self) -> u32 {
        match self {
            WindowsIconSize::Px16 => 16,
            WindowsIconSize::Px20 => 20,
            WindowsIconSize::Px24 => 24,
            WindowsIconSize::Px32 => 32,
            WindowsIconSize::Px40 => 40,
            WindowsIconSize::Px48 => 48,
            WindowsIconSize::Px64 => 64,
            WindowsIconSize::Px256 => 256,
        }
    }

    /// Returns the WindowsIconSize for a given dimension, if valid.
    pub fn from_dimension(dimension: u32) -> Option<Self> {
        match dimension {
            16 => Some(WindowsIconSize::Px16),
            20 => Some(WindowsIconSize::Px20),
            24 => Some(WindowsIconSize::Px24),
            32 => Some(WindowsIconSize::Px32),
            40 => Some(WindowsIconSize::Px40),
            48 => Some(WindowsIconSize::Px48),
            64 => Some(WindowsIconSize::Px64),
            256 => Some(WindowsIconSize::Px256),
            _ => None,
        }
    }
    /// Returns an iterator over all defined sizes.
    pub fn all() -> impl Iterator<Item = WindowsIconSize> {
        [
            WindowsIconSize::Px16,
            WindowsIconSize::Px20,
            WindowsIconSize::Px24,
            WindowsIconSize::Px32,
            WindowsIconSize::Px40,
            WindowsIconSize::Px48,
            WindowsIconSize::Px64,
            WindowsIconSize::Px256,
        ]
        .iter()
        .copied()
    }
}

/// An individual image in a Windows icon. References image data and is size-validated.
#[derive(Debug, Clone)]
pub struct WindowsIconImage<'a> {
    pub size: WindowsIconSize,
    pub image: Cow<'a, DynamicImage>,
}

impl<'a> TryFrom<&'a crate::api::IconImage> for WindowsIconImage<'a> {
    type Error = IconError;

    fn try_from(icon_image: &'a crate::api::IconImage) -> Result<Self, Self::Error> {
        let icon_image_size = icon_image.data.width();
        let size = WindowsIconSize::from_dimension(icon_image_size)
            .ok_or_else(|| IconError::IconImage(format!("Invalid size: {:?}", icon_image_size)))?;
        Ok(WindowsIconImage {
            size,
            image: Cow::Borrowed(&icon_image.data),
        })
    }
}

impl<'a> From<WindowsIconImage<'a>> for crate::api::IconImage {
    fn from(icon_image: WindowsIconImage<'a>) -> Self {
        crate::api::IconImage {
            data: icon_image.image.into_owned(),
        }
    }
}

/// A Windows icon set composed of individual sizes.
#[derive(Debug, Clone)]
pub struct WindowsIconSet<'a> {
    images: BTreeMap<WindowsIconSize, WindowsIconImage<'a>>,
}

impl<'a> WindowsIconSet<'a> {
    /// Constructor from icons; must contain no duplicates
    pub fn from_icons<I>(icons: I) -> Result<Self, IconError>
    where
        I: IntoIterator<Item = WindowsIconImage<'a>>,
    {
        let mut images = BTreeMap::new();
        for icon in icons {
            if images.contains_key(&icon.size) {
                return Err(IconError::IconSet(format!(
                    "Duplicate icon size: {:?}",
                    icon.size
                )));
            }
            images.insert(icon.size, icon);
        }
        Ok(Self { images })
    }

    /// Add or replace an image for a given size.
    pub fn add_image(&mut self, icon: WindowsIconImage<'a>) {
        self.images.insert(icon.size, icon);
    }

    /// Returns true if all standard sizes are present.
    pub fn is_complete(&self) -> bool {
        self.missing_sizes().is_empty()
    }

    /// Returns a Vec of missing sizes.
    pub fn missing_sizes(&self) -> Vec<WindowsIconSize> {
        WindowsIconSize::all()
            .filter(|s| !self.images.contains_key(s))
            .collect()
    }

    /// Optionally, provide a getter for images if needed
    pub fn get_image(&self, size: WindowsIconSize) -> Option<&WindowsIconImage<'_>> {
        self.images.get(&size)
    }
    /// Returns an iterator over all present icons (size, icon)
    pub fn iter(&self) -> impl Iterator<Item = (&WindowsIconSize, &WindowsIconImage<'_>)> {
        self.images.iter()
    }

    /// Returns a reference to the internal BTreeMap of icons.
    pub fn as_map(&self) -> &BTreeMap<WindowsIconSize, WindowsIconImage<'_>> {
        &self.images
    }
}

impl<'a> IntoIterator for &'a WindowsIconSet<'a> {
    type Item = (&'a WindowsIconSize, &'a WindowsIconImage<'a>);
    type IntoIter = std::collections::btree_map::Iter<'a, WindowsIconSize, WindowsIconImage<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.images.iter()
    }
}

// Conversion to api::IconSet
impl<'a> From<WindowsIconSet<'a>> for crate::api::IconSet {
    fn from(win_set: WindowsIconSet) -> Self {
        let images = win_set
            .images
            .into_values()
            .map(crate::api::IconImage::from)
            .collect();
        crate::api::IconSet { images }
    }
}

// Conversion from api::IconSet
impl<'a> TryFrom<&'a crate::api::IconSet> for WindowsIconSet<'a> {
    type Error = IconError;

    fn try_from(icon_set: &'a crate::api::IconSet) -> Result<Self, Self::Error> {
        let icons = icon_set
            .images
            .iter()
            .map(WindowsIconImage::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let windows_icon_set = WindowsIconSet::from_icons(icons)?;
        let missing = windows_icon_set.missing_sizes();
        windows_icon_set
            .is_complete()
            .then_some(windows_icon_set)
            .ok_or_else(|| IconError::IconSet(format!("Missing sizes: {:?}", missing)))
    }
}
