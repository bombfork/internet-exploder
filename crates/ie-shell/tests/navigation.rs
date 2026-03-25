mod harness;

use harness::BrowserHandle;
use harness::server::TestServer;

#[tokio::test]
async fn test_navigate_and_get_source() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    browser.navigate(&server.url("/hello.html")).await;
    let source = browser.get_source().await;
    assert!(source.contains("Hello World"));
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_status_code() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    let resp = browser.navigate(&server.url("/hello.html")).await;
    assert_eq!(resp.status, 200);
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_404() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    let resp = browser.navigate(&server.url("/nonexistent")).await;
    assert_eq!(resp.status, 404);
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_updates_url() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    let url = server.url("/hello.html");
    browser.navigate(&url).await;
    let tabs = browser.get_tabs().await;
    assert_eq!(tabs.len(), 1);
    assert!(tabs[0].url.as_ref().unwrap().contains("/hello.html"));
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_redirect() {
    let server = TestServer::start_with_handler(|path| {
        match path {
        "/redirect" => (
            302,
            "text/html",
            String::new(),
        ),
        "/hello.html" => (
            200,
            "text/html",
            "<!DOCTYPE html><html><head><title>Hello</title></head><body><p>Hello World</p></body></html>\n".to_string(),
        ),
        _ => (404, "text/html", "Not Found".to_string()),
    }
    });
    // The 302 needs a Location header. Let me use a custom raw handler instead.
    drop(server);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let _handle = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let n = std::io::Read::read(&mut stream, &mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);
            let path = request
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/");
            let response = if path == "/redirect" {
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    addr.port()
                )
            } else {
                let body = "<html><body>Redirected</body></html>";
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
            };
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });

    let mut browser = BrowserHandle::spawn().await;
    browser
        .navigate(&format!("http://127.0.0.1:{}/redirect", addr.port()))
        .await;
    let source = browser.get_source().await;
    assert!(source.contains("Redirected"));
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_twice() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;

    browser.navigate(&server.url("/hello.html")).await;
    let source = browser.get_source().await;
    assert!(source.contains("Hello World"));

    browser.navigate(&server.url("/titled.html")).await;
    let source = browser.get_source().await;
    assert!(source.contains("Titled Page"));

    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_large_page() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    browser.navigate(&server.url("/large.html")).await;
    let source = browser.get_source().await;
    assert!(source.len() > 100_000);
    assert!(source.contains("Large Page"));
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_invalid_url() {
    let mut browser = BrowserHandle::spawn().await;
    let resp = browser.navigate_raw("not a valid url at all").await;
    assert!(!resp["ok"].as_bool().unwrap());
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_connection_refused() {
    let mut browser = BrowserHandle::spawn().await;
    let resp = browser.navigate_raw("http://127.0.0.1:1/").await;
    assert!(!resp["ok"].as_bool().unwrap());
    assert!(resp["error"].as_str().unwrap().len() > 0);
    browser.quit().await;
}

// --- Navigation history tests ---

#[tokio::test]
async fn test_navigate_go_back() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    browser.navigate(&server.url("/hello.html")).await;
    browser.navigate(&server.url("/titled.html")).await;
    let resp = browser.go_back().await;
    assert!(resp["ok"].as_bool().unwrap());
    // Source should be restored to hello page
    let source = browser.get_source().await;
    assert!(source.contains("Hello World"));
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_go_forward() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    browser.navigate(&server.url("/hello.html")).await;
    browser.navigate(&server.url("/titled.html")).await;
    browser.go_back().await;
    let resp = browser.go_forward().await;
    assert!(resp["ok"].as_bool().unwrap());
    let source = browser.get_source().await;
    assert!(source.contains("Titled Page"));
    browser.quit().await;
}

#[tokio::test]
async fn test_go_back_at_start() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    browser.navigate(&server.url("/hello.html")).await;
    let resp = browser.go_back().await;
    assert!(!resp["ok"].as_bool().unwrap());
    browser.quit().await;
}

#[tokio::test]
async fn test_go_forward_at_end() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;
    browser.navigate(&server.url("/hello.html")).await;
    let resp = browser.go_forward().await;
    assert!(!resp["ok"].as_bool().unwrap());
    browser.quit().await;
}

// --- HTTPS-only test ---

#[tokio::test]
async fn test_https_only_blocks_http() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn_https_only().await;
    let resp = browser.navigate_raw(&server.url("/hello.html")).await;
    assert!(!resp["ok"].as_bool().unwrap());
    assert!(
        resp["error"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("http")
    );
    browser.quit().await;
}
