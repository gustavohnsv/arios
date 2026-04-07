# Arios

Arios is a small HTTP client crate written in Rust.

The project started as a study exercise and is being refactored into a publishable library crate. The current focus is on manual HTTP request/response handling, response metadata parsing, and a small public API.

## Status

Arios currently supports:

- `GET` and `POST`
- HTTP and HTTPS
- configurable `Accept` and `Content-Type` headers
- response metadata such as status code, content-type, charset, and content-length
- access to raw response bytes
- text decoding with built-in support for UTF-8, ISO-8859-1, and US-ASCII
- UTF-8 fallback when no charset is provided or when the charset is not supported

Current limitations:

- the API is still evolving
- URL parsing is intentionally simple
- there is no custom crate error type yet
- advanced HTTP features are still limited

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
arios = "0.1.2"
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

## Response API

`AriosResponse` exposes:

- `bytes()` for raw response bytes
- `text()` for decoded textual content
- public metadata fields such as `code`, `status`, `content_type`, `charset`, and `content_length`

`text()` currently supports `utf-8`, `iso-8859-1`, and `us-ascii`. If the response does not declare a charset, or declares one that Arios does not support yet, decoding falls back to UTF-8.

## Goals

The project is being built as a learning-focused HTTP client with crate-oriented organization. The immediate goals are:

- keep the public API small and understandable
- improve test coverage
- continue reducing protocol parsing edge cases
- prepare the crate for publication
