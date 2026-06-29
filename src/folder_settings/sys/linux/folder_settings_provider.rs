use std::{
    fs,
    path::Path,
    process::Command,
};

use uuid::Uuid;

use super::LinuxFolderSettingsError;
use crate::folder_settings::FolderSettingsProvider;
use crate::folder_settings::error::Result;
use crate::icon::sys::linux::LinuxIconSet;

const DEFAULT_GENERATED_ICON_PREFIX: &str = env!("CARGO_PKG_NAME");

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
    /// Freedesktop `.directory` file (KDE, XFCE) — not yet implemented.
    DirectoryFile,
}

impl LinuxBackend {
    /// Resolve `Auto` into a concrete backend using `XDG_CURRENT_DESKTOP`.
    fn resolve(self) -> std::result::Result<Self, LinuxFolderSettingsError> {
        match self {
            LinuxBackend::Auto => {
                let desktop = std::env::var("XDG_CURRENT_DESKTOP")
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if desktop.is_empty() {
                    return Err(LinuxFolderSettingsError::UndetectedDesktop);
                }
                if ["gnome", "cinnamon", "mate", "budgie", "unity"]
                    .iter()
                    .any(|de| desktop.contains(de))
                {
                    Ok(LinuxBackend::GioMetadata)
                } else if ["kde", "xfce", "lxqt"].iter().any(|de| desktop.contains(de)) {
                    Ok(LinuxBackend::DirectoryFile)
                } else {
                    Err(LinuxFolderSettingsError::UndetectedDesktop)
                }
            }
            other => Ok(other),
        }
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

pub struct LinuxFolderSettingsProvider {
    backend: LinuxBackend,
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

        Self {
            backend,
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

        match self.backend.resolve()? {
            LinuxBackend::GioMetadata => self.set_via_gio_metadata(&path, icon_set)?,
            LinuxBackend::DirectoryFile => self.set_via_directory_file(&path, icon_set)?,
            LinuxBackend::Auto => unreachable!("resolve() never returns Auto"),
        }

        self.maybe_bump_mtime(&path);
        Ok(())
    }

    fn reset_icon_for_folder_linux<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.validate_folder(&path)?;

        match self.backend.resolve()? {
            LinuxBackend::GioMetadata => self.reset_via_gio_metadata(&path)?,
            LinuxBackend::DirectoryFile => self.reset_via_directory_file(&path)?,
            LinuxBackend::Auto => unreachable!("resolve() never returns Auto"),
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

    /// Write the largest raster size as a hidden PNG and point the folder's
    /// `metadata::custom-icon` attribute at it.
    fn set_via_gio_metadata<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &LinuxIconSet,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        self.remove_existing_generated_png(&path)?;

        let largest = icon_set.largest().ok_or_else(|| {
            LinuxFolderSettingsError::IconOperation(
                path.as_ref().to_path_buf(),
                "Icon set contains no raster images".to_string(),
            )
        })?;

        let icon_name = format!("{}-{}.png", self.generated_icon_prefix, Uuid::new_v4());
        let icon_path = path.as_ref().join(&icon_name);
        largest.image.save(&icon_path).map_err(|e| {
            LinuxFolderSettingsError::IconOperation(icon_path.clone(), e.to_string())
        })?;

        let uri = format!("file://{}", icon_path.display());
        gio_set_metadata(&path, "metadata::custom-icon", Some(&uri))?;

        Ok(())
    }

    fn reset_via_gio_metadata<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        gio_set_metadata(&path, "metadata::custom-icon", None)?;
        self.remove_existing_generated_png(&path)?;
        Ok(())
    }

    /// Write the largest raster size and reference it from a `.directory` file
    /// (KDE Dolphin, XFCE Thunar). A single icon is scaled by the file manager.
    fn set_via_directory_file<P: AsRef<Path>>(
        &self,
        path: P,
        icon_set: &LinuxIconSet,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        self.remove_existing_generated_png(&path)?;

        let largest = icon_set.largest().ok_or_else(|| {
            LinuxFolderSettingsError::IconOperation(
                path.as_ref().to_path_buf(),
                "Icon set contains no raster images".to_string(),
            )
        })?;

        let icon_name = format!("{}-{}.png", self.generated_icon_prefix, Uuid::new_v4());
        let icon_path = path.as_ref().join(&icon_name);
        largest.image.save(&icon_path).map_err(|e| {
            LinuxFolderSettingsError::IconOperation(icon_path.clone(), e.to_string())
        })?;

        let directory_path = path.as_ref().join(".directory");
        // Load existing entry if present so other settings are preserved.
        let mut conf = ini::Ini::load_from_file(&directory_path).unwrap_or_default();
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
            let mut conf = ini::Ini::load_from_file(&directory_path).unwrap_or_default();
            // Surgically drop only the Icon key, keeping other settings.
            conf.with_section(Some("Desktop Entry")).delete(&"Icon");

            let section_empty = conf
                .section(Some("Desktop Entry"))
                .map(|s| s.is_empty())
                .unwrap_or(true);
            if section_empty {
                conf.delete(Some("Desktop Entry"));
            }

            // Remove the file only if no section retains any keys; otherwise
            // rewrite it with the remaining settings preserved.
            let has_keys = conf.iter().any(|(_, props)| !props.is_empty());
            if has_keys {
                conf.write_to_file(&directory_path)?;
            } else {
                fs::remove_file(&directory_path)?;
            }
        }
        self.remove_existing_generated_png(&path)?;
        Ok(())
    }

    fn remove_existing_generated_png<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> std::result::Result<(), LinuxFolderSettingsError> {
        for entry in fs::read_dir(directory.as_ref())? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("png")
                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                && name.starts_with(&self.generated_icon_prefix)
            {
                fs::remove_file(p)?;
            }
        }
        Ok(())
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

    let output = cmd
        .output()
        .map_err(|e| LinuxFolderSettingsError::Gio(format!("failed to spawn gio: {e}")))?;

    if !output.status.success() {
        return Err(LinuxFolderSettingsError::Gio(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}
