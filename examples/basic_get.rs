use arios::{Arios, AriosResult, ContentType};

fn main() -> AriosResult<()> {
    let arios = Arios::create("https://httpbin.org/get")?;
    let response = arios.get(ContentType::Json)?;
    println!("{}", response.text()?);
    Ok(())
}
