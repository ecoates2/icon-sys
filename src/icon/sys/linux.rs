use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::DynamicImage;
use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::icon::IconError;

/// An individual raster image in a Linux icon set.
///
/// Unlike Windows, freedesktop icon themes allow arbitrary pixel sizes
/// (16, 22, 24, 32, 48, 64, 128, 256, ...), so the size is just the
/// square pixel dimension rather than a fixed enum.
#[derive(Debug, Clone)]
pub struct LinuxIconImage<'a> {
    /// Square pixel dimension (width == height).
    pub size: u32,
    pub image: Cow<'a, DynamicImage>,
}

impl<'a> From<&'a crate::api::IconImage> for LinuxIconImage<'a> {
    fn from(icon_image: &'a crate::api::IconImage) -> Self {
        Self {
            size: icon_image.data.width(),
            image: Cow::Borrowed(&icon_image.data),
        }
    }
}

impl<'a> From<LinuxIconImage<'a>> for crate::api::IconImage {
    fn from(icon_image: LinuxIconImage<'a>) -> Self {
        crate::api::IconImage {
            data: icon_image.image.into_owned(),
        }
    }
}

/// A Linux icon set: a collection of raster sizes plus an optional scalable SVG.
///
/// The SVG round-trips through the platform-agnostic `IconSet`.
#[derive(Debug, Clone, Default)]
pub struct LinuxIconSet<'a> {
    raster: BTreeMap<u32, LinuxIconImage<'a>>,
    /// Raw SVG markup for the scalable variant, if present.
    svg: Option<String>,
}

impl<'a> LinuxIconSet<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructor from raster icons; must contain no duplicate sizes.
    pub fn from_icons<I>(icons: I) -> Result<Self, IconError>
    where
        I: IntoIterator<Item = LinuxIconImage<'a>>,
    {
        let mut raster = BTreeMap::new();
        for icon in icons {
            if raster.contains_key(&icon.size) {
                return Err(IconError::IconSet(format!(
                    "Duplicate icon size: {}",
                    icon.size
                )));
            }
            raster.insert(icon.size, icon);
        }
        Ok(Self { raster, svg: None })
    }

    /// Add or replace a raster image for a given size.
    pub fn add_image(&mut self, icon: LinuxIconImage<'a>) {
        self.raster.insert(icon.size, icon);
    }

    /// Set the scalable SVG variant, validating it with `usvg` before storing.
    pub fn set_svg(&mut self, svg: impl Into<String>) -> Result<(), IconError> {
        let svg = svg.into();
        usvg::Tree::from_str(&svg, &usvg::Options::default())
            .map_err(|e| IconError::IconImage(format!("invalid SVG: {e}")))?;
        self.svg = Some(svg);
        Ok(())
    }

    /// The scalable SVG variant, if any.
    pub fn svg(&self) -> Option<&str> {
        self.svg.as_deref()
    }

    /// Build a scalable, SVG-only set by embedding a raster image inside an SVG.
    ///
    /// Stock theme folder icons are vector, so a customized folder set from a
    /// plain PNG looks subtly off next to them at non-native zoom levels.
    /// Wrapping the bitmap in an SVG (`<image>` with a `viewBox`) lets the
    /// desktop scale it like any vector icon, restoring visual parity after
    /// raster editing ops. No raster sizes are stored; only the SVG.
    pub fn from_raster_as_svg(image: &DynamicImage) -> Result<LinuxIconSet<'static>, IconError> {
        let (w, h) = (image.width(), image.height());
        let mut png = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| IconError::IconImage(format!("failed to encode PNG: {e}")))?;
        let data = STANDARD.encode(png.into_inner());
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
             viewBox=\"0 0 {w} {h}\"><image width=\"{w}\" height=\"{h}\" \
             href=\"data:image/png;base64,{data}\"/></svg>"
        );
        let mut set = LinuxIconSet::default();
        set.set_svg(svg)?;
        Ok(set)
    }

    /// Returns true if the set contains no raster images and no SVG.
    pub fn is_empty(&self) -> bool {
        self.raster.is_empty() && self.svg.is_none()
    }

    pub fn get_image(&self, size: u32) -> Option<&LinuxIconImage<'_>> {
        self.raster.get(&size)
    }

    /// The largest available raster size, if any.
    pub fn largest(&self) -> Option<&LinuxIconImage<'_>> {
        self.raster.values().next_back()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u32, &LinuxIconImage<'_>)> {
        self.raster.iter()
    }
}

impl<'a> IntoIterator for &'a LinuxIconSet<'a> {
    type Item = (&'a u32, &'a LinuxIconImage<'a>);
    type IntoIter = std::collections::btree_map::Iter<'a, u32, LinuxIconImage<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.raster.iter()
    }
}

// Conversion to api::IconSet (SVG is preserved).
impl<'a> From<LinuxIconSet<'a>> for crate::api::IconSet {
    fn from(linux_set: LinuxIconSet) -> Self {
        let svg = linux_set.svg.clone();
        let images = linux_set
            .raster
            .into_values()
            .map(crate::api::IconImage::from)
            .collect();
        crate::api::IconSet { images, svg }
    }
}

// Conversion from api::IconSet (SVG is preserved + revalidated).
impl<'a> From<&'a crate::api::IconSet> for LinuxIconSet<'a> {
    fn from(icon_set: &'a crate::api::IconSet) -> Self {
        let mut raster = BTreeMap::new();
        for image in &icon_set.images {
            let linux_image = LinuxIconImage::from(image);
            raster.insert(linux_image.size, linux_image);
        }
        let mut set = Self { raster, svg: None };
        if let Some(svg) = &icon_set.svg {
            // Re-validate on import; drop the SVG if it no longer parses.
            let _ = set.set_svg(svg.clone());
        }
        set
    }
}
