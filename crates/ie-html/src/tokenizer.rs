use anyhow::Result;

pub struct Tokenizer<'a> {
    #[expect(dead_code)]
    input: &'a str,
    #[expect(dead_code)]
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        todo!("HTML tokenization per WHATWG spec")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Doctype,
    StartTag { name: String, self_closing: bool },
    EndTag { name: String },
    Character(char),
    Comment(String),
    Eof,
}
