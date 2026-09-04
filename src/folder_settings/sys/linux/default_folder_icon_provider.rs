use std::borrow::Cow;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::LinuxFolderSettingsError;
use crate::folder_settings::DefaultFolderIconProvider;
use crate::icon::sys::linux::{LinuxIconImage, LinuxIconSet};

/// Raster sizes commonly shipped by freedesktop icon themes, per the
/// freedesktop icon theme specification.
const COMMON_SIZES: [u32; 15] = [
    16, 20, 22, 24, 32, 36, 40, 48, 64, 72, 96, 128, 192, 256, 512,
];

/// Theme searched after the detected and desktop-specific themes.
///
/// `hicolor` is the freedesktop-mandated last resort but ships no `folder`
/// icon, so headless environments (CI, containers, remote shells) would
/// otherwise find nothing at all. Adwaita is the GTK reference theme and is
/// present on effectively every desktop Linux install.
const UNIVERSAL_FALLBACK_THEME: &str = "Adwaita";

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

/// Resolve the active icon theme via the desktop environment's preferred
/// mechanism.
///
/// Detection order: KDE (kreadconfig5) → XFCE (xfconf-query) → LXQt
/// (qdbus) → GNOME (gsettings) → hicolor. Each step logs a warning on
/// failure and falls through to the next. The caller is expected to fall
/// back to `hicolor` if all detection attempts fail.
fn active_theme() -> std::result::Result<String, LinuxFolderSettingsError> {
    // KDE Plasma
    if let Ok(theme) = active_theme_kde() {
        return Ok(theme);
    }
    // XFCE
    if let Ok(theme) = active_theme_xfce() {
        return Ok(theme);
    }
    // LXQt
    if let Ok(theme) = active_theme_lxqt() {
        return Ok(theme);
    }
    // GNOME / MATE / Cinnamon / Budgie
    if let Ok(theme) = active_theme_gnome() {
        return Ok(theme);
    }

    Err(LinuxFolderSettingsError::Gsettings(
        "no supported desktop environment tooling found".to_string(),
    ))
}

/// Resolve the active KDE Plasma icon theme via `kreadconfig5`.
fn active_theme_kde() -> std::result::Result<String, LinuxFolderSettingsError> {
    let output = Command::new("kreadconfig5")
        .args(["--file", "plasmarc", "--group", "General", "--key", "Theme"])
        .output()
        .map_err(|e| {
            LinuxFolderSettingsError::Gsettings(format!("failed to spawn kreadconfig5: {e}"))
        })?;

    if !output.status.success() {
        return Err(LinuxFolderSettingsError::Gsettings(
            "kreadconfig5 returned non-zero exit code".to_string(),
        ));
    }

    let theme = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if theme.is_empty() {
        return Err(LinuxFolderSettingsError::Gsettings(
            "kreadconfig5 returned an empty theme name".to_string(),
        ));
    }

    Ok(theme)
}

/// Resolve the active XFCE icon theme via `xfconf-query`.
fn active_theme_xfce() -> std::result::Result<String, LinuxFolderSettingsError> {
    let output = Command::new("xfconf-query")
        .args([
            "-c",
            "xfce4-desktop",
            "-p",
            "/backdrop/screen0/monitor0/workspace0/iconstyle",
        ])
        .output()
        .map_err(|e| {
            LinuxFolderSettingsError::Gsettings(format!("failed to spawn xfconf-query: {e}"))
        })?;

    if !output.status.success() {
        return Err(LinuxFolderSettingsError::Gsettings(
            "xfconf-query returned non-zero exit code".to_string(),
        ));
    }

    let theme = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if theme.is_empty() {
        return Err(LinuxFolderSettingsError::Gsettings(
            "xfconf-query returned an empty theme name".to_string(),
        ));
    }

    Ok(theme)
}

/// Resolve the active LXQt icon theme via `qdbus`.
fn active_theme_lxqt() -> std::result::Result<String, LinuxFolderSettingsError> {
    let output = Command::new("qdbus")
        .args([
            "org.lxqt.desktop",
            "/LXQt/Config",
            "org.lxqt.config.IConfig/value",
            "General/iconTheme",
        ])
        .output()
        .map_err(|e| LinuxFolderSettingsError::Gsettings(format!("failed to spawn qdbus: {e}")))?;

    if !output.status.success() {
        return Err(LinuxFolderSettingsError::Gsettings(
            "qdbus returned non-zero exit code".to_string(),
        ));
    }

    let theme = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if theme.is_empty() {
        return Err(LinuxFolderSettingsError::Gsettings(
            "qdbus returned an empty theme name".to_string(),
        ));
    }

    Ok(theme)
}

/// Resolve the active GNOME-family icon theme via `gsettings`.
fn active_theme_gnome() -> std::result::Result<String, LinuxFolderSettingsError> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
        .map_err(|e| {
            LinuxFolderSettingsError::Gsettings(format!("failed to spawn gsettings: {e}"))
        })?;

    if !output.status.success() {
        return Err(LinuxFolderSettingsError::Gsettings(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let theme = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('\'')
        .to_string();

    if theme.is_empty() {
        return Err(LinuxFolderSettingsError::Gsettings(
            "gsettings returned an empty theme name".to_string(),
        ));
    }

    Ok(theme)
}

/// Base directories searched for icon themes, in priority order.
///
/// Follows the XDG Base Directory Specification: home directories first,
/// then `$XDG_DATA_DIRS` (defaulting to `/usr/local/share:/usr/share`),
/// which covers Flatpak, Snap, and system-wide icon locations.
fn theme_base_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/icons"));
        dirs.push(PathBuf::from(&home).join(".icons"));
    }
    // Respect XDG_DATA_DIRS (default: /usr/local/share:/usr/share) to
    // cover Flatpak, Snap, and other system-wide icon locations.
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for dir in xdg_data_dirs.split(':') {
        let p = PathBuf::from(dir).join("icons");
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    dirs
}

/// Candidate paths for the `folder` "places" icon at a given size, across
/// both `<size>x<size>/places` and `places/<size>x<size>` theme layouts.
fn raster_candidates(base: &Path, theme: &str, size: u32) -> Vec<PathBuf> {
    vec![
        base.join(theme)
            .join(format!("{size}x{size}/places/folder.png")),
        base.join(theme)
            .join(format!("places/{size}x{size}/folder.png")),
    ]
}

fn svg_candidates(base: &Path, theme: &str) -> Vec<PathBuf> {
    vec![
        base.join(theme).join("scalable/places/folder.svg"),
        base.join(theme).join("places/scalable/folder.svg"),
    ]
}

/// Themes listed in a theme's `Inherits` key, in declaration order.
///
/// Returns an empty vector when the theme has no readable `index.theme` or
/// declares no parents.
fn inherited_themes(bases: &[PathBuf], theme: &str) -> Vec<String> {
    for base in bases {
        let Ok(index) = ini::Ini::load_from_file(base.join(theme).join("index.theme")) else {
            continue;
        };
        let Some(inherits) = index
            .section(Some("Icon Theme"))
            .and_then(|s| s.get("Inherits"))
        else {
            continue;
        };
        return inherits
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
    }
    Vec::new()
}

/// Expand seed themes into the full search order by walking each theme's
/// `Inherits` chain breadth-first, as required by the freedesktop icon theme
/// specification. Duplicates are dropped and `hicolor` is always last.
fn theme_search_order(bases: &[PathBuf], seeds: &[&str]) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut queue: VecDeque<String> = seeds
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| (*t).to_string())
        .collect();

    while let Some(theme) = queue.pop_front() {
        if theme == "hicolor" || order.contains(&theme) {
            continue;
        }
        queue.extend(inherited_themes(bases, &theme));
        order.push(theme);
    }

    order.push("hicolor".to_string());
    order
}

/// Map a desktop environment identifier to its preferred fallback icon theme.
pub(super) fn fallback_theme_for(desktop: &str) -> &'static str {
    let desktop = desktop.to_ascii_lowercase();
    let tokens: Vec<&str> = desktop.split(':').collect();
    if tokens.iter().any(|t| *t == "kde" || *t == "plasma") {
        "Breeze"
    } else if tokens.contains(&"xfce") {
        "Xfce-wallpaper"
    } else if tokens.contains(&"lxqt") {
        "lubuntu"
    } else if tokens.contains(&"pantheon") {
        "elementary"
    } else if tokens
        .iter()
        .any(|t| *t == "gnome" || *t == "cinnamon" || *t == "mate" || *t == "budgie")
    {
        "Adwaita"
    } else {
        "hicolor"
    }
}

fn load_folder_icon_set() -> Result<LinuxIconSet<'static>, LinuxFolderSettingsError> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let fallback_theme = fallback_theme_for(&desktop);

    // Resolve the active theme; fall back to the DE-specific theme, then
    // hicolor, using `log::warn!` so consumers can control or suppress output.
    let theme = active_theme().unwrap_or_else(|e| {
        log::warn!("icon-sys: theme detection failed, falling back to {fallback_theme}: {e}");
        fallback_theme.to_string()
    });
    let bases = theme_base_dirs();
    // Search the detected theme first, then the DE-specific fallback
    // (e.g. Adwaita for GNOME, Breeze for KDE), each expanded through its
    // `Inherits` chain, and finally `hicolor`.
    let themes = theme_search_order(
        &bases,
        &[theme.as_str(), fallback_theme, UNIVERSAL_FALLBACK_THEME],
    );

    let mut set = LinuxIconSet::new();

    for size in COMMON_SIZES {
        if set.get_image(size).is_some() {
            continue;
        }
        'found: for theme in &themes {
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
        'svg: for theme in &themes {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Create `<base>/<theme>/index.theme` with the given `Inherits` value.
    fn write_theme(base: &Path, theme: &str, inherits: Option<&str>) {
        let dir = base.join(theme);
        std::fs::create_dir_all(&dir).unwrap();
        let mut contents = String::from("[Icon Theme]\nName=Test\n");
        if let Some(inherits) = inherits {
            contents.push_str(&format!("Inherits={inherits}\n"));
        }
        std::fs::write(dir.join("index.theme"), contents).unwrap();
    }

    #[test]
    fn inherited_themes_reads_inherits_key() {
        let tmp = tempfile::tempdir().unwrap();
        write_theme(tmp.path(), "Adwaita", Some("AdwaitaLegacy, hicolor"));
        let bases = vec![tmp.path().to_path_buf()];

        assert_eq!(
            inherited_themes(&bases, "Adwaita"),
            vec!["AdwaitaLegacy".to_string(), "hicolor".to_string()]
        );
    }

    #[test]
    fn inherited_themes_is_empty_without_index_or_key() {
        let tmp = tempfile::tempdir().unwrap();
        write_theme(tmp.path(), "Standalone", None);
        let bases = vec![tmp.path().to_path_buf()];

        assert!(inherited_themes(&bases, "Standalone").is_empty());
        assert!(inherited_themes(&bases, "Missing").is_empty());
    }

    #[test]
    fn theme_search_order_expands_inheritance_and_ends_with_hicolor() {
        let tmp = tempfile::tempdir().unwrap();
        write_theme(tmp.path(), "Adwaita", Some("AdwaitaLegacy,hicolor"));
        write_theme(tmp.path(), "AdwaitaLegacy", Some("hicolor"));
        let bases = vec![tmp.path().to_path_buf()];

        assert_eq!(
            theme_search_order(&bases, &["Adwaita", "", "Adwaita"]),
            vec![
                "Adwaita".to_string(),
                "AdwaitaLegacy".to_string(),
                "hicolor".to_string()
            ]
        );
    }

    #[test]
    fn theme_search_order_survives_inheritance_cycles() {
        let tmp = tempfile::tempdir().unwrap();
        write_theme(tmp.path(), "A", Some("B"));
        write_theme(tmp.path(), "B", Some("A"));
        let bases = vec![tmp.path().to_path_buf()];

        assert_eq!(
            theme_search_order(&bases, &["A"]),
            vec!["A".to_string(), "B".to_string(), "hicolor".to_string()]
        );
    }
}
