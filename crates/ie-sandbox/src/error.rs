#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("connection closed by peer")]
    ConnectionClosed,

    #[error("message too large: {0} bytes (max {1})")]
    MessageTooLarge(usize, usize),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("deserialization error: {0}")]
    DeserializationError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
