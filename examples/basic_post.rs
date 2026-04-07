use arios::{Arios, AriosResult, ContentType};

fn main() -> AriosResult<()> {
    let arios = Arios::create("https://httpbin.org/post")?;
    let body = r#"{"name":"arios"}"#;
    let response = arios.post(body, ContentType::Json, ContentType::Json)?;
    println!("{}", response.text()?);
    Ok(())
}