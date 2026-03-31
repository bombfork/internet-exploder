/// Abstraction for measuring text dimensions.
pub trait TextMeasure {
    fn measure(&self, text: &str, font_size: f32) -> TextMetrics;
}

#[derive(Debug, Clone, Copy)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
}

/// Mock text measurer: width = char_count * font_size * 0.5, height = font_size.
pub struct MockTextMeasure;

impl TextMeasure for MockTextMeasure {
    fn measure(&self, text: &str, font_size: f32) -> TextMetrics {
        TextMetrics {
            width: text.len() as f32 * (font_size * 0.5),
            height: font_size,
        }
    }
}
