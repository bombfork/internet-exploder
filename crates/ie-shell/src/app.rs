use std::num::NonZeroU32;
use std::sync::Arc;

use url::Url;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

pub struct Browser {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    _url: Option<Url>,
}

impl Browser {
    pub fn new(url: Option<Url>) -> Self {
        Self {
            window: None,
            surface: None,
            _url: url,
        }
    }
}

// Dark background color (#1a1a2e)
const BG_COLOR: u32 = 0x001a1a2e;

impl ApplicationHandler for Browser {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes()
                .with_title("Internet Exploder")
                .with_maximized(true);
            match event_loop.create_window(attrs) {
                Ok(window) => {
                    let window = Arc::new(window);
                    let context = softbuffer::Context::new(window.clone())
                        .expect("failed to create softbuffer context");
                    let surface = softbuffer::Surface::new(&context, window.clone())
                        .expect("failed to create softbuffer surface");
                    self.window = Some(window);
                    self.surface = Some(surface);
                    self.window.as_ref().unwrap().request_redraw();
                }
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
                if let Some(surface) = self.surface.as_mut() {
                    let size = self.window.as_ref().unwrap().inner_size();
                    let Some(width) = NonZeroU32::new(size.width) else {
                        return;
                    };
                    let Some(height) = NonZeroU32::new(size.height) else {
                        return;
                    };
                    surface
                        .resize(width, height)
                        .expect("failed to resize surface");
                    let mut buffer = surface.buffer_mut().expect("failed to get buffer");
                    buffer.fill(BG_COLOR);
                    buffer.present().expect("failed to present buffer");
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
