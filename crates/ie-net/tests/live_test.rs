use ie_net::Client;
use url::Url;

#[tokio::test]
#[ignore]
async fn fetch_example_com() {
    let client = Client::new().unwrap();
    let url = Url::parse("https://example.com").unwrap();
    let resp = client.get(&url).await.unwrap();
    assert_eq!(resp.status, 200);
    let body = resp.body_text().unwrap().to_lowercase();
    assert!(body.contains("<html"));
}
