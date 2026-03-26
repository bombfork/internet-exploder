use anyhow::Result;
use ie_sandbox::IpcChannel;
use ie_sandbox::message::IpcMessage;

pub async fn run_renderer_process(mut channel: IpcChannel) -> Result<()> {
    // Apply sandbox BEFORE any untrusted work
    let sandbox_result = ie_sandbox::apply_sandbox(ie_sandbox::SandboxProfile::Renderer)?;
    tracing::info!("renderer process sandbox: {sandbox_result:?}");
    tracing::info!("renderer process started (stub)");
    loop {
        let msg: IpcMessage = channel.recv().await?;
        match msg {
            IpcMessage::Ping => channel.send(&IpcMessage::Pong).await?,
            IpcMessage::Shutdown => {
                tracing::info!("renderer process shutting down");
                break;
            }
            other => tracing::warn!("renderer received unhandled message: {other:?}"),
        }
    }
    Ok(())
}
