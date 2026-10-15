# icon-sys

[![CI](https://github.com/ecoates2/icon-sys/actions/workflows/ci.yml/badge.svg)](https://github.com/ecoates2/icon-sys/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/ecoates2/icon-sys/graph/badge.svg)](https://codecov.io/gh/ecoates2/icon-sys)
[![crates.io](https://img.shields.io/crates/v/icon-sys.svg)](https://crates.io/crates/icon-sys)
[![docs.rs](https://img.shields.io/docsrs/icon-sys)](https://docs.rs/icon-sys)
[![license](https://img.shields.io/crates/l/icon-sys.svg)](https://github.com/ecoates2/icon-sys#license)

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

`WindowsFolderSettingsProvider` sets and resets custom folder icons. Setting an icon writes the icon set to a
hidden, system-attributed `{prefix}-{uuid}.ico` inside the folder and points the folder's `desktop.ini` at it
(via `SHGetSetFolderCustomSettings`). A fresh UUID is used each time so Explorer's icon cache picks up the change.
Resetting removes the icon entry from `desktop.ini` (deleting the file if it becomes empty) and removes any
previously generated `.ico` files matching the prefix. Other `desktop.ini` settings are left alone.

```rust,ignore
#[cfg(windows)]
use icon_sys::folder_settings::sys::windows::{
    WindowsFolderSettingsProvider, WindowsFolderSettingsProviderExt,
};

// (block_known_folders, generated_icon_prefix)
let provider = WindowsFolderSettingsProvider::new_windows(true, None);

provider.set_icon_for_folder_windows(r"C:\path\to\folder", &windows_icon_set).unwrap();
provider.reset_icon_for_folder_windows(r"C:\path\to\folder").unwrap();
```

`FolderSettingsProvider::new()` is equivalent to `new_windows(true, None)`. When `generated_icon_prefix` is
`None`, the crate name (`icon-sys`) is used.

#### Known folder protection

Windows [known folders](https://learn.microsoft.com/en-us/windows/win32/shell/known-folders) are shell-managed
locations such as Desktop, Documents, Downloads, Pictures, and Music. Their `desktop.ini` files hold
localized names and default icons. Overwriting them can break how Explorer displays these folders.

With `block_known_folders` set to `true` (the default), the provider uses the COM `IKnownFolderManager`
to check each path before writing or resetting. If the path is a known folder, the operation fails with
a `"Folder is a known folder"` error. Notes:

- Only exact matches are blocked. Subfolders such as `Documents\Projects` can still be customized.
- The check is best-effort. If COM can't be initialized on the calling thread (for example, it is
  already in a multithreaded apartment), the provider silently skips the check.
- Pass `false` to turn off the check and skip COM initialization entirely.

## Usage (Linux)

Linux folder icons have no single mechanism, so a backend is selected based on the
desktop environment. `LinuxBackend::Auto` detects it from `XDG_CURRENT_DESKTOP`:

- **GNOME / Cinnamon / MATE / Budgie:** GVFS `metadata::custom-icon` (via `gio`).
- **KDE / XFCE / LXQt:** a freedesktop `.directory` file (settings preserved).

```rust,ignore
use icon_sys::folder_settings::sys::linux::{
    LinuxBackend, LinuxFolderSettingsProvider, LinuxFolderSettingsProviderExt,
};

// (backend, generated_icon_prefix)
let provider = LinuxFolderSettingsProvider::new_linux(LinuxBackend::Auto, None);
```

The default folder icon is read from the active icon theme (with `hicolor` fallback)
as raster sizes plus an optional, `usvg`-validated scalable SVG.

## Platform Support
- **Windows:** folder-settings
- **Linux:** folder-settings (supports GNOME/KDE/XFCE)
- **macOS:** Not supported yet- compiles but throws unimplemented

## License
MIT OR Apache-2.0

## Resources
- [**Rust Skills**](https://github.com/leonardomso/rust-skills) — 265 Rust rules across 26 categories for AI coding agents (ownership, error handling, async, concurrency, performance, anti-patterns, etc.)
- [**Awesome Copilot**](https://github.com/github/awesome-copilot) — Community-curated collection of Copilot customizations: agents, skills, instructions, hooks, workflows, and plugins

## Status
**Work in progress.** API is subject to change.
