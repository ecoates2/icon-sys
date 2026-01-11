use thiserror::Error;

#[derive(Debug, Error)]
pub enum IconError {
    #[error("icon set error: {0}")]
    IconSet(String),

    #[error("icon image error: {0}")]
    IconImage(String),
}
