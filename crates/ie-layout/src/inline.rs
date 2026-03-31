use ie_css::CssValue;
use ie_css::resolve::ResolvedStyle;
use ie_css::values::PropertyId;

use crate::box_generation::get_px;
use crate::text_measure::TextMeasure;
use crate::{BoxType, LayoutTree, Rect};

/// A positioned item within a line box.
struct LineItem {
    box_idx: usize,
    x: f32,
    width: f32,
    height: f32,
}

/// Layout inline content within a block container.
///
/// Walks inline children left-to-right, wrapping text at word boundaries
/// when it exceeds `containing_width`. Returns the total height consumed.
pub fn layout_inline_content(
    container_idx: usize,
    tree: &mut LayoutTree,
    styles: &[ResolvedStyle],
    containing_width: f32,
    offset_y: f32,
    text_measure: &dyn TextMeasure,
) -> f32 {
    let children = tree.boxes[container_idx].children.clone();
    let container_x = tree.boxes[container_idx].content_rect.x;

    let container_node = tree.boxes[container_idx].node_id;

    // text-align from container style
    let text_align = container_node
        .and_then(|id| styles.get(id))
        .and_then(|s| s.get(PropertyId::TextAlign))
        .and_then(|v| match v {
            CssValue::Keyword(k) => Some(k.as_str()),
            _ => None,
        })
        .unwrap_or("left");
    let text_align = text_align.to_string();

    // white-space from container style
    let white_space = container_node
        .and_then(|id| styles.get(id))
        .and_then(|s| s.get(PropertyId::WhiteSpace))
        .and_then(|v| match v {
            CssValue::Keyword(k) => Some(k.as_str()),
            _ => None,
        })
        .unwrap_or("normal");

    let allow_wrap = !matches!(white_space, "nowrap" | "pre");
    let collapse_whitespace = matches!(white_space, "normal" | "nowrap");
    let white_space = white_space.to_string();
    // keep `white_space` alive for later use
    let _ = &white_space;

    let default_font_size = 16.0_f32;

    let mut lines: Vec<Vec<LineItem>> = vec![Vec::new()];
    let mut current_x: f32 = 0.0;

    for &child_idx in &children {
        let box_type = tree.boxes[child_idx].box_type.clone();
        match box_type {
            BoxType::Text(ref text) => {
                // Text nodes inherit font-size from the container element
                let font_size = container_node
                    .and_then(|id| styles.get(id))
                    .map(|s| get_px(s, PropertyId::FontSize))
                    .filter(|&fs| fs > 0.0)
                    .unwrap_or(default_font_size);

                let processed = if collapse_whitespace {
                    text.split_whitespace().collect::<Vec<_>>().join(" ")
                } else {
                    text.clone()
                };

                if processed.is_empty() {
                    continue;
                }

                if !allow_wrap {
                    // No wrapping: entire text on current line
                    let metrics = text_measure.measure(&processed, font_size);
                    lines.last_mut().unwrap().push(LineItem {
                        box_idx: child_idx,
                        x: current_x,
                        width: metrics.width,
                        height: metrics.height,
                    });
                    current_x += metrics.width;
                } else {
                    // Word wrapping via split_inclusive on whitespace
                    let words: Vec<&str> = processed.split_inclusive(char::is_whitespace).collect();
                    let words = if words.is_empty() {
                        vec![processed.as_str()]
                    } else {
                        words
                    };

                    for word in words {
                        let metrics = text_measure.measure(word, font_size);
                        if current_x + metrics.width > containing_width && current_x > 0.0 {
                            lines.push(Vec::new());
                            current_x = 0.0;
                        }
                        lines.last_mut().unwrap().push(LineItem {
                            box_idx: child_idx,
                            x: current_x,
                            width: metrics.width,
                            height: metrics.height,
                        });
                        current_x += metrics.width;
                    }
                }
            }
            BoxType::Inline | BoxType::InlineBlock => {
                let style = tree.boxes[child_idx].node_id.and_then(|id| styles.get(id));
                let padding_lr = style
                    .map(|s| {
                        get_px(s, PropertyId::PaddingLeft) + get_px(s, PropertyId::PaddingRight)
                    })
                    .unwrap_or(0.0);
                let border_lr = style
                    .map(|s| {
                        get_px(s, PropertyId::BorderLeftWidth)
                            + get_px(s, PropertyId::BorderRightWidth)
                    })
                    .unwrap_or(0.0);
                let margin_lr = style
                    .map(|s| get_px(s, PropertyId::MarginLeft) + get_px(s, PropertyId::MarginRight))
                    .unwrap_or(0.0);

                let extra = padding_lr + border_lr + margin_lr;
                let font_size = style
                    .map(|s| get_px(s, PropertyId::FontSize))
                    .filter(|&v| v > 0.0)
                    .unwrap_or(default_font_size);

                // Approximate inline element width from its text children
                let child_text: String = tree.boxes[child_idx]
                    .children
                    .iter()
                    .filter_map(|&ci| match &tree.boxes[ci].box_type {
                        BoxType::Text(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect();
                let text_metrics = text_measure.measure(&child_text, font_size);
                let total_width = text_metrics.width + extra;

                if allow_wrap && current_x + total_width > containing_width && current_x > 0.0 {
                    lines.push(Vec::new());
                    current_x = 0.0;
                }

                lines.last_mut().unwrap().push(LineItem {
                    box_idx: child_idx,
                    x: current_x,
                    width: total_width,
                    height: text_metrics.height.max(font_size),
                });
                current_x += total_width;
            }
            _ => {} // Block children handled separately
        }
    }

    // Position items on each line
    let mut y = offset_y;
    for line in &lines {
        if line.is_empty() {
            continue;
        }
        let line_height = line.iter().map(|item| item.height).fold(0.0_f32, f32::max);
        let line_width: f32 = line.last().map(|item| item.x + item.width).unwrap_or(0.0);

        let align_offset = match text_align.as_str() {
            "center" => ((containing_width - line_width) / 2.0).max(0.0),
            "right" => (containing_width - line_width).max(0.0),
            _ => 0.0,
        };

        for item in line {
            let final_x = container_x + item.x + align_offset;
            tree.boxes[item.box_idx].content_rect = Rect {
                x: final_x,
                y,
                width: item.width,
                height: item.height,
            };
        }

        y += line_height;
    }

    y - offset_y
}
