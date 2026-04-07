use std::{fmt, io};

/// Error type used by the Arios crate.
#[derive(Debug)]
pub enum AriosError {
    /// The provided URL does not start with `http://` or `https://`.
    InvalidUrl,
    /// The request being built is invalid.
    InvalidRequest(&'static str),
    /// The server response could not be parsed correctly.
    InvalidResponse(&'static str),
    /// Wrapper for lower-level I/O errors.
    Io(io::Error),
}

/// Convenience alias for results returned by Arios.
pub type AriosResult<T> = Result<T, AriosError>;

impl fmt::Display for AriosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AriosError::InvalidUrl => write!(f, "invalid URL"),
            AriosError::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            AriosError::InvalidResponse(message) => write!(f, "invalid response: {message}"),
            AriosError::Io(error) => write!(f, "I/O error: {error}"),
        }
    }
}

impl std::error::Error for AriosError {}

impl From<io::Error> for AriosError {
    fn from(error: io::Error) -> Self {
        AriosError::Io(error)
    }
}
