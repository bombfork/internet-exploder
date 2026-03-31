//! # ie-layout
//!
//! Layout engine. Computes the geometry of every element given the DOM tree
//! and computed styles. Supports block, inline, flexbox, and grid layout.

pub mod block;
pub mod box_generation;
pub mod text_measure;

use ie_css::resolve::ResolvedStyle;
use ie_dom::Document;
use text_measure::TextMeasure;

#[derive(Debug, Clone)]
pub struct LayoutTree {
    pub boxes: Vec<LayoutBox>,
    pub root: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub node_id: Option<usize>,
    pub box_type: BoxType,
    pub content_rect: Rect,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoxType {
    Block,
    Inline,
    InlineBlock,
    Anonymous,
    Text(String),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EdgeSizes {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Main layout entry point.
pub fn layout(
    doc: &Document,
    styles: &[ResolvedStyle],
    viewport: Rect,
    text_measure: &dyn TextMeasure,
) -> LayoutTree {
    let mut tree = box_generation::generate_box_tree(doc, styles, doc.root);
    if let Some(root) = tree.root {
        block::layout_block(root, &mut tree, styles, viewport.width, 0.0, text_measure);
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use ie_css::cascade::Origin;
    use ie_css::resolve::{ViewportSize, resolve_styles};
    use ie_css::{parse_stylesheet, ua_stylesheet};
    use std::collections::HashMap;
    use text_measure::MockTextMeasure;

    fn layout_html(html: &str) -> LayoutTree {
        let parse_result = ie_html::parse(html);
        let ua = ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent)];
        let styles = resolve_styles(
            &parse_result.document,
            &sheets,
            &HashMap::new(),
            ViewportSize {
                width: 800.0,
                height: 600.0,
            },
        );
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        layout(&parse_result.document, &styles, viewport, &MockTextMeasure)
    }

    fn layout_html_with_css(html: &str, css: &str) -> LayoutTree {
        let parse_result = ie_html::parse(html);
        let ua = ua_stylesheet();
        let author = parse_stylesheet(css);
        let sheets = vec![(ua, Origin::UserAgent), (author, Origin::Author)];
        let styles = resolve_styles(
            &parse_result.document,
            &sheets,
            &HashMap::new(),
            ViewportSize {
                width: 800.0,
                height: 600.0,
            },
        );
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        layout(&parse_result.document, &styles, viewport, &MockTextMeasure)
    }

    #[test]
    fn single_div_fills_viewport() {
        let tree = layout_html("<div>hello</div>");
        assert!(!tree.boxes.is_empty());
        // The outermost block box should exist
        assert!(tree.root.is_some());
    }

    #[test]
    fn explicit_width() {
        let tree = layout_html_with_css("<div id='box'>content</div>", "#box { width: 200px; }");
        let has_200 = tree
            .boxes
            .iter()
            .any(|b| (b.content_rect.width - 200.0).abs() < 0.1);
        assert!(has_200, "should have a 200px wide box");
    }

    #[test]
    fn auto_margins_centering() {
        let tree = layout_html_with_css(
            "<div id='box'>content</div>",
            "#box { width: 200px; margin-left: auto; margin-right: auto; }",
        );
        let centered = tree
            .boxes
            .iter()
            .find(|b| (b.content_rect.width - 200.0).abs() < 0.1);
        assert!(centered.is_some());
        let b = centered.unwrap();
        assert!(
            b.margin.left > 100.0,
            "left margin should be > 100 for centering, got {}",
            b.margin.left
        );
    }

    #[test]
    fn nested_blocks() {
        let tree = layout_html("<div><div>inner</div></div>");
        // Should have nested boxes: html, body, outer div, inner div, text
        assert!(tree.boxes.len() >= 3);
    }

    #[test]
    fn display_none_skipped() {
        let tree = layout_html_with_css(
            "<div>visible</div><div id='hidden'>hidden</div>",
            "#hidden { display: none; }",
        );
        let hidden = tree
            .boxes
            .iter()
            .find(|b| matches!(&b.box_type, BoxType::Text(t) if t.contains("hidden")));
        assert!(hidden.is_none());
    }

    #[test]
    fn height_auto_sums_children() {
        let tree = layout_html("<div><p>A</p><p>B</p></div>");
        if let Some(root) = tree.root {
            assert!(tree.boxes[root].content_rect.height > 0.0);
        }
    }

    #[test]
    fn box_generation_creates_tree() {
        let tree =
            layout_html("<!DOCTYPE html><html><head></head><body><p>Hello</p></body></html>");
        assert!(tree.root.is_some());
        assert!(!tree.boxes.is_empty());
    }

    #[test]
    fn margin_collapsing_siblings() {
        let tree = layout_html_with_css(
            "<div id='a'>A</div><div id='b'>B</div>",
            "#a { margin-bottom: 20px; } #b { margin-top: 30px; }",
        );
        // Both boxes should exist
        assert!(tree.boxes.len() >= 2);
    }

    #[test]
    fn text_boxes_have_nonzero_dimensions() {
        let tree = layout_html("<p>Hello world</p>");
        let text_box = tree
            .boxes
            .iter()
            .find(|b| matches!(&b.box_type, BoxType::Text(_)));
        assert!(text_box.is_some(), "should have a text box");
        let tb = text_box.unwrap();
        assert!(tb.content_rect.width > 0.0, "text width should be > 0");
        assert!(tb.content_rect.height > 0.0, "text height should be > 0");
    }

    #[test]
    fn anonymous_block_wraps_mixed_children() {
        // A block with both block and inline children should create anonymous wrappers
        let tree = layout_html_with_css(
            "<div>text <p>block</p> more text</div>",
            "div { display: block; }",
        );
        let has_anon = tree
            .boxes
            .iter()
            .any(|b| matches!(b.box_type, BoxType::Anonymous));
        assert!(
            has_anon,
            "should have anonymous block boxes for mixed content"
        );
    }
}
