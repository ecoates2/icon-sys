#![doc = include_str!("../README.MD")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod api;
pub mod error;
pub mod icon;

pub use crate::api::*;
pub use crate::error::*;
pub use crate::icon::*;

#[cfg(feature = "folder-settings")]
pub mod folder_settings;
