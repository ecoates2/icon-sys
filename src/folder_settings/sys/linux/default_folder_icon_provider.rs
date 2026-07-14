use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::LinuxFolderSettingsError;
use crate::folder_settings::DefaultFolderIconProvider;
use crate::icon::sys::linux::{LinuxIconImage, LinuxIconSet};

/// Raster sizes commonly shipped by freedesktop icon themes.
const COMMON_SIZES: [u32; 8] = [16, 22, 24, 32, 48, 64, 128, 256];

pub trait LinuxDefaultFolderIconProviderExt {
    /// Dump the default folder icon from the active icon theme.
    fn dump_default_folder_icon_linux(
        &self,
    ) -> Result<LinuxIconSet<'static>, LinuxFolderSettingsError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxDefaultFolderIconProvider;

impl LinuxDefaultFolderIconProviderExt for LinuxDefaultFolderIconProvider {
    fn dump_default_folder_icon_linux(
        &self,
    ) -> Result<LinuxIconSet<'static>, LinuxFolderSettingsError> {
        load_folder_icon_set()
    }
}

impl DefaultFolderIconProvider for LinuxDefaultFolderIconProvider {
    fn dump_default_folder_icon(
        &self,
    ) -> Result<crate::api::IconSet, crate::folder_settings::FolderSettingsError> {
        let set = load_folder_icon_set()?;
        Ok(crate::api::IconSet::from(set))
    }
}

/// Resolve the active GNOME icon theme, falling back to `hicolor`.
fn active_theme() -> String {
    Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches('\'')
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "hicolor".to_string())
}

/// Base directories searched for icon themes, in priority order.
fn theme_base_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/icons"));
        dirs.push(PathBuf::from(&home).join(".icons"));
    }
    dirs.push(PathBuf::from("/usr/share/icons"));
    dirs
}

/// Candidate paths for the `folder` "places" icon at a given size, across
/// both `<size>/places` and `places/<size>` theme layouts.
fn raster_candidates(base: &Path, theme: &str, size: u32) -> Vec<PathBuf> {
    vec![
        base.join(theme)
            .join(format!("{size}x{size}/places/folder.png")),
        base.join(theme)
            .join(format!("places/{size}x{size}/folder.png")),
        base.join(theme).join(format!("{size}/places/folder.png")),
    ]
}

fn svg_candidates(base: &Path, theme: &str) -> Vec<PathBuf> {
    vec![
        base.join(theme).join("scalable/places/folder.svg"),
        base.join(theme).join("places/scalable/folder.svg"),
    ]
}

fn load_folder_icon_set() -> Result<LinuxIconSet<'static>, LinuxFolderSettingsError> {
    let theme = active_theme();
    let bases = theme_base_dirs();
    // Search the detected theme first, then Adwaita (the common GNOME default,
    // present whenever `adwaita-icon-theme` is installed) as a practical
    // fallback for headless/server environments where theme detection fails,
    // and finally `hicolor` as the freedesktop-mandated last resort.
    let themes = [theme.as_str(), "Adwaita", "hicolor"];

    let mut set = LinuxIconSet::new();

    for size in COMMON_SIZES {
        if set.get_image(size).is_some() {
            continue;
        }
        'found: for theme in themes {
            for base in &bases {
                for candidate in raster_candidates(base, theme, size) {
                    if candidate.exists()
                        && let Ok(img) = image::open(&candidate)
                    {
                        set.add_image(LinuxIconImage {
                            size,
                            image: Cow::Owned(img),
                        });
                        break 'found;
                    }
                }
            }
        }
    }

    if set.svg().is_none() {
        'svg: for theme in themes {
            for base in &bases {
                for candidate in svg_candidates(base, theme) {
                    if let Ok(svg) = std::fs::read_to_string(&candidate) {
                        // Parse + validate before keeping it; skip malformed SVGs.
                        if set.set_svg(svg).is_ok() {
                            break 'svg;
                        }
                    }
                }
            }
        }
    }

    if set.is_empty() {
        return Err(LinuxFolderSettingsError::Error(
            "could not locate a folder icon in any installed theme".to_string(),
        ));
    }

    Ok(set)
}
