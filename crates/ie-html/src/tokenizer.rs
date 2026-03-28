use std::collections::VecDeque;

use crate::token::{Attribute, Token};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenizerState {
    Data,
    RcData,
    RawText,
    ScriptData,
    PlainText,
    TagOpen,
    EndTagOpen,
    TagName,
    RcDataLessThanSign,
    RcDataEndTagOpen,
    RcDataEndTagName,
    RawTextLessThanSign,
    RawTextEndTagOpen,
    RawTextEndTagName,
    ScriptDataLessThanSign,
    ScriptDataEndTagOpen,
    ScriptDataEndTagName,
    ScriptDataEscapeStart,
    ScriptDataEscapeStartDash,
    ScriptDataEscaped,
    ScriptDataEscapedDash,
    ScriptDataEscapedDashDash,
    ScriptDataEscapedLessThanSign,
    ScriptDataEscapedEndTagOpen,
    ScriptDataEscapedEndTagName,
    ScriptDataDoubleEscapeStart,
    ScriptDataDoubleEscaped,
    ScriptDataDoubleEscapedDash,
    ScriptDataDoubleEscapedDashDash,
    ScriptDataDoubleEscapedLessThanSign,
    ScriptDataDoubleEscapeEnd,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    BogusComment,
    MarkupDeclarationOpen,
    CommentStart,
    CommentStartDash,
    Comment,
    CommentLessThanSign,
    CommentLessThanSignBang,
    CommentLessThanSignBangDash,
    CommentLessThanSignBangDashDash,
    CommentEndDash,
    CommentEnd,
    CommentEndBang,
    Doctype,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    AfterDoctypePublicKeyword,
    BeforeDoctypePublicIdentifier,
    DoctypePublicIdentifierDoubleQuoted,
    DoctypePublicIdentifierSingleQuoted,
    AfterDoctypePublicIdentifier,
    BetweenDoctypePublicAndSystemIdentifiers,
    AfterDoctypeSystemKeyword,
    BeforeDoctypeSystemIdentifier,
    DoctypeSystemIdentifierDoubleQuoted,
    DoctypeSystemIdentifierSingleQuoted,
    AfterDoctypeSystemIdentifier,
    BogusDoctype,
    CDataSection,
    CDataSectionBracket,
    CDataSectionEnd,
    CharacterReference,
    NumericCharacterReference,
    HexadecimalCharacterReferenceStart,
    DecimalCharacterReferenceStart,
    HexadecimalCharacterReference,
    DecimalCharacterReference,
    NumericCharacterReferenceEnd,
    NamedCharacterReference,
    AmbiguousAmpersand,
}

struct TagBuilder {
    name: String,
    attributes: Vec<Attribute>,
    self_closing: bool,
    is_end_tag: bool,
}

impl TagBuilder {
    fn new_start() -> Self {
        Self {
            name: String::new(),
            attributes: Vec::new(),
            self_closing: false,
            is_end_tag: false,
        }
    }

    fn new_end() -> Self {
        Self {
            name: String::new(),
            attributes: Vec::new(),
            self_closing: false,
            is_end_tag: true,
        }
    }

    fn start_new_attribute(&mut self) {
        self.attributes.push(Attribute {
            name: String::new(),
            value: String::new(),
        });
    }

    fn current_attr_mut(&mut self) -> Option<&mut Attribute> {
        self.attributes.last_mut()
    }

    fn into_token(self) -> Token {
        if self.is_end_tag {
            Token::EndTag { name: self.name }
        } else {
            Token::StartTag {
                name: self.name,
                attributes: self.attributes,
                self_closing: self.self_closing,
            }
        }
    }
}

#[derive(Default)]
struct DoctypeBuilder {
    name: Option<String>,
    public_id: Option<String>,
    system_id: Option<String>,
    force_quirks: bool,
}

impl DoctypeBuilder {
    fn into_token(self) -> Token {
        Token::Doctype {
            name: self.name,
            public_id: self.public_id,
            system_id: self.system_id,
            force_quirks: self.force_quirks,
        }
    }
}

pub struct Tokenizer<'a> {
    #[allow(dead_code)]
    input: &'a str,
    chars: Vec<char>,
    pos: usize,
    state: TokenizerState,
    #[allow(dead_code)]
    return_state: TokenizerState,
    pending_tokens: VecDeque<Token>,
    current_tag: Option<TagBuilder>,
    current_comment: String,
    current_doctype: DoctypeBuilder,
    #[allow(dead_code)]
    temp_buffer: String,
    last_start_tag_name: Option<String>,
    finished: bool,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().collect(),
            pos: 0,
            state: TokenizerState::Data,
            return_state: TokenizerState::Data,
            pending_tokens: VecDeque::new(),
            current_tag: None,
            current_comment: String::new(),
            current_doctype: DoctypeBuilder::default(),
            temp_buffer: String::new(),
            last_start_tag_name: None,
            finished: false,
        }
    }

    pub fn set_state(&mut self, state: TokenizerState) {
        self.state = state;
    }

    fn next_char(&mut self) -> Option<char> {
        if self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }

    fn reconsume(&mut self) {
        if self.pos > 0 {
            self.pos -= 1;
        }
    }

    #[allow(dead_code)]
    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn push_to_tag_name(&mut self, c: char) {
        if let Some(tag) = &mut self.current_tag {
            tag.name.push(c);
        }
    }

    fn push_to_attr_name(&mut self, c: char) {
        if let Some(tag) = &mut self.current_tag
            && let Some(attr) = tag.current_attr_mut()
        {
            attr.name.push(c);
        }
    }

    fn push_to_attr_value(&mut self, c: char) {
        if let Some(tag) = &mut self.current_tag
            && let Some(attr) = tag.current_attr_mut()
        {
            attr.value.push(c);
        }
    }

    fn emit(&mut self, token: Token) {
        if let Token::StartTag { ref name, .. } = token {
            self.last_start_tag_name = Some(name.clone());
        }
        self.pending_tokens.push_back(token);
    }

    fn emit_current_tag(&mut self) {
        if let Some(tag) = self.current_tag.take() {
            let token = tag.into_token();
            self.emit(token);
        }
    }

    fn emit_current_comment(&mut self) {
        let comment = std::mem::take(&mut self.current_comment);
        self.emit(Token::Comment(comment));
    }

    fn emit_current_doctype(&mut self) {
        let doctype = std::mem::take(&mut self.current_doctype);
        self.emit(doctype.into_token());
    }

    fn step(&mut self) {
        match self.state {
            TokenizerState::Data => match self.next_char() {
                Some('<') => self.state = TokenizerState::TagOpen,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                }
                Some(c) => self.emit(Token::Character(c)),
                None => self.emit(Token::Eof),
            },
            TokenizerState::TagOpen => match self.next_char() {
                Some('!') => self.state = TokenizerState::MarkupDeclarationOpen,
                Some('/') => self.state = TokenizerState::EndTagOpen,
                Some(c) if c.is_ascii_alphabetic() => {
                    self.current_tag = Some(TagBuilder::new_start());
                    self.reconsume();
                    self.state = TokenizerState::TagName;
                }
                Some('?') => {
                    tracing::warn!(
                        "HTML parse error: unexpected-question-mark-instead-of-tag-name"
                    );
                    self.current_comment = String::new();
                    self.reconsume();
                    self.state = TokenizerState::BogusComment;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-before-tag-name");
                    self.emit(Token::Character('<'));
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    tracing::warn!("HTML parse error: invalid-first-character-of-tag-name");
                    self.emit(Token::Character('<'));
                    self.reconsume();
                    self.state = TokenizerState::Data;
                }
            },
            TokenizerState::EndTagOpen => match self.next_char() {
                Some(c) if c.is_ascii_alphabetic() => {
                    self.current_tag = Some(TagBuilder::new_end());
                    self.reconsume();
                    self.state = TokenizerState::TagName;
                }
                Some('>') => {
                    tracing::warn!("HTML parse error: missing-end-tag-name");
                    self.state = TokenizerState::Data;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-before-tag-name");
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    tracing::warn!("HTML parse error: invalid-first-character-of-tag-name");
                    self.current_comment = String::new();
                    self.reconsume();
                    self.state = TokenizerState::BogusComment;
                }
            },
            TokenizerState::TagName => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Some('/') => self.state = TokenizerState::SelfClosingStartTag,
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.push_to_tag_name('\u{FFFD}');
                }
                Some(c) => {
                    self.push_to_tag_name(c.to_ascii_lowercase());
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-tag");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::BeforeAttributeName => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('/' | '>') | None => {
                        self.reconsume();
                        self.state = TokenizerState::AfterAttributeName;
                    }
                    Some('=') => {
                        tracing::warn!(
                            "HTML parse error: unexpected-equals-sign-before-attribute-name"
                        );
                        if let Some(tag) = &mut self.current_tag {
                            tag.start_new_attribute();
                        }
                        self.push_to_attr_name('=');
                        self.state = TokenizerState::AttributeName;
                    }
                    Some(_) => {
                        if let Some(tag) = &mut self.current_tag {
                            tag.start_new_attribute();
                        }
                        self.reconsume();
                        self.state = TokenizerState::AttributeName;
                    }
                }
            }
            TokenizerState::AttributeName => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') | Some('/') | Some('>') | None => {
                    self.reconsume();
                    self.state = TokenizerState::AfterAttributeName;
                }
                Some('=') => self.state = TokenizerState::BeforeAttributeValue,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.push_to_attr_name('\u{FFFD}');
                }
                Some(c @ ('"' | '\'' | '<')) => {
                    tracing::warn!("HTML parse error: unexpected-character-in-attribute-name");
                    self.push_to_attr_name(c.to_ascii_lowercase());
                }
                Some(c) => {
                    self.push_to_attr_name(c.to_ascii_lowercase());
                }
            },
            TokenizerState::AfterAttributeName => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('/') => self.state = TokenizerState::SelfClosingStartTag,
                    Some('=') => self.state = TokenizerState::BeforeAttributeValue,
                    Some('>') => {
                        self.state = TokenizerState::Data;
                        self.emit_current_tag();
                    }
                    None => {
                        tracing::warn!("HTML parse error: eof-in-tag");
                        self.emit(Token::Eof);
                    }
                    Some(_) => {
                        if let Some(tag) = &mut self.current_tag {
                            tag.start_new_attribute();
                        }
                        self.reconsume();
                        self.state = TokenizerState::AttributeName;
                    }
                }
            }
            TokenizerState::BeforeAttributeValue => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('"') => self.state = TokenizerState::AttributeValueDoubleQuoted,
                    Some('\'') => self.state = TokenizerState::AttributeValueSingleQuoted,
                    Some('>') => {
                        tracing::warn!("HTML parse error: missing-attribute-value");
                        self.state = TokenizerState::Data;
                        self.emit_current_tag();
                    }
                    _ => {
                        self.reconsume();
                        self.state = TokenizerState::AttributeValueUnquoted;
                    }
                }
            }
            TokenizerState::AttributeValueDoubleQuoted => match self.next_char() {
                Some('"') => self.state = TokenizerState::AfterAttributeValueQuoted,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.push_to_attr_value('\u{FFFD}');
                }
                Some(c) => {
                    self.push_to_attr_value(c);
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-tag");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::AttributeValueSingleQuoted => match self.next_char() {
                Some('\'') => self.state = TokenizerState::AfterAttributeValueQuoted,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.push_to_attr_value('\u{FFFD}');
                }
                Some(c) => {
                    self.push_to_attr_value(c);
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-tag");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::AttributeValueUnquoted => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.push_to_attr_value('\u{FFFD}');
                }
                Some(c @ ('"' | '\'' | '<' | '=' | '`')) => {
                    tracing::warn!(
                        "HTML parse error: unexpected-character-in-unquoted-attribute-value"
                    );
                    self.push_to_attr_value(c);
                }
                Some(c) => {
                    self.push_to_attr_value(c);
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-tag");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::AfterAttributeValueQuoted => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Some('/') => self.state = TokenizerState::SelfClosingStartTag,
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-tag");
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    tracing::warn!("HTML parse error: missing-whitespace-between-attributes");
                    self.reconsume();
                    self.state = TokenizerState::BeforeAttributeName;
                }
            },
            TokenizerState::SelfClosingStartTag => match self.next_char() {
                Some('>') => {
                    if let Some(tag) = &mut self.current_tag {
                        tag.self_closing = true;
                    }
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-tag");
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    tracing::warn!("HTML parse error: unexpected-solidus-in-tag");
                    self.reconsume();
                    self.state = TokenizerState::BeforeAttributeName;
                }
            },
            TokenizerState::BogusComment => match self.next_char() {
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_comment();
                }
                None => {
                    self.emit_current_comment();
                    self.emit(Token::Eof);
                }
                Some('\0') => {
                    self.current_comment.push('\u{FFFD}');
                }
                Some(c) => {
                    self.current_comment.push(c);
                }
            },
            TokenizerState::MarkupDeclarationOpen => {
                // Check for "--" (comment), "DOCTYPE", or "[CDATA["
                if self.starts_with("--") {
                    self.pos += 2;
                    self.current_comment = String::new();
                    self.state = TokenizerState::CommentStart;
                } else if self.starts_with_ci("DOCTYPE") {
                    self.pos += 7;
                    self.state = TokenizerState::Doctype;
                } else if self.starts_with("[CDATA[") {
                    self.pos += 7;
                    // In HTML, CDATA is only valid in foreign content (SVG/MathML).
                    // For now, treat as bogus comment.
                    tracing::warn!("HTML parse error: cdata-in-html-content");
                    self.current_comment = String::from("[CDATA[");
                    self.state = TokenizerState::BogusComment;
                } else {
                    tracing::warn!("HTML parse error: incorrectly-opened-comment");
                    self.current_comment = String::new();
                    self.state = TokenizerState::BogusComment;
                }
            }
            TokenizerState::CommentStart => match self.next_char() {
                Some('-') => self.state = TokenizerState::CommentStartDash,
                Some('>') => {
                    tracing::warn!("HTML parse error: abrupt-closing-of-empty-comment");
                    self.state = TokenizerState::Data;
                    self.emit_current_comment();
                }
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::Comment;
                }
            },
            TokenizerState::CommentStartDash => match self.next_char() {
                Some('-') => self.state = TokenizerState::CommentEnd,
                Some('>') => {
                    tracing::warn!("HTML parse error: abrupt-closing-of-empty-comment");
                    self.state = TokenizerState::Data;
                    self.emit_current_comment();
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-comment");
                    self.emit_current_comment();
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    self.current_comment.push('-');
                    self.reconsume();
                    self.state = TokenizerState::Comment;
                }
            },
            TokenizerState::Comment => match self.next_char() {
                Some('<') => {
                    self.current_comment.push('<');
                    self.state = TokenizerState::CommentLessThanSign;
                }
                Some('-') => self.state = TokenizerState::CommentEndDash,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.current_comment.push('\u{FFFD}');
                }
                Some(c) => self.current_comment.push(c),
                None => {
                    tracing::warn!("HTML parse error: eof-in-comment");
                    self.emit_current_comment();
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::CommentLessThanSign => match self.next_char() {
                Some('!') => {
                    self.current_comment.push('!');
                    self.state = TokenizerState::CommentLessThanSignBang;
                }
                Some('<') => self.current_comment.push('<'),
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::Comment;
                }
            },
            TokenizerState::CommentLessThanSignBang => match self.next_char() {
                Some('-') => self.state = TokenizerState::CommentLessThanSignBangDash,
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::Comment;
                }
            },
            TokenizerState::CommentLessThanSignBangDash => match self.next_char() {
                Some('-') => self.state = TokenizerState::CommentLessThanSignBangDashDash,
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::CommentEndDash;
                }
            },
            TokenizerState::CommentLessThanSignBangDashDash => match self.next_char() {
                Some('>') | None => {
                    self.reconsume();
                    self.state = TokenizerState::CommentEnd;
                }
                Some(_) => {
                    tracing::warn!("HTML parse error: nested-comment");
                    self.reconsume();
                    self.state = TokenizerState::CommentEnd;
                }
            },
            TokenizerState::CommentEndDash => match self.next_char() {
                Some('-') => self.state = TokenizerState::CommentEnd,
                None => {
                    tracing::warn!("HTML parse error: eof-in-comment");
                    self.emit_current_comment();
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    self.current_comment.push('-');
                    self.reconsume();
                    self.state = TokenizerState::Comment;
                }
            },
            TokenizerState::CommentEnd => match self.next_char() {
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_comment();
                }
                Some('!') => self.state = TokenizerState::CommentEndBang,
                Some('-') => self.current_comment.push('-'),
                None => {
                    tracing::warn!("HTML parse error: eof-in-comment");
                    self.emit_current_comment();
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    self.current_comment.push('-');
                    self.current_comment.push('-');
                    self.reconsume();
                    self.state = TokenizerState::Comment;
                }
            },
            TokenizerState::CommentEndBang => match self.next_char() {
                Some('-') => {
                    self.current_comment.push('-');
                    self.current_comment.push('-');
                    self.current_comment.push('!');
                    self.state = TokenizerState::CommentEndDash;
                }
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_comment();
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-comment");
                    self.emit_current_comment();
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    self.current_comment.push('-');
                    self.current_comment.push('-');
                    self.current_comment.push('!');
                    self.reconsume();
                    self.state = TokenizerState::Comment;
                }
            },
            TokenizerState::Doctype => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::BeforeDoctypeName;
                }
                Some('>') => {
                    self.reconsume();
                    self.state = TokenizerState::BeforeDoctypeName;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-doctype");
                    self.current_doctype.force_quirks = true;
                    self.emit_current_doctype();
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    tracing::warn!("HTML parse error: missing-whitespace-before-doctype-name");
                    self.reconsume();
                    self.state = TokenizerState::BeforeDoctypeName;
                }
            },
            TokenizerState::BeforeDoctypeName => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('\0') => {
                        tracing::warn!("HTML parse error: unexpected-null-character");
                        self.current_doctype.name = Some("\u{FFFD}".to_string());
                        self.state = TokenizerState::DoctypeName;
                    }
                    Some('>') => {
                        tracing::warn!("HTML parse error: missing-doctype-name");
                        self.current_doctype.force_quirks = true;
                        self.state = TokenizerState::Data;
                        self.emit_current_doctype();
                    }
                    None => {
                        tracing::warn!("HTML parse error: eof-in-doctype");
                        self.current_doctype.force_quirks = true;
                        self.emit_current_doctype();
                        self.emit(Token::Eof);
                    }
                    Some(c) => {
                        self.current_doctype.name = Some(c.to_ascii_lowercase().to_string());
                        self.state = TokenizerState::DoctypeName;
                    }
                }
            }
            TokenizerState::DoctypeName => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::AfterDoctypeName;
                }
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_doctype();
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    if let Some(name) = &mut self.current_doctype.name {
                        name.push('\u{FFFD}');
                    }
                }
                Some(c) => {
                    if let Some(name) = &mut self.current_doctype.name {
                        name.push(c.to_ascii_lowercase());
                    }
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-doctype");
                    self.current_doctype.force_quirks = true;
                    self.emit_current_doctype();
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::AfterDoctypeName => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('>') => {
                        self.state = TokenizerState::Data;
                        self.emit_current_doctype();
                    }
                    None => {
                        tracing::warn!("HTML parse error: eof-in-doctype");
                        self.current_doctype.force_quirks = true;
                        self.emit_current_doctype();
                        self.emit(Token::Eof);
                    }
                    Some(_) => {
                        // Check for PUBLIC or SYSTEM keywords
                        self.reconsume();
                        if self.starts_with_ci("PUBLIC") {
                            self.pos += 6;
                            self.state = TokenizerState::AfterDoctypePublicKeyword;
                        } else if self.starts_with_ci("SYSTEM") {
                            self.pos += 6;
                            self.state = TokenizerState::AfterDoctypeSystemKeyword;
                        } else {
                            tracing::warn!(
                                "HTML parse error: invalid-character-sequence-after-doctype-name"
                            );
                            self.current_doctype.force_quirks = true;
                            self.reconsume();
                            self.state = TokenizerState::BogusDoctype;
                        }
                    }
                }
            }
            // Simplified doctype PUBLIC/SYSTEM handling
            TokenizerState::AfterDoctypePublicKeyword
            | TokenizerState::BeforeDoctypePublicIdentifier
            | TokenizerState::DoctypePublicIdentifierDoubleQuoted
            | TokenizerState::DoctypePublicIdentifierSingleQuoted
            | TokenizerState::AfterDoctypePublicIdentifier
            | TokenizerState::BetweenDoctypePublicAndSystemIdentifiers
            | TokenizerState::AfterDoctypeSystemKeyword
            | TokenizerState::BeforeDoctypeSystemIdentifier
            | TokenizerState::DoctypeSystemIdentifierDoubleQuoted
            | TokenizerState::DoctypeSystemIdentifierSingleQuoted
            | TokenizerState::AfterDoctypeSystemIdentifier => {
                // Consume until '>' for simplicity in Phase 2
                match self.next_char() {
                    Some('>') => {
                        self.state = TokenizerState::Data;
                        self.emit_current_doctype();
                    }
                    None => {
                        self.current_doctype.force_quirks = true;
                        self.emit_current_doctype();
                        self.emit(Token::Eof);
                    }
                    Some(_) => {} // consume
                }
            }
            TokenizerState::BogusDoctype => {
                match self.next_char() {
                    Some('>') => {
                        self.state = TokenizerState::Data;
                        self.emit_current_doctype();
                    }
                    None => {
                        self.emit_current_doctype();
                        self.emit(Token::Eof);
                    }
                    Some(_) => {} // consume and ignore
                }
            }
            TokenizerState::CDataSection => match self.next_char() {
                Some(']') => self.state = TokenizerState::CDataSectionBracket,
                None => {
                    tracing::warn!("HTML parse error: eof-in-cdata");
                    self.emit(Token::Eof);
                }
                Some(c) => self.emit(Token::Character(c)),
            },
            TokenizerState::CDataSectionBracket => match self.next_char() {
                Some(']') => self.state = TokenizerState::CDataSectionEnd,
                _ => {
                    self.emit(Token::Character(']'));
                    self.reconsume();
                    self.state = TokenizerState::CDataSection;
                }
            },
            TokenizerState::CDataSectionEnd => match self.next_char() {
                Some(']') => self.emit(Token::Character(']')),
                Some('>') => self.state = TokenizerState::Data,
                _ => {
                    self.emit(Token::Character(']'));
                    self.emit(Token::Character(']'));
                    self.reconsume();
                    self.state = TokenizerState::CDataSection;
                }
            },
            // States that will be implemented in Step 1b
            TokenizerState::RcData
            | TokenizerState::RawText
            | TokenizerState::ScriptData
            | TokenizerState::PlainText
            | TokenizerState::RcDataLessThanSign
            | TokenizerState::RcDataEndTagOpen
            | TokenizerState::RcDataEndTagName
            | TokenizerState::RawTextLessThanSign
            | TokenizerState::RawTextEndTagOpen
            | TokenizerState::RawTextEndTagName
            | TokenizerState::ScriptDataLessThanSign
            | TokenizerState::ScriptDataEndTagOpen
            | TokenizerState::ScriptDataEndTagName
            | TokenizerState::ScriptDataEscapeStart
            | TokenizerState::ScriptDataEscapeStartDash
            | TokenizerState::ScriptDataEscaped
            | TokenizerState::ScriptDataEscapedDash
            | TokenizerState::ScriptDataEscapedDashDash
            | TokenizerState::ScriptDataEscapedLessThanSign
            | TokenizerState::ScriptDataEscapedEndTagOpen
            | TokenizerState::ScriptDataEscapedEndTagName
            | TokenizerState::ScriptDataDoubleEscapeStart
            | TokenizerState::ScriptDataDoubleEscaped
            | TokenizerState::ScriptDataDoubleEscapedDash
            | TokenizerState::ScriptDataDoubleEscapedDashDash
            | TokenizerState::ScriptDataDoubleEscapedLessThanSign
            | TokenizerState::ScriptDataDoubleEscapeEnd
            | TokenizerState::CharacterReference
            | TokenizerState::NumericCharacterReference
            | TokenizerState::HexadecimalCharacterReferenceStart
            | TokenizerState::DecimalCharacterReferenceStart
            | TokenizerState::HexadecimalCharacterReference
            | TokenizerState::DecimalCharacterReference
            | TokenizerState::NumericCharacterReferenceEnd
            | TokenizerState::NamedCharacterReference
            | TokenizerState::AmbiguousAmpersand => {
                // Placeholder: emit characters as-is until these states are implemented
                match self.next_char() {
                    Some(c) => self.emit(Token::Character(c)),
                    None => self.emit(Token::Eof),
                }
            }
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        let remaining: String = self.chars[self.pos..].iter().collect();
        remaining.starts_with(s)
    }

    fn starts_with_ci(&self, s: &str) -> bool {
        let remaining: String = self.chars[self.pos..].iter().collect();
        remaining
            .get(..s.len())
            .is_some_and(|r| r.eq_ignore_ascii_case(s))
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        if self.finished {
            return None;
        }

        loop {
            if let Some(token) = self.pending_tokens.pop_front() {
                if token == Token::Eof {
                    self.finished = true;
                }
                return Some(token);
            }
            self.step();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::{Attribute, Token};

    fn tokenize(input: &str) -> Vec<Token> {
        Tokenizer::new(input).collect()
    }

    #[test]
    fn simple_tag() {
        let tokens = tokenize("<div>");
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "div".to_string(),
                    attributes: vec![],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn self_closing() {
        let tokens = tokenize("<br/>");
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "br".to_string(),
                    attributes: vec![],
                    self_closing: true,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn attributes_double_quoted() {
        let tokens = tokenize(r#"<a href="url" class="c">"#);
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "a".to_string(),
                    attributes: vec![
                        Attribute {
                            name: "href".to_string(),
                            value: "url".to_string()
                        },
                        Attribute {
                            name: "class".to_string(),
                            value: "c".to_string()
                        },
                    ],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn end_tag() {
        let tokens = tokenize("</div>");
        assert_eq!(
            tokens,
            vec![
                Token::EndTag {
                    name: "div".to_string()
                },
                Token::Eof
            ]
        );
    }

    #[test]
    fn comment() {
        let tokens = tokenize("<!-- hello -->");
        assert_eq!(
            tokens,
            vec![Token::Comment(" hello ".to_string()), Token::Eof]
        );
    }

    #[test]
    fn doctype() {
        let tokens = tokenize("<!DOCTYPE html>");
        assert_eq!(
            tokens,
            vec![
                Token::Doctype {
                    name: Some("html".to_string()),
                    public_id: None,
                    system_id: None,
                    force_quirks: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn character_data() {
        let tokens = tokenize("hello");
        assert_eq!(
            tokens,
            vec![
                Token::Character('h'),
                Token::Character('e'),
                Token::Character('l'),
                Token::Character('l'),
                Token::Character('o'),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn nested_tags() {
        let tokens = tokenize("<div><p>text</p></div>");
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "div".to_string(),
                    attributes: vec![],
                    self_closing: false,
                },
                Token::StartTag {
                    name: "p".to_string(),
                    attributes: vec![],
                    self_closing: false,
                },
                Token::Character('t'),
                Token::Character('e'),
                Token::Character('x'),
                Token::Character('t'),
                Token::EndTag {
                    name: "p".to_string()
                },
                Token::EndTag {
                    name: "div".to_string()
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn eof_in_tag() {
        let tokens = tokenize("<div");
        assert_eq!(tokens, vec![Token::Eof]);
    }

    #[test]
    fn unquoted_attribute() {
        let tokens = tokenize("<div id=main>");
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "div".to_string(),
                    attributes: vec![Attribute {
                        name: "id".to_string(),
                        value: "main".to_string(),
                    }],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn single_quoted_attribute() {
        let tokens = tokenize("<div class='test'>");
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "div".to_string(),
                    attributes: vec![Attribute {
                        name: "class".to_string(),
                        value: "test".to_string(),
                    }],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn mixed_quoting_attributes() {
        let tokens = tokenize(r#"<div id=main class="foo" data-x='bar'>"#);
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "div".to_string(),
                    attributes: vec![
                        Attribute {
                            name: "id".to_string(),
                            value: "main".to_string(),
                        },
                        Attribute {
                            name: "class".to_string(),
                            value: "foo".to_string(),
                        },
                        Attribute {
                            name: "data-x".to_string(),
                            value: "bar".to_string(),
                        },
                    ],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn boolean_attribute() {
        let tokens = tokenize("<input disabled>");
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "input".to_string(),
                    attributes: vec![Attribute {
                        name: "disabled".to_string(),
                        value: String::new(),
                    }],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn uppercase_tag_lowercased() {
        let tokens = tokenize("<DIV>");
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "div".to_string(),
                    attributes: vec![],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn full_html_page() {
        let input =
            "<!DOCTYPE html><html><head><title>Test</title></head><body><p>Hello</p></body></html>";
        let tokens = tokenize(input);
        // Should start with Doctype, then tags and characters, end with Eof
        assert!(matches!(tokens[0], Token::Doctype { .. }));
        assert!(matches!(tokens.last(), Some(Token::Eof)));
        assert!(tokens.iter().any(|t| t.is_start_tag("html")));
        assert!(tokens.iter().any(|t| t.is_start_tag("body")));
        assert!(tokens.iter().any(|t| t.is_end_tag("html")));
    }

    #[test]
    fn empty_comment() {
        let tokens = tokenize("<!---->");
        assert_eq!(tokens, vec![Token::Comment(String::new()), Token::Eof]);
    }

    #[test]
    fn iterator_based() {
        let mut tok = Tokenizer::new("<div>hi</div>");
        assert!(tok.next().unwrap().is_start_tag("div"));
        assert_eq!(tok.next(), Some(Token::Character('h')));
        assert_eq!(tok.next(), Some(Token::Character('i')));
        assert!(tok.next().unwrap().is_end_tag("div"));
        assert_eq!(tok.next(), Some(Token::Eof));
        assert_eq!(tok.next(), None);
    }
}
