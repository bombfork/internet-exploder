use ie_dom::NodeId;

use crate::token::Attribute;

#[derive(Debug, Clone)]
pub enum FormattingEntry {
    Element {
        node_id: NodeId,
        tag_name: String,
        attributes: Vec<Attribute>,
    },
    Marker,
}

impl FormattingEntry {
    pub fn is_marker(&self) -> bool {
        matches!(self, FormattingEntry::Marker)
    }

    pub fn node_id(&self) -> Option<NodeId> {
        match self {
            FormattingEntry::Element { node_id, .. } => Some(*node_id),
            FormattingEntry::Marker => None,
        }
    }

    pub fn tag_name(&self) -> Option<&str> {
        match self {
            FormattingEntry::Element { tag_name, .. } => Some(tag_name),
            FormattingEntry::Marker => None,
        }
    }
}
