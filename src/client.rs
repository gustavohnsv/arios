use std::io::{BufRead, BufReader, Read, Write};

use crate::transport::connect_stream;
use crate::{AriosError, AriosResponse, AriosResult};

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
    fn resolve_method(http_method: &str) -> AriosResult<&'static str> {
        match http_method {
            "get" => Ok("GET"),
            "post" => Ok("POST"),
            "delete" => Ok("DELETE"),
            "put" => Ok("PUT"),
            "patch" => Ok("PATCH"),
            "head" => Ok("HEAD"),
            "options" => Ok("OPTIONS"),
            _ => Err(AriosError::InvalidRequest("invalid HTTP method")),
        }
    }

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

    fn validate_status_code(code: u16, status: String) -> AriosResult<String> {
        if (200..400).contains(&code) {
            Ok(status)
        } else {
            Err(AriosError::HttpStatus(code, status))
        }
    }

    fn response_can_have_body(http_method: &str) -> bool {
        http_method != "head"
    }

    fn default_port(url: &str) -> u16 {
        if url.starts_with("https") { 443 } else { 80 }
    }

    fn parse_url(url: &str) -> (String, String, u16) {
        let mut port = Self::default_port(url);
        let addr: String = match url.find("://") {
            Some(index) => String::from(&url[index + 3..]),
            None => String::from(url),
        };
        match addr.find("/") {
            Some(index) => {
                let authority = &addr[..index];
                let path = &addr[index..];
                let mut host = authority;
                if let Some((parsed_host, explicit_port)) = authority.rsplit_once(':') {
                    port = explicit_port.parse::<u16>().unwrap_or(port);
                    host = parsed_host;
                }
                (String::from(host), String::from(path), port)
            }
            None => {
                let mut host = addr.as_str();
                if let Some((parsed_host, explicit_port)) = addr.rsplit_once(':') {
                    port = explicit_port.parse::<u16>().unwrap_or(port);
                    host = parsed_host;
                }
                (String::from(host), String::from("/"), port)
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

    /// Sends a `HEAD` request and returns the parsed response headers.
    ///
    /// `HEAD` responses are returned with an empty body by design.
    pub fn head(&self, res_content_type: ContentType) -> AriosResult<AriosResponse> {
        self.request("head", None, None, Some(res_content_type))
    }

    /// Sends an `OPTIONS` request and returns the parsed response.
    ///
    /// `res_content_type` controls the `Accept` header sent to the server.
    pub fn options(&self, res_content_type: ContentType) -> AriosResult<AriosResponse> {
        self.request("options", None, None, Some(res_content_type))
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

    /// Sends a `PUT` request with a body and returns the parsed response.
    ///
    /// `req_content_type` controls the `Content-Type` header sent to the server.
    /// `res_content_type` controls the `Accept` header sent to the server.
    pub fn put(
        &self,
        body: &str,
        req_content_type: ContentType,
        res_content_type: ContentType,
    ) -> AriosResult<AriosResponse> {
        self.request(
            "put",
            Some(body),
            Some(req_content_type),
            Some(res_content_type),
        )
    }

    /// Sends a `PATCH` request with a body and returns the parsed response.
    ///
    /// `req_content_type` controls the `Content-Type` header sent to the server.
    /// `res_content_type` controls the `Accept` header sent to the server.
    pub fn patch(
        &self,
        body: &str,
        req_content_type: ContentType,
        res_content_type: ContentType,
    ) -> AriosResult<AriosResponse> {
        self.request(
            "patch",
            Some(body),
            Some(req_content_type),
            Some(res_content_type),
        )
    }

    /// Sends a `DELETE` request and returns the parsed response.
    ///
    /// `res_content_type` controls the `Accept` header sent to the server.
    pub fn delete(&self, res_content_type: ContentType) -> AriosResult<AriosResponse> {
        self.request("delete", None, None, Some(res_content_type))
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
        let default_port = Self::default_port(&self.base_url);
        let host_header = if port == default_port {
            host.clone()
        } else {
            format!("{}:{}", host, port)
        };

        // Select HTTP method
        let method = Self::resolve_method(http_method)?;

        // Prepare HTTP header
        let mut req_header: Vec<String> = vec![
            format!("{} {} HTTP/1.1", method, path),
            format!("Host: {}", host_header),
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
        let mut stream = connect_stream(&addr, &host, port == 443)?;

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
        let raw_body = match Self::response_can_have_body(http_method) {
            false => vec![],
            true => match transfer_encoding.as_str() {
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
            },
        };

        // Treating response header
        let (protocol, code, status) = Self::parse_status_line(&header)?;
        let status = Self::validate_status_code(code, status)?;
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
    fn resolve_method_supports_all_public_verbs() {
        assert_eq!(Arios::resolve_method("get").unwrap(), "GET");
        assert_eq!(Arios::resolve_method("post").unwrap(), "POST");
        assert_eq!(Arios::resolve_method("put").unwrap(), "PUT");
        assert_eq!(Arios::resolve_method("patch").unwrap(), "PATCH");
        assert_eq!(Arios::resolve_method("delete").unwrap(), "DELETE");
        assert_eq!(Arios::resolve_method("head").unwrap(), "HEAD");
        assert_eq!(Arios::resolve_method("options").unwrap(), "OPTIONS");
    }

    #[test]
    fn resolve_method_rejects_unknown_verbs() {
        let err = Arios::resolve_method("trace").unwrap_err();
        assert!(matches!(
            err,
            AriosError::InvalidRequest("invalid HTTP method")
        ));
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
        assert_eq!(host, "localhost");
        assert_eq!(path, "/api");
        assert_eq!(port, 8000);
    }

    #[test]
    fn parse_url_supports_https_localhost_with_explicit_port() {
        let (host, path, port) = Arios::parse_url("https://localhost:8443");
        assert_eq!(host, "localhost");
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

    #[test]
    fn validate_status_code_accepts_success_status() {
        let status = Arios::validate_status_code(204, String::from("No Content")).unwrap();
        assert_eq!(status, "No Content");
    }

    #[test]
    fn validate_status_code_rejects_client_error_status() {
        let err = Arios::validate_status_code(405, String::from("Method Not Allowed")).unwrap_err();
        match err {
            AriosError::HttpStatus(code, status) => {
                assert_eq!(code, 405);
                assert_eq!(status, "Method Not Allowed");
            }
            _ => panic!("expected HttpStatus error"),
        }
    }

    #[test]
    fn validate_status_code_rejects_server_error_status() {
        let err =
            Arios::validate_status_code(500, String::from("Internal Server Error")).unwrap_err();
        match err {
            AriosError::HttpStatus(code, status) => {
                assert_eq!(code, 500);
                assert_eq!(status, "Internal Server Error");
            }
            _ => panic!("expected HttpStatus error"),
        }
    }

    #[test]
    fn response_can_have_body_rejects_head() {
        assert!(!Arios::response_can_have_body("head"));
    }

    #[test]
    fn response_can_have_body_allows_body_for_other_verbs() {
        assert!(Arios::response_can_have_body("get"));
        assert!(Arios::response_can_have_body("post"));
        assert!(Arios::response_can_have_body("put"));
        assert!(Arios::response_can_have_body("patch"));
        assert!(Arios::response_can_have_body("delete"));
        assert!(Arios::response_can_have_body("options"));
    }
}
