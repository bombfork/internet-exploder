//! # ie-render
//!
//! GPU-accelerated renderer using wgpu (WebGPU).
//! Paints the layout tree to the screen. The browser chrome (address bar,
//! tab overlay) is rendered through the same pipeline as page content.

use anyhow::Result;

pub struct Renderer {
    _device: wgpu::Device,
    _queue: wgpu::Queue,
}

impl Renderer {
    pub async fn new(window: &impl wgpu::WindowHandle) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("no suitable GPU adapter found"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await?;

        Ok(Self {
            _device: device,
            _queue: queue,
        })
    }

    pub fn render(&mut self, _layout: &ie_layout::LayoutTree) -> Result<()> {
        todo!("GPU rendering pass")
    }
}
