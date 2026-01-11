use core::slice;

use super::WindowsFolderSettingsError;
use crate::folder_settings::DefaultFolderIconProvider;
use crate::icon::sys::windows::{WindowsIconImage, WindowsIconSet, WindowsIconSize};

use std::borrow::Cow;

use core::ffi::c_void;
use image::RgbaImage;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, GetDIBits,
    GetObjectW,
};
use windows::Win32::System::LibraryLoader::FindResourceW;
use windows::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW, LoadResource, LockResource, SizeofResource,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, GetIconInfo, HICON, ICONINFO, LR_DEFAULTCOLOR, RT_GROUP_ICON, RT_ICON,
};
use windows::core::{HSTRING, PCWSTR};

// Rust implementation of the MAKEINTRESOURCE() macro in the Win32 API
macro_rules! make_int_resource_w {
    ($id:expr) => {{
        let id: u16 = $id;
        PCWSTR(id as usize as *mut u16)
    }};
}

const SHELL_32_DLL: &str = "shell32.dll";

// The default folder icon resource in shell32.dll
const FOLDER_ICON_RESOURCE: PCWSTR = make_int_resource_w!(4);

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

fn load_icon_set_from_shell32<'a>() -> Result<WindowsIconSet<'a>, WindowsFolderSettingsError> {
    // Load shell32.dll into the program's address space as an image resource.
    let shell32_hmod = load_shell32_dll()?;

    // Get a list containing each individual size of the system folder icon as resource
    // metadata.
    let icon_directory = get_icon_directory(shell32_hmod)?;

    let mut icons: Vec<WindowsIconImage> = Vec::with_capacity(WindowsIconSize::NUM_SIZES);

    // Create a memory device context (DC) compatible with the screen, for drawing each bitmap.
    let hdc_screen = {
        let mut hdc_screen = unsafe { CreateCompatibleDC(None) };
        hdc_screen = (!hdc_screen.is_invalid())
            .then_some(hdc_screen)
            .ok_or_else(windows::core::Error::from_win32)?;
        hdc_screen
    };

    for item in icon_directory {
        let h_icon = load_specific_icon(shell32_hmod, item.n_id)?;
        let mut icon_info = ICONINFO::default();
        unsafe { GetIconInfo(h_icon, &mut icon_info) }?;

        let mut bmp = BITMAP::default();

        let get_obj_bytes_written = unsafe {
            GetObjectW(
                icon_info.hbmColor,
                std::mem::size_of::<BITMAP>().try_into().unwrap(),
                Some(&mut bmp as *mut _ as *mut c_void),
            )
        };
        if get_obj_bytes_written == 0 {
            return Err(WindowsFolderSettingsError::Win32(
                windows::core::Error::from_win32(),
            ));
        }

        // Create the BITMAPINFO structure
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bmp.bmWidth,
                biHeight: -bmp.bmHeight, // Negative to indicate top-down bitmap
                biPlanes: 1,
                biBitCount: 32, // 32 bits-per-pixel (RGBA)
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pixels: Vec<u8> = vec![0; (bmp.bmWidth * bmp.bmHeight * 4) as usize];

        let lines_copied = unsafe {
            GetDIBits(
                hdc_screen,
                icon_info.hbmColor,
                0,
                bmp.bmHeight as u32,
                Some(pixels.as_mut_ptr() as *mut c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };

        if lines_copied == 0 {
            return Err(WindowsFolderSettingsError::Win32(
                windows::core::Error::from_win32(),
            ));
        }

        // Windows returns BGRA. Need to swap channels to RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            let (b, g, r, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            chunk[0] = r; // Red
            chunk[1] = g; // Green
            chunk[2] = b; // Blue
            chunk[3] = a; // Alpha
        }

        // Convert pixel buffer to image::RgbaImage
        let rgba_image =
            RgbaImage::from_raw(bmp.bmWidth as u32, bmp.bmHeight as u32, pixels).unwrap();
        let dyn_image = image::DynamicImage::ImageRgba8(rgba_image);
        let size = WindowsIconSize::from_dimension(bmp.bmWidth as u32).ok_or_else(|| {
            WindowsFolderSettingsError::ProviderError(format!("Unknown icon size: {}", bmp.bmWidth))
        })?;
        icons.push(WindowsIconImage {
            size,
            image: Cow::Owned(dyn_image),
        });
    }

    // Clean up
    let delete_dc = unsafe { windows::Win32::Graphics::Gdi::DeleteDC(hdc_screen) }.as_bool();
    if !delete_dc {
        return Err(WindowsFolderSettingsError::Win32(
            windows::core::Error::from_win32(),
        ));
    }

    // Create the struct here...

    let icon_set = WindowsIconSet::from_icons(icons)?;

    Ok(icon_set)
}

fn load_shell32_dll() -> Result<HMODULE, windows::core::Error> {
    let handle = unsafe {
        LoadLibraryExW(
            &HSTRING::from(SHELL_32_DLL),
            None,
            LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }?;

    Ok(handle)
}

// Returns a Vec of icon metadata
fn get_icon_directory<'a>(
    h_mod: HMODULE,
) -> Result<Vec<&'a GrpIconDirEntry>, WindowsFolderSettingsError> {
    // Find and load icon group resource data
    let h_rsrc = {
        let mut h_rsrc = unsafe { FindResourceW(h_mod, FOLDER_ICON_RESOURCE, RT_GROUP_ICON) };
        h_rsrc = (!h_rsrc.is_invalid())
            .then_some(h_rsrc)
            .ok_or_else(windows::core::Error::from_win32)?;
        h_rsrc
    };

    let h_global = unsafe { LoadResource(h_mod, h_rsrc) }?;

    let raw_res_ptr = {
        let mut raw_res_ptr = unsafe { LockResource(h_global) };
        raw_res_ptr = (!raw_res_ptr.is_null())
            .then_some(raw_res_ptr)
            .ok_or_else(windows::core::Error::from_win32)?;
        raw_res_ptr
    };

    // Cast raw icon group resource data into a parseable format
    let grp_icon_dir_ptr = raw_res_ptr as *const GrpIconDir;

    // Grab reference for convenience
    let grp_icon_dir = unsafe { &*grp_icon_dir_ptr };

    // GrpIconDir provides the entries as flexible array members. We must get them using
    // pointer arithmetic...
    let first_entry_ptr = grp_icon_dir.id_entries.as_ptr();

    let count = grp_icon_dir.id_count as usize;

    let mut icon_dir = Vec::with_capacity(count);

    for i in 0..count {
        let entry_ptr = unsafe { first_entry_ptr.add(i) };

        icon_dir.push(unsafe { &*entry_ptr });
    }

    Ok(icon_dir)
}

fn load_specific_icon(h_mod: HMODULE, id: u16) -> Result<HICON, WindowsFolderSettingsError> {
    let h_rsrc = {
        let mut h_rsrc = unsafe { FindResourceW(h_mod, make_int_resource_w!(id), RT_ICON) };
        h_rsrc = (!h_rsrc.is_invalid())
            .then_some(h_rsrc)
            .ok_or_else(windows::core::Error::from_win32)?;
        h_rsrc
    };

    let h_global = unsafe { LoadResource(h_mod, h_rsrc) }?;

    let lp_data = {
        let mut lp_data = unsafe { LockResource(h_global) };
        lp_data = (!lp_data.is_null())
            .then_some(lp_data)
            .ok_or_else(windows::core::Error::from_win32)?;
        lp_data
    };

    let dw_size = {
        let mut dw_size = unsafe { SizeofResource(h_mod, h_rsrc) };
        dw_size = (dw_size != 0)
            .then_some(dw_size)
            .ok_or_else(windows::core::Error::from_win32)?;
        dw_size
    };

    let lp_data_as_byte_slice =
        unsafe { slice::from_raw_parts(lp_data as *const u8, dw_size as usize) };

    // dw_ver is set to a magical constant...
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

    Ok(h_icon)
}
