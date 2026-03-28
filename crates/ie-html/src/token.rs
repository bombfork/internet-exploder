#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Doctype {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        force_quirks: bool,
    },
    StartTag {
        name: String,
        attributes: Vec<Attribute>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Character(char),
    Comment(String),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

impl Token {
    pub fn is_start_tag(&self, name: &str) -> bool {
        matches!(self, Token::StartTag { name: n, .. } if n == name)
    }

    pub fn is_end_tag(&self, name: &str) -> bool {
        matches!(self, Token::EndTag { name: n } if n == name)
    }
}
