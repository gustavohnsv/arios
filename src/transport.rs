//! Transport helpers for plain TCP and TLS-backed connections.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use crate::{AriosError, AriosResult};

/// Shared stream abstraction used by the HTTP client transport layer.
pub trait HttpStream: Read + Write {}

impl<T: Read + Write> HttpStream for T {}

static TLS_CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();

fn load_root_cert_store() -> AriosResult<RootCertStore> {
    let native_certs = rustls_native_certs::load_native_certs();
    let mut roots = RootCertStore::empty();

    for cert in native_certs.certs {
        roots
            .add(cert)
            .map_err(|error| AriosError::Io(std::io::Error::other(error.to_string())))?;
    }

    if roots.is_empty() {
        let message = native_certs
            .errors
            .first()
            .map(ToString::to_string)
            .unwrap_or_else(|| String::from("no native TLS certificates were loaded"));
        return Err(AriosError::Io(std::io::Error::other(message)));
    }

    Ok(roots)
}

fn server_name(host: &str) -> AriosResult<ServerName<'static>> {
    ServerName::try_from(host.to_owned())
        .map_err(|_| AriosError::InvalidRequest("invalid TLS server name"))
}

fn tls_config() -> AriosResult<Arc<ClientConfig>> {
    if let Some(config) = TLS_CONFIG.get() {
        return Ok(Arc::clone(config));
    }

    let config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(load_root_cert_store()?)
            .with_no_client_auth(),
    );

    let _ = TLS_CONFIG.set(Arc::clone(&config));
    Ok(config)
}

fn connect_http(addr: &str) -> AriosResult<Box<dyn HttpStream>> {
    Ok(Box::new(TcpStream::connect(addr)?))
}

fn connect_https(addr: &str, host: &str) -> AriosResult<Box<dyn HttpStream>> {
    let tcp = TcpStream::connect(addr)?;
    let connection = ClientConnection::new(tls_config()?, server_name(host)?)
        .map_err(|error| AriosError::Io(std::io::Error::other(error.to_string())))?;

    Ok(Box::new(StreamOwned::new(connection, tcp)))
}

/// Opens a TCP stream for HTTP or wraps it with `rustls` for HTTPS.
///
/// When `use_tls` is `true`, Arios loads native platform root certificates and
/// reuses a cached `rustls::ClientConfig` for subsequent HTTPS connections.
pub fn connect_stream(addr: &str, host: &str, use_tls: bool) -> AriosResult<Box<dyn HttpStream>> {
    if use_tls {
        connect_https(addr, host)
    } else {
        connect_http(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_name_accepts_valid_dns_name() {
        assert!(server_name("example.com").is_ok());
    }

    #[test]
    fn server_name_rejects_invalid_dns_name() {
        let err = server_name("bad host").unwrap_err();
        assert!(matches!(
            err,
            AriosError::InvalidRequest("invalid TLS server name")
        ));
    }

    #[test]
    fn connect_stream_without_tls_propagates_tcp_errors() {
        let result = connect_stream("127.0.0.1:0", "127.0.0.1", false);
        assert!(matches!(result, Err(AriosError::Io(_))));
    }
}
