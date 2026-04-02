use std::{fs::File, io::{Read, Write}, net::TcpStream};

struct AriosResponse {
    protocol: String,
    status: String,
    code: u16,
    header: String,
    body: String
}

struct Arios {
    base_url: String
}

impl Arios {

    fn parse_url(url: &str) -> (String, String, u16) {
        let mut port: u16 = 80;
        if url.starts_with("https://") {
            port = 443;
        }
        let addr: String = url.replace("http://", "");
        match addr.find("/") {
            Some(index) => {
                let host: &str = &addr[..index];
                let path: &str = &addr[index..];
                return (String::from(host), String::from(path), port);
            }
            None => {
                return (addr, String::from("/"), port);
            }
        }
    }

    fn parse_res(res: String) -> (String, String) {
        match res.find("\r\n\r\n") {
            Some(index) => {
                let header: &str = &res[..index];
                let body: &str = &res[index+4..];
                return (String::from(header), String::from(body));
            }
            None => {
                return (String::from(""), String::from(""));
            }
        }
    }

    pub fn create(url: &str) -> Result<Arios, String> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(String::from("Invalid URL. Try adding 'http://' or 'https://'"))
        }
        Ok(Arios { base_url: String::from(url) })
    }

    pub fn request(&self, http_method: &str, body: Option<&str>) -> std::io::Result<AriosResponse> {
        let (host, path, port) = Self::parse_url(&self.base_url);
        let addr: String = format!("{}:{}", host, port);
        let method: &str = match http_method {
            "get" => "GET",
            "post" => "POST",
            &_ => {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid method"));
            }    
        };
        let mut headers: Vec<String> = vec![
            format!("{} {} HTTP/1.1", method, path),
            format!("Host: {}", host),
            String::from("Connection: close")
        ];
        if let Some(b) = body {
            headers.push(format!("Content-Length: {}", b.len()));
        }
        let mut headers_join: String = headers.join("\r\n");
        headers_join.push_str("\r\n\r\n");
        if let Some(b) = body {
            headers_join.push_str(b);
        }
        let req: String = headers_join;
        let mut stream: TcpStream = TcpStream::connect(&addr)?;
        stream.write_all(req.as_bytes())?;
        let mut res: String = String::new();
        stream.read_to_string(&mut res)?;
        let (h, b) = Self::parse_res(res);
        let mut h_vars = h.lines().next().unwrap_or("").split_whitespace();
        let p: String = h_vars.nth(0).unwrap_or("HTTP/1.1").to_string();
        let c_text: &str = h_vars.nth(0).unwrap_or("0");
        let c: u16 = c_text.parse::<u16>().unwrap_or(0);
        let s: String = h_vars.collect::<Vec<&str>>().join(" ");
        Ok(AriosResponse { protocol: p, status: s, code: c, header: h, body: b})
    }
}

fn main() -> std::io::Result<()> {
    let url: &str = "http://jsonplaceholder.typicode.com/posts";
    let arios: Arios = match Arios::create(url) {
        Ok(a) => a,
        Err(e) => {
            println!("Error creating arios's instance. See: {}", e);
            return Ok(());
        }
    };
    let post_body: &str = r#"
        {
            "title": "foo",
            "body": "bar",
            "userId": 1,
        }
    "#;
    let response: AriosResponse  = arios.request("post", Some(post_body))?;
    println!(
        "[Protocol, Status, Code, Header (len), Body (len)]: [{}, {}, {}, {}, {}]", 
        response.protocol,
        response.status,
        response.code,
        response.header.len(), 
        response.body.len()
    );
    let mut fh: File = File::create("header.log")?;
    let mut fb: File = File::create("body.log")?;
    writeln!(fh, "Header: \n{}", response.header)?;
    writeln!(fb, "Body: \n{}", response.body)?;
    fh.flush()?;
    fb.flush()?;
    Ok(())
}