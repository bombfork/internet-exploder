//! # ie-dom
//!
//! DOM tree data structures. Arena-allocated for cache-friendly traversal
//! and low allocation overhead.
//!
//! Arena memory note: `remove_child` detaches nodes but does not free arena
//! slots. Removed nodes remain in the `Vec<Node>`. Future optimization: use
//! a generational arena (e.g. `slotmap`) for slot reuse.

pub mod document;
pub mod error;
pub mod node;
pub mod traversal;

pub use document::Document;
pub use error::DomError;
pub use node::{Node, NodeId, NodeKind};
pub use traversal::{AncestorsIter, DescendantsIter};
