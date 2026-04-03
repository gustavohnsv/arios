use std::{env, fs::File, io::{BufRead, BufReader, Read, Write}};

use tcp_stream::{TLSConfig, TcpStream};

#[derive(Debug)]
enum HttpBody {
    Text(String),
    Binary(Vec<u8>)
}

trait HtppStream: Read + Write {}

impl<T: Read + Write> HtppStream for T {}

struct AriosResponse {
    protocol: String,
    status: String,
    code: u16,
    content_type: String,
    content_length: usize,
    header: String,
    body: HttpBody
}

impl AriosResponse {
    pub fn bytes(&self) -> &[u8] {
        match &self.body {
            HttpBody::Text(text) => text.as_bytes(),
            HttpBody::Binary(bin) => bin.as_slice()
        }
    }
}

struct Arios {
    base_url: String
}

impl Arios {

    fn parse_url(url: &str) -> (String, String, u16) {
        let mut port: u16 = 80;
        if url.starts_with("https") {
            port = 443;
        }
        let addr: String = match url.find("://") {
            Some(index) => String::from(&url[index+3..]),
            None => String::from(url)
        };
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

    #[allow(dead_code)]
    fn parse_res(res: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
        let delimiter: [u8; 4] = [13, 10, 13, 10];
        match res.windows(4).position(|window| window == delimiter) {
            Some(index) => {
                let header = &res[..index];
                let body = &res[index+4..];
                return (header.to_vec(), body.to_vec());
            }
            None => {
                return ([].to_vec(), [].to_vec());
            }
        }
    }

    pub fn create(url: &str) -> Result<Arios, String> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(String::from("Invalid URL. Try adding 'http://' or 'https://'"))
        }
        Ok(Arios { base_url: String::from(url) })
    }

    #[allow(dead_code)]
    pub fn get(&self) -> std::io::Result<AriosResponse> {
        return self.request("get", None);
    }

    #[allow(dead_code)]
    pub fn post(&self, body: &str) -> std::io::Result<AriosResponse> {
        return self.request("post", Some(body));
    }

    fn request(&self, http_method: &str, body: Option<&str>) -> std::io::Result<AriosResponse> {

        // Prepare host, path, and port (who is receiving)
        let (host, path, port) = Self::parse_url(&self.base_url);
        let addr: String = format!("{}:{}", host, port);
        println!("{}", addr);
        
        // Select HTTP method
        let method: &str = match http_method {
            "get" => "GET",
            "post" => "POST",
            &_ => {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid method"));
            }    
        };
        
        // Prepare HTTP header
        let mut req_header: Vec<String> = vec![
            format!("{} {} HTTP/1.1", method, path),
            format!("Host: {}", host),
            String::from("Connection: close")
        ];
        if let Some(b) = body { // Prepare HTTP body (if exists)
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
        let mut stream: Box<dyn HtppStream> = if port == 443 {
            let s = TcpStream::connect(&addr).unwrap();
            let tls = s.into_tls(&host, TLSConfig::default()).unwrap();
            Box::new(tls)
        } else {
            let s = TcpStream::connect(&addr).unwrap();
            Box::new(s)
        };

        // Write message (data receiving) in memory buffer
        stream.write_all(req.as_bytes())?;

        // Preparing to receive response header
        let mut reader = BufReader::new(stream);
        let mut res_header = Vec::new();
        let mut content_length: usize = 0;

        // Receivening header
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line)?;
            if line.to_lowercase().contains("content-length") {
                let content_length_text = line.split(":").nth(1).unwrap_or("0").trim();
                content_length = content_length_text.parse::<usize>().unwrap_or(0);
            }
            if line.trim().is_empty() || bytes_read == 0 {
                break;
            }
            res_header.push(line.to_string());
        };

        // Organizing header in one string (Vec<String> -> String)
        let header = res_header.join("\r\n");

        // Receivening exact bytes number from body
        let mut res_bytes= vec![0; content_length];
        reader.read_exact(&mut res_bytes)?;

        // Transfering ownership - for better understanding
        let body_bytes = res_bytes;

        // Treating response header
        let mut header_vars: std::str::SplitWhitespace<'_> = header.lines().next().unwrap_or("").split_whitespace();
        let protocol: String = header_vars.nth(0).unwrap_or("HTTP/1.1").to_string();
        let code_text: &str = header_vars.nth(0).unwrap_or("0");
        let code: u16 = code_text.parse::<u16>().unwrap_or(0);
        let status: String = header_vars.collect::<Vec<&str>>().join(" ");
        let mut content_type = String::new();

        // Treating response body
        let body: HttpBody = match header.lines().find(|line| line.to_lowercase().contains("content-type")) {
            Some(line) => {
                content_type = line.split(":").nth(1).unwrap_or("").trim().split(";").next().unwrap_or("text/plain").to_string();
                if line.contains("application/json") || line.contains("text/html") || line.contains("text/plain") {
                    let text = String::from_utf8_lossy(&body_bytes).to_string();
                    HttpBody::Text(text)
                } else {
                    HttpBody::Binary(body_bytes)
                }
            }
            None => {
                HttpBody::Binary(body_bytes)
            }
        };

        // Return response
        Ok(AriosResponse { protocol, status, code, content_type, content_length, header, body})
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    let url = match args.iter().nth(1) {
        Some(link) => link,
        None => "http://google.com"
    };
    let arios: Arios = match Arios::create(url) {
        Ok(a) => a,
        Err(e) => {
            println!("Error creating arios's instance. See: {}", e);
            return Ok(());
        }
    };
    let response: AriosResponse  = arios.get()?;
    println!(
        "[Protocol, Status, Code, Content-Type, Content-Length, Header (len), Body (len)]: [{}, {}, {}, {}, {}, {}, {}]", 
        response.protocol,
        response.status,
        response.code,
        response.content_type,
        response.content_length,
        response.header.len(),
        match response.body {
            HttpBody::Text(ref text) => text.len(),
            HttpBody::Binary(ref bin) => bin.len(),
        }
    );
    let mut fh: File = File::create("out/header.txt")?;
    fh.write_all(response.header.as_bytes())?;

    let extension = response.content_type.split("/").nth(1).unwrap_or("txt");
    let mut fb: File = File::create(format!("out/body.{}", extension))?;
    fb.write_all(response.bytes())?;

    fh.flush()?;
    fb.flush()?;
    Ok(())
}