use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use ie_sandbox::IpcChannel;
use ie_sandbox::message::IpcMessage;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use url::Url;

use crate::navigation::{NavigationError, NavigationResult, NavigationService};

const NAVIGATE_TIMEOUT: Duration = Duration::from_secs(30);

type PendingMap =
    Arc<Mutex<HashMap<u64, oneshot::Sender<Result<NavigationResult, NavigationError>>>>>;

pub struct IpcNavigator {
    send_tx: mpsc::Sender<IpcMessage>,
    pending: PendingMap,
    next_request_id: AtomicU64,
    https_only: bool,
    _writer_task: JoinHandle<()>,
    _reader_task: JoinHandle<()>,
}

impl IpcNavigator {
    pub fn new(channel: IpcChannel, https_only: bool) -> Self {
        let (mut sender, mut receiver) = channel.into_halves();
        let (send_tx, mut send_rx) = mpsc::channel::<IpcMessage>(64);
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

        // Writer task: forwards messages to the IPC channel
        let writer_task = tokio::spawn(async move {
            while let Some(msg) = send_rx.recv().await {
                if let Err(e) = sender.send(&msg).await {
                    tracing::error!("IPC write error: {e}");
                    break;
                }
            }
        });

        // Reader task: dispatches responses to pending requests
        let reader_pending = pending.clone();
        let reader_task = tokio::spawn(async move {
            loop {
                match receiver.recv::<IpcMessage>().await {
                    Ok(IpcMessage::FetchResponse {
                        id,
                        status,
                        headers,
                        body,
                        final_url,
                    }) => {
                        let result = validate_response(status, &final_url, &headers, body);
                        let mut pending = reader_pending.lock().await;
                        if let Some(tx) = pending.remove(&id) {
                            let _ = tx.send(result);
                        }
                    }
                    Ok(IpcMessage::FetchError { id, error }) => {
                        let mut pending = reader_pending.lock().await;
                        if let Some(tx) = pending.remove(&id) {
                            let _ = tx.send(Err(NavigationError::Net(
                                ie_net::NetError::ConnectionFailed(error),
                            )));
                        }
                    }
                    Ok(IpcMessage::Pong) => {}
                    Ok(other) => {
                        tracing::warn!("IPC navigator received unexpected message: {other:?}");
                    }
                    Err(ie_sandbox::IpcError::ConnectionClosed) => {
                        tracing::error!("network process disconnected");
                        let mut pending = reader_pending.lock().await;
                        for (_, tx) in pending.drain() {
                            let _ = tx.send(Err(NavigationError::Net(
                                ie_net::NetError::ConnectionFailed(
                                    "network process disconnected".into(),
                                ),
                            )));
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::error!("IPC read error: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            send_tx,
            pending,
            next_request_id: AtomicU64::new(1),
            https_only,
            _writer_task: writer_task,
            _reader_task: reader_task,
        }
    }
}

fn validate_response(
    status: u16,
    final_url: &str,
    headers: &HashMap<String, String>,
    body: Vec<u8>,
) -> Result<NavigationResult, NavigationError> {
    if !(100..=599).contains(&status) {
        return Err(NavigationError::Net(ie_net::NetError::ConnectionFailed(
            format!("invalid status code: {status}"),
        )));
    }

    let parsed_url = Url::parse(final_url)
        .map_err(|e| NavigationError::InvalidUrl(format!("invalid final URL: {e}")))?;

    let content_type = headers.get("content-type").cloned();

    Ok(NavigationResult {
        status,
        final_url: parsed_url,
        body,
        content_type,
    })
}

#[async_trait]
impl NavigationService for IpcNavigator {
    async fn navigate(&self, url: &Url) -> Result<NavigationResult, NavigationError> {
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(NavigationError::InvalidScheme(url.scheme().to_string()));
        }
        if self.https_only && url.scheme() == "http" {
            return Err(NavigationError::HttpBlocked);
        }

        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        self.pending.lock().await.insert(id, tx);

        self.send_tx
            .send(IpcMessage::FetchRequest {
                id,
                url: url.to_string(),
            })
            .await
            .map_err(|_| {
                NavigationError::Net(ie_net::NetError::ConnectionFailed(
                    "network process channel closed".into(),
                ))
            })?;

        match tokio::time::timeout(NAVIGATE_TIMEOUT, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(NavigationError::Net(ie_net::NetError::ConnectionFailed(
                "response channel dropped".into(),
            ))),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(NavigationError::Net(ie_net::NetError::Timeout))
            }
        }
    }
}

impl Drop for IpcNavigator {
    fn drop(&mut self) {
        self._writer_task.abort();
        self._reader_task.abort();
    }
}
