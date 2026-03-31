use crate::paint::{Color, PaintCommand};

/// Render paint commands to an ARGB pixel buffer.
pub fn render_to_buffer(commands: &[PaintCommand], width: u32, height: u32) -> Vec<u32> {
    let mut buffer = vec![0xFFFF_FFFFu32; (width * height) as usize];

    for cmd in commands {
        match cmd {
            PaintCommand::FillRect {
                x,
                y,
                width: w,
                height: h,
                color,
            } => {
                fill_rect(
                    &mut buffer,
                    width as i32,
                    height as i32,
                    *x,
                    *y,
                    *w,
                    *h,
                    color,
                );
            }
            PaintCommand::Text {
                text,
                x,
                y,
                font_size,
                color,
            } => {
                render_text_simple(
                    &mut buffer,
                    width as i32,
                    height as i32,
                    text,
                    *x,
                    *y,
                    *font_size,
                    color,
                );
            }
        }
    }

    buffer
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    buffer: &mut [u32],
    buf_width: i32,
    buf_height: i32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: &Color,
) {
    let x0 = (x as i32).max(0);
    let y0 = (y as i32).max(0);
    let x1 = ((x + w) as i32).min(buf_width);
    let y1 = ((y + h) as i32).min(buf_height);

    let pixel = color.to_argb();

    if color.a == 255 {
        for py in y0..y1 {
            for px in x0..x1 {
                buffer[(py * buf_width + px) as usize] = pixel;
            }
        }
    } else if color.a > 0 {
        let sa = color.a as u32;
        let sr = color.r as u32;
        let sg = color.g as u32;
        let sb = color.b as u32;
        for py in y0..y1 {
            for px in x0..x1 {
                let idx = (py * buf_width + px) as usize;
                let dst = buffer[idx];
                let dr = (dst >> 16) & 0xFF;
                let dg = (dst >> 8) & 0xFF;
                let db = dst & 0xFF;
                let r = (sr * sa + dr * (255 - sa)) / 255;
                let g = (sg * sa + dg * (255 - sa)) / 255;
                let b = (sb * sa + db * (255 - sa)) / 255;
                buffer[idx] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_text_simple(
    buffer: &mut [u32],
    buf_width: i32,
    buf_height: i32,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: &Color,
) {
    let char_width = font_size * 0.5;
    let char_height = font_size;
    let mut cx = x;

    for ch in text.chars() {
        if ch == ' ' {
            cx += char_width;
            continue;
        }
        let glyph_w = char_width * 0.7;
        let glyph_h = char_height * 0.7;
        let glyph_y = y + (char_height - glyph_h) * 0.5;
        fill_rect(
            buffer, buf_width, buf_height, cx, glyph_y, glyph_w, glyph_h, color,
        );
        cx += char_width;
    }
}

/// Text measurer matching the software renderer metrics.
pub struct SoftwareTextMeasure;

impl ie_layout::text_measure::TextMeasure for SoftwareTextMeasure {
    fn measure(&self, text: &str, font_size: f32) -> ie_layout::text_measure::TextMetrics {
        ie_layout::text_measure::TextMetrics {
            width: text.len() as f32 * font_size * 0.5,
            height: font_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Color;
    use crate::render_to_buffer;

    #[test]
    fn software_render_produces_pixels() {
        let commands = vec![PaintCommand::FillRect {
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 50.0,
            color: Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
        }];
        let buffer = render_to_buffer(&commands, 100, 100);
        assert_eq!(buffer.len(), 10000);
        let px = buffer[30 * 100 + 30];
        assert_eq!(px, 0xFFFF0000, "pixel should be red");
    }

    #[test]
    fn white_background_default() {
        let buffer = render_to_buffer(&[], 10, 10);
        assert_eq!(buffer[0], 0xFFFFFFFF, "default background should be white");
    }
}
