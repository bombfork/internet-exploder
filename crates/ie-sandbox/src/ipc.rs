use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcMessage {
    Navigate { url: String },
    LoadComplete,
    RenderReady,
    Shutdown,
}

pub fn send(_msg: &IpcMessage) -> Result<()> {
    todo!("IPC send")
}

pub fn recv() -> Result<IpcMessage> {
    todo!("IPC recv")
}
