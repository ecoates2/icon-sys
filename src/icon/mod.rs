pub mod sys {
    #[cfg(target_os = "windows")]
    pub mod windows;

    #[cfg(target_os = "macos")]
    pub mod macos;

    #[cfg(target_os = "linux")]
    pub mod linux;
}

pub mod error;
pub use error::IconError;
