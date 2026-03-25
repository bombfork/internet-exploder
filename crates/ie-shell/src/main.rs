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
#[allow(dead_code)]
mod bookmarks;
mod cli;
mod headless;
#[allow(dead_code)]
mod navigation;
#[allow(dead_code)]
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
        Mode::Gui { url } => run_gui(url),
        Mode::Headless { url, action } => headless::run_headless(url, action, cli.allow_http),
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

fn run_gui(url: Option<url::Url>) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut browser = app::Browser::new(url);
    event_loop.run_app(&mut browser)?;
    Ok(())
}
