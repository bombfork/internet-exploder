//! # ie-css
//!
//! CSS parser and style resolution engine.
//! Targets latest CSS spec only — no vendor prefixes, no legacy properties.

pub mod cascade;
pub mod parser;
pub mod selector;
pub mod style;
pub mod tokenizer;
pub mod values;

pub use cascade::cascade;
pub use parser::{Declaration, Rule, Stylesheet, parse_declarations, parse_stylesheet};
pub use selector::{
    Selector, Specificity, matches as selector_matches, parse_selector, parse_selector_list,
    specificity,
};
pub use style::ComputedStyle;
pub use tokenizer::{CssToken, CssTokenizer};
pub use values::{CssColor, CssValue, LengthUnit, PropertyId};
