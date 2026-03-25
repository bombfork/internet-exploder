use std::net::SocketAddr;
use std::path::PathBuf;

pub struct TestServer {
    addr: SocketAddr,
    _handle: std::thread::JoinHandle<()>,
}

impl TestServer {
    pub fn start() -> Self {
        Self::start_with_handler(|path| {
            let fixtures_dir = fixtures_dir();
            let file_path = fixtures_dir.join(path.trim_start_matches('/'));
            if file_path.exists() {
                let body = std::fs::read_to_string(&file_path).unwrap();
                (200, "text/html", body)
            } else {
                (404, "text/html", "Not Found".to_string())
            }
        })
    }

    pub fn start_with_handler<F>(handler: F) -> Self
    where
        F: Fn(&str) -> (u16, &'static str, String) + Send + 'static,
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 4096];
                let n = std::io::Read::read(&mut stream, &mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                let (status, content_type, body) = handler(path);
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            }
        });
        Self {
            addr,
            _handle: handle,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.addr.port())
    }
}

fn fixtures_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Go up from crates/ie-shell to project root
    dir.pop();
    dir.pop();
    dir.push("tests/fixtures");
    dir
}
