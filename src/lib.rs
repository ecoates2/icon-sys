#![doc = include_str!("../README.md")]

pub mod api;
pub mod error;
pub mod icon;

pub use crate::api::*;
pub use crate::error::*;
pub use crate::icon::*;

#[cfg(feature = "folder-settings")]
pub mod folder_settings;
