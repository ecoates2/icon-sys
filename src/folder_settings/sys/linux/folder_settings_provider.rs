use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use uuid::Uuid;

use super::LinuxFolderSettingsError;
use crate::folder_settings::FolderSettingsProvider;
use crate::folder_settings::error::Result;
use crate::icon::sys::linux::LinuxIconSet;

const DEFAULT_GENERATED_ICON_PREFIX: &str = env!("CARGO_PKG_NAME");

/// File extensions this crate generates for folder icons, used both when
/// writing a new icon and when cleaning up previously generated ones.
const GENERATED_ICON_EXTENSIONS: [&str; 2] = ["png", "svg"];

/// Mechanism used to apply a custom folder icon on Linux.
///
/// Linux has no single API: GNOME-family file managers (Nautilus, Cinnamon,
/// MATE, Budgie) read the `metadata::custom-icon` GVFS attribute, while KDE
/// (Dolphin) and XFCE (Thunar) read a `.directory` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxBackend {
    /// Detect the mechanism from `XDG_CURRENT_DESKTOP`.
    Auto,
    /// GVFS `metadata::custom-icon` (GNOME, Cinnamon, MATE, Budgie).
    GioMetadata,
    /// Freedesktop `.directory` file (KDE, XFCE).
    DirectoryFile,
}

/// Internal representation: a resolved backend with optional gio availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedBackend {
    GioMetadata,
    DirectoryFile,
}

impl LinuxBackend {
    /// Resolve `Auto` into a concrete backend using `XDG_CURRENT_DESKTOP`.
    fn resolve(self) -> std::result::Result<ResolvedBackend, LinuxFolderSettingsError> {
        match self {
            LinuxBackend::Auto => {
                let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
                detect_backend(&desktop)
            }
            LinuxBackend::GioMetadata => {
                // Check if gio is available; return an error if not so the
                // user gets a clear message rather than a silent failure.
                if !gio_available() {
                    return Err(LinuxFolderSettingsError::GioNotFound);
                }
                Ok(ResolvedBackend::GioMetadata)
            }
            LinuxBackend::DirectoryFile => Ok(ResolvedBackend::DirectoryFile),
        }
    }
}

/// Check whether the `gio` binary is available on PATH.
fn gio_available() -> bool {
    Command::new("gio")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Map an `XDG_CURRENT_DESKTOP` value to a concrete backend.
///
/// `XDG_CURRENT_DESKTOP` can contain multiple colon-separated tokens
/// (e.g. `"GNOME:Wayland"`), so we split on `:` and check each token
/// against known desktop environment identifiers. A simple substring
/// match is avoided because names like `"my-custom-desktop-mate"`
/// would produce false positives.
///
/// Comparison is case-insensitive since `XDG_CURRENT_DESKTOP` values
/// may vary in casing across distributions.
fn detect_backend(desktop: &str) -> std::result::Result<ResolvedBackend, LinuxFolderSettingsError> {
    if desktop.is_empty() {
        return Err(LinuxFolderSettingsError::UndetectedDesktop);
    }

    let has_token = |tokens: &[&str]| {
        desktop
            .split(':')
            .any(|token| tokens.contains(&token.to_ascii_lowercase().as_str()))
    };

    let gio_tokens = ["gnome", "cinnamon", "mate", "budgie", "unity", "pantheon"];
    let dirfile_tokens = ["kde", "xfce", "lxqt"];

    // Check GioMetadata family first (more specific).
    if has_token(&gio_tokens) {
        Ok(ResolvedBackend::GioMetadata)
    } else if has_token(&dirfile_tokens) {
        Ok(ResolvedBackend::DirectoryFile)
    } else {
        Err(LinuxFolderSettingsError::UndetectedDesktop)
    }
}

/// Linux-specific extension to the cross-platform folder settings provider.
pub trait LinuxFolderSettingsProviderExt {
    /// Construct with an explicit backend choice and optional prefix for
    /// generated icon files. Pass `None` to use the default prefix.
    fn new_linux(
        backend: LinuxBackend,
        generated_icon_prefix: Option<&str>,
        bump_mtime: bool,
    ) -> Self;

    /// Set the icon for a folder using a Linux icon set.
    fn set_icon_for_folder_linux<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &LinuxIconSet,
    ) -> Result<()>;

    /// Reset the icon for a folder.
    fn reset_icon_for_folder_linux<P: AsRef<Path>>(&self, path: P) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct LinuxFolderSettingsProvider {
    backend: ResolvedBackend,
    generated_icon_prefix: String,
    bump_mtime: bool,
}

impl FolderSettingsProvider for LinuxFolderSettingsProvider {
    fn new() -> Self {
        LinuxFolderSettingsProvider::new_linux(LinuxBackend::Auto, None, true)
    }

    fn set_icon_for_folder<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        icon_set: &crate::IconSet,
    ) -> Result<()> {
        let linux_icon_set = LinuxIconSet::from(icon_set);
        self.set_icon_for_folder_linux(path, &linux_icon_set)
    }

    fn reset_icon_for_folder<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.reset_icon_for_folder_linux(path)
    }
}

impl LinuxFolderSettingsProviderExt for LinuxFolderSettingsProvider {
    fn new_linux(
        backend: LinuxBackend,
        generated_icon_prefix: Option<&str>,
        bump_mtime: bool,
    ) -> Self {
        let generated_icon_prefix = generated_icon_prefix
            .map(|p| p.to_owned())
            .unwrap_or_else(|| DEFAULT_GENERATED_ICON_PREFIX.to_owned());

        // Resolve the backend once at construction time so we don't
        // re-detect on every operation.
        let backend = backend.resolve();

        Self {
            backend: backend.unwrap_or_else(|e| {
                // Log the failure; fall back to DirectoryFile which has
                // no external dependencies. GioMetadata requires the gio CLI.
                log::warn!(
                    "icon-sys: backend resolution failed, falling back to DirectoryFile: {e}"
                );
                ResolvedBackend::DirectoryFile
            }),
            generated_icon_prefix,
            bump_mtime,
        }
    }

    fn set_icon_for_folder_linux<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &LinuxIconSet,
    ) -> Result<()> {
        self.validate_folder(&path)?;

        match self.backend {
            ResolvedBackend::GioMetadata => self.set_via_gio_metadata(&path, icon_set)?,
            ResolvedBackend::DirectoryFile => self.set_via_directory_file(&path, icon_set)?,
        }

        self.maybe_bump_mtime(&path);
        Ok(())
    }

    fn reset_icon_for_folder_linux<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.validate_folder(&path)?;

        match self.backend {
            ResolvedBackend::GioMetadata => self.reset_via_gio_metadata(&path)?,
            ResolvedBackend::DirectoryFile => self.reset_via_directory_file(&path)?,
        }

        self.maybe_bump_mtime(&path);
        Ok(())
    }
}

impl LinuxFolderSettingsProvider {
    /// Nudge file-manager monitors to refresh by bumping the folder's mtime.
    /// Best-effort: failures are ignored since the icon change still applies.
    fn maybe_bump_mtime<P: AsRef<Path>>(&self, path: P) {
        if self.bump_mtime {
            let now = filetime::FileTime::now();
            let _ = filetime::set_file_mtime(path.as_ref(), now);
        }
    }

    fn validate_folder<P: AsRef<Path>>(&self, directory: P) -> Result<()> {
        let dir = directory.as_ref();
        if !dir.exists() {
            return Err(LinuxFolderSettingsError::IconOperation(
                dir.to_path_buf(),
                "Directory does not exist on filesystem".to_string(),
            )
            .into());
        }
        if !dir.is_dir() {
            return Err(LinuxFolderSettingsError::IconOperation(
                dir.to_path_buf(),
                "Path is not a directory".to_string(),
            )
            .into());
        }
        Ok(())
    }

    /// Write the icon set to a hidden generated file and point the folder's
    /// `metadata::custom-icon` attribute at it.
    fn set_via_gio_metadata<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &LinuxIconSet,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        let icon_path = self.write_generated_icon(&path, icon_set)?;
        // `gio` stores the custom icon as an absolute `file://` URI.
        let uri = format!("file://{}", icon_path.display());
        gio_set_metadata(path.as_ref(), "metadata::custom-icon", Some(&uri))?;
        Ok(())
    }

    fn reset_via_gio_metadata<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        gio_set_metadata(path.as_ref(), "metadata::custom-icon", None)?;
        self.remove_generated_icons(&path)?;
        Ok(())
    }

    /// Write the icon set to a generated file and reference it from a
    /// `.directory` file (KDE Dolphin, XFCE Thunar). A single icon is scaled
    /// by the file manager.
    fn set_via_directory_file<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &LinuxIconSet,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        let icon_path = self.write_generated_icon(&path, icon_set)?;

        let directory_path = path.as_ref().join(".directory");
        // Load existing entry if present so other settings are preserved.
        // If the file doesn't exist yet (first time setting an icon),
        // start with a fresh INI. Only treat parse errors as failures.
        let mut conf = match ini::Ini::load_from_file(&directory_path) {
            Ok(conf) => conf,
            Err(ini::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                ini::Ini::new()
            }
            Err(e) => {
                return Err(LinuxFolderSettingsError::DirectoryFileParse(
                    directory_path.clone(),
                    e.to_string(),
                ));
            }
        };
        // `icon_path` is absolute so file managers treat it as a file rather
        // than an icon-theme name.
        conf.with_section(Some("Desktop Entry"))
            .set("Icon", icon_path.to_string_lossy().as_ref());
        conf.write_to_file(&directory_path)?;

        Ok(())
    }

    fn reset_via_directory_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        let directory_path = path.as_ref().join(".directory");
        if directory_path.exists() {
            let conf = ini::Ini::load_from_file(&directory_path).map_err(|e| {
                LinuxFolderSettingsError::DirectoryFileParse(directory_path.clone(), e.to_string())
            })?;
            match strip_icon_entry(conf) {
                // Rewrite with remaining settings preserved...
                Some(conf) => conf.write_to_file(&directory_path)?,
                // ...or drop the file entirely if nothing meaningful remains.
                None => fs::remove_file(&directory_path)?,
            }
        }
        self.remove_generated_icons(&path)?;
        Ok(())
    }

    /// Write the icon set to a generated file in `directory`, preferring the
    /// scalable SVG (for visual parity with vector theme icons) and falling
    /// back to the largest raster size. Returns the written file's path.
    ///
    /// Any previously generated icon files are removed first so backends never
    /// leave a stale `.png` behind when switching to `.svg` or vice versa.
    ///
    /// The returned path is always absolute. Unlike Windows (where `desktop.ini`
    /// stores a bare filename so the icon survives folder moves), both Linux
    /// backends need an absolute reference: `gio` requires an absolute
    /// `file://` URI, and a bare name in a `.directory` `Icon=` key is treated
    /// as an icon-theme lookup rather than a sibling file.
    fn write_generated_icon<P: AsRef<Path>>(
        &self,
        directory: P,
        icon_set: &LinuxIconSet,
    ) -> std::result::Result<PathBuf, LinuxFolderSettingsError> {
        self.remove_generated_icons(&directory)?;

        // Resolve to an absolute path up front (without following symlinks or
        // requiring the target to exist yet).
        let dir = std::path::absolute(directory.as_ref()).map_err(|e| {
            LinuxFolderSettingsError::IconOperation(directory.as_ref().to_path_buf(), e.to_string())
        })?;

        if let Some(svg) = icon_set.svg() {
            let icon_path = dir.join(format!(
                "{}-{}.svg",
                self.generated_icon_prefix,
                Uuid::new_v4()
            ));
            // Write to a temp file first, then rename for atomicity.
            let tmp_path = icon_path.with_extension("svg.tmp");
            fs::write(&tmp_path, svg).map_err(|e| {
                LinuxFolderSettingsError::IconOperation(icon_path.clone(), e.to_string())
            })?;
            fs::rename(&tmp_path, &icon_path).map_err(|e| {
                LinuxFolderSettingsError::IconOperation(icon_path.clone(), e.to_string())
            })?;
            return Ok(icon_path);
        }

        let largest = icon_set.largest().ok_or_else(|| {
            LinuxFolderSettingsError::IconOperation(
                dir.clone(),
                "Icon set contains neither an SVG nor any raster images".to_string(),
            )
        })?;
        let icon_path = dir.join(format!(
            "{}-{}.png",
            self.generated_icon_prefix,
            Uuid::new_v4()
        ));
        largest.image.save(&icon_path).map_err(|e| {
            LinuxFolderSettingsError::IconOperation(icon_path.clone(), e.to_string())
        })?;
        Ok(icon_path)
    }

    fn remove_generated_icons<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        for entry in fs::read_dir(directory.as_ref())? {
            let entry = entry?;
            let p = entry.path();
            let has_generated_ext = p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| GENERATED_ICON_EXTENSIONS.contains(&e));
            if has_generated_ext
                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                && name.starts_with(&self.generated_icon_prefix)
            {
                // Ignore NotFound errors to avoid TOCTOU race conditions
                // where another process deletes the file between read_dir
                // and remove_file.
                let _ = fs::remove_file(p);
            }
        }
        Ok(())
    }
}

/// Remove the `Icon` key from a parsed `.directory` config, returning the
/// remaining config to rewrite, or `None` if nothing meaningful is left and
/// the file should be deleted.
///
/// Kept free of filesystem access so the preserve-vs-delete decision can be
/// unit-tested directly.
pub fn strip_icon_entry(mut conf: ini::Ini) -> Option<ini::Ini> {
    // Surgically drop only the Icon key, keeping other settings.
    conf.with_section(Some("Desktop Entry")).delete(&"Icon");

    // Drop the section header if Icon was its only key.
    let section_empty = conf
        .section(Some("Desktop Entry"))
        .map(|s| s.is_empty())
        .unwrap_or(true);
    if section_empty {
        conf.delete(Some("Desktop Entry"));
    }

    // Keep the file only if some section still retains a key.
    if conf.iter().any(|(_, props)| !props.is_empty()) {
        Some(conf)
    } else {
        None
    }
}

/// Set or unset a GVFS metadata attribute via the `gio` CLI.
fn gio_set_metadata<P: AsRef<Path>>(
    path: P,
    key: &str,
    value: Option<&str>,
) -> std::result::Result<(), LinuxFolderSettingsError> {
    let mut cmd = Command::new("gio");
    cmd.arg("set");
    if value.is_none() {
        cmd.arg("-t").arg("unset");
    }
    cmd.arg(path.as_ref()).arg(key);
    if let Some(value) = value {
        cmd.arg(value);
    }

    let output = cmd.output().map_err(|e| LinuxFolderSettingsError::Gio {
        path: path.as_ref().to_path_buf(),
        key: key.to_string(),
        value: value.map(|v| v.to_string()),
        detail: format!("failed to spawn gio: {e}"),
    })?;

    if !output.status.success() {
        return Err(LinuxFolderSettingsError::Gio {
            path: path.as_ref().to_path_buf(),
            key: key.to_string(),
            value: value.map(|v| v.to_string()),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::default_folder_icon_provider::fallback_theme_for;
    use super::*;

    #[test]
    fn detect_backend_gnome_family_uses_gio() {
        for de in [
            "GNOME",
            "Cinnamon",
            "MATE",
            "Budgie:GNOME",
            "Unity",
            "Pantheon",
        ] {
            assert_eq!(detect_backend(de).unwrap(), ResolvedBackend::GioMetadata);
        }
    }

    #[test]
    fn detect_backend_kde_family_uses_directory_file() {
        for de in ["KDE", "XFCE", "LXQt"] {
            assert_eq!(detect_backend(de).unwrap(), ResolvedBackend::DirectoryFile);
        }
    }

    #[test]
    fn detect_backend_with_session_type() {
        // XDG_CURRENT_DESKTOP often includes a colon-separated session type.
        assert_eq!(
            detect_backend("GNOME:Wayland").unwrap(),
            ResolvedBackend::GioMetadata
        );
        assert_eq!(
            detect_backend("KDE:Wayland").unwrap(),
            ResolvedBackend::DirectoryFile
        );
    }

    #[test]
    fn detect_backend_is_case_insensitive() {
        assert_eq!(
            detect_backend("gnome").unwrap(),
            ResolvedBackend::GioMetadata
        );
        assert_eq!(
            detect_backend("kde").unwrap(),
            ResolvedBackend::DirectoryFile
        );
    }

    #[test]
    fn detect_backend_empty_is_error() {
        assert!(matches!(
            detect_backend(""),
            Err(LinuxFolderSettingsError::UndetectedDesktop)
        ));
    }

    #[test]
    fn detect_backend_unknown_is_error() {
        assert!(matches!(
            detect_backend("Enlightenment"),
            Err(LinuxFolderSettingsError::UndetectedDesktop)
        ));
    }

    #[test]
    fn detect_backend_rejects_false_positives() {
        // A desktop whose name contains a known substring as a token should
        // not match — e.g. "my-custom-desktop-mate" is not the MATE desktop.
        assert!(matches!(
            detect_backend("my-custom-desktop-mate"),
            Err(LinuxFolderSettingsError::UndetectedDesktop)
        ));
    }

    #[test]
    fn explicit_directory_file_backend_resolves_to_itself() {
        assert_eq!(
            LinuxBackend::DirectoryFile.resolve().unwrap(),
            ResolvedBackend::DirectoryFile
        );
    }

    #[test]
    fn explicit_gio_backend_resolves_when_gio_available() {
        // GioMetadata resolution depends on gio being on PATH.
        // If gio is available, it should resolve; otherwise it returns GioNotFound.
        let result = LinuxBackend::GioMetadata.resolve();
        if gio_available() {
            assert_eq!(result.unwrap(), ResolvedBackend::GioMetadata);
        } else {
            assert!(matches!(result, Err(LinuxFolderSettingsError::GioNotFound)));
        }
    }

    #[test]
    fn fallback_theme_for_kde_returns_breeze() {
        assert_eq!(fallback_theme_for("KDE"), "Breeze");
        assert_eq!(fallback_theme_for("KDE:Wayland"), "Breeze");
        assert_eq!(fallback_theme_for("plasma"), "Breeze");
    }

    #[test]
    fn fallback_theme_for_xfce_returns_xfce_wallpaper() {
        assert_eq!(fallback_theme_for("XFCE"), "Xfce-wallpaper");
        assert_eq!(fallback_theme_for("xfce"), "Xfce-wallpaper");
    }

    #[test]
    fn fallback_theme_for_lxqt_returns_lubuntu() {
        assert_eq!(fallback_theme_for("LXQt"), "lubuntu");
        assert_eq!(fallback_theme_for("lxqt"), "lubuntu");
    }

    #[test]
    fn fallback_theme_for_gnome_returns_adwaita() {
        assert_eq!(fallback_theme_for("GNOME"), "Adwaita");
        assert_eq!(fallback_theme_for("cinnamon"), "Adwaita");
        assert_eq!(fallback_theme_for("MATE"), "Adwaita");
    }

    #[test]
    fn fallback_theme_for_unknown_returns_hicolor() {
        assert_eq!(fallback_theme_for("Enlightenment"), "hicolor");
        assert_eq!(fallback_theme_for(""), "hicolor");
    }

    fn ini_of(s: &str) -> ini::Ini {
        ini::Ini::load_from_str(s).unwrap()
    }

    #[test]
    fn strip_icon_entry_deletes_file_when_only_icon() {
        let conf = ini_of("[Desktop Entry]\nIcon=/tmp/foo.png\n");
        assert!(strip_icon_entry(conf).is_none());
    }

    #[test]
    fn strip_icon_entry_preserves_other_keys_in_same_section() {
        let conf = ini_of("[Desktop Entry]\nIcon=/tmp/foo.png\nName=Docs\n");
        let result = strip_icon_entry(conf).expect("file should be kept");
        let section = result.section(Some("Desktop Entry")).unwrap();
        assert_eq!(section.get("Name"), Some("Docs"));
        assert!(section.get("Icon").is_none());
    }

    #[test]
    fn strip_icon_entry_preserves_other_sections() {
        let conf = ini_of("[Desktop Entry]\nIcon=/tmp/foo.png\n\n[Settings]\nSortOrder=name\n");
        let result = strip_icon_entry(conf).expect("file should be kept");
        // Icon-only Desktop Entry section is dropped...
        assert!(result.section(Some("Desktop Entry")).is_none());
        // ...but unrelated sections survive.
        assert_eq!(
            result.section(Some("Settings")).unwrap().get("SortOrder"),
            Some("name")
        );
    }

    #[test]
    fn strip_icon_entry_deletes_file_when_empty() {
        assert!(strip_icon_entry(ini::Ini::new()).is_none());
    }

    #[test]
    fn strip_icon_entry_is_idempotent_when_no_icon() {
        let conf = ini_of("[Desktop Entry]\nName=Docs\n");
        let result = strip_icon_entry(conf).expect("file should be kept");
        assert_eq!(
            result.section(Some("Desktop Entry")).unwrap().get("Name"),
            Some("Docs")
        );
    }
}
