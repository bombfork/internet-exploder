use std::process::Command;

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

#[test]
fn headless_dump_status_exits_cleanly() {
    let output = Command::new(binary_path())
        .args([
            "--headless",
            "--dump-status",
            "--url",
            "https://example.com",
        ])
        .output()
        .expect("failed to run binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn headless_interactive_exits_cleanly() {
    let output = Command::new(binary_path())
        .args(["--headless"])
        .output()
        .expect("failed to run binary");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
