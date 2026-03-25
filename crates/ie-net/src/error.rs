#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("TLS error: {0}")]
    TlsError(String),

    #[error("request timed out")]
    Timeout,

    #[error("too many redirects")]
    TooManyRedirects,

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("HTTPS to HTTP downgrade blocked")]
    HttpsDowngrade,

    #[error("plain HTTP blocked (HTTPS-only mode)")]
    HttpBlocked,
}
