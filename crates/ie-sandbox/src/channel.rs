use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

use crate::error::IpcError;

const LENGTH_PREFIX_SIZE: usize = 4; // u32 big-endian
const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024; // 64 MB

// Unix implementation
#[cfg(unix)]
mod platform {
    use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
    pub type ReadHalf = OwnedReadHalf;
    pub type WriteHalf = OwnedWriteHalf;
}

#[cfg(unix)]
pub use platform::*;

pub struct IpcChannel {
    reader: BufReader<ReadHalf>,
    writer: WriteHalf,
}

impl IpcChannel {
    /// Create a connected pair of IPC channels.
    #[cfg(unix)]
    pub fn pair() -> Result<(IpcChannel, IpcChannel), IpcError> {
        let (a, b) = tokio::net::UnixStream::pair()?;
        let (a_read, a_write) = a.into_split();
        let (b_read, b_write) = b.into_split();
        Ok((
            IpcChannel {
                reader: BufReader::new(a_read),
                writer: a_write,
            },
            IpcChannel {
                reader: BufReader::new(b_read),
                writer: b_write,
            },
        ))
    }

    /// Create a pair suitable for process spawning: returns (parent_channel, child_raw_fd).
    /// The child fd is kept open (not CLOEXEC) so it survives fork+exec.
    #[cfg(unix)]
    pub fn pair_for_spawn() -> Result<(IpcChannel, std::os::unix::io::RawFd), IpcError> {
        use std::os::unix::io::IntoRawFd;
        let (std_a, std_b) = std::os::unix::net::UnixStream::pair()?;
        let child_fd = std_b.into_raw_fd();
        // Clear CLOEXEC on child fd so it survives exec
        unsafe {
            let flags = libc::fcntl(child_fd, libc::F_GETFD);
            libc::fcntl(child_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
        }
        std_a.set_nonblocking(true)?;
        let tok_a = tokio::net::UnixStream::from_std(std_a)?;
        let (a_read, a_write) = tok_a.into_split();
        let parent_channel = IpcChannel {
            reader: BufReader::new(a_read),
            writer: a_write,
        };
        Ok((parent_channel, child_fd))
    }

    /// Reconstruct a channel from a raw file descriptor (for child processes).
    #[cfg(unix)]
    pub fn from_raw_fd(fd: std::os::unix::io::RawFd) -> Result<Self, IpcError> {
        use std::os::unix::io::FromRawFd;
        let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(fd) };
        std_stream.set_nonblocking(true)?;
        let stream = tokio::net::UnixStream::from_std(std_stream)?;
        let (read, write) = stream.into_split();
        Ok(IpcChannel {
            reader: BufReader::new(read),
            writer: write,
        })
    }

    /// Send a serializable message with length prefix.
    pub async fn send<T: Serialize>(&mut self, msg: &T) -> Result<(), IpcError> {
        let payload =
            serde_json::to_vec(msg).map_err(|e| IpcError::SerializationError(e.to_string()))?;
        if payload.len() > MAX_MESSAGE_SIZE {
            return Err(IpcError::MessageTooLarge(payload.len(), MAX_MESSAGE_SIZE));
        }
        self.writer
            .write_all(&(payload.len() as u32).to_be_bytes())
            .await?;
        self.writer.write_all(&payload).await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Receive a deserialized message with length prefix.
    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T, IpcError> {
        let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
        match self.reader.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(IpcError::ConnectionClosed);
            }
            Err(e) => return Err(IpcError::Io(e)),
        }
        let size = u32::from_be_bytes(len_buf) as usize;
        if size > MAX_MESSAGE_SIZE {
            return Err(IpcError::MessageTooLarge(size, MAX_MESSAGE_SIZE));
        }
        let mut buf = vec![0u8; size];
        match self.reader.read_exact(&mut buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(IpcError::ConnectionClosed);
            }
            Err(e) => return Err(IpcError::Io(e)),
        }
        serde_json::from_slice(&buf).map_err(|e| IpcError::DeserializationError(e.to_string()))
    }

    /// Send raw bytes with length prefix (for testing malformed messages).
    #[cfg(test)]
    pub(crate) async fn send_raw(&mut self, data: &[u8]) -> Result<(), IpcError> {
        self.writer
            .write_all(&(data.len() as u32).to_be_bytes())
            .await?;
        self.writer.write_all(data).await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Write raw bytes directly to the writer (for testing oversized length prefixes).
    #[cfg(test)]
    pub(crate) async fn write_raw(&mut self, data: &[u8]) -> Result<(), IpcError> {
        self.writer.write_all(data).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::message::IpcMessage;

    use super::*;

    #[tokio::test]
    async fn pair_creation() {
        let (_a, _b) = IpcChannel::pair().unwrap();
    }

    #[tokio::test]
    async fn simple_send_recv() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        a.send(&IpcMessage::Ping).await.unwrap();
        let msg: IpcMessage = b.recv().await.unwrap();
        assert_eq!(msg, IpcMessage::Ping);
    }

    #[tokio::test]
    async fn round_trip_struct() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        let req = IpcMessage::FetchRequest {
            id: 42,
            url: "https://example.com".to_string(),
        };
        a.send(&req).await.unwrap();
        let msg: IpcMessage = b.recv().await.unwrap();
        assert_eq!(msg, req);
    }

    #[tokio::test]
    async fn bidirectional() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        a.send(&IpcMessage::Ping).await.unwrap();
        let msg: IpcMessage = b.recv().await.unwrap();
        assert_eq!(msg, IpcMessage::Ping);
        b.send(&IpcMessage::Pong).await.unwrap();
        let msg: IpcMessage = a.recv().await.unwrap();
        assert_eq!(msg, IpcMessage::Pong);
    }

    #[tokio::test]
    async fn multiple_messages() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        for i in 0..100 {
            a.send(&IpcMessage::FetchRequest {
                id: i,
                url: format!("https://example.com/{i}"),
            })
            .await
            .unwrap();
        }
        for i in 0..100 {
            let msg: IpcMessage = b.recv().await.unwrap();
            assert_eq!(
                msg,
                IpcMessage::FetchRequest {
                    id: i,
                    url: format!("https://example.com/{i}"),
                }
            );
        }
    }

    #[tokio::test]
    async fn large_fetch_response() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        let body = vec![0xABu8; 100_000]; // 100KB
        let msg = IpcMessage::FetchResponse {
            id: 1,
            status: 200,
            headers: HashMap::new(),
            body: body.clone(),
            final_url: "https://example.com".to_string(),
        };
        a.send(&msg).await.unwrap();
        let received: IpcMessage = b.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn connection_closed_sender_drops() {
        let (a, mut b) = IpcChannel::pair().unwrap();
        drop(a);
        let result: Result<IpcMessage, _> = b.recv().await;
        assert!(matches!(result, Err(IpcError::ConnectionClosed)));
    }

    #[tokio::test]
    async fn connection_closed_receiver_drops() {
        let (mut a, b) = IpcChannel::pair().unwrap();
        drop(b);
        // Sending may succeed (buffered) or fail — the key is it doesn't hang
        let _ = a.send(&IpcMessage::Ping).await;
        // A second send after the pipe is broken should definitely fail
        let result = a.send(&IpcMessage::Ping).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn deserialization_error() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        a.send_raw(b"not valid json").await.unwrap();
        let result: Result<IpcMessage, _> = b.recv().await;
        assert!(matches!(result, Err(IpcError::DeserializationError(_))));
    }

    #[tokio::test]
    async fn zero_length_message() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        // Write a zero-length prefix
        a.write_raw(&0u32.to_be_bytes()).await.unwrap();
        let result: Result<IpcMessage, _> = b.recv().await;
        assert!(matches!(result, Err(IpcError::DeserializationError(_))));
    }

    #[tokio::test]
    async fn message_too_large() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        // Write a length prefix claiming 128MB
        let huge_size = (128 * 1024 * 1024u32).to_be_bytes();
        a.write_raw(&huge_size).await.unwrap();
        let result: Result<IpcMessage, _> = b.recv().await;
        assert!(matches!(result, Err(IpcError::MessageTooLarge(_, _))));
    }

    #[tokio::test]
    async fn concurrent_send_recv() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        let n = 100;

        let sender = tokio::spawn(async move {
            for i in 0..n {
                a.send(&IpcMessage::FetchRequest {
                    id: i,
                    url: format!("https://example.com/{i}"),
                })
                .await
                .unwrap();
            }
        });

        let receiver = tokio::spawn(async move {
            for i in 0..n {
                let msg: IpcMessage = b.recv().await.unwrap();
                assert_eq!(
                    msg,
                    IpcMessage::FetchRequest {
                        id: i,
                        url: format!("https://example.com/{i}"),
                    }
                );
            }
        });

        sender.await.unwrap();
        receiver.await.unwrap();
    }

    #[tokio::test]
    async fn interleaved_ping_pong() {
        let (mut a, mut b) = IpcChannel::pair().unwrap();
        a.send(&IpcMessage::Ping).await.unwrap();
        let msg: IpcMessage = b.recv().await.unwrap();
        assert_eq!(msg, IpcMessage::Ping);
        b.send(&IpcMessage::Pong).await.unwrap();
        let msg: IpcMessage = a.recv().await.unwrap();
        assert_eq!(msg, IpcMessage::Pong);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn from_raw_fd_round_trip() {
        use std::os::unix::io::AsRawFd;

        // Create a Unix socketpair manually
        let (std_a, std_b) = std::os::unix::net::UnixStream::pair().unwrap();
        let fd_b = std_b.as_raw_fd();

        // Reconstruct channel from raw fd
        std_a.set_nonblocking(true).unwrap();
        std_b.set_nonblocking(true).unwrap();
        let tok_a = tokio::net::UnixStream::from_std(std_a).unwrap();
        let (a_read, a_write) = tok_a.into_split();
        let mut chan_a = IpcChannel {
            reader: BufReader::new(a_read),
            writer: a_write,
        };

        // Leak std_b so from_raw_fd can take ownership
        std::mem::forget(std_b);
        let mut chan_b = IpcChannel::from_raw_fd(fd_b).unwrap();

        chan_a.send(&IpcMessage::Ping).await.unwrap();
        let msg: IpcMessage = chan_b.recv().await.unwrap();
        assert_eq!(msg, IpcMessage::Ping);
    }
}
