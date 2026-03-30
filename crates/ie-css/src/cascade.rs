use std::collections::HashMap;

use ie_dom::{Document, NodeId};

use crate::parser::Stylesheet;
use crate::selector::{Specificity, matches, parse_selector_list, specificity};
use crate::values::{CssValue, PropertyId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    UserAgent,
    Author,
}

struct CascadeEntry {
    property: PropertyId,
    value: CssValue,
    important: bool,
    origin: Origin,
    specificity: Specificity,
    source_order: usize,
}

/// Resolve cascaded values for a node
pub fn cascade(
    stylesheets: &[(Stylesheet, Origin)],
    node: NodeId,
    doc: &Document,
) -> HashMap<PropertyId, CssValue> {
    let mut entries: Vec<CascadeEntry> = Vec::new();
    let mut order = 0;

    for (stylesheet, origin) in stylesheets {
        for rule in &stylesheet.rules {
            let selectors: Vec<_> = rule
                .selectors
                .iter()
                .flat_map(|s| parse_selector_list(s))
                .collect();

            let max_specificity = selectors
                .iter()
                .filter(|sel| matches(sel, node, doc))
                .map(specificity)
                .max();

            if let Some(spec) = max_specificity {
                for decl in &rule.declarations {
                    entries.push(CascadeEntry {
                        property: decl.property,
                        value: decl.value.clone(),
                        important: decl.important,
                        origin: *origin,
                        specificity: spec,
                        source_order: order,
                    });
                    order += 1;
                }
            }
        }
    }

    // Sort: important wins, then origin, then specificity, then source order
    entries.sort_by(|a, b| {
        match (a.important, b.important) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => {}
        }
        a.origin
            .cmp(&b.origin)
            .then(a.specificity.cmp(&b.specificity))
            .then(a.source_order.cmp(&b.source_order))
    });

    // Last entry for each property wins
    let mut result = HashMap::new();
    for entry in entries {
        result.insert(entry.property, entry.value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_stylesheet;
    use crate::values::{CssColor, CssValue, PropertyId};
    use ie_dom::Document;

    fn make_doc_with_div(class: &str, id: &str) -> (Document, NodeId) {
        let mut doc = Document::new();
        let root = doc.root;
        let div = doc.create_element("div");
        doc.append_child(root, div).unwrap();
        doc.set_attribute(div, "class", class);
        doc.set_attribute(div, "id", id);
        (doc, div)
    }

    #[test]
    fn cascade_higher_specificity_wins() {
        let (doc, div) = make_doc_with_div("main", "app");
        let ss = parse_stylesheet("div { color: red; } #app { color: blue; }");
        let sheets = vec![(ss, Origin::Author)];
        let result = cascade(&sheets, div, &doc);
        assert_eq!(
            result.get(&PropertyId::Color),
            Some(&CssValue::Color(CssColor::rgb(0, 0, 255)))
        );
    }

    #[test]
    fn cascade_later_rule_wins_same_specificity() {
        let (doc, div) = make_doc_with_div("", "");
        let ss = parse_stylesheet("div { color: red; } div { color: blue; }");
        let sheets = vec![(ss, Origin::Author)];
        let result = cascade(&sheets, div, &doc);
        assert_eq!(
            result.get(&PropertyId::Color),
            Some(&CssValue::Color(CssColor::rgb(0, 0, 255)))
        );
    }

    #[test]
    fn cascade_important_overrides() {
        let (doc, div) = make_doc_with_div("", "app");
        let ss = parse_stylesheet("div { color: red !important; } #app { color: blue; }");
        let sheets = vec![(ss, Origin::Author)];
        let result = cascade(&sheets, div, &doc);
        assert_eq!(
            result.get(&PropertyId::Color),
            Some(&CssValue::Color(CssColor::rgb(255, 0, 0)))
        );
    }
}
