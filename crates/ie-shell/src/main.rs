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
        Mode::Headless { url, action } => headless::run_headless(url, action, cli.allow_http),
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
