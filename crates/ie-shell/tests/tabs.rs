mod harness;

use harness::BrowserHandle;
use harness::server::TestServer;

#[tokio::test]
async fn test_starts_with_one_tab() {
    let mut browser = BrowserHandle::spawn().await;
    let tabs = browser.get_tabs().await;
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].state, "blank");
    browser.quit().await;
}

#[tokio::test]
async fn test_new_tab() {
    let mut browser = BrowserHandle::spawn().await;
    browser.new_tab().await;
    let tabs = browser.get_tabs().await;
    assert_eq!(tabs.len(), 2);
    browser.quit().await;
}

#[tokio::test]
async fn test_close_tab() {
    let mut browser = BrowserHandle::spawn().await;
    let new_id = browser.new_tab().await;
    let tabs = browser.get_tabs().await;
    assert_eq!(tabs.len(), 2);
    let resp = browser.close_tab(new_id).await;
    assert!(resp["ok"].as_bool().unwrap());
    let tabs = browser.get_tabs().await;
    assert_eq!(tabs.len(), 1);
    browser.quit().await;
}

#[tokio::test]
async fn test_close_nonexistent_tab() {
    let mut browser = BrowserHandle::spawn().await;
    let resp = browser.close_tab(9999).await;
    assert!(!resp["ok"].as_bool().unwrap());
    browser.quit().await;
}

#[tokio::test]
async fn test_switch_tab() {
    let mut browser = BrowserHandle::spawn().await;
    let tabs_before = browser.get_tabs().await;
    let first_id = tabs_before[0].id;
    browser.new_tab().await;
    // Active is now the new tab (second)
    let resp = browser.switch_tab(first_id).await;
    assert!(resp["ok"].as_bool().unwrap());
    // Verify tabs still exist
    let tabs = browser.get_tabs().await;
    assert_eq!(tabs.len(), 2);
    browser.quit().await;
}

#[tokio::test]
async fn test_navigate_different_tabs() {
    let server = TestServer::start();
    let mut browser = BrowserHandle::spawn().await;

    // Tab 1: navigate to hello
    let tabs = browser.get_tabs().await;
    let tab1_id = tabs[0].id;
    browser.navigate(&server.url("/hello.html")).await;

    // New tab 2: navigate to titled
    browser.new_tab().await;
    browser.navigate(&server.url("/titled.html")).await;

    // Active is tab 2 — source should be titled
    let source = browser.get_source().await;
    assert!(source.contains("Titled Page"));

    // Switch to tab 1 — source should be hello
    browser.switch_tab(tab1_id).await;
    let source = browser.get_source().await;
    assert!(source.contains("Hello World"));

    browser.quit().await;
}

#[tokio::test]
async fn test_close_all_tabs() {
    let mut browser = BrowserHandle::spawn().await;
    let tabs = browser.get_tabs().await;
    let first_id = tabs[0].id;
    browser.close_tab(first_id).await;
    // TabManager always keeps at least one tab
    let tabs = browser.get_tabs().await;
    assert_eq!(tabs.len(), 1);
    browser.quit().await;
}
