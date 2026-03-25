mod harness;

use harness::BrowserHandle;

#[tokio::test]
async fn test_bookmark_add_and_list() {
    let mut browser = BrowserHandle::spawn().await;
    browser.bookmark_add("https://example.com", "Example").await;
    let bookmarks = browser.bookmark_list().await;
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0].url, "https://example.com");
    assert_eq!(bookmarks[0].title, "Example");
    browser.quit().await;
}

#[tokio::test]
async fn test_bookmark_multiple() {
    let mut browser = BrowserHandle::spawn().await;
    browser.bookmark_add("https://example.com", "Example").await;
    browser.bookmark_add("https://rust-lang.org", "Rust").await;
    browser.bookmark_add("https://github.com", "GitHub").await;
    let bookmarks = browser.bookmark_list().await;
    assert_eq!(bookmarks.len(), 3);
    browser.quit().await;
}

#[tokio::test]
async fn test_bookmark_persists_across_restart() {
    let data_dir = tempfile::tempdir().unwrap();
    let data_dir_str = data_dir.path().to_string_lossy().to_string();

    // First session: add bookmark
    {
        let mut browser = BrowserHandle::spawn_with_data_dir(&data_dir_str).await;
        browser.bookmark_add("https://example.com", "Example").await;
        browser.quit().await;
    }

    // Second session: verify bookmark persists
    {
        let mut browser = BrowserHandle::spawn_with_data_dir(&data_dir_str).await;
        let bookmarks = browser.bookmark_list().await;
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].url, "https://example.com");
        browser.quit().await;
    }
}

#[tokio::test]
async fn test_bookmark_isolation() {
    let mut browser_a = BrowserHandle::spawn().await;
    let mut browser_b = BrowserHandle::spawn().await;

    browser_a
        .bookmark_add("https://example.com", "Example")
        .await;

    let bookmarks_b = browser_b.bookmark_list().await;
    assert_eq!(bookmarks_b.len(), 0);

    browser_a.quit().await;
    browser_b.quit().await;
}
