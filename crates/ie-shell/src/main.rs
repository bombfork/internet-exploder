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

use anyhow::Result;
use tracing_subscriber::EnvFilter;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct Browser {
    window: Option<Window>,
}

impl Browser {
    fn new() -> Self {
        Self { window: None }
    }
}

impl ApplicationHandler for Browser {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes()
                .with_title("Internet Exploder")
                .with_maximized(true);
            match event_loop.create_window(attrs) {
                Ok(window) => self.window = Some(window),
                Err(e) => {
                    tracing::error!("failed to create window: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // TODO: render current page
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let event_loop = EventLoop::new()?;
    let mut browser = Browser::new();
    event_loop.run_app(&mut browser)?;

    Ok(())
}
