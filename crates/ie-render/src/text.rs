use glyphon::{
    Attrs, Buffer, Color as GlyphonColor, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer, Viewport,
    Weight,
};

use crate::paint::PaintCommand;

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    renderer: GlyphonTextRenderer,
}

impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = glyphon::Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, surface_format);
        let viewport = Viewport::new(device, &cache);
        let renderer =
            GlyphonTextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);

        Self {
            font_system,
            swash_cache,
            atlas,
            viewport,
            renderer,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        commands: &[PaintCommand],
        viewport_width: u32,
        viewport_height: u32,
    ) {
        self.viewport.update(
            queue,
            Resolution {
                width: viewport_width,
                height: viewport_height,
            },
        );

        // Collect text commands with their prepared buffers
        let mut buffers: Vec<(Buffer, f32, f32, [u8; 4])> = Vec::new();

        for cmd in commands {
            if let PaintCommand::Text {
                text,
                x,
                y,
                font_size,
                color,
            } = cmd
            {
                let mut buffer = Buffer::new(
                    &mut self.font_system,
                    Metrics::new(*font_size, *font_size * 1.2),
                );
                buffer.set_size(
                    &mut self.font_system,
                    Some(viewport_width as f32),
                    Some(viewport_height as f32),
                );

                let attrs = Attrs::new()
                    .family(Family::SansSerif)
                    .weight(Weight::NORMAL);

                buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
                buffer.shape_until_scroll(&mut self.font_system, false);

                buffers.push((buffer, *x, *y, [color.r, color.g, color.b, color.a]));
            }
        }

        // Build text areas referencing the buffers
        let mut text_areas: Vec<TextArea> = Vec::new();
        for (buffer, x, y, color) in &buffers {
            text_areas.push(TextArea {
                buffer,
                left: *x,
                top: *y,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: viewport_width as i32,
                    bottom: viewport_height as i32,
                },
                default_color: GlyphonColor::rgba(color[0], color[1], color[2], color[3]),
                custom_glyphs: &[],
            });
        }

        self.renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas,
                &mut self.swash_cache,
            )
            .expect("failed to prepare text");
    }

    pub fn render<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>) {
        self.renderer
            .render(&self.atlas, &self.viewport, pass)
            .expect("failed to render text");
    }
}

/// Text measurer using a simple heuristic (matching software renderer).
/// TODO: Use actual cosmic-text font metrics for accurate measurement.
pub struct GlyphonTextMeasure;

impl ie_layout::text_measure::TextMeasure for GlyphonTextMeasure {
    fn measure(&self, text: &str, font_size: f32) -> ie_layout::text_measure::TextMetrics {
        let char_count = text.chars().count() as f32;
        ie_layout::text_measure::TextMetrics {
            width: char_count * font_size * 0.5,
            height: font_size,
        }
    }
}
