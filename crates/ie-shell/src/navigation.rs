use async_trait::async_trait;
use url::Url;

#[derive(Debug, Clone)]
pub struct NavigationResult {
    pub status: u16,
    pub final_url: Url,
    pub body: Vec<u8>,
    #[allow(dead_code)]
    pub content_type: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum NavigationError {
    #[error("network error: {0}")]
    Net(#[from] ie_net::NetError),

    #[allow(dead_code)]
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("invalid scheme: {0}")]
    InvalidScheme(String),

    #[error("plain HTTP blocked (HTTPS-only mode)")]
    HttpBlocked,

    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
}

#[async_trait]
pub trait NavigationService: Send + Sync {
    async fn navigate(&self, url: &Url) -> Result<NavigationResult, NavigationError>;
}

pub struct InProcessNavigator {
    client: ie_net::Client,
    https_only: bool,
}

impl InProcessNavigator {
    pub fn new() -> Result<Self, ie_net::NetError> {
        Ok(Self {
            client: ie_net::Client::new()?.with_https_only(false),
            https_only: true,
        })
    }

    pub fn with_https_only(mut self, https_only: bool) -> Self {
        self.https_only = https_only;
        self
    }
}

#[async_trait]
impl NavigationService for InProcessNavigator {
    async fn navigate(&self, url: &Url) -> Result<NavigationResult, NavigationError> {
        if self.https_only && url.scheme() == "http" {
            return Err(NavigationError::HttpBlocked);
        }
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(NavigationError::InvalidScheme(url.scheme().to_string()));
        }
        let response = self.client.get(url).await?;
        let content_type = response.headers.get("content-type").cloned();
        if let Some(ref ct) = content_type
            && !ct.starts_with("text/html")
        {
            return Err(NavigationError::UnsupportedContentType(ct.clone()));
        }
        Ok(NavigationResult {
            status: response.status,
            final_url: response.url,
            body: response.body,
            content_type,
        })
    }
}

// Verify object safety at compile time
const _: () = {
    fn _assert_object_safe(_: std::sync::Arc<dyn NavigationService>) {}
};

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::net::SocketAddr;

    use bytes::Bytes;
    use http_body_util::Full;
    use hyper::Response as HyperResponse;
    use hyper::body::Incoming;
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper_util::rt::TokioIo;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    use super::*;

    async fn start_test_server<F, Fut>(handler: F) -> (SocketAddr, JoinHandle<()>)
    where
        F: Fn(hyper::Request<Incoming>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HyperResponse<Full<Bytes>>, Infallible>> + Send,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handler = std::sync::Arc::new(handler);
        let handle = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let handler = handler.clone();
                tokio::spawn(async move {
                    let svc = service_fn(move |req| {
                        let handler = handler.clone();
                        async move { handler(req).await }
                    });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), svc)
                        .await;
                });
            }
        });
        (addr, handle)
    }

    fn html_response(body: &str) -> Result<HyperResponse<Full<Bytes>>, Infallible> {
        Ok(HyperResponse::builder()
            .header("content-type", "text/html")
            .body(Full::new(Bytes::from(body.to_string())))
            .unwrap())
    }

    #[tokio::test]
    async fn navigator_new_succeeds() {
        InProcessNavigator::new().unwrap();
    }

    #[tokio::test]
    async fn navigate_to_local_server() {
        let (addr, _handle) =
            start_test_server(|_req| async { html_response("<html>hello</html>") }).await;
        let nav = InProcessNavigator::new().unwrap().with_https_only(false);
        let url = Url::parse(&format!("http://{addr}/")).unwrap();
        let result = nav.navigate(&url).await.unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body, b"<html>hello</html>");
        assert_eq!(result.final_url, url);
        assert_eq!(result.content_type.as_deref(), Some("text/html"));
    }

    #[tokio::test]
    async fn navigate_invalid_scheme() {
        let nav = InProcessNavigator::new().unwrap();
        let url = Url::parse("ftp://example.com").unwrap();
        let err = nav.navigate(&url).await.unwrap_err();
        assert!(matches!(err, NavigationError::InvalidScheme(_)));
    }

    #[tokio::test]
    async fn https_only_blocks_http() {
        let nav = InProcessNavigator::new().unwrap(); // https_only=true by default
        let url = Url::parse("http://example.com").unwrap();
        let err = nav.navigate(&url).await.unwrap_err();
        assert!(matches!(err, NavigationError::HttpBlocked));
    }

    #[tokio::test]
    async fn https_only_disabled_allows_http() {
        let (addr, _handle) =
            start_test_server(|_req| async { html_response("<html>ok</html>") }).await;
        let nav = InProcessNavigator::new().unwrap().with_https_only(false);
        let url = Url::parse(&format!("http://{addr}/")).unwrap();
        let result = nav.navigate(&url).await.unwrap();
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn non_html_content_type_rejected() {
        let (addr, _handle) = start_test_server(|_req| async {
            Ok(HyperResponse::builder()
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from("{}")))
                .unwrap())
        })
        .await;
        let nav = InProcessNavigator::new().unwrap().with_https_only(false);
        let url = Url::parse(&format!("http://{addr}/")).unwrap();
        let err = nav.navigate(&url).await.unwrap_err();
        assert!(matches!(err, NavigationError::UnsupportedContentType(_)));
    }

    #[tokio::test]
    async fn missing_content_type_succeeds() {
        let (addr, _handle) = start_test_server(|_req| async {
            Ok(HyperResponse::new(Full::new(Bytes::from(
                "<html>no ct</html>",
            ))))
        })
        .await;
        let nav = InProcessNavigator::new().unwrap().with_https_only(false);
        let url = Url::parse(&format!("http://{addr}/")).unwrap();
        let result = nav.navigate(&url).await.unwrap();
        assert!(result.content_type.is_none());
    }
}
