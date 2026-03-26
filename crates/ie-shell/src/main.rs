//! # Internet Exploder
//!
//! A minimal, fast, private-by-default web browser.
//!
//! UI philosophy: maximum viewport, minimum chrome.
//! - No visible menu bar
//! - Tabs hidden while browsing (keyboard shortcut to reveal)
//! - Address bar appears on demand
//! - Bookmarks accessible via shortcut, no persistent bar
//! - No background prefetch/preload
//! - No address bar completion
//! - No spell checking

mod app;
mod bookmarks;
mod child_network;
mod child_renderer;
mod cli;
mod headless;
mod keybindings;
mod navigation;
mod overlay;
mod tab;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

use cli::{Cli, Mode};

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing();

    match cli.mode()? {
        Mode::Gui { url } => run_gui(url, cli.allow_http),
        Mode::Headless { url, action } => {
            headless::run_headless(url, action, cli.allow_http, cli.data_dir)
        }
        Mode::Subprocess { kind } => run_subprocess(kind),
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

fn run_gui(url: Option<url::Url>, allow_http: bool) -> Result<()> {
    let event_loop = EventLoop::<app::UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut browser = app::Browser::new(url, allow_http, proxy);
    event_loop.run_app(&mut browser)?;
    Ok(())
}

fn run_subprocess(kind: ie_sandbox::ProcessKind) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let channel = reconstruct_ipc_channel()?;
        match kind {
            ie_sandbox::ProcessKind::Network => child_network::run_network_process(channel).await,
            ie_sandbox::ProcessKind::Renderer => {
                child_renderer::run_renderer_process(channel).await
            }
            ie_sandbox::ProcessKind::Browser => {
                anyhow::bail!("browser process cannot be a subprocess")
            }
        }
    })
}

#[cfg(unix)]
fn reconstruct_ipc_channel() -> Result<ie_sandbox::IpcChannel> {
    let fd_str = std::env::var("IE_IPC_FD")
        .map_err(|_| anyhow::anyhow!("IE_IPC_FD environment variable not set"))?;
    let fd: i32 = fd_str
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid IE_IPC_FD: {e}"))?;
    Ok(ie_sandbox::IpcChannel::from_raw_fd(fd)?)
}
