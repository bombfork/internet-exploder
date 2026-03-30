use ie_dom::{Document, NodeId, NodeKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub compounds: Vec<SelectorComponent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectorComponent {
    Compound(CompoundSelector),
    Combinator(Combinator),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combinator {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompoundSelector {
    pub type_selector: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<AttributeSelector>,
    pub pseudo_classes: Vec<PseudoClass>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeSelector {
    pub name: String,
    pub op: Option<AttributeOp>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttributeOp {
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PseudoClass {
    Hover,
    Focus,
    Active,
    FirstChild,
    LastChild,
    Root,
    Empty,
    Not(Box<Selector>),
}

// --- Specificity ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity(pub u32, pub u32, pub u32);

pub fn specificity(selector: &Selector) -> Specificity {
    let mut a = 0u32;
    let mut b = 0u32;
    let mut c = 0u32;

    for component in &selector.compounds {
        if let SelectorComponent::Compound(compound) = component {
            specificity_compound(compound, &mut a, &mut b, &mut c);
        }
    }

    Specificity(a, b, c)
}

fn specificity_compound(compound: &CompoundSelector, a: &mut u32, b: &mut u32, c: &mut u32) {
    if let Some(id) = &compound.id {
        let _ = id;
        *a += 1;
    }
    *b += compound.classes.len() as u32;
    *b += compound.attributes.len() as u32;
    for pseudo in &compound.pseudo_classes {
        match pseudo {
            PseudoClass::Not(inner) => {
                let Specificity(ia, ib, ic) = specificity(inner);
                *a += ia;
                *b += ib;
                *c += ic;
            }
            _ => *b += 1,
        }
    }
    if let Some(ty) = &compound.type_selector
        && ty != "*"
    {
        *c += 1;
    }
}

// --- Parsing ---

pub fn parse_selector_list(input: &str) -> Vec<Selector> {
    let mut result = Vec::new();
    // Split on commas, but be careful about commas inside parens
    let mut depth = 0;
    let mut start = 0;
    let bytes = input.as_bytes();

    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b',' if depth == 0 => {
                let part = input[start..i].trim();
                if let Some(sel) = parse_selector(part) {
                    result.push(sel);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let part = input[start..].trim();
    if let Some(sel) = parse_selector(part) {
        result.push(sel);
    }
    result
}

pub fn parse_selector(input: &str) -> Option<Selector> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let mut parser = SelectorParser::new(input);
    parser.parse()
}

struct SelectorParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> SelectorParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            self.advance();
        }
    }

    fn consume_ident(&mut self) -> Option<String> {
        let start = self.pos;
        // Allow leading hyphen or underscore
        if self.peek().is_some_and(|c| c == '-' || c == '_') {
            self.advance();
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            self.advance();
        }
        if self.pos == start {
            return None;
        }
        Some(self.input[start..self.pos].to_string())
    }

    fn consume_string_or_ident(&mut self) -> Option<String> {
        self.skip_whitespace();
        match self.peek()? {
            '"' | '\'' => {
                let quote = self.advance().unwrap();
                let start = self.pos;
                while self.peek().is_some_and(|c| c != quote) {
                    self.advance();
                }
                let val = self.input[start..self.pos].to_string();
                self.advance(); // closing quote
                Some(val)
            }
            _ => self.consume_ident(),
        }
    }

    fn parse(&mut self) -> Option<Selector> {
        let mut compounds = Vec::new();

        self.skip_whitespace();
        let compound = self.parse_compound()?;
        compounds.push(SelectorComponent::Compound(compound));

        loop {
            let ws_before = self.pos;
            self.skip_whitespace();
            let had_whitespace = self.pos > ws_before;

            match self.peek() {
                None => break,
                Some('>') => {
                    self.advance();
                    self.skip_whitespace();
                    compounds.push(SelectorComponent::Combinator(Combinator::Child));
                    let compound = self.parse_compound()?;
                    compounds.push(SelectorComponent::Compound(compound));
                }
                Some('+') => {
                    self.advance();
                    self.skip_whitespace();
                    compounds.push(SelectorComponent::Combinator(Combinator::NextSibling));
                    let compound = self.parse_compound()?;
                    compounds.push(SelectorComponent::Compound(compound));
                }
                Some('~') => {
                    self.advance();
                    self.skip_whitespace();
                    compounds.push(SelectorComponent::Combinator(Combinator::SubsequentSibling));
                    let compound = self.parse_compound()?;
                    compounds.push(SelectorComponent::Compound(compound));
                }
                Some(_) if had_whitespace => {
                    compounds.push(SelectorComponent::Combinator(Combinator::Descendant));
                    let compound = self.parse_compound()?;
                    compounds.push(SelectorComponent::Compound(compound));
                }
                _ => break,
            }
        }

        Some(Selector { compounds })
    }

    fn parse_compound(&mut self) -> Option<CompoundSelector> {
        let mut sel = CompoundSelector::default();
        let mut has_something = false;

        // Optional type selector
        match self.peek() {
            Some('*') => {
                self.advance();
                sel.type_selector = Some("*".to_string());
                has_something = true;
            }
            Some(c) if c.is_ascii_alphabetic() || c == '-' || c == '_' => {
                if let Some(ident) = self.consume_ident() {
                    sel.type_selector = Some(ident.to_ascii_lowercase());
                    has_something = true;
                }
            }
            _ => {}
        }

        // Simple selectors: #id, .class, [attr], :pseudo
        loop {
            match self.peek() {
                Some('#') => {
                    self.advance();
                    sel.id = self.consume_ident();
                    has_something = true;
                }
                Some('.') => {
                    self.advance();
                    if let Some(class) = self.consume_ident() {
                        sel.classes.push(class);
                        has_something = true;
                    }
                }
                Some('[') => {
                    if let Some(attr) = self.parse_attribute() {
                        sel.attributes.push(attr);
                        has_something = true;
                    }
                }
                Some(':') => {
                    if let Some(pseudo) = self.parse_pseudo() {
                        sel.pseudo_classes.push(pseudo);
                        has_something = true;
                    }
                }
                _ => break,
            }
        }

        if has_something { Some(sel) } else { None }
    }

    fn parse_attribute(&mut self) -> Option<AttributeSelector> {
        self.advance(); // consume '['
        self.skip_whitespace();

        let name = self.consume_ident()?;
        self.skip_whitespace();

        if self.peek() == Some(']') {
            self.advance();
            return Some(AttributeSelector {
                name,
                op: None,
                value: None,
            });
        }

        // Parse operator
        let op = self.parse_attribute_op()?;
        self.skip_whitespace();

        let value = self.consume_string_or_ident();
        self.skip_whitespace();

        // Consume ']'
        if self.peek() == Some(']') {
            self.advance();
        }

        Some(AttributeSelector {
            name,
            op: Some(op),
            value,
        })
    }

    fn parse_attribute_op(&mut self) -> Option<AttributeOp> {
        match self.peek()? {
            '=' => {
                self.advance();
                Some(AttributeOp::Equals)
            }
            '~' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                }
                Some(AttributeOp::Includes)
            }
            '|' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                }
                Some(AttributeOp::DashMatch)
            }
            '^' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                }
                Some(AttributeOp::Prefix)
            }
            '$' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                }
                Some(AttributeOp::Suffix)
            }
            '*' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                }
                Some(AttributeOp::Substring)
            }
            _ => None,
        }
    }

    fn parse_pseudo(&mut self) -> Option<PseudoClass> {
        self.advance(); // consume ':'
        // Skip second colon for pseudo-elements (we don't support them but don't crash)
        if self.peek() == Some(':') {
            self.advance();
        }

        let name = self.consume_ident()?;
        match name.to_ascii_lowercase().as_str() {
            "hover" => Some(PseudoClass::Hover),
            "focus" => Some(PseudoClass::Focus),
            "active" => Some(PseudoClass::Active),
            "first-child" => Some(PseudoClass::FirstChild),
            "last-child" => Some(PseudoClass::LastChild),
            "root" => Some(PseudoClass::Root),
            "empty" => Some(PseudoClass::Empty),
            "not" => {
                // Consume '('
                if self.peek() != Some('(') {
                    return None;
                }
                self.advance();

                // Find matching ')'
                let start = self.pos;
                let mut depth = 1;
                while let Some(ch) = self.peek() {
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    self.advance();
                }
                let inner_str = &self.input[start..self.pos];
                // Consume ')'
                if self.peek() == Some(')') {
                    self.advance();
                }

                let inner = parse_selector(inner_str)?;
                Some(PseudoClass::Not(Box::new(inner)))
            }
            _ => None,
        }
    }
}

// --- Matching ---

pub fn matches(selector: &Selector, node: NodeId, doc: &Document) -> bool {
    let compounds = &selector.compounds;
    if compounds.is_empty() {
        return false;
    }

    // Must be an element node
    if !doc.node(node).is_some_and(|n| n.is_element()) {
        return false;
    }

    // Walk right-to-left
    let mut idx = compounds.len() - 1;

    // Rightmost must be a compound
    let SelectorComponent::Compound(ref rightmost) = compounds[idx] else {
        return false;
    };

    if !matches_compound(rightmost, node, doc) {
        return false;
    }

    if idx == 0 {
        return true;
    }

    // Process combinator + compound pairs going left
    let mut current_node = node;
    idx -= 1; // now at a combinator

    loop {
        let SelectorComponent::Combinator(combinator) = compounds[idx] else {
            return false;
        };

        if idx == 0 {
            return false; // combinator without a compound before it
        }
        idx -= 1;

        let SelectorComponent::Compound(ref compound) = compounds[idx] else {
            return false;
        };

        match combinator {
            Combinator::Descendant => {
                let mut found = false;
                let mut ancestor = doc.parent(current_node);
                while let Some(anc) = ancestor {
                    if doc.node(anc).is_some_and(|n| n.is_element())
                        && matches_compound(compound, anc, doc)
                    {
                        current_node = anc;
                        found = true;
                        break;
                    }
                    ancestor = doc.parent(anc);
                }
                if !found {
                    return false;
                }
            }
            Combinator::Child => {
                let Some(parent) = doc.parent(current_node) else {
                    return false;
                };
                if !doc.node(parent).is_some_and(|n| n.is_element())
                    || !matches_compound(compound, parent, doc)
                {
                    return false;
                }
                current_node = parent;
            }
            Combinator::NextSibling => {
                let Some(prev) = prev_element_sibling(current_node, doc) else {
                    return false;
                };
                if !matches_compound(compound, prev, doc) {
                    return false;
                }
                current_node = prev;
            }
            Combinator::SubsequentSibling => {
                let mut found = false;
                let mut sib = prev_element_sibling(current_node, doc);
                while let Some(s) = sib {
                    if matches_compound(compound, s, doc) {
                        current_node = s;
                        found = true;
                        break;
                    }
                    sib = prev_element_sibling(s, doc);
                }
                if !found {
                    return false;
                }
            }
        }

        if idx == 0 {
            return true;
        }
        idx -= 1;
    }
}

fn matches_compound(compound: &CompoundSelector, node: NodeId, doc: &Document) -> bool {
    let Some(n) = doc.node(node) else {
        return false;
    };

    // Type selector
    if let Some(ty) = &compound.type_selector
        && ty != "*"
    {
        let Some(elem_name) = n.element_name() else {
            return false;
        };
        if !elem_name.eq_ignore_ascii_case(ty) {
            return false;
        }
    }

    // ID
    if let Some(id) = &compound.id {
        let node_id = n.attributes.get("id");
        if node_id.map(|s| s.as_str()) != Some(id.as_str()) {
            return false;
        }
    }

    // Classes
    if !compound.classes.is_empty() {
        let class_attr = n.attributes.get("class").map(|s| s.as_str()).unwrap_or("");
        let node_classes: Vec<&str> = class_attr.split_ascii_whitespace().collect();
        for class in &compound.classes {
            if !node_classes.iter().any(|nc| nc.eq_ignore_ascii_case(class)) {
                return false;
            }
        }
    }

    // Attributes
    for attr_sel in &compound.attributes {
        let attr_val = n.attributes.get(&attr_sel.name).map(|s| s.as_str());
        match (&attr_sel.op, &attr_sel.value) {
            (None, _) => {
                // Presence check
                if attr_val.is_none() {
                    return false;
                }
            }
            (Some(op), Some(expected)) => {
                let Some(actual) = attr_val else {
                    return false;
                };
                if !match_attribute_op(*op, actual, expected) {
                    return false;
                }
            }
            (Some(_), None) => return false,
        }
    }

    // Pseudo-classes
    for pseudo in &compound.pseudo_classes {
        if !matches_pseudo(pseudo, node, doc) {
            return false;
        }
    }

    true
}

fn match_attribute_op(op: AttributeOp, actual: &str, expected: &str) -> bool {
    match op {
        AttributeOp::Equals => actual == expected,
        AttributeOp::Includes => actual.split_ascii_whitespace().any(|w| w == expected),
        AttributeOp::DashMatch => actual == expected || actual.starts_with(&format!("{expected}-")),
        AttributeOp::Prefix => actual.starts_with(expected),
        AttributeOp::Suffix => actual.ends_with(expected),
        AttributeOp::Substring => actual.contains(expected),
    }
}

fn matches_pseudo(pseudo: &PseudoClass, node: NodeId, doc: &Document) -> bool {
    match pseudo {
        PseudoClass::Hover | PseudoClass::Focus | PseudoClass::Active => false,
        PseudoClass::FirstChild => {
            let Some(parent) = doc.parent(node) else {
                return false;
            };
            doc.children(parent)
                .iter()
                .find(|&&c| doc.node(c).is_some_and(|n| n.is_element()))
                == Some(&node)
        }
        PseudoClass::LastChild => {
            let Some(parent) = doc.parent(node) else {
                return false;
            };
            doc.children(parent)
                .iter()
                .rfind(|&&c| doc.node(c).is_some_and(|n| n.is_element()))
                == Some(&node)
        }
        PseudoClass::Root => {
            let Some(parent) = doc.parent(node) else {
                return false;
            };
            doc.node(parent)
                .is_some_and(|n| matches!(n.kind, NodeKind::Document))
        }
        PseudoClass::Empty => doc.children(node).is_empty(),
        PseudoClass::Not(inner) => !matches(inner, node, doc),
    }
}

fn prev_element_sibling(node: NodeId, doc: &Document) -> Option<NodeId> {
    let parent = doc.parent(node)?;
    let children = doc.children(parent);
    let pos = children.iter().position(|&c| c == node)?;
    children[..pos]
        .iter()
        .rfind(|&&c| doc.node(c).is_some_and(|n| n.is_element()))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ie_dom::Document;

    // --- Parsing tests ---

    #[test]
    fn parse_tag_selector() {
        let sel = parse_selector("div").unwrap();
        assert_eq!(sel.compounds.len(), 1);
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!("expected compound");
        };
        assert_eq!(c.type_selector, Some("div".to_string()));
    }

    #[test]
    fn parse_class_selector() {
        let sel = parse_selector(".foo").unwrap();
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!();
        };
        assert_eq!(c.classes, vec!["foo"]);
    }

    #[test]
    fn parse_id_selector() {
        let sel = parse_selector("#main").unwrap();
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!();
        };
        assert_eq!(c.id, Some("main".to_string()));
    }

    #[test]
    fn parse_compound() {
        let sel = parse_selector("div.foo#bar").unwrap();
        assert_eq!(sel.compounds.len(), 1);
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!();
        };
        assert_eq!(c.type_selector, Some("div".to_string()));
        assert_eq!(c.classes, vec!["foo"]);
        assert_eq!(c.id, Some("bar".to_string()));
    }

    #[test]
    fn parse_descendant() {
        let sel = parse_selector("div p").unwrap();
        assert_eq!(sel.compounds.len(), 3);
        assert!(matches!(
            sel.compounds[1],
            SelectorComponent::Combinator(Combinator::Descendant)
        ));
    }

    #[test]
    fn parse_child() {
        let sel = parse_selector("div > p").unwrap();
        assert_eq!(sel.compounds.len(), 3);
        assert!(matches!(
            sel.compounds[1],
            SelectorComponent::Combinator(Combinator::Child)
        ));
    }

    #[test]
    fn parse_attribute() {
        let sel = parse_selector("[href]").unwrap();
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!();
        };
        assert_eq!(c.attributes.len(), 1);
        assert_eq!(c.attributes[0].name, "href");
        assert_eq!(c.attributes[0].op, None);
    }

    #[test]
    fn parse_attribute_equals() {
        let sel = parse_selector("[type=text]").unwrap();
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!();
        };
        assert_eq!(c.attributes[0].op, Some(AttributeOp::Equals));
        assert_eq!(c.attributes[0].value, Some("text".to_string()));
    }

    #[test]
    fn parse_pseudo_first_child() {
        let sel = parse_selector(":first-child").unwrap();
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!();
        };
        assert_eq!(c.pseudo_classes, vec![PseudoClass::FirstChild]);
    }

    #[test]
    fn parse_not() {
        let sel = parse_selector(":not(.hidden)").unwrap();
        let SelectorComponent::Compound(ref c) = sel.compounds[0] else {
            panic!();
        };
        assert_eq!(c.pseudo_classes.len(), 1);
        let PseudoClass::Not(ref inner) = c.pseudo_classes[0] else {
            panic!("expected Not");
        };
        let SelectorComponent::Compound(ref inner_c) = inner.compounds[0] else {
            panic!();
        };
        assert_eq!(inner_c.classes, vec!["hidden"]);
    }

    // --- Specificity tests ---

    #[test]
    fn specificity_id() {
        let sel = parse_selector("#id").unwrap();
        assert_eq!(specificity(&sel), Specificity(1, 0, 0));
    }

    #[test]
    fn specificity_class() {
        let sel = parse_selector(".class").unwrap();
        assert_eq!(specificity(&sel), Specificity(0, 1, 0));
    }

    #[test]
    fn specificity_tag() {
        let sel = parse_selector("div").unwrap();
        assert_eq!(specificity(&sel), Specificity(0, 0, 1));
    }

    #[test]
    fn specificity_compound() {
        let sel = parse_selector("div.foo#bar").unwrap();
        assert_eq!(specificity(&sel), Specificity(1, 1, 1));
    }

    // --- Matching tests ---

    fn build_test_doc() -> (Document, NodeId, NodeId, NodeId) {
        // root -> html -> body -> div.container#main -> p.intro
        let mut doc = Document::new();
        let root = doc.root;
        let html = doc.create_element("html");
        doc.append_child(root, html).unwrap();
        let body = doc.create_element("body");
        doc.append_child(html, body).unwrap();
        let div = doc.create_element("div");
        doc.append_child(body, div).unwrap();
        doc.set_attribute(div, "class", "container");
        doc.set_attribute(div, "id", "main");
        let p = doc.create_element("p");
        doc.append_child(div, p).unwrap();
        doc.set_attribute(p, "class", "intro");
        (doc, div, p, html)
    }

    #[test]
    fn selector_matches_tag() {
        let (doc, div, _, _) = build_test_doc();
        let sel_div = parse_selector("div").unwrap();
        let sel_p = parse_selector("p").unwrap();
        assert!(matches(&sel_div, div, &doc));
        assert!(!matches(&sel_p, div, &doc));
    }

    #[test]
    fn selector_matches_class() {
        let (doc, div, _, _) = build_test_doc();
        let sel = parse_selector(".container").unwrap();
        assert!(matches(&sel, div, &doc));
    }

    #[test]
    fn selector_matches_descendant() {
        let (doc, _, p, _) = build_test_doc();
        let sel = parse_selector("div p").unwrap();
        assert!(matches(&sel, p, &doc));
    }

    #[test]
    fn selector_matches_child_combinator() {
        let (doc, _, p, _) = build_test_doc();
        let sel = parse_selector("div > p").unwrap();
        assert!(matches(&sel, p, &doc));
        // body > p should not match (p's parent is div, not body)
        let sel2 = parse_selector("body > p").unwrap();
        assert!(!matches(&sel2, p, &doc));
    }

    #[test]
    fn selector_matches_id() {
        let (doc, div, _, _) = build_test_doc();
        let sel = parse_selector("#main").unwrap();
        assert!(matches(&sel, div, &doc));
    }

    #[test]
    fn selector_matches_not() {
        let (doc, div, p, _) = build_test_doc();
        let sel = parse_selector(":not(.intro)").unwrap();
        assert!(matches(&sel, div, &doc));
        assert!(!matches(&sel, p, &doc));
    }

    #[test]
    fn selector_matches_first_child() {
        let mut doc = Document::new();
        let root = doc.root;
        let parent = doc.create_element("div");
        doc.append_child(root, parent).unwrap();
        let first = doc.create_element("p");
        let second = doc.create_element("p");
        doc.append_child(parent, first).unwrap();
        doc.append_child(parent, second).unwrap();

        let sel = parse_selector(":first-child").unwrap();
        assert!(matches(&sel, first, &doc));
        assert!(!matches(&sel, second, &doc));
    }

    #[test]
    fn selector_list_parsing() {
        let list = parse_selector_list("div, .foo, #bar");
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn selector_matches_attribute_presence() {
        let mut doc = Document::new();
        let root = doc.root;
        let a = doc.create_element("a");
        doc.append_child(root, a).unwrap();
        doc.set_attribute(a, "href", "https://example.com");

        let sel = parse_selector("[href]").unwrap();
        assert!(matches(&sel, a, &doc));

        let sel2 = parse_selector("[title]").unwrap();
        assert!(!matches(&sel2, a, &doc));
    }

    #[test]
    fn selector_matches_sibling() {
        let mut doc = Document::new();
        let root = doc.root;
        let parent = doc.create_element("div");
        doc.append_child(root, parent).unwrap();
        let h1 = doc.create_element("h1");
        let p = doc.create_element("p");
        doc.append_child(parent, h1).unwrap();
        doc.append_child(parent, p).unwrap();

        let sel = parse_selector("h1 + p").unwrap();
        assert!(matches(&sel, p, &doc));
    }
}
