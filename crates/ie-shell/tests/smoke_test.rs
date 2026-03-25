use std::io::Write;
use std::net::SocketAddr;
use std::process::{Command, Stdio};

fn binary_path() -> String {
    let mut path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    path.push("ie-shell");
    path.to_string_lossy().to_string()
}

// --- Test server helper ---

fn start_test_server(
    body: &'static str,
    status: u16,
    content_type: &'static str,
) -> (SocketAddr, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        // Accept connections in a loop, serve simple HTTP
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });
    (addr, handle)
}

// --- One-shot headless tests ---

#[test]
fn dump_status_200() {
    let (addr, _handle) = start_test_server("<html>ok</html>", 200, "text/html");
    let output = Command::new(binary_path())
        .args([
            "--headless",
            "--dump-status",
            "--url",
            &format!("http://{addr}/"),
            "--allow-http",
        ])
        .output()
        .expect("failed to run binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "200");
}

#[test]
fn dump_status_404() {
    let (addr, _handle) = start_test_server("not found", 404, "text/html");
    let output = Command::new(binary_path())
        .args([
            "--headless",
            "--dump-status",
            "--url",
            &format!("http://{addr}/"),
            "--allow-http",
        ])
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "404");
}

#[test]
fn dump_source_matches() {
    let (addr, _handle) = start_test_server("<html>test</html>", 200, "text/html");
    let output = Command::new(binary_path())
        .args([
            "--headless",
            "--dump-source",
            "--url",
            &format!("http://{addr}/"),
            "--allow-http",
        ])
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "<html>test</html>");
}

#[test]
fn dump_source_error() {
    let output = Command::new(binary_path())
        .args([
            "--headless",
            "--dump-source",
            "--url",
            "http://127.0.0.1:1/",
            "--allow-http",
        ])
        .output()
        .expect("failed to run binary");
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).is_empty());
}

// --- Interactive headless tests ---

fn interactive_session(commands: &[&str], allow_http: bool) -> Vec<serde_json::Value> {
    let mut child = Command::new(binary_path())
        .args(if allow_http {
            vec!["--headless", "--allow-http"]
        } else {
            vec!["--headless"]
        })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    let mut stdin = child.stdin.take().unwrap();
    for cmd in commands {
        writeln!(stdin, "{cmd}").unwrap();
    }
    drop(stdin);

    let output = child.wait_with_output().expect("failed to wait");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("invalid JSON response"))
        .collect()
}

#[test]
fn interactive_navigate() {
    let (addr, _handle) = start_test_server("<html>hello</html>", 200, "text/html");
    let responses = interactive_session(
        &[
            &format!(r#"{{"cmd":"navigate","url":"http://{addr}/"}}"#),
            r#"{"cmd":"get_source"}"#,
            r#"{"cmd":"quit"}"#,
        ],
        true,
    );
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[0]["data"]["status"], 200);
    assert_eq!(responses[1]["ok"], true);
    assert_eq!(responses[1]["data"], "<html>hello</html>");
    assert_eq!(responses[2]["ok"], true);
}

#[test]
fn interactive_tabs() {
    let responses = interactive_session(
        &[
            r#"{"cmd":"new_tab"}"#,
            r#"{"cmd":"get_tabs"}"#,
            &format!(r#"{{"cmd":"close_tab","id":{}}}"#, 1), // close the new tab (id=1)
            r#"{"cmd":"get_tabs"}"#,
            r#"{"cmd":"quit"}"#,
        ],
        false,
    );
    assert_eq!(responses[0]["ok"], true); // new_tab
    assert_eq!(responses[1]["data"].as_array().unwrap().len(), 2); // 2 tabs
    assert_eq!(responses[2]["ok"], true); // close_tab
    assert_eq!(responses[3]["data"].as_array().unwrap().len(), 1); // 1 tab
}

#[test]
fn interactive_switch_tab() {
    let responses = interactive_session(
        &[
            r#"{"cmd":"new_tab"}"#,
            r#"{"cmd":"switch_tab","id":0}"#,
            r#"{"cmd":"get_tabs"}"#,
            r#"{"cmd":"quit"}"#,
        ],
        false,
    );
    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[1]["ok"], true);
    // Verify we can read tabs after switching
    assert_eq!(responses[2]["ok"], true);
    assert_eq!(responses[2]["data"].as_array().unwrap().len(), 2);
}

#[test]
fn interactive_go_back_forward() {
    let (addr, _handle) = start_test_server("<html>page</html>", 200, "text/html");
    let responses = interactive_session(
        &[
            &format!(r#"{{"cmd":"navigate","url":"http://{addr}/a"}}"#),
            &format!(r#"{{"cmd":"navigate","url":"http://{addr}/b"}}"#),
            r#"{"cmd":"go_back"}"#,
            r#"{"cmd":"go_forward"}"#,
            r#"{"cmd":"quit"}"#,
        ],
        true,
    );
    assert_eq!(responses[0]["ok"], true); // navigate A
    assert_eq!(responses[1]["ok"], true); // navigate B
    assert_eq!(responses[2]["ok"], true); // go_back
    assert!(responses[2]["data"]["url"].as_str().unwrap().contains("/a"));
    assert_eq!(responses[3]["ok"], true); // go_forward
    assert!(responses[3]["data"]["url"].as_str().unwrap().contains("/b"));
}

#[test]
fn interactive_bookmarks() {
    let responses = interactive_session(
        &[
            r#"{"cmd":"bookmark_add","url":"https://example.com","title":"Example"}"#,
            r#"{"cmd":"bookmark_list"}"#,
            r#"{"cmd":"quit"}"#,
        ],
        false,
    );
    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[1]["ok"], true);
    let bookmarks = responses[1]["data"].as_array().unwrap();
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0]["url"], "https://example.com");
}

#[test]
fn interactive_invalid_json() {
    let responses = interactive_session(
        &[
            "this is not json",
            r#"{"cmd":"quit"}"#, // should still work after error
        ],
        false,
    );
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["ok"], false);
    assert!(responses[0]["error"].as_str().unwrap().contains("invalid"));
    assert_eq!(responses[1]["ok"], true);
}

#[test]
fn interactive_quit() {
    let responses = interactive_session(&[r#"{"cmd":"quit"}"#], false);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["ok"], true);
}
