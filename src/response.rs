use encoding::{Encoding, all::ISO_8859_1};

use crate::{AriosError, AriosResult};

/// Parsed HTTP response returned by Arios.
pub struct AriosResponse {
    /// HTTP protocol string from the status line.
    pub protocol: String,
    /// Human-readable status text from the status line.
    pub status: String,
    /// Numeric HTTP status code.
    pub code: u16,
    /// Parsed `Content-Type` header, when present.
    pub content_type: Option<String>,
    /// Parsed `charset` value from `Content-Type`, when present.
    pub charset: Option<String>,
    /// Parsed `Content-Length` header, when present.
    pub content_length: Option<usize>,
    /// Raw response header text.
    pub header: String,
    /// Raw response body bytes.
    pub raw_body: Vec<u8>,
}

impl AriosResponse {
    /// Returns the raw response body bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.raw_body
    }

    /// Decodes the response body as text using the detected charset.
    ///
    /// UTF-8 is used as the default fallback when no charset is present.
    pub fn text(&self) -> AriosResult<String> {
        match self
            .charset
            .as_deref()
            .unwrap_or("utf-8")
            .to_lowercase()
            .as_str()
        {
            "iso-8859-1" => ISO_8859_1
                .decode(&self.raw_body, encoding::DecoderTrap::Replace)
                .map_err(|_| AriosError::InvalidResponse("invalid response body encoding")),
            _ => Ok(String::from_utf8_lossy(&self.raw_body).to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_with_body(raw_body: Vec<u8>, charset: Option<&str>) -> AriosResponse {
        AriosResponse {
            protocol: String::from("HTTP/1.1"),
            status: String::from("OK"),
            code: 200,
            content_type: Some(String::from("text/plain")),
            charset: charset.map(String::from),
            content_length: Some(raw_body.len()),
            header: String::from("HTTP/1.1 200 OK\r\n"),
            raw_body,
        }
    }

    #[test]
    fn bytes_returns_raw_body_slice() {
        let response = response_with_body(vec![0x41, 0x42, 0x43], None);
        assert_eq!(response.bytes(), &[0x41, 0x42, 0x43]);
    }

    #[test]
    fn text_decodes_utf8_by_default() {
        let response = response_with_body("Olá".as_bytes().to_vec(), None);
        let text = response.text().unwrap();
        assert_eq!(text, "Olá");
    }

    #[test]
    fn text_decodes_iso_8859_1_when_charset_is_set() {
        let response = response_with_body(vec![0x4F, 0x6C, 0xE1], Some("iso-8859-1"));
        let text = response.text().unwrap();
        assert_eq!(text, "Olá");
    }

    #[test]
    fn text_treats_charset_case_insensitively() {
        let response = response_with_body(vec![0x4F, 0x6C, 0xE1], Some("ISO-8859-1"));
        let text = response.text().unwrap();
        assert_eq!(text, "Olá");
    }
}
