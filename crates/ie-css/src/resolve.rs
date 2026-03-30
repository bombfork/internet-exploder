use std::collections::HashMap;

use ie_dom::{Document, NodeId};

use crate::cascade::{Origin, cascade};
use crate::parser::{Declaration, Stylesheet};
use crate::values::{CssColor, CssValue, LengthUnit, PropertyId, initial_value, is_inherited};

#[derive(Debug, Clone, Copy)]
pub struct ViewportSize {
    pub width: f64,
    pub height: f64,
}

impl Default for ViewportSize {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1080.0,
        }
    }
}

/// Resolved computed style for a node — all values in absolute units.
#[derive(Debug, Clone, Default)]
pub struct ResolvedStyle {
    pub properties: HashMap<PropertyId, CssValue>,
}

impl ResolvedStyle {
    pub fn get(&self, prop: PropertyId) -> Option<&CssValue> {
        self.properties.get(&prop)
    }

    pub fn display(&self) -> &str {
        match self.properties.get(&PropertyId::Display) {
            Some(CssValue::Keyword(k)) => k.as_str(),
            Some(CssValue::None) => "none",
            _ => "inline",
        }
    }

    pub fn get_length_px(&self, prop: PropertyId) -> f64 {
        match self.properties.get(&prop) {
            Some(CssValue::Length(v, LengthUnit::Px)) => *v,
            Some(CssValue::Number(v)) => *v,
            _ => 0.0,
        }
    }

    pub fn get_color(&self, prop: PropertyId) -> Option<&CssColor> {
        match self.properties.get(&prop) {
            Some(CssValue::Color(c)) => Some(c),
            _ => None,
        }
    }
}

/// Resolve styles for all element nodes in a document.
pub fn resolve_styles(
    doc: &Document,
    stylesheets: &[(Stylesheet, Origin)],
    inline_styles: &HashMap<NodeId, Vec<Declaration>>,
    viewport: ViewportSize,
) -> Vec<ResolvedStyle> {
    let node_count = doc.node_count();
    let mut styles: Vec<ResolvedStyle> =
        (0..node_count).map(|_| ResolvedStyle::default()).collect();

    resolve_node(
        doc,
        doc.root,
        &mut styles,
        stylesheets,
        inline_styles,
        viewport,
        None,
    );
    styles
}

fn resolve_node(
    doc: &Document,
    node_id: NodeId,
    styles: &mut [ResolvedStyle],
    stylesheets: &[(Stylesheet, Origin)],
    inline_styles: &HashMap<NodeId, Vec<Declaration>>,
    viewport: ViewportSize,
    parent_style: Option<&ResolvedStyle>,
) {
    let is_element = doc.node(node_id).is_some_and(|n| n.is_element());

    if is_element {
        // Cascade: collect matched declarations from all stylesheets
        let mut cascaded = cascade(stylesheets, node_id, doc);

        // Inline styles override (highest priority)
        if let Some(inline) = inline_styles.get(&node_id) {
            for decl in inline {
                cascaded.insert(decl.property, decl.value.clone());
            }
        }

        // Resolve each property: cascaded → inherited → initial → computed
        let mut resolved = HashMap::new();
        for &prop in ALL_PROPERTIES {
            let value = if let Some(v) = cascaded.get(&prop) {
                match v {
                    CssValue::Inherit => inherit_or_initial(prop, parent_style),
                    CssValue::Initial => initial_value(prop),
                    other => other.clone(),
                }
            } else if is_inherited(prop) {
                inherit_or_initial(prop, parent_style)
            } else {
                initial_value(prop)
            };

            let computed = compute_value(prop, &value, parent_style, viewport);
            resolved.insert(prop, computed);
        }

        styles[node_id] = ResolvedStyle {
            properties: resolved,
        };
    }

    // Collect children before recursing to avoid borrow issues
    let children: Vec<NodeId> = doc
        .node(node_id)
        .map(|n| n.children.clone())
        .unwrap_or_default();

    // Build a pointer to the current node's style for use as parent.
    // SAFETY: We never modify styles[node_id] again after this point,
    // and resolve_node only writes to descendant indices.
    let parent_ref: Option<&ResolvedStyle> = if is_element && node_id < styles.len() {
        Some(unsafe { &*(std::ptr::from_ref(&styles[node_id])) })
    } else {
        parent_style
    };

    for &child_id in &children {
        resolve_node(
            doc,
            child_id,
            styles,
            stylesheets,
            inline_styles,
            viewport,
            parent_ref,
        );
    }
}

fn inherit_or_initial(prop: PropertyId, parent: Option<&ResolvedStyle>) -> CssValue {
    parent
        .and_then(|ps| ps.get(prop).cloned())
        .unwrap_or_else(|| initial_value(prop))
}

fn compute_value(
    _prop: PropertyId,
    value: &CssValue,
    parent: Option<&ResolvedStyle>,
    viewport: ViewportSize,
) -> CssValue {
    match value {
        CssValue::Length(v, LengthUnit::Em) => {
            let parent_font_size = parent
                .and_then(|ps| ps.get(PropertyId::FontSize))
                .and_then(|v| match v {
                    CssValue::Length(px, LengthUnit::Px) => Some(*px),
                    _ => None,
                })
                .unwrap_or(16.0);
            CssValue::Length(v * parent_font_size, LengthUnit::Px)
        }
        CssValue::Length(v, LengthUnit::Rem) => {
            // Root font-size is always 16px
            CssValue::Length(v * 16.0, LengthUnit::Px)
        }
        CssValue::Length(v, LengthUnit::Vw) => {
            CssValue::Length(v / 100.0 * viewport.width, LengthUnit::Px)
        }
        CssValue::Length(v, LengthUnit::Vh) => {
            CssValue::Length(v / 100.0 * viewport.height, LengthUnit::Px)
        }
        CssValue::Length(v, LengthUnit::Pt) => CssValue::Length(v * 4.0 / 3.0, LengthUnit::Px),
        CssValue::Length(v, LengthUnit::Vmin) => {
            let min = viewport.width.min(viewport.height);
            CssValue::Length(v / 100.0 * min, LengthUnit::Px)
        }
        CssValue::Length(v, LengthUnit::Vmax) => {
            let max = viewport.width.max(viewport.height);
            CssValue::Length(v / 100.0 * max, LengthUnit::Px)
        }
        // Px, Percent, and other values pass through
        other => other.clone(),
    }
}

const ALL_PROPERTIES: &[PropertyId] = &[
    PropertyId::Display,
    PropertyId::Width,
    PropertyId::Height,
    PropertyId::MinWidth,
    PropertyId::MaxWidth,
    PropertyId::MinHeight,
    PropertyId::MaxHeight,
    PropertyId::MarginTop,
    PropertyId::MarginRight,
    PropertyId::MarginBottom,
    PropertyId::MarginLeft,
    PropertyId::PaddingTop,
    PropertyId::PaddingRight,
    PropertyId::PaddingBottom,
    PropertyId::PaddingLeft,
    PropertyId::BorderTopWidth,
    PropertyId::BorderRightWidth,
    PropertyId::BorderBottomWidth,
    PropertyId::BorderLeftWidth,
    PropertyId::BorderTopStyle,
    PropertyId::BorderRightStyle,
    PropertyId::BorderBottomStyle,
    PropertyId::BorderLeftStyle,
    PropertyId::BorderTopColor,
    PropertyId::BorderRightColor,
    PropertyId::BorderBottomColor,
    PropertyId::BorderLeftColor,
    PropertyId::BoxSizing,
    PropertyId::FontFamily,
    PropertyId::FontSize,
    PropertyId::FontWeight,
    PropertyId::FontStyle,
    PropertyId::LineHeight,
    PropertyId::TextAlign,
    PropertyId::TextDecoration,
    PropertyId::Color,
    PropertyId::WhiteSpace,
    PropertyId::Position,
    PropertyId::Top,
    PropertyId::Right,
    PropertyId::Bottom,
    PropertyId::Left,
    PropertyId::ZIndex,
    PropertyId::FlexDirection,
    PropertyId::FlexWrap,
    PropertyId::JustifyContent,
    PropertyId::AlignItems,
    PropertyId::AlignSelf,
    PropertyId::FlexGrow,
    PropertyId::FlexShrink,
    PropertyId::FlexBasis,
    PropertyId::BackgroundColor,
    PropertyId::Overflow,
    PropertyId::Visibility,
    PropertyId::Opacity,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cascade::Origin;
    use crate::parser::{parse_declarations, parse_stylesheet};

    fn make_doc_with_element(tag: &str) -> (Document, NodeId) {
        let mut doc = Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let body = doc.create_element("body");
        let _ = doc.append_child(html, body);
        let el = doc.create_element(tag);
        let _ = doc.append_child(body, el);
        (doc, el)
    }

    #[test]
    fn simple_color() {
        let (doc, el) = make_doc_with_element("p");
        let sheet = parse_stylesheet("p { color: red; }");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent), (sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        assert_eq!(
            styles[el].get(PropertyId::Color),
            Some(&CssValue::Color(CssColor::rgb(255, 0, 0)))
        );
    }

    #[test]
    fn inheritance() {
        let mut doc = Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let body = doc.create_element("body");
        let _ = doc.append_child(html, body);
        let p = doc.create_element("p");
        let _ = doc.append_child(body, p);

        let sheet = parse_stylesheet("body { color: blue; }");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent), (sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        // p inherits color from body
        assert_eq!(
            styles[p].get(PropertyId::Color),
            Some(&CssValue::Color(CssColor::rgb(0, 0, 255)))
        );
    }

    #[test]
    fn non_inherited_property_gets_initial() {
        let mut doc = Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let body = doc.create_element("body");
        let _ = doc.append_child(html, body);
        let p = doc.create_element("p");
        let _ = doc.append_child(body, p);

        let sheet = parse_stylesheet("body { margin-top: 10px; }");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent), (sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        // p does NOT inherit margin — gets initial (auto)
        let margin = styles[p].get(PropertyId::MarginTop);
        assert_ne!(margin, Some(&CssValue::Length(10.0, LengthUnit::Px)));
    }

    #[test]
    fn em_resolution() {
        let mut doc = Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let body = doc.create_element("body");
        let _ = doc.append_child(html, body);
        let p = doc.create_element("p");
        let _ = doc.append_child(body, p);

        let sheet = parse_stylesheet("body { font-size: 20px; } p { font-size: 1.5em; }");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent), (sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        // 20 * 1.5 = 30px
        assert_eq!(
            styles[p].get(PropertyId::FontSize),
            Some(&CssValue::Length(30.0, LengthUnit::Px))
        );
    }

    #[test]
    fn rem_resolution() {
        let mut doc = Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let body = doc.create_element("body");
        let _ = doc.append_child(html, body);
        let p = doc.create_element("p");
        let _ = doc.append_child(body, p);

        let sheet = parse_stylesheet("p { font-size: 2rem; }");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent), (sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        // 2 * 16 = 32px
        assert_eq!(
            styles[p].get(PropertyId::FontSize),
            Some(&CssValue::Length(32.0, LengthUnit::Px))
        );
    }

    #[test]
    fn viewport_units() {
        let (doc, el) = make_doc_with_element("div");
        let sheet = parse_stylesheet("div { width: 50vw; height: 25vh; }");
        let viewport = ViewportSize {
            width: 800.0,
            height: 600.0,
        };
        let sheets = vec![(sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), viewport);
        assert_eq!(
            styles[el].get(PropertyId::Width),
            Some(&CssValue::Length(400.0, LengthUnit::Px))
        );
        assert_eq!(
            styles[el].get(PropertyId::Height),
            Some(&CssValue::Length(150.0, LengthUnit::Px))
        );
    }

    #[test]
    fn ua_defaults() {
        let (doc, div_id) = make_doc_with_element("div");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        assert_eq!(styles[div_id].display(), "block");
    }

    #[test]
    fn inline_style_override() {
        let (doc, el) = make_doc_with_element("p");
        let sheet = parse_stylesheet("p { color: red; }");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent), (sheet, Origin::Author)];
        let inline_decls = parse_declarations("color: green;");
        let mut inline_map = HashMap::new();
        inline_map.insert(el, inline_decls);
        let styles = resolve_styles(&doc, &sheets, &inline_map, ViewportSize::default());
        assert_eq!(
            styles[el].get(PropertyId::Color),
            Some(&CssValue::Color(CssColor::rgb(0, 128, 0)))
        );
    }

    #[test]
    fn display_none_from_ua() {
        let mut doc = Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let head = doc.create_element("head");
        let _ = doc.append_child(html, head);

        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        assert_eq!(styles[head].display(), "none");
    }

    #[test]
    fn deep_inheritance_chain() {
        let mut doc = Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let body = doc.create_element("body");
        let _ = doc.append_child(html, body);
        let div = doc.create_element("div");
        let _ = doc.append_child(body, div);
        let span = doc.create_element("span");
        let _ = doc.append_child(div, span);

        let sheet = parse_stylesheet("body { color: navy; }");
        let ua = crate::ua_stylesheet();
        let sheets = vec![(ua, Origin::UserAgent), (sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        // Color should propagate through div → span
        assert_eq!(
            styles[span].get(PropertyId::Color),
            Some(&CssValue::Color(CssColor::rgb(0, 0, 128)))
        );
    }

    #[test]
    fn initial_value_for_unset_properties() {
        let (doc, el) = make_doc_with_element("div");
        let sheets: Vec<(Stylesheet, Origin)> = vec![];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        // Default display for element with no rules is "inline" (initial)
        assert_eq!(styles[el].display(), "inline");
        // Default opacity is 1.0
        assert_eq!(
            styles[el].get(PropertyId::Opacity),
            Some(&CssValue::Number(1.0))
        );
    }

    #[test]
    fn get_length_px_helper() {
        let (doc, el) = make_doc_with_element("div");
        let sheet = parse_stylesheet("div { width: 200px; }");
        let sheets = vec![(sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        assert!((styles[el].get_length_px(PropertyId::Width) - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn get_color_helper() {
        let (doc, el) = make_doc_with_element("div");
        let sheet = parse_stylesheet("div { color: red; }");
        let sheets = vec![(sheet, Origin::Author)];
        let styles = resolve_styles(&doc, &sheets, &HashMap::new(), ViewportSize::default());
        assert_eq!(
            styles[el].get_color(PropertyId::Color),
            Some(&CssColor::rgb(255, 0, 0))
        );
    }
}
