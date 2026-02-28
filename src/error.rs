//! Error types for httpress.
//!
//! This module defines the error types used throughout the library.
//! The main error type is [`enum@Error`], with a type alias [`Result<T>`]
//! for convenience.
//!
//! # Examples
//!
//! ```
//! use httpress::{Error, Result};
//!
//! fn validate_url(url: &str) -> Result<()> {
//!     if !url.starts_with("http") {
//!         return Err(Error::InvalidUrl(url.to_string()));
//!     }
//!     Ok(())
//! }
//! ```

use thiserror::Error;

/// Main error type for httpress operations.
///
/// This error type covers configuration errors, HTTP errors, and I/O errors
/// that can occur during benchmark setup and execution.
#[derive(Debug, Error)]
pub enum Error {
    /// Invalid duration format provided.
    ///
    /// Durations must be specified with a suffix: "10s" (seconds), "1m" (minutes), or "500ms" (milliseconds).
    #[error("Invalid duration: '{0}'. Use format like 10s, 1m, 500ms")]
    InvalidDuration(String),

    /// Invalid header format provided.
    ///
    /// Headers must be in the format "Key: Value" with a colon separator.
    #[error("Invalid header: '{0}'. Use format 'Key: Value'")]
    InvalidHeader(String),

    /// Invalid URL provided.
    ///
    /// The URL must be a valid HTTP or HTTPS URL.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// URL was not provided when required.
    ///
    /// Either `url()` or `request_fn()` must be called on the builder.
    #[error("URL is required")]
    MissingUrl,

    /// Invalid benchmark configuration.
    ///
    /// This error occurs when conflicting options are specified, such as:
    /// - Using both `url()` and `request_fn()`
    /// - Using both `rate()` and `rate_fn()`
    /// - Using `method()`, `header()`, or `body()` with `request_fn()`
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// HTTP client or request error.
    ///
    /// This wraps errors from the underlying HTTP client (hyper).
    #[error("HTTP error: {0}")]
    Http(Box<dyn std::error::Error + Send + Sync>),

    /// Request timeout occurred.
    ///
    /// A request exceeded the configured timeout duration.
    #[error("Request timeout")]
    Timeout,

    /// I/O error occurred.
    ///
    /// This wraps standard I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for httpress operations.
///
/// This is a convenience alias for `Result<T, Error>` used throughout the library.
///
/// # Examples
///
/// ```
/// use httpress::Result;
///
/// fn my_function() -> Result<()> {
///     // Your code here
///     Ok(())
/// }
/// ```
pub type Result<T> = std::result::Result<T, Error>;
