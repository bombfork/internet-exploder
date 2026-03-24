//! # ie-layout
//!
//! Layout engine. Computes the geometry of every element given the DOM tree
//! and computed styles. Supports block, inline, flexbox, and grid layout.

pub struct LayoutTree {
    pub boxes: Vec<LayoutBox>,
}

pub struct LayoutBox {
    pub rect: Rect,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub fn layout(
    _doc: &ie_dom::Document,
    _styles: &[ie_css::ComputedStyle],
    _viewport: Rect,
) -> LayoutTree {
    todo!("Layout computation")
}
