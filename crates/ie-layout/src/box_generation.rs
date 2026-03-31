use ie_css::resolve::ResolvedStyle;
use ie_css::values::{CssValue, LengthUnit, PropertyId};
use ie_dom::{Document, NodeId, NodeKind};

use crate::{BoxType, EdgeSizes, LayoutBox, LayoutTree, Rect};

/// Walk the styled DOM and generate a box tree.
pub fn generate_box_tree(
    doc: &Document,
    styles: &[ResolvedStyle],
    root_node: NodeId,
) -> LayoutTree {
    let mut tree = LayoutTree {
        boxes: Vec::new(),
        root: None,
    };
    tree.root = generate_boxes(doc, styles, root_node, &mut tree);
    tree
}

fn generate_boxes(
    doc: &Document,
    styles: &[ResolvedStyle],
    node_id: NodeId,
    tree: &mut LayoutTree,
) -> Option<usize> {
    let node = doc.node(node_id)?;

    match &node.kind {
        NodeKind::Element(_) => {
            let style = styles.get(node_id)?;
            let display = style.display();

            if display == "none" {
                return None;
            }

            let box_type = match display {
                "block" | "list-item" | "table" | "flex" => BoxType::Block,
                "inline" => BoxType::Inline,
                "inline-block" => BoxType::InlineBlock,
                _ => BoxType::Block,
            };

            let (padding, border, margin) = resolve_edges(style);
            let layout_box = LayoutBox {
                node_id: Some(node_id),
                box_type: box_type.clone(),
                content_rect: Rect::default(),
                padding,
                border,
                margin,
                children: Vec::new(),
            };
            let box_idx = tree.boxes.len();
            tree.boxes.push(layout_box);

            // Generate children
            let child_ids: Vec<NodeId> = node.children.clone();
            let mut child_indices = Vec::new();
            for &child_id in &child_ids {
                if let Some(child_idx) = generate_boxes(doc, styles, child_id, tree) {
                    child_indices.push(child_idx);
                }
            }

            // Insert anonymous block boxes when mixing block+inline children
            let has_block = child_indices
                .iter()
                .any(|&i| matches!(tree.boxes[i].box_type, BoxType::Block));
            let has_inline = child_indices
                .iter()
                .any(|&i| !matches!(tree.boxes[i].box_type, BoxType::Block));

            if has_block && has_inline && matches!(box_type, BoxType::Block) {
                let mut wrapped = Vec::new();
                let mut inline_run = Vec::new();

                for &idx in &child_indices {
                    if matches!(tree.boxes[idx].box_type, BoxType::Block) {
                        if !inline_run.is_empty() {
                            let anon = LayoutBox {
                                node_id: None,
                                box_type: BoxType::Anonymous,
                                content_rect: Rect::default(),
                                padding: EdgeSizes::default(),
                                border: EdgeSizes::default(),
                                margin: EdgeSizes::default(),
                                children: std::mem::take(&mut inline_run),
                            };
                            let anon_idx = tree.boxes.len();
                            tree.boxes.push(anon);
                            wrapped.push(anon_idx);
                        }
                        wrapped.push(idx);
                    } else {
                        inline_run.push(idx);
                    }
                }
                if !inline_run.is_empty() {
                    let anon = LayoutBox {
                        node_id: None,
                        box_type: BoxType::Anonymous,
                        content_rect: Rect::default(),
                        padding: EdgeSizes::default(),
                        border: EdgeSizes::default(),
                        margin: EdgeSizes::default(),
                        children: inline_run,
                    };
                    let anon_idx = tree.boxes.len();
                    tree.boxes.push(anon);
                    wrapped.push(anon_idx);
                }
                tree.boxes[box_idx].children = wrapped;
            } else {
                tree.boxes[box_idx].children = child_indices;
            }

            Some(box_idx)
        }
        NodeKind::Text(text) => {
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            let layout_box = LayoutBox {
                node_id: Some(node_id),
                box_type: BoxType::Text(text.to_string()),
                content_rect: Rect::default(),
                padding: EdgeSizes::default(),
                border: EdgeSizes::default(),
                margin: EdgeSizes::default(),
                children: Vec::new(),
            };
            let idx = tree.boxes.len();
            tree.boxes.push(layout_box);
            Some(idx)
        }
        NodeKind::Document => {
            // Walk children of the Document node (find html element)
            let child_ids: Vec<NodeId> = node.children.clone();
            for &child_id in &child_ids {
                let result = generate_boxes(doc, styles, child_id, tree);
                if result.is_some() {
                    return result;
                }
            }
            None
        }
        _ => None, // Skip Comment, Doctype nodes
    }
}

fn resolve_edges(style: &ResolvedStyle) -> (EdgeSizes, EdgeSizes, EdgeSizes) {
    let padding = EdgeSizes {
        top: get_px(style, PropertyId::PaddingTop),
        right: get_px(style, PropertyId::PaddingRight),
        bottom: get_px(style, PropertyId::PaddingBottom),
        left: get_px(style, PropertyId::PaddingLeft),
    };
    let border = EdgeSizes {
        top: get_px(style, PropertyId::BorderTopWidth),
        right: get_px(style, PropertyId::BorderRightWidth),
        bottom: get_px(style, PropertyId::BorderBottomWidth),
        left: get_px(style, PropertyId::BorderLeftWidth),
    };
    let margin = EdgeSizes {
        top: get_px(style, PropertyId::MarginTop),
        right: get_px(style, PropertyId::MarginRight),
        bottom: get_px(style, PropertyId::MarginBottom),
        left: get_px(style, PropertyId::MarginLeft),
    };
    (padding, border, margin)
}

pub fn get_px(style: &ResolvedStyle, prop: PropertyId) -> f32 {
    match style.get(prop) {
        Some(CssValue::Length(v, LengthUnit::Px)) => *v as f32,
        Some(CssValue::Number(v)) => *v as f32,
        _ => 0.0,
    }
}

pub fn get_px_or(style: &ResolvedStyle, prop: PropertyId, default: f32) -> f32 {
    match style.get(prop) {
        Some(CssValue::Length(v, LengthUnit::Px)) => *v as f32,
        Some(CssValue::Number(v)) => *v as f32,
        Some(CssValue::Auto) => default,
        _ => default,
    }
}

pub fn is_auto(style: &ResolvedStyle, prop: PropertyId) -> bool {
    matches!(style.get(prop), Some(CssValue::Auto) | None)
}
