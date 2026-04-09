//! Arios is a small learning-focused HTTP client crate.
//!
//! It currently supports `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, and `OPTIONS`
//! requests over HTTP and HTTPS, plus basic response metadata parsing.
//! HTTPS connections are handled with `rustls` and native platform root certificates.

pub mod client;
pub mod error;
pub mod response;
pub mod transport;

pub use client::{Arios, ContentType};
pub use error::{AriosError, AriosResult};
pub use response::AriosResponse;
