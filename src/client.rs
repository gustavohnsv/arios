use std::io::{BufRead, BufReader, Read, Write};

use tcp_stream::{TLSConfig, TcpStream};

use crate::{AriosError, AriosResponse, AriosResult};

trait HttpStream: Read + Write {}

impl<T: Read + Write> HttpStream for T {}

/// MIME-like content types supported by Arios request and response headers.
pub enum ContentType {
    Json,
    Html,
    Text,
    Png,
    Jpg,
    Jpeg,
    Webp,
}

impl ContentType {
    fn as_header_value(&self) -> &'static str {
        match self {
            ContentType::Json => "application/json",
            ContentType::Html => "text/html",
            ContentType::Text => "text/plain",
            ContentType::Png => "image/png",
            ContentType::Jpg => "image/jpg",
            ContentType::Jpeg => "image/jpeg",
            ContentType::Webp => "image/webp",
        }
    }
}

/// Minimal HTTP client used to send requests to a single base URL.
pub struct Arios {
    base_url: String,
}

impl Arios {
    fn parse_status_line(header: &str) -> AriosResult<(String, u16, String)> {
        let mut parts = header
            .lines()
            .next()
            .ok_or(AriosError::InvalidResponse("missing status line"))?
            .split_whitespace();

        let protocol = parts
            .next()
            .ok_or(AriosError::InvalidResponse("missing protocol"))?
            .to_string();
        let code = parts
            .next()
            .ok_or(AriosError::InvalidResponse("missing status code"))?
            .parse::<u16>()
            .map_err(|_| AriosError::InvalidResponse("invalid status code"))?;
        let status = parts.collect::<Vec<&str>>().join(" ");

        Ok((protocol, code, status))
    }

    fn parse_content_metadata(header: &str) -> (Option<String>, Option<String>) {
        let mut content_type = None;
        let mut charset = None;

        if let Some(line) = header
            .lines()
            .find(|line| line.to_lowercase().contains("content-type"))
        {
            content_type = line
                .split(':')
                .nth(1)
                .map(|s| s.trim())
                .and_then(|s| s.split(';').next())
                .map(|s| s.trim().to_string());
            charset = line
                .split(';')
                .find(|part| part.contains("charset="))
                .and_then(|part| part.split('=').nth(1))
                .map(|s| s.trim().to_string());
        }

        (content_type, charset)
    }

    fn parse_url(url: &str) -> (String, String, u16) {
        let mut port: u16 = 80;
        if url.starts_with("https") {
            port = 443;
        }
        let addr: String = match url.find("://") {
            Some(index) => String::from(&url[index + 3..]),
            None => String::from(url),
        };
        match addr.find("/") {
            Some(index) => {
                let host: &str = &addr[..index];
                let path: &str = &addr[index..];
                if let Some(explicit_port) = host.split(":").nth(1) {
                    port = explicit_port.parse::<u16>().unwrap_or(port)
                }
                (String::from(host), String::from(path), port)
            }
            None => {
                if let Some(explicit_port) = addr.split(":").nth(1) {
                    port = explicit_port.parse::<u16>().unwrap_or(port)
                }
                (addr, String::from("/"), port)
            }
        }
    }

    /// Creates a new client for the provided URL.
    ///
    /// The URL must start with `http://` or `https://`.
    pub fn create(url: &str) -> AriosResult<Arios> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(AriosError::InvalidUrl);
        }
        Ok(Arios {
            base_url: String::from(url),
        })
    }

    /// Sends a `GET` request and returns the parsed response.
    pub fn get(&self, res_content_type: ContentType) -> AriosResult<AriosResponse> {
        self.request("get", None, None, Some(res_content_type))
    }

    /// Sends a `POST` request with a body and returns the parsed response.
    ///
    /// `req_content_type` controls the `Content-Type` header sent to the server.
    /// `res_content_type` controls the `Accept` header sent to the server.
    pub fn post(
        &self,
        body: &str,
        req_content_type: ContentType,
        res_content_type: ContentType,
    ) -> AriosResult<AriosResponse> {
        self.request(
            "post",
            Some(body),
            Some(req_content_type),
            Some(res_content_type),
        )
    }

    fn request(
        &self,
        http_method: &str,
        body: Option<&str>,
        request_content_type: Option<ContentType>,
        response_content_type: Option<ContentType>,
    ) -> AriosResult<AriosResponse> {
        // Prepare host, path, and port (who is receiving)
        let (host, path, port) = Self::parse_url(&self.base_url);
        let addr: String = format!("{}:{}", host, port);

        // Select HTTP method
        let method: &str = match http_method {
            "get" => "GET",
            "post" => "POST",
            _ => return Err(AriosError::InvalidRequest("invalid HTTP method")),
        };

        // Prepare HTTP header
        let mut req_header: Vec<String> = vec![
            format!("{} {} HTTP/1.1", method, path),
            format!("Host: {}", host),
            String::from("Connection: close"),
            String::from("User-Agent: Arios/0.1"),
        ];
        match response_content_type {
            Some(content_type) => {
                req_header.push(format!("Accept: {}", content_type.as_header_value()))
            }
            None => req_header.push(String::from("Accept: */*")),
        };
        if let Some(b) = body {
            // Prepare HTTP body (if exists)
            match request_content_type {
                Some(content_type) => {
                    req_header.push(format!("Content-Type: {}", content_type.as_header_value()))
                }
                None => req_header.push(String::from("Content-Type: application/json")),
            };
            req_header.push(format!("Content-Length: {}", b.len()));
        }
        let mut req_header_join: String = req_header.join("\r\n"); // Join HTTP headers
        req_header_join.push_str("\r\n\r\n");
        if let Some(b) = body {
            req_header_join.push_str(b);
        }

        // Prepare request message
        let req: String = req_header_join;

        // Send message to recipient
        let mut stream: Box<dyn HttpStream> = if port == 443 {
            let s = TcpStream::connect(&addr)?;
            let tls = s
                .into_tls(&host, TLSConfig::default())
                .map_err(|e| AriosError::Io(std::io::Error::other(e)))?;
            Box::new(tls)
        } else {
            let s = TcpStream::connect(&addr)?;
            Box::new(s)
        };

        // Write message (data receiving) in memory buffer
        stream.write_all(req.as_bytes())?;

        // Preparing to receive response header
        let mut reader = BufReader::new(stream);
        let mut res_header = Vec::new();
        let mut content_length = None;
        let mut transfer_encoding = String::from("identity");

        // Receive header
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line)?;
            if line.to_lowercase().contains("content-length") {
                if let Some(str) = line.split(":").nth(1) {
                    content_length = Some(
                        str.trim()
                            .parse::<usize>()
                            .map_err(|_| AriosError::InvalidResponse("invalid content-length"))?,
                    );
                }
            } else if line.to_lowercase().contains("transfer-encoding") {
                transfer_encoding = line
                    .split(":")
                    .nth(1)
                    .unwrap_or("identity")
                    .trim()
                    .to_string();
            }
            if line.trim().is_empty() || bytes_read == 0 {
                break;
            }
            res_header.push(line.to_string());
        }

        // Organizing header in one string (Vec<String> -> String)
        let header = res_header.join("");

        // Receive body, chunked or not
        let raw_body = match transfer_encoding.as_str() {
            "chunked" => {
                let mut res_chunked_bytes: Vec<u8> = vec![];
                loop {
                    let mut bytes_line = String::new();
                    let bytes_read = reader.read_line(&mut bytes_line)?;
                    bytes_line = bytes_line
                        .trim()
                        .split(";")
                        .next()
                        .unwrap_or("")
                        .to_string();
                    if bytes_line.is_empty() {
                        continue;
                    }
                    if bytes_line.eq("0") || bytes_read == 0 {
                        break;
                    }
                    let bytes = usize::from_str_radix(bytes_line.trim(), 16)
                        .map_err(|_| AriosError::InvalidResponse("invalid chunk size"))?;
                    let mut buffer = vec![0; bytes];
                    reader.read_exact(&mut buffer)?;
                    let mut trash = vec![0; 2];
                    let _ = reader.read_exact(&mut trash);
                    res_chunked_bytes.append(&mut buffer);
                }
                res_chunked_bytes
            }
            _ => match content_length {
                Some(bytes) => {
                    let mut res_bytes = vec![0; bytes];
                    reader.read_exact(&mut res_bytes)?;
                    res_bytes
                }
                None => {
                    let mut res_bytes = vec![];
                    reader.read_to_end(&mut res_bytes)?;
                    res_bytes
                }
            },
        };

        // Treating response header
        let (protocol, code, status) = Self::parse_status_line(&header)?;
        let (content_type, charset) = Self::parse_content_metadata(&header);

        // Return response
        Ok(AriosResponse {
            protocol,
            status,
            code,
            content_type,
            charset,
            content_length,
            header,
            raw_body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_accepts_http_url() {
        let res = Arios::create("http://example.com");
        assert!(res.is_ok());
    }

    #[test]
    fn create_accepts_https_url() {
        let res = Arios::create("https://example.com");
        assert!(res.is_ok());
    }

    #[test]
    fn create_rejects_url_without_scheme() {
        let res = Arios::create("example.com");
        assert!(matches!(res, Err(AriosError::InvalidUrl)));
    }

    #[test]
    fn create_stores_base_url() {
        let client = Arios::create("https://example.com").unwrap();
        assert_eq!(client.base_url, "https://example.com");
    }

    #[test]
    fn create_accepts_localhost_with_explicit_port() {
        let res = Arios::create("http://localhost:8000");
        assert!(res.is_ok());
    }

    #[test]
    fn create_rejects_empty_url() {
        let res = Arios::create("");
        assert!(matches!(res, Err(AriosError::InvalidUrl)));
    }

    #[test]
    fn create_rejects_unsupported_scheme() {
        let res = Arios::create("ftp://example.com");
        assert!(matches!(res, Err(AriosError::InvalidUrl)));
    }

    #[test]
    fn parse_url_returns_default_http_values() {
        let (host, path, port) = Arios::parse_url("http://example.com");
        assert_eq!(host, "example.com");
        assert_eq!(path, "/");
        assert_eq!(port, 80);
    }

    #[test]
    fn parse_url_returns_default_https_values() {
        let (host, path, port) = Arios::parse_url("https://example.com");
        assert_eq!(host, "example.com");
        assert_eq!(path, "/");
        assert_eq!(port, 443);
    }

    #[test]
    fn parse_url_keeps_requested_path() {
        let (host, path, port) = Arios::parse_url("https://example.com/search?q=rust");
        assert_eq!(host, "example.com");
        assert_eq!(path, "/search?q=rust");
        assert_eq!(port, 443);
    }

    #[test]
    fn parse_url_supports_localhost_with_explicit_port() {
        let (host, path, port) = Arios::parse_url("http://localhost:8000/api");
        assert_eq!(host, "localhost:8000");
        assert_eq!(path, "/api");
        assert_eq!(port, 8000);
    }

    #[test]
    fn parse_url_supports_https_localhost_with_explicit_port() {
        let (host, path, port) = Arios::parse_url("https://localhost:8443");
        assert_eq!(host, "localhost:8443");
        assert_eq!(path, "/");
        assert_eq!(port, 8443);
    }

    #[test]
    fn parse_status_line_extracts_protocol_code_and_status() {
        let (protocol, code, status) =
            Arios::parse_status_line("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n").unwrap();
        assert_eq!(protocol, "HTTP/1.1");
        assert_eq!(code, 200);
        assert_eq!(status, "OK");
    }

    #[test]
    fn parse_status_line_rejects_missing_status_line() {
        let err = Arios::parse_status_line("").unwrap_err();
        assert!(matches!(
            err,
            AriosError::InvalidResponse("missing status line")
        ));
    }

    #[test]
    fn parse_status_line_rejects_invalid_status_code() {
        let err = Arios::parse_status_line("HTTP/1.1 abc OK\r\n").unwrap_err();
        assert!(matches!(
            err,
            AriosError::InvalidResponse("invalid status code")
        ));
    }

    #[test]
    fn parse_content_metadata_extracts_content_type_and_charset() {
        let header = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=iso-8859-1\r\n";
        let (content_type, charset) = Arios::parse_content_metadata(header);
        assert_eq!(content_type.as_deref(), Some("text/html"));
        assert_eq!(charset.as_deref(), Some("iso-8859-1"));
    }

    #[test]
    fn parse_content_metadata_handles_missing_charset() {
        let header = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n";
        let (content_type, charset) = Arios::parse_content_metadata(header);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(charset, None);
    }

    #[test]
    fn parse_content_metadata_handles_missing_content_type() {
        let header = "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n";
        let (content_type, charset) = Arios::parse_content_metadata(header);
        assert_eq!(content_type, None);
        assert_eq!(charset, None);
    }
}
