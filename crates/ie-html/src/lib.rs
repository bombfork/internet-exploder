//! # ie-html
//!
//! WHATWG HTML Living Standard parser.
//! Targets latest spec only — no quirks mode, no legacy element support.

pub mod entities;
pub mod insertion_mode;
pub mod token;
pub mod tokenizer;
pub mod tree_builder;

pub use token::Token;
pub use tokenizer::Tokenizer;
pub use tree_builder::{ParseResult, parse};
