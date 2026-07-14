use core::ffi::c_void;
use core::slice;
use std::borrow::Cow;

use image::{DynamicImage, RgbaImage};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HDC, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::{
    FindResourceW, LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW, LoadResource, LockResource,
    SizeofResource,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, DestroyIcon, GetIconInfo, HICON, ICONINFO, LR_DEFAULTCOLOR,
    RT_GROUP_ICON, RT_ICON,
};
use windows::core::{HSTRING, PCWSTR};

use super::WindowsFolderSettingsError;
use crate::folder_settings::DefaultFolderIconProvider;
use crate::icon::sys::windows::{WindowsIconImage, WindowsIconSet, WindowsIconSize};

/// Rust implementation of the Win32 `MAKEINTRESOURCEW` macro: packs a numeric
/// resource identifier into a `PCWSTR` sentinel that the resource APIs treat as
/// an ID rather than a pointer to a wide string.
#[allow(non_snake_case)]
fn MAKEINTRESOURCEW(id: i32) -> PCWSTR {
    unsafe { std::mem::transmute::<usize, PCWSTR>(id as usize) }
}

const SHELL_32_DLL: &str = "shell32.dll";

// The default folder icon resource id in shell32.dll.
const FOLDER_ICON_RESOURCE_ID: i32 = 4;

// Struct definitions for parsing group icon directory resources in the
// Win32 API.
// See https://devblogs.microsoft.com/oldnewthing/20120720-00/?p=7083
#[repr(C, packed(2))]
struct GrpIconDirEntry {
    b_width: u8,          // Width, in pixels, of the image
    b_height: u8,         // Height, in pixels, of the image
    b_color_count: u8,    // Number of colors in image (0 if >=8bpp)
    b_reserved: u8,       // Reserved
    w_planes: u16,        // Color Planes
    w_bit_count: u16,     // Bits per pixel
    dw_bytes_in_res: u32, // How many bytes in this resource?
    n_id: u16,            // The ID
}

#[repr(C, packed(2))]
struct GrpIconDir {
    id_reserved: u16,                 // Reserved (must be 0)
    id_type: u16,                     // Resource type (1 for icons)
    id_count: u16,                    // How many images?
    id_entries: [GrpIconDirEntry; 1], // Array of entries for each image
}

pub trait WindowsDefaultFolderIconProviderExt {
    /// Dump the default folder icon
    fn dump_default_folder_icon_windows(
        &self,
    ) -> Result<WindowsIconSet<'_>, WindowsFolderSettingsError>;
}

/// Provides default system folder icon operations
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsDefaultFolderIconProvider;

impl WindowsDefaultFolderIconProviderExt for WindowsDefaultFolderIconProvider {
    fn dump_default_folder_icon_windows(
        &self,
    ) -> Result<WindowsIconSet<'_>, WindowsFolderSettingsError> {
        load_icon_set_from_shell32()
    }
}

impl DefaultFolderIconProvider for WindowsDefaultFolderIconProvider {
    fn dump_default_folder_icon(
        &self,
    ) -> Result<crate::api::IconSet, crate::folder_settings::FolderSettingsError> {
        let windows_icon_set = load_icon_set_from_shell32()?;
        Ok(crate::api::IconSet::from(windows_icon_set))
    }
}

/// RAII wrapper that deletes a GDI memory device context on drop.
struct OwnedDc(HDC);

impl OwnedDc {
    /// Creates a memory DC compatible with the screen.
    fn create_compatible() -> Result<Self, WindowsFolderSettingsError> {
        let hdc = unsafe { CreateCompatibleDC(None) };
        if hdc.is_invalid() {
            return Err(WindowsFolderSettingsError::Win32(
                windows::core::Error::from_thread(),
            ));
        }
        Ok(Self(hdc))
    }

    /// Returns the underlying device context handle.
    fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for OwnedDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

/// RAII wrapper that destroys an icon handle on drop.
struct OwnedIcon(HICON);

impl OwnedIcon {
    /// Returns the underlying icon handle.
    fn handle(&self) -> HICON {
        self.0
    }
}

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.0);
        }
    }
}

/// RAII wrapper that deletes a GDI object (such as a bitmap) on drop.
struct OwnedGdiObject(HGDIOBJ);

impl Drop for OwnedGdiObject {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0);
        }
    }
}

/// Loads the default folder icon set from `shell32.dll`.
fn load_icon_set_from_shell32<'a>() -> Result<WindowsIconSet<'a>, WindowsFolderSettingsError> {
    let shell32 = load_module(SHELL_32_DLL)?;
    load_icon_group(shell32, MAKEINTRESOURCEW(FOLDER_ICON_RESOURCE_ID))
}

/// Loads every size of an icon group resource from a module into a
/// [`WindowsIconSet`].
fn load_icon_group<'a>(
    module: HMODULE,
    group_resource: PCWSTR,
) -> Result<WindowsIconSet<'a>, WindowsFolderSettingsError> {
    // Metadata describing each individual size present in the group.
    let icon_directory = get_icon_directory(module, group_resource)?;

    // A memory DC compatible with the screen, used to draw each icon's bitmap.
    let dc = OwnedDc::create_compatible()?;

    let mut icons: Vec<WindowsIconImage> = Vec::with_capacity(icon_directory.len());

    for entry in icon_directory {
        let icon = load_specific_icon(module, entry.n_id)?;
        let (dimension, image) = icon_to_rgba_image(dc.handle(), &icon)?;

        let size = WindowsIconSize::from_dimension(dimension).ok_or_else(|| {
            WindowsFolderSettingsError::ProviderError(format!("Unknown icon size: {dimension}"))
        })?;

        icons.push(WindowsIconImage {
            size,
            image: Cow::Owned(image),
        });
    }

    Ok(WindowsIconSet::from_icons(icons)?)
}

/// Loads a module into the process address space purely as an image resource.
fn load_module(name: &str) -> Result<HMODULE, windows::core::Error> {
    unsafe { LoadLibraryExW(&HSTRING::from(name), None, LOAD_LIBRARY_AS_IMAGE_RESOURCE) }
}

/// Renders a single icon to a top-down RGBA image using the provided memory DC.
///
/// Returns the square pixel dimension alongside the decoded image. All interim
/// GDI objects (the color and mask bitmaps produced by `GetIconInfo`) are
/// released via RAII guards before returning, even on error paths.
fn icon_to_rgba_image(
    hdc: HDC,
    icon: &OwnedIcon,
) -> Result<(u32, DynamicImage), WindowsFolderSettingsError> {
    let mut icon_info = ICONINFO::default();
    unsafe { GetIconInfo(icon.handle(), &mut icon_info) }?;

    // GetIconInfo transfers ownership of the color and mask bitmaps to us; own
    // them so they are always freed regardless of how this function returns.
    let _color_bmp = OwnedGdiObject(icon_info.hbmColor.into());
    let _mask_bmp = OwnedGdiObject(icon_info.hbmMask.into());

    let mut bmp = BITMAP::default();
    let bytes_written = unsafe {
        GetObjectW(
            icon_info.hbmColor.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut _ as *mut c_void),
        )
    };
    if bytes_written == 0 {
        return Err(WindowsFolderSettingsError::Win32(
            windows::core::Error::from_thread(),
        ));
    }

    let (width, height) = (bmp.bmWidth, bmp.bmHeight);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // Negative to request a top-down bitmap.
            biPlanes: 1,
            biBitCount: 32, // 32 bits-per-pixel; GDI returns these as BGRA.
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    let lines_copied = unsafe {
        GetDIBits(
            hdc,
            icon_info.hbmColor,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    };
    if lines_copied == 0 {
        return Err(WindowsFolderSettingsError::Win32(
            windows::core::Error::from_thread(),
        ));
    }

    // GDI returns BGRA; convert in place to the RGBA order `image` expects.
    swap_bgra_to_rgba(&mut pixels);

    let rgba = RgbaImage::from_raw(width as u32, height as u32, pixels).ok_or_else(|| {
        WindowsFolderSettingsError::ProviderError(
            "pixel buffer size did not match icon dimensions".to_string(),
        )
    })?;

    Ok((width as u32, DynamicImage::ImageRgba8(rgba)))
}

/// Converts a 32-bit BGRA pixel buffer (as produced by `GetDIBits`) to RGBA in
/// place by swapping the red and blue channels of each pixel.
///
/// Any trailing bytes that do not form a complete 4-byte pixel are left
/// untouched.
fn swap_bgra_to_rgba(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

/// Returns metadata for each size contained in an icon group resource.
fn get_icon_directory<'a>(
    module: HMODULE,
    group_resource: PCWSTR,
) -> Result<Vec<&'a GrpIconDirEntry>, WindowsFolderSettingsError> {
    // Find and load the icon group resource data.
    let h_rsrc = {
        let mut h_rsrc = unsafe { FindResourceW(Some(module), group_resource, RT_GROUP_ICON) };
        h_rsrc = (!h_rsrc.is_invalid())
            .then_some(h_rsrc)
            .ok_or_else(windows::core::Error::from_thread)?;
        h_rsrc
    };

    let h_global = unsafe { LoadResource(Some(module), h_rsrc) }?;

    let raw_res_ptr = {
        let mut raw_res_ptr = unsafe { LockResource(h_global) };
        raw_res_ptr = (!raw_res_ptr.is_null())
            .then_some(raw_res_ptr)
            .ok_or_else(windows::core::Error::from_thread)?;
        raw_res_ptr
    };

    // Reinterpret the raw resource bytes as a parseable group icon directory.
    let grp_icon_dir = unsafe { &*(raw_res_ptr as *const GrpIconDir) };

    // The entries are a flexible array member, so walk them with pointer
    // arithmetic starting from the first entry.
    let first_entry_ptr = grp_icon_dir.id_entries.as_ptr();
    let count = grp_icon_dir.id_count as usize;

    let mut icon_dir = Vec::with_capacity(count);
    for i in 0..count {
        let entry_ptr = unsafe { first_entry_ptr.add(i) };
        icon_dir.push(unsafe { &*entry_ptr });
    }

    Ok(icon_dir)
}

/// Loads a single icon image resource by ID and wraps it in an owning handle.
fn load_specific_icon(module: HMODULE, id: u16) -> Result<OwnedIcon, WindowsFolderSettingsError> {
    let h_rsrc = {
        let mut h_rsrc =
            unsafe { FindResourceW(Some(module), MAKEINTRESOURCEW(id as i32), RT_ICON) };
        h_rsrc = (!h_rsrc.is_invalid())
            .then_some(h_rsrc)
            .ok_or_else(windows::core::Error::from_thread)?;
        h_rsrc
    };

    let h_global = unsafe { LoadResource(Some(module), h_rsrc) }?;

    let lp_data = {
        let mut lp_data = unsafe { LockResource(h_global) };
        lp_data = (!lp_data.is_null())
            .then_some(lp_data)
            .ok_or_else(windows::core::Error::from_thread)?;
        lp_data
    };

    let dw_size = {
        let mut dw_size = unsafe { SizeofResource(Some(module), h_rsrc) };
        dw_size = (dw_size != 0)
            .then_some(dw_size)
            .ok_or_else(windows::core::Error::from_thread)?;
        dw_size
    };

    let lp_data_as_byte_slice =
        unsafe { slice::from_raw_parts(lp_data as *const u8, dw_size as usize) };

    // dw_ver is set to a magic constant required by the API.
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createiconfromresourceex
    let h_icon = unsafe {
        CreateIconFromResourceEx(
            lp_data_as_byte_slice,
            true,
            0x00030000,
            0,
            0,
            LR_DEFAULTCOLOR,
        )
    }?;

    Ok(OwnedIcon(h_icon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_bgra_to_rgba_swaps_red_and_blue_channels() {
        let mut pixels = vec![1u8, 2, 3, 4, 10, 20, 30, 40];
        swap_bgra_to_rgba(&mut pixels);
        // Bytes 0 and 2 swap; green (index 1) and alpha (index 3) are untouched.
        assert_eq!(pixels, vec![3, 2, 1, 4, 30, 20, 10, 40]);
    }

    #[test]
    fn swap_bgra_to_rgba_is_its_own_inverse() {
        let original = vec![9u8, 8, 7, 6, 5, 4, 3, 2];
        let mut pixels = original.clone();
        swap_bgra_to_rgba(&mut pixels);
        swap_bgra_to_rgba(&mut pixels);
        assert_eq!(pixels, original);
    }

    #[test]
    fn swap_bgra_to_rgba_leaves_trailing_partial_pixel_untouched() {
        let mut pixels = vec![1u8, 2, 3, 4, 99];
        swap_bgra_to_rgba(&mut pixels);
        assert_eq!(pixels, vec![3, 2, 1, 4, 99]);
    }

    #[test]
    fn folder_icon_resource_id_is_four() {
        assert_eq!(FOLDER_ICON_RESOURCE_ID, 4);
    }

    #[test]
    fn makeintresourcew_encodes_id_in_the_pointer() {
        let resource = MAKEINTRESOURCEW(42);
        assert_eq!(resource.0 as usize, 42);
    }
}
