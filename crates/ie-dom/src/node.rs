use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type NodeId = usize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Document,
    Element(String),
    Text(String),
    Comment(String),
}

impl Node {
    pub fn is_element(&self) -> bool {
        matches!(self.kind, NodeKind::Element(_))
    }

    pub fn is_text(&self) -> bool {
        matches!(self.kind, NodeKind::Text(_))
    }

    pub fn element_name(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Element(name) => Some(name),
            _ => None,
        }
    }

    pub fn text_content(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Text(text) | NodeKind::Comment(text) => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_element(tag: &str) -> Node {
        Node {
            kind: NodeKind::Element(tag.to_string()),
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    fn make_text(text: &str) -> Node {
        Node {
            kind: NodeKind::Text(text.to_string()),
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    fn make_comment(text: &str) -> Node {
        Node {
            kind: NodeKind::Comment(text.to_string()),
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    #[test]
    fn is_element() {
        assert!(make_element("div").is_element());
        assert!(!make_text("hello").is_element());
    }

    #[test]
    fn is_text() {
        assert!(make_text("hello").is_text());
        assert!(!make_element("div").is_text());
    }

    #[test]
    fn element_name() {
        assert_eq!(make_element("div").element_name(), Some("div"));
        assert_eq!(make_text("hello").element_name(), None);
    }

    #[test]
    fn text_content() {
        assert_eq!(make_text("hello").text_content(), Some("hello"));
        assert_eq!(make_comment("note").text_content(), Some("note"));
        assert_eq!(make_element("div").text_content(), None);
    }
}
