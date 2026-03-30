//! # ie-css
//!
//! CSS parser and style resolution engine.
//! Targets latest CSS spec only — no vendor prefixes, no legacy properties.

pub mod parser;
pub mod selector;
pub mod style;
pub mod tokenizer;

pub use parser::parse_stylesheet;
pub use style::ComputedStyle;
pub use tokenizer::{CssToken, CssTokenizer};
