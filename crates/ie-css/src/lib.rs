//! # ie-css
//!
//! CSS parser and style resolution engine.
//! Targets latest CSS spec only — no vendor prefixes, no legacy properties.

pub mod cascade;
pub mod parser;
pub mod resolve;
pub mod selector;
pub mod style;
pub mod tokenizer;
pub mod values;

pub use cascade::cascade;
pub use parser::{Declaration, Rule, Stylesheet, parse_declarations, parse_stylesheet};
pub use resolve::{ResolvedStyle, ViewportSize, resolve_styles};
pub use selector::{
    Selector, Specificity, matches as selector_matches, parse_selector, parse_selector_list,
    specificity,
};
pub use style::ComputedStyle;
pub use tokenizer::{CssToken, CssTokenizer};
pub use values::{CssColor, CssValue, LengthUnit, PropertyId};

/// The built-in user-agent stylesheet (WHATWG default rendering).
pub fn ua_stylesheet() -> Stylesheet {
    parse_stylesheet(include_str!("../data/ua.css"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cascade::Origin;

    fn ua_value_for(tag: &str, prop: PropertyId) -> Option<CssValue> {
        let ua = ua_stylesheet();
        let mut doc = ie_dom::Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let body = doc.create_element("body");
        let _ = doc.append_child(html, body);
        let el = doc.create_element(tag);
        let _ = doc.append_child(body, el);
        let styles = cascade(&[(ua, Origin::UserAgent)], el, &doc);
        styles.get(&prop).cloned()
    }

    #[test]
    fn ua_div_display_block() {
        let val = ua_value_for("div", PropertyId::Display);
        assert_eq!(val, Some(CssValue::Keyword("block".into())));
    }

    #[test]
    fn ua_span_display_inline() {
        let val = ua_value_for("span", PropertyId::Display);
        assert_eq!(val, Some(CssValue::Keyword("inline".into())));
    }

    #[test]
    fn ua_head_display_none() {
        // head is child of html, not body
        let ua = ua_stylesheet();
        let mut doc = ie_dom::Document::new();
        let html = doc.create_element("html");
        let _ = doc.append_child(doc.root, html);
        let head = doc.create_element("head");
        let _ = doc.append_child(html, head);
        let styles = cascade(&[(ua, Origin::UserAgent)], head, &doc);
        assert_eq!(styles.get(&PropertyId::Display), Some(&CssValue::None));
    }

    #[test]
    fn ua_h1_larger_than_p() {
        let h1_size = ua_value_for("h1", PropertyId::FontSize);
        let p_size = ua_value_for("p", PropertyId::FontSize);
        // h1 should have an explicit font-size, p should not (or smaller)
        match h1_size {
            Some(CssValue::Length(h1_px, _)) => {
                let p_px = match p_size {
                    Some(CssValue::Length(px, _)) => px,
                    _ => 16.0, // default
                };
                assert!(
                    h1_px > p_px,
                    "h1 ({h1_px}px) should be larger than p ({p_px}px)"
                );
            }
            other => panic!("expected h1 font-size as Length, got {other:?}"),
        }
    }

    #[test]
    fn ua_stylesheet_parses_without_panic() {
        let ua = ua_stylesheet();
        assert!(!ua.rules.is_empty(), "UA stylesheet should have rules");
    }
}
