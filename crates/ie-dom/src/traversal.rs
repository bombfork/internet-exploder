use crate::{Document, NodeId};

/// Depth-first pre-order traversal of descendants (excludes the starting node).
pub struct DescendantsIter<'a> {
    doc: &'a Document,
    stack: Vec<NodeId>,
}

impl<'a> DescendantsIter<'a> {
    pub(crate) fn new(doc: &'a Document, start: NodeId) -> Self {
        let mut stack = Vec::new();
        if let Some(node) = doc.node(start) {
            // Push children in reverse so leftmost is popped first
            for &child in node.children.iter().rev() {
                stack.push(child);
            }
        }
        Self { doc, stack }
    }
}

impl<'a> Iterator for DescendantsIter<'a> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.stack.pop()?;
        if let Some(node) = self.doc.node(id) {
            for &child in node.children.iter().rev() {
                self.stack.push(child);
            }
        }
        Some(id)
    }
}

/// Walks ancestor chain (excludes the starting node).
pub struct AncestorsIter<'a> {
    doc: &'a Document,
    current: Option<NodeId>,
}

impl<'a> AncestorsIter<'a> {
    pub(crate) fn new(doc: &'a Document, start: NodeId) -> Self {
        let current = doc.node(start).and_then(|n| n.parent);
        Self { doc, current }
    }
}

impl<'a> Iterator for AncestorsIter<'a> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.current?;
        self.current = self.doc.node(id).and_then(|n| n.parent);
        Some(id)
    }
}
