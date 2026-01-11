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

```rust
#[cfg(windows)]
use icon_sys::folder_settings::sys::windows::{WindowsDefaultFolderIconProvider, WindowsDefaultFolderIconProviderExt};

let default_folder_icon_provider = WindowsDefaultFolderIconProvider;

// Windows icon set; has logic for validating windows-specific image sizes, etc.
let windows_icon_set = default_folder_icon_provider.dump_default_folder_icon_windows().unwrap();
```

## Platform Support
- **Windows:** Full support
- **Other platforms:** Planned support

## Development
- Run integration tests: `cargo test`

## License
MIT OR Apache-2.0

## Status
**Work in progress.** Not yet published to crates.io. API may change. Only Windows is implemented for now.
