use ie_css::resolve::ResolvedStyle;
use ie_css::values::{CssValue, PropertyId};

use crate::box_generation::{get_px, is_auto};
use crate::text_measure::TextMeasure;
use crate::{BoxType, EdgeSizes, LayoutTree, Rect};

/// Lay out a flex container and all its flex items.
pub fn layout_flex(
    box_idx: usize,
    tree: &mut LayoutTree,
    styles: &[ResolvedStyle],
    containing_width: f32,
    offset_y: f32,
    text_measure: &dyn TextMeasure,
) {
    let node_id = tree.boxes[box_idx].node_id;
    let style = node_id.and_then(|id| styles.get(id));

    // 1. Compute container dimensions
    let padding_left = style
        .map(|s| get_px(s, PropertyId::PaddingLeft))
        .unwrap_or(0.0);
    let padding_right = style
        .map(|s| get_px(s, PropertyId::PaddingRight))
        .unwrap_or(0.0);
    let padding_top = style
        .map(|s| get_px(s, PropertyId::PaddingTop))
        .unwrap_or(0.0);
    let padding_bottom = style
        .map(|s| get_px(s, PropertyId::PaddingBottom))
        .unwrap_or(0.0);
    let border_left = style
        .map(|s| get_px(s, PropertyId::BorderLeftWidth))
        .unwrap_or(0.0);
    let border_right = style
        .map(|s| get_px(s, PropertyId::BorderRightWidth))
        .unwrap_or(0.0);
    let border_top = style
        .map(|s| get_px(s, PropertyId::BorderTopWidth))
        .unwrap_or(0.0);
    let border_bottom = style
        .map(|s| get_px(s, PropertyId::BorderBottomWidth))
        .unwrap_or(0.0);
    let margin_left = style
        .map(|s| get_px(s, PropertyId::MarginLeft))
        .unwrap_or(0.0);
    let margin_right = style
        .map(|s| get_px(s, PropertyId::MarginRight))
        .unwrap_or(0.0);
    let margin_top = style
        .map(|s| get_px(s, PropertyId::MarginTop))
        .unwrap_or(0.0);
    let margin_bottom = style
        .map(|s| get_px(s, PropertyId::MarginBottom))
        .unwrap_or(0.0);

    let content_width = if style.is_some_and(|s| !is_auto(s, PropertyId::Width)) {
        get_px(style.unwrap(), PropertyId::Width)
    } else {
        (containing_width
            - margin_left
            - margin_right
            - padding_left
            - padding_right
            - border_left
            - border_right)
            .max(0.0)
    };

    // 2. Read flex properties
    let flex_direction = style
        .and_then(|s| s.get(PropertyId::FlexDirection))
        .and_then(|v| match v {
            CssValue::Keyword(k) => Some(k.as_str().to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "row".to_string());

    let flex_wrap = style
        .and_then(|s| s.get(PropertyId::FlexWrap))
        .and_then(|v| match v {
            CssValue::Keyword(k) => Some(k.as_str().to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "nowrap".to_string());

    let justify_content = style
        .and_then(|s| s.get(PropertyId::JustifyContent))
        .and_then(|v| match v {
            CssValue::Keyword(k) => Some(k.as_str().to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "flex-start".to_string());

    let align_items = style
        .and_then(|s| s.get(PropertyId::AlignItems))
        .and_then(|v| match v {
            CssValue::Keyword(k) => Some(k.as_str().to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "stretch".to_string());

    let is_row = flex_direction == "row" || flex_direction == "row-reverse";
    let is_reverse = flex_direction == "row-reverse" || flex_direction == "column-reverse";

    let main_size = if is_row {
        content_width
    } else {
        // For column, main size is height (auto = unconstrained)
        style
            .and_then(|s| {
                if !is_auto(s, PropertyId::Height) {
                    Some(get_px(s, PropertyId::Height))
                } else {
                    None
                }
            })
            .unwrap_or(f32::MAX)
    };

    // Set container position
    let container_x = margin_left + border_left + padding_left;
    let container_y = offset_y + margin_top + border_top + padding_top;
    tree.boxes[box_idx].content_rect.x = container_x;
    tree.boxes[box_idx].content_rect.y = container_y;
    tree.boxes[box_idx].content_rect.width = content_width;
    tree.boxes[box_idx].padding = EdgeSizes {
        top: padding_top,
        right: padding_right,
        bottom: padding_bottom,
        left: padding_left,
    };
    tree.boxes[box_idx].border = EdgeSizes {
        top: border_top,
        right: border_right,
        bottom: border_bottom,
        left: border_left,
    };
    tree.boxes[box_idx].margin = EdgeSizes {
        top: margin_top,
        right: margin_right,
        bottom: margin_bottom,
        left: margin_left,
    };

    // 3. Collect flex items and their base sizes
    let children = tree.boxes[box_idx].children.clone();
    if children.is_empty() {
        tree.boxes[box_idx].content_rect.height = 0.0;
        return;
    }

    struct FlexItem {
        idx: usize,
        base_size: f32,
        flex_grow: f32,
        flex_shrink: f32,
        cross_size: f32,
        main_pos: f32,
        cross_pos: f32,
        final_main_size: f32,
    }

    let mut items: Vec<FlexItem> = Vec::new();
    for &child_idx in &children {
        let child_style = tree.boxes[child_idx].node_id.and_then(|id| styles.get(id));

        // Flex basis
        let flex_basis = child_style.and_then(|s| match s.get(PropertyId::FlexBasis) {
            Some(CssValue::Length(v, _)) => Some(*v as f32),
            _ => None,
        });

        // Fall back to width/height
        let explicit_main = if is_row {
            child_style.and_then(|s| {
                if !is_auto(s, PropertyId::Width) {
                    Some(get_px(s, PropertyId::Width))
                } else {
                    None
                }
            })
        } else {
            child_style.and_then(|s| {
                if !is_auto(s, PropertyId::Height) {
                    Some(get_px(s, PropertyId::Height))
                } else {
                    None
                }
            })
        };

        // Content size as fallback
        let content_size = if is_row {
            let text = collect_text(child_idx, tree);
            let font_size = child_style
                .map(|s| get_px(s, PropertyId::FontSize))
                .unwrap_or(16.0);
            if text.is_empty() {
                0.0
            } else {
                text_measure.measure(&text, font_size).width
            }
        } else {
            child_style
                .map(|s| get_px(s, PropertyId::FontSize))
                .unwrap_or(16.0) // single line height as default
        };

        let base_size = flex_basis.or(explicit_main).unwrap_or(content_size);

        let flex_grow = child_style
            .and_then(|s| match s.get(PropertyId::FlexGrow) {
                Some(CssValue::Number(v)) => Some(*v as f32),
                _ => None,
            })
            .unwrap_or(0.0);

        let flex_shrink = child_style
            .and_then(|s| match s.get(PropertyId::FlexShrink) {
                Some(CssValue::Number(v)) => Some(*v as f32),
                _ => None,
            })
            .unwrap_or(1.0);

        // Cross size
        let cross_size = if is_row {
            child_style
                .and_then(|s| {
                    if !is_auto(s, PropertyId::Height) {
                        Some(get_px(s, PropertyId::Height))
                    } else {
                        None
                    }
                })
                .unwrap_or(content_size.max(16.0))
        } else {
            child_style
                .and_then(|s| {
                    if !is_auto(s, PropertyId::Width) {
                        Some(get_px(s, PropertyId::Width))
                    } else {
                        None
                    }
                })
                .unwrap_or(content_width)
        };

        items.push(FlexItem {
            idx: child_idx,
            base_size,
            flex_grow,
            flex_shrink,
            cross_size,
            main_pos: 0.0,
            cross_pos: 0.0,
            final_main_size: base_size,
        });
    }

    // 4. Flex lines (single line for nowrap, split for wrap)
    let mut lines: Vec<Vec<usize>> = vec![];
    if flex_wrap == "nowrap" {
        lines.push((0..items.len()).collect());
    } else {
        let mut line: Vec<usize> = Vec::new();
        let mut line_main = 0.0f32;
        for (i, item) in items.iter().enumerate() {
            if !line.is_empty() && line_main + item.base_size > main_size {
                lines.push(std::mem::take(&mut line));
                line_main = 0.0;
            }
            line_main += item.base_size;
            line.push(i);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }

    if flex_wrap == "wrap-reverse" {
        lines.reverse();
    }

    // 5. Resolve flexible lengths per line
    for line in &lines {
        let total_base: f32 = line.iter().map(|&i| items[i].base_size).sum();
        let free_space = main_size - total_base;

        if free_space > 0.0 {
            let total_grow: f32 = line.iter().map(|&i| items[i].flex_grow).sum();
            if total_grow > 0.0 {
                for &i in line {
                    items[i].final_main_size =
                        items[i].base_size + free_space * (items[i].flex_grow / total_grow);
                }
            }
        } else if free_space < 0.0 {
            let total_shrink: f32 = line
                .iter()
                .map(|&i| items[i].flex_shrink * items[i].base_size)
                .sum();
            if total_shrink > 0.0 {
                for &i in line {
                    let shrink_ratio = (items[i].flex_shrink * items[i].base_size) / total_shrink;
                    items[i].final_main_size =
                        (items[i].base_size + free_space * shrink_ratio).max(0.0);
                }
            }
        }
    }

    // 6. Position items
    let mut cross_offset = 0.0f32;
    for line in &lines {
        // Line cross size = max cross size of items in line
        let line_cross = if is_row {
            line.iter()
                .map(|&i| items[i].cross_size)
                .fold(0.0f32, f32::max)
        } else {
            content_width
        };

        // Main axis positioning
        let total_main: f32 = line.iter().map(|&i| items[i].final_main_size).sum();
        let remaining = main_size - total_main;

        let (mut main_pos, item_gap) = match justify_content.as_str() {
            "flex-end" => (remaining.max(0.0), 0.0),
            "center" => ((remaining / 2.0).max(0.0), 0.0),
            "space-between" => {
                let g = if line.len() > 1 {
                    remaining / (line.len() - 1) as f32
                } else {
                    0.0
                };
                (0.0, g.max(0.0))
            }
            "space-around" => {
                let g = if !line.is_empty() {
                    remaining / line.len() as f32
                } else {
                    0.0
                };
                ((g / 2.0).max(0.0), g.max(0.0))
            }
            "space-evenly" => {
                let g = remaining / (line.len() + 1) as f32;
                (g.max(0.0), g.max(0.0))
            }
            _ => (0.0, 0.0), // flex-start
        };

        let line_items: Vec<usize> = if is_reverse {
            line.iter().rev().copied().collect()
        } else {
            line.clone()
        };

        for (j, &i) in line_items.iter().enumerate() {
            items[i].main_pos = main_pos;

            // Cross axis alignment (check per-item align-self)
            let child_style = tree.boxes[items[i].idx]
                .node_id
                .and_then(|id| styles.get(id));
            let item_align = child_style
                .and_then(|s| s.get(PropertyId::AlignSelf))
                .and_then(|v| match v {
                    CssValue::Keyword(k) => Some(k.as_str().to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| align_items.clone());

            let item_cross = items[i].cross_size;
            items[i].cross_pos = match item_align.as_str() {
                "center" => cross_offset + (line_cross - item_cross) / 2.0,
                "flex-end" => cross_offset + line_cross - item_cross,
                "stretch" => {
                    items[i].cross_size = line_cross;
                    cross_offset
                }
                _ => cross_offset, // flex-start
            };

            main_pos += items[i].final_main_size;
            if j < line_items.len() - 1 {
                main_pos += item_gap;
            }
        }

        cross_offset += line_cross;
    }

    // 7. Write positions to layout boxes
    for item in &items {
        let (x, y, w, h) = if is_row {
            (
                container_x + item.main_pos,
                container_y + item.cross_pos,
                item.final_main_size,
                item.cross_size,
            )
        } else {
            (
                container_x + item.cross_pos,
                container_y + item.main_pos,
                item.cross_size,
                item.final_main_size,
            )
        };
        tree.boxes[item.idx].content_rect = Rect {
            x,
            y,
            width: w,
            height: h,
        };

        // Recursively layout children of flex items.
        // We lay out children within the item's content area, using the
        // flex-computed position as the origin.
        let child_children = tree.boxes[item.idx].children.clone();
        if !child_children.is_empty() {
            layout_flex_item_children(item.idx, tree, styles, x, y, w, text_measure);
        }
    }

    // 8. Set container height
    let content_height = if style.is_some_and(|s| !is_auto(s, PropertyId::Height)) {
        get_px(style.unwrap(), PropertyId::Height)
    } else if is_row {
        cross_offset
    } else {
        // For column direction, height is sum of main sizes
        items.iter().map(|item| item.final_main_size).sum()
    };
    tree.boxes[box_idx].content_rect.height = content_height;
}

/// Lay out the children of a flex item within its content area.
fn layout_flex_item_children(
    item_idx: usize,
    tree: &mut LayoutTree,
    styles: &[ResolvedStyle],
    item_x: f32,
    item_y: f32,
    item_width: f32,
    text_measure: &dyn TextMeasure,
) {
    let node_id = tree.boxes[item_idx].node_id;
    let children = tree.boxes[item_idx].children.clone();
    let mut child_y = item_y;

    for &child_idx in &children {
        match &tree.boxes[child_idx].box_type {
            BoxType::Block | BoxType::Anonymous => {
                crate::block::layout_block(
                    child_idx,
                    tree,
                    styles,
                    item_width,
                    child_y,
                    text_measure,
                );
                // Offset x to be relative to flex item position
                let child_own_x = tree.boxes[child_idx].content_rect.x;
                tree.boxes[child_idx].content_rect.x = item_x + child_own_x;
            }
            BoxType::Flex => {
                layout_flex(child_idx, tree, styles, item_width, child_y, text_measure);
                let child_own_x = tree.boxes[child_idx].content_rect.x;
                tree.boxes[child_idx].content_rect.x = item_x + child_own_x;
            }
            BoxType::Text(text) => {
                let font_size = node_id
                    .and_then(|id| styles.get(id))
                    .map(|s| get_px(s, PropertyId::FontSize))
                    .filter(|&v| v > 0.0)
                    .unwrap_or(16.0);
                let text = text.clone();
                let metrics = text_measure.measure(&text, font_size);
                tree.boxes[child_idx].content_rect = Rect {
                    x: item_x,
                    y: child_y,
                    width: metrics.width,
                    height: metrics.height,
                };
            }
            BoxType::Inline | BoxType::InlineBlock => {
                crate::block::layout_block(
                    child_idx,
                    tree,
                    styles,
                    item_width,
                    child_y,
                    text_measure,
                );
                let child_own_x = tree.boxes[child_idx].content_rect.x;
                tree.boxes[child_idx].content_rect.x = item_x + child_own_x;
            }
        }

        let child_box = &tree.boxes[child_idx];
        child_y += child_box.content_rect.height
            + child_box.padding.top
            + child_box.padding.bottom
            + child_box.border.top
            + child_box.border.bottom
            + child_box.margin.top
            + child_box.margin.bottom;
    }
}

/// Recursively collect text content from a layout box subtree.
fn collect_text(box_idx: usize, tree: &LayoutTree) -> String {
    let mut text = String::new();
    match &tree.boxes[box_idx].box_type {
        BoxType::Text(t) => text.push_str(t),
        _ => {
            for &child in &tree.boxes[box_idx].children {
                text.push_str(&collect_text(child, tree));
            }
        }
    }
    text
}
