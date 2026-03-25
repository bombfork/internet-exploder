use crate::NodeId;

#[derive(Debug, thiserror::Error)]
pub enum DomError {
    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("cycle detected")]
    CycleDetected,

    #[error("not a child")]
    NotAChild,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        assert_eq!(DomError::NodeNotFound(42).to_string(), "node not found: 42");
        assert_eq!(DomError::CycleDetected.to_string(), "cycle detected");
        assert_eq!(DomError::NotAChild.to_string(), "not a child");
    }
}
