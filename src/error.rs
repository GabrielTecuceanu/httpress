use thiserror::Error;

/// Main error type for httpress
#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid duration: '{0}'. Use format like 10s, 1m, 500ms")]
    InvalidDuration(String),

    #[error("Invalid header: '{0}'. Use format 'Key: Value'")]
    InvalidHeader(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Request timeout")]
    Timeout,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for httpress
pub type Result<T> = std::result::Result<T, Error>;
