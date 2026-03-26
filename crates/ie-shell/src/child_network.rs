use anyhow::Result;
use ie_sandbox::IpcChannel;
use ie_sandbox::message::IpcMessage;
use url::Url;

pub async fn run_network_process(mut channel: IpcChannel) -> Result<()> {
    let client = ie_net::Client::new()?.with_https_only(false);
    tracing::info!("network process started");

    loop {
        let msg: IpcMessage = channel.recv().await?;
        match msg {
            IpcMessage::FetchRequest { id, url } => match Url::parse(&url) {
                Ok(parsed_url) => match client.get(&parsed_url).await {
                    Ok(response) => {
                        channel
                            .send(&IpcMessage::FetchResponse {
                                id,
                                status: response.status,
                                headers: response.headers,
                                body: response.body,
                                final_url: response.url.to_string(),
                            })
                            .await?;
                    }
                    Err(e) => {
                        channel
                            .send(&IpcMessage::FetchError {
                                id,
                                error: e.to_string(),
                            })
                            .await?;
                    }
                },
                Err(e) => {
                    channel
                        .send(&IpcMessage::FetchError {
                            id,
                            error: format!("invalid URL: {e}"),
                        })
                        .await?;
                }
            },
            IpcMessage::Ping => {
                channel.send(&IpcMessage::Pong).await?;
            }
            IpcMessage::Shutdown => {
                tracing::info!("network process shutting down");
                break;
            }
            other => {
                tracing::warn!("network process received unexpected message: {other:?}");
            }
        }
    }
    Ok(())
}
