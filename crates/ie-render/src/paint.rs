use ie_css::resolve::ResolvedStyle;
use ie_css::values::{CssColor, CssValue, PropertyId};
use ie_layout::{BoxType, LayoutTree};

#[derive(Debug, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }

    pub fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    pub fn from_css(c: &CssColor) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }

    pub fn to_argb(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

#[derive(Debug, Clone)]
pub enum PaintCommand {
    FillRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
    },
    Text {
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
    },
}

pub fn build_display_list(tree: &LayoutTree, styles: &[ResolvedStyle]) -> Vec<PaintCommand> {
    let mut commands = Vec::new();
    if let Some(root) = tree.root {
        paint_box(root, tree, styles, &mut commands);
    }
    commands
}

fn paint_box(
    idx: usize,
    tree: &LayoutTree,
    styles: &[ResolvedStyle],
    commands: &mut Vec<PaintCommand>,
) {
    let layout_box = &tree.boxes[idx];

    let style = layout_box.node_id.and_then(|id| styles.get(id));

    // Check visibility
    if let Some(s) = style
        && let Some(CssValue::Keyword(v)) = s.get(PropertyId::Visibility)
        && v == "hidden"
    {
        return;
    }

    // Border box rect
    let border_x = layout_box.content_rect.x - layout_box.padding.left - layout_box.border.left;
    let border_y = layout_box.content_rect.y - layout_box.padding.top - layout_box.border.top;
    let border_w = layout_box.content_rect.width
        + layout_box.padding.left
        + layout_box.padding.right
        + layout_box.border.left
        + layout_box.border.right;
    let border_h = layout_box.content_rect.height
        + layout_box.padding.top
        + layout_box.padding.bottom
        + layout_box.border.top
        + layout_box.border.bottom;

    // 1. Background
    if let Some(s) = style
        && let Some(CssValue::Color(c)) = s.get(PropertyId::BackgroundColor)
        && c.a > 0
    {
        commands.push(PaintCommand::FillRect {
            x: border_x,
            y: border_y,
            width: border_w,
            height: border_h,
            color: Color::from_css(c),
        });
    }

    // 2. Borders
    if let Some(s) = style {
        let border_color = match s.get(PropertyId::BorderTopColor) {
            Some(CssValue::Color(c)) => Color::from_css(c),
            _ => Color::black(),
        };

        let bt = layout_box.border.top;
        let br = layout_box.border.right;
        let bb = layout_box.border.bottom;
        let bl = layout_box.border.left;

        if bt > 0.0 {
            commands.push(PaintCommand::FillRect {
                x: border_x,
                y: border_y,
                width: border_w,
                height: bt,
                color: border_color.clone(),
            });
        }
        if bb > 0.0 {
            commands.push(PaintCommand::FillRect {
                x: border_x,
                y: border_y + border_h - bb,
                width: border_w,
                height: bb,
                color: border_color.clone(),
            });
        }
        if bl > 0.0 {
            commands.push(PaintCommand::FillRect {
                x: border_x,
                y: border_y,
                width: bl,
                height: border_h,
                color: border_color.clone(),
            });
        }
        if br > 0.0 {
            commands.push(PaintCommand::FillRect {
                x: border_x + border_w - br,
                y: border_y,
                width: br,
                height: border_h,
                color: border_color,
            });
        }
    }

    // 3. Text
    if let BoxType::Text(ref text) = layout_box.box_type {
        let font_size = style
            .map(|s| match s.get(PropertyId::FontSize) {
                Some(CssValue::Length(v, _)) => *v as f32,
                _ => 16.0,
            })
            .unwrap_or(16.0);
        let color = style
            .and_then(|s| match s.get(PropertyId::Color) {
                Some(CssValue::Color(c)) => Some(Color::from_css(c)),
                _ => None,
            })
            .unwrap_or(Color::black());

        commands.push(PaintCommand::Text {
            text: text.clone(),
            x: layout_box.content_rect.x,
            y: layout_box.content_rect.y,
            font_size,
            color,
        });
    }

    // 4. Children
    for &child_idx in &layout_box.children {
        paint_box(child_idx, tree, styles, commands);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::software;

    #[test]
    fn display_list_from_colored_div() {
        let html = r#"<style>div { background-color: red; width: 100px; height: 100px; }</style><div></div>"#;
        let result = ie_html::parse(html);
        let ua = ie_css::ua_stylesheet();
        let mut sheets = vec![(ua, ie_css::cascade::Origin::UserAgent)];
        for css in &result.style_elements {
            sheets.push((
                ie_css::parse_stylesheet(css),
                ie_css::cascade::Origin::Author,
            ));
        }
        let styles = ie_css::resolve::resolve_styles(
            &result.document,
            &sheets,
            &std::collections::HashMap::new(),
            ie_css::resolve::ViewportSize::default(),
        );
        let viewport = ie_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let tree = ie_layout::layout(
            &result.document,
            &styles,
            viewport,
            &software::SoftwareTextMeasure,
        );
        let commands = build_display_list(&tree, &styles);
        let has_rect = commands
            .iter()
            .any(|c| matches!(c, PaintCommand::FillRect { .. }));
        assert!(has_rect, "should have at least one FillRect command");
    }

    #[test]
    fn display_list_has_text() {
        let html = "<p>Hello World</p>";
        let result = ie_html::parse(html);
        let ua = ie_css::ua_stylesheet();
        let sheets = vec![(ua, ie_css::cascade::Origin::UserAgent)];
        let styles = ie_css::resolve::resolve_styles(
            &result.document,
            &sheets,
            &std::collections::HashMap::new(),
            ie_css::resolve::ViewportSize::default(),
        );
        let viewport = ie_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let tree = ie_layout::layout(
            &result.document,
            &styles,
            viewport,
            &software::SoftwareTextMeasure,
        );
        let commands = build_display_list(&tree, &styles);
        let has_text = commands
            .iter()
            .any(|c| matches!(c, PaintCommand::Text { .. }));
        assert!(has_text, "should have text paint commands");
    }
}
