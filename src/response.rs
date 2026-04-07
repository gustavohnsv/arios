use crate::{AriosError, AriosResult};

enum Charset {
    Utf8,
    Latin1,
    Ascii,
}

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

    fn decode(&self, charset: Charset) -> AriosResult<String> {
        match charset {
            Charset::Latin1 => Ok(self
                .raw_body
                .as_slice()
                .iter()
                .map(|&c| char::from(c))
                .collect()),
            Charset::Ascii => {
                if self.raw_body.iter().all(|byte| byte.is_ascii()) {
                    Ok(self.raw_body.iter().map(|&byte| char::from(byte)).collect())
                } else {
                    Err(AriosError::InvalidResponse(
                        "response body contains non-ASCII bytes",
                    ))
                }
            }
            Charset::Utf8 => String::from_utf8(self.raw_body.clone())
                .map_err(|_| AriosError::InvalidResponse("response body is not valid UTF-8")),
        }
    }

    /// Decodes the response body as text using the detected charset.
    ///
    /// Supported charsets are `utf-8`, `iso-8859-1`, and `us-ascii`.
    /// UTF-8 is used as the default fallback when no charset is present or supported.
    pub fn text(&self) -> AriosResult<String> {
        let charset_type = match self
            .charset
            .as_deref()
            .unwrap_or("utf-8")
            .to_lowercase()
            .as_str()
        {
            "iso-8859-1" => Charset::Latin1,
            "us-ascii" => Charset::Ascii,
            "utf-8" => Charset::Utf8,
            _ => Charset::Utf8,
        };

        self.decode(charset_type)
            .map_err(|_| AriosError::InvalidResponse("failed to decode response body"))
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

    #[test]
    fn text_decodes_us_ascii_when_charset_is_set() {
        let response = response_with_body(b"hello".to_vec(), Some("us-ascii"));
        let text = response.text().unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn text_falls_back_to_utf8_for_unsupported_charset() {
        let response = response_with_body("Olá".as_bytes().to_vec(), Some("windows-1252"));
        let text = response.text().unwrap();
        assert_eq!(text, "Olá");
    }

    #[test]
    fn text_rejects_invalid_utf8() {
        let response = response_with_body(vec![0xFF], None);
        let err = response.text().unwrap_err();
        assert!(matches!(
            err,
            AriosError::InvalidResponse("failed to decode response body")
        ));
    }

    #[test]
    fn text_rejects_non_ascii_bytes_when_charset_is_ascii() {
        let response = response_with_body(vec![0x4F, 0x6C, 0xE1], Some("us-ascii"));
        let err = response.text().unwrap_err();
        assert!(matches!(
            err,
            AriosError::InvalidResponse("failed to decode response body")
        ));
    }
}
