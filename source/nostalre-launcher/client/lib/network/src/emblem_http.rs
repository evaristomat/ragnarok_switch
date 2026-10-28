//! Guild emblems through rAthena's web server, as the 20220406 client handles them: an emblem is uploaded to
//! `/emblem/upload` (then announced to the map server with 0x0b46) and downloaded from `/emblem/download`, both
//! multipart POSTs authorized by the web token of the login reply. Blocking: run them off the game thread.

use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// For the connection, and for the whole request after it.
const TIMEOUT: Duration = Duration::from_secs(5);
/// rAthena stores emblems up to 50000 bytes (`MAX_EMBLEM_SIZE`); a reply past this is not one.
const MAX_REPLY: usize = 64 * 1024;
const BOUNDARY: &str = "nostalre-emblem-boundary";

/// What the web server checks: the account's web token (enabled while it is online) and the world, the char
/// server's name, under which emblems are stored.
#[derive(Debug, Clone)]
pub struct WebAuth {
    /// host:port of the web server
    pub addr: String,
    pub token: String,
    pub aid: u32,
    pub world: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebError {
    /// No web server answered (the phone runs none): the next request would fail the same way.
    Unreachable(String),
    /// It answered with an error, or a reply that is not one.
    Failed(String),
}

impl fmt::Display for WebError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WebError::Unreachable(e) | WebError::Failed(e) => f.write_str(e),
        }
    }
}

impl From<WebError> for String {
    fn from(e: WebError) -> String {
        e.to_string()
    }
}

/// The guild's current emblem, a BMP or a GIF.
pub fn download(auth: &WebAuth, gdid: u32) -> Result<Vec<u8>, WebError> {
    post(&auth.addr, "/emblem/download", &auth.fields(gdid))
}

/// Stores `bmp` as the guild's emblem and returns its new version, for 0x0b46.
pub fn upload(auth: &WebAuth, gdid: u32, bmp: &[u8]) -> Result<u32, WebError> {
    let mut fields = auth.fields(gdid);
    fields.push(Part::text("ImgType", "BMP"));
    fields.push(Part {
        name: "Img",
        file: Some(("emblem.bmp", "image/bmp")),
        value: bmp.to_vec(),
    });
    let body = post(&auth.addr, "/emblem/upload", &fields)?;
    // {"Type":1,"version":N}
    let text = String::from_utf8_lossy(&body);
    text.split("\"version\":")
        .nth(1)
        .and_then(|rest| {
            let digits: String = rest
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        })
        .ok_or_else(|| WebError::Failed(format!("no version in the upload reply: {text}")))
}

/// One multipart field; a file carries its name and content type.
struct Part {
    name: &'static str,
    file: Option<(&'static str, &'static str)>,
    value: Vec<u8>,
}

impl Part {
    fn text(name: &'static str, value: &str) -> Self {
        Self {
            name,
            file: None,
            value: value.as_bytes().to_vec(),
        }
    }
}

impl WebAuth {
    fn fields(&self, gdid: u32) -> Vec<Part> {
        vec![
            Part::text("AuthToken", &self.token),
            Part::text("AID", &self.aid.to_string()),
            Part::text("GDID", &gdid.to_string()),
            Part::text("WorldName", &self.world),
        ]
    }
}

fn multipart(parts: &[Part]) -> Vec<u8> {
    let mut body = Vec::new();
    for part in parts {
        let disposition = match part.file {
            Some((filename, content_type)) => format!(
                "name=\"{}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}",
                part.name
            ),
            None => format!("name=\"{}\"", part.name),
        };
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; {disposition}\r\n\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(&part.value);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

/// Connects to the first address of `addr` that answers.
fn connect(addr: &str) -> Result<TcpStream, WebError> {
    let unreachable = |e: String| WebError::Unreachable(format!("{addr}: {e}"));
    let mut last = String::from("no address");
    for socket in addr
        .to_socket_addrs()
        .map_err(|e| unreachable(e.to_string()))?
    {
        match TcpStream::connect_timeout(&socket, TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(e) => last = e.to_string(),
        }
    }
    Err(unreachable(last))
}

fn post(addr: &str, path: &str, parts: &[Part]) -> Result<Vec<u8>, WebError> {
    let failed = |e: String| WebError::Failed(format!("{addr}{path}: {e}"));
    let body = multipart(parts);
    let mut stream = connect(addr)?;
    let deadline = Instant::now() + TIMEOUT;
    stream.set_write_timeout(Some(TIMEOUT)).ok();
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: multipart/form-data; boundary={BOUNDARY}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(&body))
        .map_err(|e| failed(e.to_string()))?;
    let reply = read_reply(&mut stream, deadline).map_err(failed)?;
    response_body(&reply).map_err(failed)
}

/// The reply up to the end of the connection, within `deadline` and `MAX_REPLY` bytes.
fn read_reply(stream: &mut TcpStream, deadline: Instant) -> Result<Vec<u8>, String> {
    let mut reply = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("timed out".into());
        }
        stream
            .set_read_timeout(Some(left))
            .map_err(|e| e.to_string())?;
        match stream.read(&mut chunk).map_err(|e| e.to_string())? {
            0 => return Ok(reply),
            n if reply.len() + n > MAX_REPLY => {
                return Err(format!("reply past {MAX_REPLY} bytes"));
            }
            n => reply.extend_from_slice(&chunk[..n]),
        }
    }
}

/// The body of a `200` reply, cut to its `Content-Length` when it has one.
fn response_body(reply: &[u8]) -> Result<Vec<u8>, String> {
    let split = reply
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("no end of headers")?;
    let head = String::from_utf8_lossy(&reply[..split]);
    let mut lines = head.lines();
    let status = lines.next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(status.to_string());
    }
    let body = &reply[split + 4..];
    let length = lines.find_map(|l| {
        let (name, value) = l.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });
    Ok(body[..length.unwrap_or(body.len()).min(body.len())].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A web server for one request: replies `reply` and hands back what it read.
    fn serve_once(reply: &'static [u8]) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            // headers, then as many body bytes as Content-Length says
            loop {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            // the client may stop reading first (a reply past the limit)
            let _ = stream.write_all(reply);
            request
        });
        (addr, handle)
    }

    fn auth(addr: String) -> WebAuth {
        WebAuth {
            addr,
            token: "1e3320c7170bae56".into(),
            aid: 2000000,
            world: "rAthena".into(),
        }
    }

    fn field(request: &[u8], name: &str) -> String {
        let text = String::from_utf8_lossy(request);
        let start = format!("name=\"{name}\"");
        let after = &text[text.find(&start).unwrap()..];
        let value = &after[after.find("\r\n\r\n").unwrap() + 4..];
        value[..value.find("\r\n--").unwrap()].to_string()
    }

    #[test]
    fn download_posts_the_web_token_and_returns_the_image() {
        let (addr, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Type: image/bmp\r\nContent-Length: 4\r\n\r\nBM\x01\x02",
        );
        assert_eq!(download(&auth(addr), 7).unwrap(), b"BM\x01\x02");
        let request = server.join().unwrap();
        assert!(request.starts_with(b"POST /emblem/download HTTP/1.1\r\n"));
        assert_eq!(field(&request, "AuthToken"), "1e3320c7170bae56");
        assert_eq!(field(&request, "AID"), "2000000");
        assert_eq!(field(&request, "GDID"), "7");
        assert_eq!(field(&request, "WorldName"), "rAthena");
    }

    #[test]
    fn upload_sends_the_bitmap_and_reads_the_new_version() {
        let (addr, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 22\r\n\r\n{\"Type\":1,\"version\":3}",
        );
        assert_eq!(upload(&auth(addr), 7, b"BMxyz").unwrap(), 3);
        let request = server.join().unwrap();
        assert!(request.starts_with(b"POST /emblem/upload HTTP/1.1\r\n"));
        assert_eq!(field(&request, "ImgType"), "BMP");
        assert_eq!(field(&request, "Img"), "BMxyz");
    }

    #[test]
    fn a_refusal_is_an_error() {
        let (addr, server) =
            serve_once(b"HTTP/1.1 404 Not Found\r\nContent-Length: 5\r\n\r\nError");
        assert_eq!(
            download(&auth(addr.clone()), 7).unwrap_err(),
            WebError::Failed(format!("{addr}/emblem/download: HTTP/1.1 404 Not Found"))
        );
        server.join().unwrap();
    }

    #[test]
    fn no_server_is_unreachable() {
        // a port nothing listens on: bind one, then let it go
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();
        assert!(matches!(
            download(&auth(addr), 7),
            Err(WebError::Unreachable(_))
        ));
    }

    #[test]
    fn a_reply_past_the_emblem_limit_is_refused() {
        let mut reply = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
        reply.resize(MAX_REPLY + 100, b'x');
        let (addr, server) = serve_once(reply.leak());
        let err = download(&auth(addr), 7).unwrap_err();
        assert!(
            matches!(&err, WebError::Failed(e) if e.ends_with("reply past 65536 bytes")),
            "{err}"
        );
        server.join().unwrap();
    }
}
