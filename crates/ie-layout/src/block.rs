use ie_css::CssValue;
use ie_css::resolve::ResolvedStyle;
use ie_css::values::PropertyId;

use crate::box_generation::{get_px, get_px_or, is_auto};
use crate::inline::layout_inline_content;
use crate::text_measure::TextMeasure;
use crate::{BoxType, LayoutTree, Rect};

/// Lay out a block-level box and all its descendants.
pub fn layout_block(
    box_idx: usize,
    tree: &mut LayoutTree,
    styles: &[ResolvedStyle],
    containing_width: f32,
    offset_y: f32,
    text_measure: &dyn TextMeasure,
) {
    let (node_id, _box_type) = {
        let b = &tree.boxes[box_idx];
        (b.node_id, b.box_type.clone())
    };

    let style = node_id.and_then(|id| styles.get(id));

    // --- 1. Compute width ---
    let margin_left;
    let margin_right;
    let padding_left;
    let padding_right;
    let border_left;
    let border_right;
    let content_width;

    if let Some(style) = style {
        padding_left = get_px(style, PropertyId::PaddingLeft);
        padding_right = get_px(style, PropertyId::PaddingRight);
        border_left = get_px(style, PropertyId::BorderLeftWidth);
        border_right = get_px(style, PropertyId::BorderRightWidth);

        let is_border_box = style
            .get(PropertyId::BoxSizing)
            .is_some_and(|v| matches!(v, CssValue::Keyword(k) if k == "border-box"));

        let specified_width = if is_auto(style, PropertyId::Width) {
            None
        } else {
            Some(get_px(style, PropertyId::Width))
        };

        let auto_margin_left = is_auto(style, PropertyId::MarginLeft);
        let auto_margin_right = is_auto(style, PropertyId::MarginRight);

        let non_content = padding_left + padding_right + border_left + border_right;

        match specified_width {
            Some(w) => {
                let box_width = if is_border_box { w } else { w + non_content };
                let remaining = containing_width - box_width;
                if auto_margin_left && auto_margin_right {
                    let m = (remaining / 2.0).max(0.0);
                    margin_left = m;
                    margin_right = m;
                } else if auto_margin_left {
                    margin_right = get_px(style, PropertyId::MarginRight);
                    margin_left = (remaining - margin_right).max(0.0);
                } else if auto_margin_right {
                    margin_left = get_px(style, PropertyId::MarginLeft);
                    margin_right = (remaining - margin_left).max(0.0);
                } else {
                    margin_left = get_px(style, PropertyId::MarginLeft);
                    margin_right = get_px(style, PropertyId::MarginRight);
                }
                content_width = if is_border_box {
                    (w - padding_left - padding_right - border_left - border_right).max(0.0)
                } else {
                    w
                };
            }
            None => {
                margin_left = if auto_margin_left {
                    0.0
                } else {
                    get_px(style, PropertyId::MarginLeft)
                };
                margin_right = if auto_margin_right {
                    0.0
                } else {
                    get_px(style, PropertyId::MarginRight)
                };
                content_width =
                    (containing_width - margin_left - margin_right - non_content).max(0.0);
            }
        }
    } else {
        // Anonymous box — takes full containing width
        margin_left = 0.0;
        margin_right = 0.0;
        padding_left = 0.0;
        padding_right = 0.0;
        border_left = 0.0;
        border_right = 0.0;
        content_width = containing_width;
    }

    // Apply min/max width constraints
    let content_width = if let Some(style) = style {
        let min_w = get_px(style, PropertyId::MinWidth);
        let max_w = get_px_or(style, PropertyId::MaxWidth, f32::MAX);
        content_width.max(min_w).min(max_w)
    } else {
        content_width
    };

    // Set box position and dimensions
    let x = margin_left + padding_left + border_left;
    let y = offset_y
        + tree.boxes[box_idx].margin.top
        + tree.boxes[box_idx].padding.top
        + tree.boxes[box_idx].border.top;

    tree.boxes[box_idx].content_rect.x = x;
    tree.boxes[box_idx].content_rect.y = y;
    tree.boxes[box_idx].content_rect.width = content_width;
    tree.boxes[box_idx].margin.left = margin_left;
    tree.boxes[box_idx].margin.right = margin_right;
    tree.boxes[box_idx].padding.left = padding_left;
    tree.boxes[box_idx].padding.right = padding_right;
    tree.boxes[box_idx].border.left = border_left;
    tree.boxes[box_idx].border.right = border_right;

    // --- 2. Layout children ---
    let children = tree.boxes[box_idx].children.clone();
    let content_y = tree.boxes[box_idx].content_rect.y;

    // If all children are inline/text, use the inline formatting context
    let all_inline = !children.is_empty()
        && children.iter().all(|&idx| {
            matches!(
                tree.boxes[idx].box_type,
                BoxType::Inline | BoxType::InlineBlock | BoxType::Text(_)
            )
        });

    if all_inline {
        let inline_height = layout_inline_content(
            box_idx,
            tree,
            styles,
            content_width,
            content_y,
            text_measure,
        );
        let content_height = if let Some(style) = style {
            if is_auto(style, PropertyId::Height) {
                inline_height
            } else {
                let h = get_px(style, PropertyId::Height);
                let min_h = get_px(style, PropertyId::MinHeight);
                let max_h = get_px_or(style, PropertyId::MaxHeight, f32::MAX);
                h.max(min_h).min(max_h)
            }
        } else {
            inline_height
        };
        tree.boxes[box_idx].content_rect.height = content_height;
        return;
    }

    let mut child_y = content_y;
    let mut prev_margin_bottom: f32 = 0.0;

    for (i, &child_idx) in children.iter().enumerate() {
        // Margin collapsing between siblings
        let child_margin_top = tree.boxes[child_idx].margin.top;
        if i > 0 {
            let collapsed = prev_margin_bottom.max(child_margin_top);
            child_y -= prev_margin_bottom; // undo previous margin
            child_y += collapsed; // apply collapsed margin
        } else {
            child_y += child_margin_top;
        }

        match &tree.boxes[child_idx].box_type {
            BoxType::Block | BoxType::Anonymous => {
                layout_block(
                    child_idx,
                    tree,
                    styles,
                    content_width,
                    child_y,
                    text_measure,
                );
            }
            BoxType::Text(text) => {
                // Text nodes don't have resolved styles; inherit from parent
                let font_size = node_id
                    .and_then(|id| styles.get(id))
                    .map(|s| get_px(s, PropertyId::FontSize))
                    .filter(|&v| v > 0.0)
                    .unwrap_or(16.0);
                let text = text.clone();
                let metrics = text_measure.measure(&text, font_size);
                tree.boxes[child_idx].content_rect = Rect {
                    x: tree.boxes[box_idx].content_rect.x,
                    y: child_y,
                    width: metrics.width,
                    height: metrics.height,
                };
            }
            BoxType::Inline | BoxType::InlineBlock => {
                // Simplified: treat as block for now
                layout_block(
                    child_idx,
                    tree,
                    styles,
                    content_width,
                    child_y,
                    text_measure,
                );
            }
        }

        let child_height = tree.boxes[child_idx].content_rect.height
            + tree.boxes[child_idx].padding.top
            + tree.boxes[child_idx].padding.bottom
            + tree.boxes[child_idx].border.top
            + tree.boxes[child_idx].border.bottom;
        child_y += child_height;
        prev_margin_bottom = tree.boxes[child_idx].margin.bottom;
    }

    child_y += prev_margin_bottom; // final child's bottom margin

    // --- 3. Compute height ---
    let auto_height = child_y - content_y;
    let content_height = if let Some(style) = style {
        if is_auto(style, PropertyId::Height) {
            auto_height
        } else {
            let h = get_px(style, PropertyId::Height);
            let min_h = get_px(style, PropertyId::MinHeight);
            let max_h = get_px_or(style, PropertyId::MaxHeight, f32::MAX);
            h.max(min_h).min(max_h)
        }
    } else {
        auto_height
    };

    tree.boxes[box_idx].content_rect.height = content_height;
}
