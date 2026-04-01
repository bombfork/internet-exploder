use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::NodeId;
use crate::error::DomError;
use crate::node::{Node, NodeKind};
use crate::traversal::{AncestorsIter, DescendantsIter};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    // --- Node creation ---

    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            kind: NodeKind::Element(tag.to_string()),
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        });
        id
    }

    pub fn create_text(&mut self, text: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            kind: NodeKind::Text(text.to_string()),
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        });
        id
    }

    pub fn create_comment(&mut self, text: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            kind: NodeKind::Comment(text.to_string()),
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        });
        id
    }

    pub fn create_doctype(
        &mut self,
        name: &str,
        public_id: Option<&str>,
        system_id: Option<&str>,
    ) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(Node {
            kind: NodeKind::Doctype {
                name: name.to_string(),
                public_id: public_id.map(|s| s.to_string()),
                system_id: system_id.map(|s| s.to_string()),
            },
            parent: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        });
        id
    }

    // --- Accessors ---

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.parent
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        match self.node(id) {
            Some(node) => &node.children,
            None => &[],
        }
    }

    pub fn get_attribute(&self, id: NodeId, name: &str) -> Option<&str> {
        self.node(id)?.attributes.get(name).map(|s| s.as_str())
    }

    pub fn set_attribute(&mut self, id: NodeId, name: &str, value: &str) {
        if let Some(node) = self.node_mut(id) {
            node.attributes.insert(name.to_string(), value.to_string());
        }
    }

    // --- Tree mutation ---

    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        self.validate_exists(parent)?;
        self.validate_exists(child)?;
        self.check_cycle(parent, child)?;
        self.detach(child);
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
        Ok(())
    }

    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        self.validate_exists(parent)?;
        self.validate_exists(child)?;
        if self.nodes[child].parent != Some(parent) {
            return Err(DomError::NotAChild);
        }
        self.nodes[parent].children.retain(|&id| id != child);
        self.nodes[child].parent = None;
        Ok(())
    }

    pub fn insert_before(
        &mut self,
        parent: NodeId,
        new_child: NodeId,
        reference: NodeId,
    ) -> Result<(), DomError> {
        self.validate_exists(parent)?;
        self.validate_exists(new_child)?;
        self.validate_exists(reference)?;
        if self.nodes[reference].parent != Some(parent) {
            return Err(DomError::NotAChild);
        }
        self.check_cycle(parent, new_child)?;
        self.detach(new_child);
        let pos = self.nodes[parent]
            .children
            .iter()
            .position(|&id| id == reference)
            .expect("reference is a child of parent");
        self.nodes[parent].children.insert(pos, new_child);
        self.nodes[new_child].parent = Some(parent);
        Ok(())
    }

    // --- Traversal ---

    pub fn descendants(&self, id: NodeId) -> DescendantsIter<'_> {
        DescendantsIter::new(self, id)
    }

    pub fn ancestors(&self, id: NodeId) -> AncestorsIter<'_> {
        AncestorsIter::new(self, id)
    }

    // --- Queries ---

    pub fn get_elements_by_tag_name(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        self.descendants(root)
            .filter(|&id| {
                self.node(id)
                    .is_some_and(|n| matches!(&n.kind, NodeKind::Element(name) if name == tag))
            })
            .collect()
    }

    pub fn get_element_by_id(&self, root: NodeId, id: &str) -> Option<NodeId> {
        self.descendants(root).find(|&node_id| {
            self.node(node_id)
                .is_some_and(|n| n.attributes.get("id").is_some_and(|v| v == id))
        })
    }

    // --- Arena stats ---

    /// Total allocated slots (including detached nodes).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Nodes actually in the tree: nodes with a parent, plus the root.
    pub fn live_node_count(&self) -> usize {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| n.parent.is_some() || *i == self.root)
            .count()
    }

    // --- Internal helpers ---

    fn validate_exists(&self, id: NodeId) -> Result<(), DomError> {
        if id >= self.nodes.len() {
            return Err(DomError::NodeNotFound(id));
        }
        Ok(())
    }

    fn check_cycle(&self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        if parent == child {
            return Err(DomError::CycleDetected);
        }
        // Walk ancestors of parent; if child is among them, it's a cycle
        for ancestor in self.ancestors(parent) {
            if ancestor == child {
                return Err(DomError::CycleDetected);
            }
        }
        Ok(())
    }

    /// Detach node from its current parent (if any).
    fn detach(&mut self, id: NodeId) {
        if let Some(old_parent) = self.nodes[id].parent {
            self.nodes[old_parent].children.retain(|&c| c != id);
            self.nodes[id].parent = None;
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Document & creation ---

    #[test]
    fn new_creates_root_document_node() {
        let doc = Document::new();
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.node(doc.root).unwrap().kind, NodeKind::Document);
    }

    #[test]
    fn create_element() {
        let mut doc = Document::new();
        let id = doc.create_element("div");
        let node = doc.node(id).unwrap();
        assert_eq!(node.kind, NodeKind::Element("div".to_string()));
        assert!(node.parent.is_none());
        assert!(node.children.is_empty());
    }

    #[test]
    fn create_text() {
        let mut doc = Document::new();
        let id = doc.create_text("hello");
        assert_eq!(
            doc.node(id).unwrap().kind,
            NodeKind::Text("hello".to_string())
        );
    }

    #[test]
    fn create_comment() {
        let mut doc = Document::new();
        let id = doc.create_comment("note");
        assert_eq!(
            doc.node(id).unwrap().kind,
            NodeKind::Comment("note".to_string())
        );
    }

    #[test]
    fn created_nodes_have_no_parent_no_children() {
        let mut doc = Document::new();
        let el = doc.create_element("p");
        let txt = doc.create_text("hi");
        let cmt = doc.create_comment("c");
        for id in [el, txt, cmt] {
            let node = doc.node(id).unwrap();
            assert!(node.parent.is_none());
            assert!(node.children.is_empty());
        }
    }

    // --- Tree mutation ---

    #[test]
    fn append_child_basic() {
        let mut doc = Document::new();
        let child = doc.create_element("div");
        doc.append_child(doc.root, child).unwrap();
        assert_eq!(doc.children(doc.root), &[child]);
        assert_eq!(doc.parent(child), Some(doc.root));
    }

    #[test]
    fn append_child_twice_same_parent() {
        let mut doc = Document::new();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let root = doc.root;
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();
        assert_eq!(doc.children(root), &[a, b]);
    }

    #[test]
    fn append_child_reparenting() {
        let mut doc = Document::new();
        let old_parent = doc.create_element("div");
        let new_parent = doc.create_element("span");
        let child = doc.create_element("p");
        let root = doc.root;
        doc.append_child(root, old_parent).unwrap();
        doc.append_child(root, new_parent).unwrap();
        doc.append_child(old_parent, child).unwrap();
        assert_eq!(doc.children(old_parent), &[child]);

        // Reparent child to new_parent
        doc.append_child(new_parent, child).unwrap();
        assert!(doc.children(old_parent).is_empty());
        assert_eq!(doc.children(new_parent), &[child]);
        assert_eq!(doc.parent(child), Some(new_parent));
    }

    #[test]
    fn remove_child_basic() {
        let mut doc = Document::new();
        let child = doc.create_element("div");
        let root = doc.root;
        doc.append_child(root, child).unwrap();
        doc.remove_child(root, child).unwrap();
        assert!(doc.children(root).is_empty());
        assert!(doc.parent(child).is_none());
    }

    #[test]
    fn remove_child_wrong_parent() {
        let mut doc = Document::new();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let root = doc.root;
        doc.append_child(root, a).unwrap();
        assert!(matches!(
            doc.remove_child(root, b),
            Err(DomError::NotAChild)
        ));
    }

    #[test]
    fn remove_child_nonexistent_ids() {
        let mut doc = Document::new();
        assert!(matches!(
            doc.remove_child(999, 0),
            Err(DomError::NodeNotFound(999))
        ));
    }

    #[test]
    fn insert_before_basic() {
        let mut doc = Document::new();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let root = doc.root;
        doc.append_child(root, b).unwrap();
        doc.insert_before(root, a, b).unwrap();
        assert_eq!(doc.children(root), &[a, b]);
    }

    #[test]
    fn insert_before_wrong_reference() {
        let mut doc = Document::new();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let root = doc.root;
        // b is not a child of root
        assert!(matches!(
            doc.insert_before(root, a, b),
            Err(DomError::NotAChild)
        ));
    }

    #[test]
    fn insert_before_reparenting() {
        let mut doc = Document::new();
        let old_parent = doc.create_element("old");
        let new_parent = doc.create_element("new");
        let child = doc.create_element("child");
        let ref_node = doc.create_element("ref");
        let root = doc.root;
        doc.append_child(root, old_parent).unwrap();
        doc.append_child(root, new_parent).unwrap();
        doc.append_child(old_parent, child).unwrap();
        doc.append_child(new_parent, ref_node).unwrap();

        doc.insert_before(new_parent, child, ref_node).unwrap();
        assert!(doc.children(old_parent).is_empty());
        assert_eq!(doc.children(new_parent), &[child, ref_node]);
    }

    // --- Cycle detection ---

    #[test]
    fn cycle_self_loop() {
        let mut doc = Document::new();
        let a = doc.create_element("a");
        assert!(matches!(
            doc.append_child(a, a),
            Err(DomError::CycleDetected)
        ));
    }

    #[test]
    fn cycle_chain() {
        let mut doc = Document::new();
        let root = doc.root;
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let c = doc.create_element("c");
        doc.append_child(root, a).unwrap();
        doc.append_child(a, b).unwrap();
        doc.append_child(b, c).unwrap();
        assert!(matches!(
            doc.append_child(c, a),
            Err(DomError::CycleDetected)
        ));
    }

    #[test]
    fn cycle_root_self() {
        let mut doc = Document::new();
        let root = doc.root;
        assert!(matches!(
            doc.append_child(root, root),
            Err(DomError::CycleDetected)
        ));
    }

    #[test]
    fn cycle_deep_chain() {
        let mut doc = Document::new();
        let root = doc.root;
        let mut prev = root;
        let mut nodes = Vec::new();
        for i in 0..10 {
            let n = doc.create_element(&format!("n{i}"));
            doc.append_child(prev, n).unwrap();
            nodes.push(n);
            prev = n;
        }
        // Try to append root as child of deepest
        assert!(matches!(
            doc.append_child(*nodes.last().unwrap(), root),
            Err(DomError::CycleDetected)
        ));
    }

    // --- Accessors ---

    #[test]
    fn node_accessor() {
        let doc = Document::new();
        assert!(doc.node(doc.root).is_some());
        assert!(doc.node(999).is_none());
    }

    #[test]
    fn attribute_round_trip() {
        let mut doc = Document::new();
        let el = doc.create_element("div");
        doc.set_attribute(el, "class", "main");
        assert_eq!(doc.get_attribute(el, "class"), Some("main"));
    }

    #[test]
    fn get_attribute_nonexistent() {
        let mut doc = Document::new();
        let el = doc.create_element("div");
        assert_eq!(doc.get_attribute(el, "nope"), None);
    }

    #[test]
    fn children_correct_slice() {
        let mut doc = Document::new();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let root = doc.root;
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();
        assert_eq!(doc.children(root), &[a, b]);
    }

    #[test]
    fn children_nonexistent_node() {
        let doc = Document::new();
        assert!(doc.children(999).is_empty());
    }

    // --- Traversal ---

    #[test]
    fn descendants_depth_first() {
        // root → [A, B], A → [C, D], B → [E]
        let mut doc = Document::new();
        let root = doc.root;
        let a = doc.create_element("A");
        let b = doc.create_element("B");
        let c = doc.create_element("C");
        let d = doc.create_element("D");
        let e = doc.create_element("E");
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();
        doc.append_child(a, c).unwrap();
        doc.append_child(a, d).unwrap();
        doc.append_child(b, e).unwrap();

        let result: Vec<_> = doc.descendants(root).collect();
        assert_eq!(result, vec![a, c, d, b, e]);
    }

    #[test]
    fn descendants_leaf() {
        let mut doc = Document::new();
        let leaf = doc.create_element("leaf");
        let root = doc.root;
        doc.append_child(root, leaf).unwrap();
        let result: Vec<_> = doc.descendants(leaf).collect();
        assert!(result.is_empty());
    }

    #[test]
    fn descendants_single_child() {
        let mut doc = Document::new();
        let root = doc.root;
        let parent = doc.create_element("parent");
        let child = doc.create_element("child");
        let grandchild = doc.create_element("grandchild");
        doc.append_child(root, parent).unwrap();
        doc.append_child(parent, child).unwrap();
        doc.append_child(child, grandchild).unwrap();
        let result: Vec<_> = doc.descendants(parent).collect();
        assert_eq!(result, vec![child, grandchild]);
    }

    #[test]
    fn ancestors_basic() {
        let mut doc = Document::new();
        let root = doc.root;
        let b = doc.create_element("B");
        let e = doc.create_element("E");
        doc.append_child(root, b).unwrap();
        doc.append_child(b, e).unwrap();
        let result: Vec<_> = doc.ancestors(e).collect();
        assert_eq!(result, vec![b, root]);
    }

    #[test]
    fn ancestors_root_is_empty() {
        let doc = Document::new();
        let result: Vec<_> = doc.ancestors(doc.root).collect();
        assert!(result.is_empty());
    }

    // --- Queries ---

    #[test]
    fn get_elements_by_tag_name_finds_divs() {
        let mut doc = Document::new();
        let root = doc.root;
        let div1 = doc.create_element("div");
        let span = doc.create_element("span");
        let div2 = doc.create_element("div");
        doc.append_child(root, div1).unwrap();
        doc.append_child(root, span).unwrap();
        doc.append_child(span, div2).unwrap();
        let result = doc.get_elements_by_tag_name(root, "div");
        assert_eq!(result, vec![div1, div2]);
    }

    #[test]
    fn get_elements_by_tag_name_no_matches() {
        let doc = Document::new();
        assert!(doc.get_elements_by_tag_name(doc.root, "div").is_empty());
    }

    #[test]
    fn get_element_by_id_found() {
        let mut doc = Document::new();
        let root = doc.root;
        let el = doc.create_element("div");
        doc.append_child(root, el).unwrap();
        doc.set_attribute(el, "id", "main");
        assert_eq!(doc.get_element_by_id(root, "main"), Some(el));
    }

    #[test]
    fn get_element_by_id_not_found() {
        let doc = Document::new();
        assert_eq!(doc.get_element_by_id(doc.root, "nope"), None);
    }

    #[test]
    fn get_element_by_id_returns_first() {
        let mut doc = Document::new();
        let root = doc.root;
        let a = doc.create_element("div");
        let b = doc.create_element("div");
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();
        doc.set_attribute(a, "id", "dup");
        doc.set_attribute(b, "id", "dup");
        assert_eq!(doc.get_element_by_id(root, "dup"), Some(a));
    }

    // --- Arena memory ---

    #[test]
    fn node_count_includes_all() {
        let mut doc = Document::new();
        for _ in 0..5 {
            doc.create_element("div");
        }
        assert_eq!(doc.node_count(), 6); // root + 5
    }

    #[test]
    fn live_node_count_only_attached() {
        let mut doc = Document::new();
        let root = doc.root;
        let mut attached = Vec::new();
        for _ in 0..3 {
            let n = doc.create_element("div");
            doc.append_child(root, n).unwrap();
            attached.push(n);
        }
        // 2 detached
        doc.create_element("orphan");
        doc.create_element("orphan");
        assert_eq!(doc.live_node_count(), 4); // root + 3 attached
    }

    #[test]
    fn remove_child_arena_stats() {
        let mut doc = Document::new();
        let root = doc.root;
        let child = doc.create_element("div");
        doc.append_child(root, child).unwrap();
        assert_eq!(doc.node_count(), 2);
        assert_eq!(doc.live_node_count(), 2);

        doc.remove_child(root, child).unwrap();
        assert_eq!(doc.node_count(), 2); // unchanged
        assert_eq!(doc.live_node_count(), 1); // root only
    }

    // --- Serialization ---

    #[test]
    fn serde_round_trip() {
        let mut doc = Document::new();
        let root = doc.root;
        let div = doc.create_element("div");
        let text = doc.create_text("hello");
        doc.append_child(root, div).unwrap();
        doc.append_child(div, text).unwrap();
        doc.set_attribute(div, "class", "main");

        let json = serde_json::to_string(&doc).unwrap();
        let restored: Document = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.nodes.len(), doc.nodes.len());
        assert_eq!(restored.root, doc.root);
        assert_eq!(
            restored.node(div).unwrap().kind,
            NodeKind::Element("div".to_string())
        );
        assert_eq!(
            restored.node(text).unwrap().kind,
            NodeKind::Text("hello".to_string())
        );
        assert_eq!(restored.node(div).unwrap().children, vec![text]);
        assert_eq!(
            restored
                .node(div)
                .unwrap()
                .attributes
                .get("class")
                .map(|s| s.as_str()),
            Some("main")
        );
    }
}
