use std::{env, fs::{self, File}, io::{BufRead, BufReader, Read, Write}};

use encoding::{Encoding, all::ISO_8859_1};
use tcp_stream::{TLSConfig, TcpStream};

#[derive(Debug)]
enum HttpBody {
    Text(String),
    Binary(Vec<u8>)
}

trait HttpStream: Read + Write {}

impl<T: Read + Write> HttpStream for T {}

#[allow(dead_code)]
struct AriosResponse {
    protocol: String,
    status: String,
    code: u16,
    content_type: String,
    charset: Option<String>,
    content_length: Option<usize>,
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
            String::from("Connection: close"),
            String::from("User-Agent: Arios/0.1"),
            String::from("Accept: */*")
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
        let mut stream: Box<dyn HttpStream> = if port == 443 {
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
        let mut content_length = None;
        let mut transfer_encoding = String::from("identity");

        // Receive header
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line)?;
            if line.to_lowercase().contains("content-length") {
                if let Some(str) = line.split(":").nth(1) {
                    content_length = str.trim().parse::<usize>().ok()
                }
            } 
            else if line.to_lowercase().contains("transfer-encoding") {
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
        };

        // Organizing header in one string (Vec<String> -> String)
        let header = res_header.join("");

        if transfer_encoding.contains("chunked") { // Chunked body
            println!("chunked response!");
        }

        // Receive body, chunked or not
        let body_bytes = match transfer_encoding.as_str() {
            "chunked" => {
                let mut res_chunked_bytes: Vec<u8> = vec![];
                loop {
                    let mut bytes_line = String::new();
                    let bytes_read = reader.read_line(&mut bytes_line)?;
                    bytes_line = bytes_line.trim().split(";").nth(0).unwrap_or("").to_string();
                    if bytes_line.is_empty() || bytes_line.eq("0") || bytes_read == 0 {
                        break;
                    }
                    let bytes = usize::from_str_radix(&bytes_line.trim(), 16).unwrap_or(0);
                    let mut buffer = vec![0; bytes];
                    reader.read_exact(&mut buffer)?;
                    let mut trash = vec![0; 2];
                    let _ = reader.read_exact(&mut trash);
                    res_chunked_bytes.append(&mut buffer);
                }
                res_chunked_bytes
            },
            _ => {
                match content_length {
                    Some(bytes) => {
                        let mut res_bytes= vec![0; bytes];
                        reader.read_exact(&mut res_bytes)?;
                        res_bytes
                    },
                    None => {
                        let mut res_bytes = vec![];
                        reader.read_to_end(&mut res_bytes)?;
                        res_bytes
                    }
                }
            }
        };

        // Treating response header
        let mut parts = header.lines().next().unwrap_or("").split_whitespace();

        let protocol: String = parts.next().unwrap_or("HTTP/1.1").to_string();
        let code: u16 = parts.next().unwrap_or("0").parse::<u16>().unwrap_or(0);
        let status: String = parts.collect::<Vec<&str>>().join(" ");

        let mut content_type = String::new();
        let mut charset = None;

        // Treating response body
        let body: HttpBody = match header.lines().find(|line| line.to_lowercase().contains("content-type")) {
            Some(line) => {
                content_type = line
                                .split(":")
                                .nth(1)
                                .unwrap_or("")
                                .trim()
                                .split(";")
                                .next()
                                .unwrap_or("text/plain")
                                .to_string();
                if line.contains("charset") {
                    let mut line_split = line.split(";"); // Agora temos algo como [Content-Type: text/html, charset=ISO-8859-1, boundary...]
                    let line_charset = line_split.find(|t| t.contains("charset=")).unwrap_or(""); // Vai retornar o charset=ISO...
                    let line_extract = line_charset.split("=").nth(1).map(|s| s.trim().to_string()); // Vai retornar algo como Some("ISO..") ou None 
                    charset = line_extract
                }
                if line.contains("application/json") || line.contains("text/html") || line.contains("text/plain") {
                    let text = match charset.clone().unwrap_or(String::from("utf-8")).to_lowercase().as_str() {
                        "iso-8859-1" => {
                            ISO_8859_1.decode(&body_bytes, encoding::DecoderTrap::Replace).unwrap()
                        }
                        _ => String::from_utf8_lossy(&body_bytes).to_string()
                    };
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
        Ok(AriosResponse { protocol, status, code, content_type, charset, content_length, header, body})
    }
}

fn main() -> std::io::Result<()> {
    // Collect arguments
    let args: Vec<String> = env::args().collect();

    // Receive URL
    let url = match args.contains(&String::from("--url")) {
        true => {
            let index = args.iter().position(|t| t == "--url").unwrap();
            args[index+1].as_str()
        },
        false => "https://www.google.com"
    };

    // Receive if the output files must be saved
    let save_files = match args.contains(&String::from("--save-files")) {
        true => {
            let index = args.iter().position(|t| t == "--save-files").unwrap();
            args[index+1].parse().unwrap()
        },
        false => false
    };

    // Create Arios's instance with URL
    let arios: Arios = match Arios::create(url) {
        Ok(a) => a,
        Err(e) => {
            println!("Error creating arios's instance. See: {}", e);
            return Ok(());
        }
    };

    // Receive URL response (request -> response)
    let response: AriosResponse  = arios.get()?;

    // DEBUG
    println!("{}", response.header);

    // (Optional) Save output files
    if save_files {
        fs::create_dir_all("out/")?;
        let mut fh: File = File::create("out/header.txt")?;
        fh.write_all(response.header.as_bytes())?;
        
        let extension = response.content_type.split("/").nth(1).unwrap_or("txt");
        let mut fb: File = File::create(format!("out/body.{}", extension))?;
        fb.write_all(response.bytes())?;
        
        fh.flush()?;
        fb.flush()?;
    }

    // Program return
    Ok(())
}