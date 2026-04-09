# Arios

Arios is a small HTTP client crate written in Rust.

The project started as a study exercise and is being refactored into a publishable library crate. The current focus is on manual HTTP request/response handling, response metadata parsing, and a small public API.

## Status

Arios currently supports:

- `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, and `OPTIONS`
- HTTP and HTTPS
- HTTPS backed by `rustls` with native platform root certificates
- configurable `Accept` and `Content-Type` headers
- response metadata such as status code, content-type, charset, and content-length
- access to raw response bytes
- text decoding with built-in support for UTF-8, ISO-8859-1, and US-ASCII
- UTF-8 fallback when no charset is provided or when the charset is not supported
- explicit `AriosError::HttpStatus` errors for `4xx` and `5xx` responses

Current limitations:

- the API is still evolving
- URL parsing is intentionally simple
- advanced HTTP features are still limited
- `HEAD` responses are represented with an empty response body
- HTTPS depends on the operating system certificate store being available

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
arios = "0.2.0"
```

## Example: GET

```rust
use arios::{Arios, AriosResult, ContentType};

fn main() -> AriosResult<()> {
    let arios = Arios::create("https://httpbin.org/get")?;
    let response = arios.get(ContentType::Json)?;
    println!("{}", response.text()?);
    Ok(())
}
```

See also: `examples/basic_get.rs`

## Example: POST

```rust
use arios::{Arios, AriosResult, ContentType};

fn main() -> AriosResult<()> {
    let arios = Arios::create("https://httpbin.org/post")?;
    let body = r#"{"name":"arios"}"#;
    let response = arios.post(body, ContentType::Json, ContentType::Json)?;
    println!("{}", response.text()?);
    Ok(())
}
```

See also: `examples/basic_post.rs`

## Other Request Methods

`Arios` also exposes:

- `head()` for header-only requests
- `options()` for capability and method discovery
- `put()` and `patch()` for body-carrying updates
- `delete()` for resource deletion requests

`head()` returns an `AriosResponse` with an empty body. Calls that receive `4xx`
or `5xx` statuses return `AriosError::HttpStatus` instead of a successful
response value.

## Response API

`AriosResponse` exposes:

- `bytes()` for raw response bytes
- `text()` for decoded textual content
- public metadata fields such as `code`, `status`, `content_type`, `charset`, and `content_length`

`text()` currently supports `utf-8`, `iso-8859-1`, and `us-ascii`. If the response does not declare a charset, or declares one that Arios does not support yet, decoding falls back to UTF-8.
