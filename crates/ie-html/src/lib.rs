//! # ie-html
//!
//! WHATWG HTML Living Standard parser.
//! Targets latest spec only — no quirks mode, no legacy element support.

pub mod tokenizer;
pub mod tree_builder;

pub use tree_builder::parse;
