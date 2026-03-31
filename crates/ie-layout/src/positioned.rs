use ie_css::resolve::ResolvedStyle;
use ie_css::values::{CssValue, LengthUnit, PropertyId};

use crate::{LayoutTree, Rect};

/// Apply CSS positioned layout as a post-pass after normal flow.
pub fn apply_positioned(tree: &mut LayoutTree, styles: &[ResolvedStyle], viewport: Rect) {
    let count = tree.boxes.len();
    for i in 0..count {
        let node_id = match tree.boxes[i].node_id {
            Some(id) => id,
            None => continue,
        };
        let style = match styles.get(node_id) {
            Some(s) => s,
            None => continue,
        };
        let position = style
            .get(PropertyId::Position)
            .and_then(|v| match v {
                CssValue::Keyword(k) => Some(k.as_str()),
                _ => None,
            })
            .unwrap_or("static");

        match position {
            "relative" => apply_relative(i, tree, style),
            "absolute" => apply_absolute(i, tree, styles, viewport),
            "fixed" => apply_fixed(i, tree, style, viewport),
            _ => {} // static: nothing to do
        }
    }
}

fn apply_relative(box_idx: usize, tree: &mut LayoutTree, style: &ResolvedStyle) {
    let top = get_offset(style, PropertyId::Top);
    let left = get_offset(style, PropertyId::Left);
    let bottom = get_offset(style, PropertyId::Bottom);
    let right = get_offset(style, PropertyId::Right);

    // top takes precedence over bottom, left over right
    let dx = if left != 0.0 { left } else { -right };
    let dy = if top != 0.0 { top } else { -bottom };

    tree.boxes[box_idx].content_rect.x += dx;
    tree.boxes[box_idx].content_rect.y += dy;
}

fn apply_absolute(box_idx: usize, tree: &mut LayoutTree, styles: &[ResolvedStyle], viewport: Rect) {
    let node_id = match tree.boxes[box_idx].node_id {
        Some(id) => id,
        None => return,
    };
    let style = match styles.get(node_id) {
        Some(s) => s,
        None => return,
    };

    // Find containing block: nearest positioned ancestor, or viewport
    let containing = find_containing_block(box_idx, tree, styles).unwrap_or(viewport);

    let top = get_offset_option(style, PropertyId::Top);
    let left = get_offset_option(style, PropertyId::Left);
    let bottom = get_offset_option(style, PropertyId::Bottom);
    let right = get_offset_option(style, PropertyId::Right);

    if let Some(t) = top {
        tree.boxes[box_idx].content_rect.y = containing.y + t;
    } else if let Some(b) = bottom {
        tree.boxes[box_idx].content_rect.y =
            containing.y + containing.height - tree.boxes[box_idx].content_rect.height - b;
    }

    if let Some(l) = left {
        tree.boxes[box_idx].content_rect.x = containing.x + l;
    } else if let Some(r) = right {
        tree.boxes[box_idx].content_rect.x =
            containing.x + containing.width - tree.boxes[box_idx].content_rect.width - r;
    }
}

fn apply_fixed(box_idx: usize, tree: &mut LayoutTree, style: &ResolvedStyle, viewport: Rect) {
    let top = get_offset_option(style, PropertyId::Top);
    let left = get_offset_option(style, PropertyId::Left);
    let bottom = get_offset_option(style, PropertyId::Bottom);
    let right = get_offset_option(style, PropertyId::Right);

    if let Some(t) = top {
        tree.boxes[box_idx].content_rect.y = viewport.y + t;
    } else if let Some(b) = bottom {
        tree.boxes[box_idx].content_rect.y =
            viewport.y + viewport.height - tree.boxes[box_idx].content_rect.height - b;
    }

    if let Some(l) = left {
        tree.boxes[box_idx].content_rect.x = viewport.x + l;
    } else if let Some(r) = right {
        tree.boxes[box_idx].content_rect.x =
            viewport.x + viewport.width - tree.boxes[box_idx].content_rect.width - r;
    }
}

fn find_containing_block(
    box_idx: usize,
    tree: &LayoutTree,
    styles: &[ResolvedStyle],
) -> Option<Rect> {
    // Walk up the tree to find nearest positioned ancestor
    for i in (0..box_idx).rev() {
        if tree.boxes[i].children.contains(&box_idx) {
            if let Some(node_id) = tree.boxes[i].node_id
                && let Some(style) = styles.get(node_id)
            {
                let pos = style
                    .get(PropertyId::Position)
                    .and_then(|v| match v {
                        CssValue::Keyword(k) => Some(k.as_str()),
                        _ => None,
                    })
                    .unwrap_or("static");
                if pos != "static" {
                    return Some(tree.boxes[i].content_rect);
                }
            }
            // Continue searching upward from this parent
            return find_containing_block(i, tree, styles);
        }
    }
    None
}

fn get_offset(style: &ResolvedStyle, prop: PropertyId) -> f32 {
    match style.get(prop) {
        Some(CssValue::Length(v, LengthUnit::Px)) => *v as f32,
        Some(CssValue::Number(v)) => *v as f32,
        _ => 0.0,
    }
}

fn get_offset_option(style: &ResolvedStyle, prop: PropertyId) -> Option<f32> {
    match style.get(prop) {
        Some(CssValue::Length(v, LengthUnit::Px)) => Some(*v as f32),
        Some(CssValue::Number(v)) => Some(*v as f32),
        Some(CssValue::Auto) | None => None,
        _ => None,
    }
}
