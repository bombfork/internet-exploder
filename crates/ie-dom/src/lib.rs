//! # ie-dom
//!
//! DOM tree data structures. Arena-allocated for cache-friendly traversal
//! and low allocation overhead.

use std::collections::HashMap;

pub type NodeId = usize;

pub struct Document {
    pub nodes: Vec<Node>,
    pub root: NodeId,
}

impl Document {
    pub fn new() -> Self {
        let root = Node {
            kind: NodeKind::Document,
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        };
        Self {
            nodes: vec![root],
            root: 0,
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Node {
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub attributes: HashMap<String, String>,
}

pub enum NodeKind {
    Document,
    Element(String),
    Text(String),
    Comment(String),
}
