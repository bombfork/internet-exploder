use std::path::PathBuf;
use std::time::Duration;

use ie_sandbox::message::IpcMessage;
use ie_sandbox::{ProcessKind, spawn_child_with_exe};

fn ie_shell_binary() -> PathBuf {
    let mut path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    path.push("ie-shell");
    path
}

async fn spawn(kind: ProcessKind) -> ie_sandbox::ChildHandle {
    spawn_child_with_exe(kind, ie_shell_binary())
        .await
        .expect("failed to spawn child")
}

#[tokio::test]
async fn network_ping_pong() {
    let mut child = spawn(ProcessKind::Network).await;
    child.channel().send(&IpcMessage::Ping).await.unwrap();
    let msg: IpcMessage = child.channel().recv().await.unwrap();
    assert_eq!(msg, IpcMessage::Pong);
    child.shutdown().await.unwrap();
}

#[tokio::test]
async fn renderer_ping_pong() {
    let mut child = spawn(ProcessKind::Renderer).await;
    child.channel().send(&IpcMessage::Ping).await.unwrap();
    let msg: IpcMessage = child.channel().recv().await.unwrap();
    assert_eq!(msg, IpcMessage::Pong);
    child.shutdown().await.unwrap();
}

#[tokio::test]
async fn is_alive_before_and_after_shutdown() {
    let mut child = spawn(ProcessKind::Renderer).await;
    assert!(child.is_alive());
    child.shutdown().await.unwrap();
    assert!(!child.is_alive());
}

#[tokio::test]
async fn graceful_shutdown() {
    let mut child = spawn(ProcessKind::Network).await;
    child.shutdown().await.unwrap();
    assert!(!child.is_alive());
}

#[tokio::test]
async fn drop_cleanup() {
    let child = spawn(ProcessKind::Renderer).await;
    let pid = child.process_id();
    drop(child);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
    assert!(!alive, "process should be killed on drop");
}

#[tokio::test]
async fn network_fetch_via_ipc() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let _server = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            let body = "<html>hello from ipc</html>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });

    let mut child = spawn(ProcessKind::Network).await;
    child
        .channel()
        .send(&IpcMessage::FetchRequest {
            id: 1,
            url: format!("http://{addr}/hello"),
        })
        .await
        .unwrap();
    let msg: IpcMessage = child.channel().recv().await.unwrap();
    match msg {
        IpcMessage::FetchResponse {
            id, status, body, ..
        } => {
            assert_eq!(id, 1);
            assert_eq!(status, 200);
            assert_eq!(
                String::from_utf8_lossy(&body),
                "<html>hello from ipc</html>"
            );
        }
        other => panic!("expected FetchResponse, got: {other:?}"),
    }
    child.shutdown().await.unwrap();
}

#[tokio::test]
async fn network_fetch_error() {
    let mut child = spawn(ProcessKind::Network).await;
    child
        .channel()
        .send(&IpcMessage::FetchRequest {
            id: 2,
            url: "http://127.0.0.1:1/nothing".to_string(),
        })
        .await
        .unwrap();
    let msg: IpcMessage = child.channel().recv().await.unwrap();
    assert!(matches!(msg, IpcMessage::FetchError { id: 2, .. }));
    child.shutdown().await.unwrap();
}

#[tokio::test]
async fn network_invalid_url() {
    let mut child = spawn(ProcessKind::Network).await;
    child
        .channel()
        .send(&IpcMessage::FetchRequest {
            id: 3,
            url: "not a valid url".to_string(),
        })
        .await
        .unwrap();
    let msg: IpcMessage = child.channel().recv().await.unwrap();
    assert!(matches!(msg, IpcMessage::FetchError { id: 3, .. }));
    child.shutdown().await.unwrap();
}

#[tokio::test]
async fn network_multiple_sequential_requests() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let _server = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);
            let body = "ok";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });

    let mut child = spawn(ProcessKind::Network).await;
    for i in 1..=3u64 {
        child
            .channel()
            .send(&IpcMessage::FetchRequest {
                id: i,
                url: format!("http://{addr}/{i}"),
            })
            .await
            .unwrap();
        let msg: IpcMessage = child.channel().recv().await.unwrap();
        match msg {
            IpcMessage::FetchResponse { id, status, .. } => {
                assert_eq!(id, i);
                assert_eq!(status, 200);
            }
            other => panic!("expected FetchResponse for id={i}, got: {other:?}"),
        }
    }
    child.shutdown().await.unwrap();
}
