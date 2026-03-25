pub mod server;

use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

const CMD_TIMEOUT: Duration = Duration::from_secs(10);

pub struct BrowserHandle {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    reader: BufReader<ChildStdout>,
    _data_dir: tempfile::TempDir,
}

#[derive(Debug, Deserialize)]
pub struct NavigateResponse {
    pub status: u64,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct TabInfo {
    pub id: u64,
    pub url: Option<String>,
    pub title: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct BookmarkInfo {
    pub url: String,
    pub title: String,
}

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

impl BrowserHandle {
    pub async fn spawn() -> Self {
        Self::spawn_with_args(&["--allow-http"]).await
    }

    pub async fn spawn_https_only() -> Self {
        Self::spawn_with_args(&[]).await
    }

    pub async fn spawn_with_data_dir(data_dir: &str) -> Self {
        let child_result = Command::new(binary_path())
            .args(["--headless", "--allow-http", "--data-dir", data_dir])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn();
        let mut child = child_result.expect("failed to spawn ie-shell");
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let reader = BufReader::new(child.stdout.take().unwrap());
        // Use a dummy TempDir that won't be used for actual storage
        let _data_dir = tempfile::tempdir().unwrap();
        Self {
            child,
            stdin,
            reader,
            _data_dir,
        }
    }

    async fn spawn_with_args(extra_args: &[&str]) -> Self {
        let data_dir = tempfile::tempdir().unwrap();
        let data_dir_str = data_dir.path().to_string_lossy().to_string();
        let mut args = vec!["--headless", "--data-dir", &data_dir_str];
        args.extend(extra_args);
        let child_result = Command::new(binary_path())
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn();
        let mut child = child_result.expect("failed to spawn ie-shell");
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let reader = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            reader,
            _data_dir: data_dir,
        }
    }

    pub async fn send_command(&mut self, cmd: serde_json::Value) -> serde_json::Value {
        let json = serde_json::to_string(&cmd).unwrap();
        self.stdin
            .write_all(json.as_bytes())
            .await
            .expect("write to stdin");
        self.stdin.write_all(b"\n").await.expect("write newline");
        self.stdin.flush().await.expect("flush stdin");

        let mut line = String::new();
        timeout(CMD_TIMEOUT, self.reader.read_line(&mut line))
            .await
            .unwrap_or_else(|_| panic!("command timed out after {CMD_TIMEOUT:?}: {cmd}"))
            .expect("read from stdout");

        serde_json::from_str(&line).unwrap_or_else(|e| {
            panic!("invalid JSON response: {e}\nraw: {line}\ncommand was: {cmd}")
        })
    }

    pub async fn navigate(&mut self, url: &str) -> NavigateResponse {
        let resp = self
            .send_command(serde_json::json!({"cmd": "navigate", "url": url}))
            .await;
        assert!(
            resp["ok"].as_bool().unwrap(),
            "navigate failed: {}",
            resp["error"]
        );
        serde_json::from_value(resp["data"].clone()).unwrap()
    }

    pub async fn navigate_raw(&mut self, url: &str) -> serde_json::Value {
        self.send_command(serde_json::json!({"cmd": "navigate", "url": url}))
            .await
    }

    pub async fn get_source(&mut self) -> String {
        let resp = self
            .send_command(serde_json::json!({"cmd": "get_source"}))
            .await;
        assert!(
            resp["ok"].as_bool().unwrap(),
            "get_source failed: {}",
            resp["error"]
        );
        resp["data"].as_str().unwrap().to_string()
    }

    pub async fn get_tabs(&mut self) -> Vec<TabInfo> {
        let resp = self
            .send_command(serde_json::json!({"cmd": "get_tabs"}))
            .await;
        assert!(
            resp["ok"].as_bool().unwrap(),
            "get_tabs failed: {}",
            resp["error"]
        );
        serde_json::from_value(resp["data"].clone()).unwrap()
    }

    pub async fn new_tab(&mut self) -> u64 {
        let resp = self
            .send_command(serde_json::json!({"cmd": "new_tab"}))
            .await;
        assert!(
            resp["ok"].as_bool().unwrap(),
            "new_tab failed: {}",
            resp["error"]
        );
        resp["data"]["id"].as_u64().unwrap()
    }

    pub async fn close_tab(&mut self, id: u64) -> serde_json::Value {
        self.send_command(serde_json::json!({"cmd": "close_tab", "id": id}))
            .await
    }

    pub async fn switch_tab(&mut self, id: u64) -> serde_json::Value {
        self.send_command(serde_json::json!({"cmd": "switch_tab", "id": id}))
            .await
    }

    pub async fn go_back(&mut self) -> serde_json::Value {
        self.send_command(serde_json::json!({"cmd": "go_back"}))
            .await
    }

    pub async fn go_forward(&mut self) -> serde_json::Value {
        self.send_command(serde_json::json!({"cmd": "go_forward"}))
            .await
    }

    pub async fn bookmark_add(&mut self, url: &str, title: &str) {
        let resp = self
            .send_command(serde_json::json!({"cmd": "bookmark_add", "url": url, "title": title}))
            .await;
        assert!(
            resp["ok"].as_bool().unwrap(),
            "bookmark_add failed: {}",
            resp["error"]
        );
    }

    pub async fn bookmark_list(&mut self) -> Vec<BookmarkInfo> {
        let resp = self
            .send_command(serde_json::json!({"cmd": "bookmark_list"}))
            .await;
        assert!(
            resp["ok"].as_bool().unwrap(),
            "bookmark_list failed: {}",
            resp["error"]
        );
        serde_json::from_value(resp["data"].clone()).unwrap()
    }

    pub async fn quit(&mut self) {
        let _ = self.send_command(serde_json::json!({"cmd": "quit"})).await;
        let _ = timeout(Duration::from_secs(2), self.child.wait()).await;
    }
}

impl Drop for BrowserHandle {
    fn drop(&mut self) {
        // Best-effort kill
        let _ = self.child.start_kill();
    }
}
