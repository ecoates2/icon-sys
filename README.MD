# icon-sys

Cross-platform Rust library for reading, writing, and managing system shell icons.

## Features
- `folder-settings`: Provides functionality for dumping the default folder icon and setting/resetting icons for directories.

## Usage (Windows)

Each system's set of functionality is exported under the common aliases `PlatformDefaultFolderIconProvider` and `PlatformFolderSettingsProvider`.
These structs implement the traits `DefaultFolderIconProvider` and `FolderSettingsProvider` respectively and use the platform-agnostic `Icon` + `IconSet` APIs.
This is useful if you don't want to bother with conditional compilation.

```rust
use icon_sys::folder_settings::{PlatformDefaultFolderIconProvider, DefaultFolderIconProvider};

let default_folder_icon_provider = PlatformDefaultFolderIconProvider;

// Platform-agnostic icon set; just contains a collection of images
let icon_set = default_folder_icon_provider.dump_default_folder_icon().unwrap();
```

Alternatively, you can import the `sys` module and work with platform-specific code directly.

```rust,ignore
#[cfg(windows)]
use icon_sys::folder_settings::sys::windows::{WindowsDefaultFolderIconProvider, WindowsDefaultFolderIconProviderExt};

let default_folder_icon_provider = WindowsDefaultFolderIconProvider;

// Windows icon set; has logic for validating windows-specific image sizes, etc.
let windows_icon_set = default_folder_icon_provider.dump_default_folder_icon_windows().unwrap();
```

## Usage (Linux)

Linux folder icons have no single mechanism, so a backend is selected based on the
desktop environment. `LinuxBackend::Auto` detects it from `XDG_CURRENT_DESKTOP`:

- **GNOME / Cinnamon / MATE / Budgie:** GVFS `metadata::custom-icon` (via `gio`).
- **KDE / XFCE / LXQt:** a freedesktop `.directory` file (settings preserved).

```rust,ignore
use icon_sys::folder_settings::sys::linux::{
    LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
};

// (backend, generated_icon_prefix, bump_mtime)
let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::Auto, None, true);
```

The default folder icon is read from the active icon theme (with `hicolor` fallback)
as raster sizes plus an optional, `usvg`-validated scalable SVG. Setting a single
large image is sufficient — file managers scale it for all zoom levels.

## Platform Support
- **Windows:** Full support
- **Linux:** Folder settings (GNOME/KDE/XFCE backends) and default icon dump
- **macOS:** Planned support

## Development
- Run integration tests: `cargo test`

## License
MIT OR Apache-2.0

## Status
**Work in progress.** Not yet published to crates.io. API may change. Windows and Linux are implemented; macOS is planned.
