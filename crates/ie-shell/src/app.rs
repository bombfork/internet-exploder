use url::Url;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

pub struct Browser {
    window: Option<Window>,
    _url: Option<Url>,
}

impl Browser {
    pub fn new(url: Option<Url>) -> Self {
        Self {
            window: None,
            _url: url,
        }
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
