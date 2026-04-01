use std::collections::{HashSet, VecDeque};

use crate::entities;
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
    return_state: TokenizerState,
    pending_tokens: VecDeque<Token>,
    current_tag: Option<TagBuilder>,
    current_comment: String,
    current_doctype: DoctypeBuilder,
    temp_buffer: String,
    last_start_tag_name: Option<String>,
    last_consumed: bool,
    char_ref_code: u32,
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
            last_consumed: false,
            char_ref_code: 0,
            finished: false,
        }
    }

    pub fn set_state(&mut self, state: TokenizerState) {
        self.state = state;
    }

    pub fn set_last_start_tag(&mut self, name: &str) {
        self.last_start_tag_name = Some(name.to_string());
    }

    fn next_char(&mut self) -> Option<char> {
        if self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            self.pos += 1;
            self.last_consumed = true;
            Some(c)
        } else {
            self.last_consumed = false;
            None
        }
    }

    fn reconsume(&mut self) {
        if self.last_consumed && self.pos > 0 {
            self.pos -= 1;
            self.last_consumed = false;
        }
    }

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
        if let Some(mut tag) = self.current_tag.take() {
            // Deduplicate attributes: first occurrence wins (WHATWG spec)
            let mut seen = HashSet::new();
            tag.attributes.retain(|attr| seen.insert(attr.name.clone()));
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
                Some('&') => {
                    self.return_state = TokenizerState::Data;
                    self.state = TokenizerState::CharacterReference;
                }
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
                Some('&') => {
                    self.return_state = TokenizerState::AttributeValueDoubleQuoted;
                    self.state = TokenizerState::CharacterReference;
                }
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
                Some('&') => {
                    self.return_state = TokenizerState::AttributeValueSingleQuoted;
                    self.state = TokenizerState::CharacterReference;
                }
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
                Some('&') => {
                    self.return_state = TokenizerState::AttributeValueUnquoted;
                    self.state = TokenizerState::CharacterReference;
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
                Some(_) => {
                    self.reconsume();
                    self.state = TokenizerState::CommentEndDash;
                }
                None => {
                    self.state = TokenizerState::CommentEndDash;
                }
            },
            TokenizerState::CommentLessThanSignBangDashDash => match self.peek_char() {
                Some('>') | None => {
                    // Don't consume; CommentEnd will handle it
                    self.state = TokenizerState::CommentEnd;
                }
                Some(_) => {
                    tracing::warn!("HTML parse error: nested-comment");
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
            TokenizerState::AfterDoctypePublicKeyword => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::BeforeDoctypePublicIdentifier;
                }
                Some('"') => {
                    tracing::warn!(
                        "HTML parse error: missing-whitespace-after-doctype-public-keyword"
                    );
                    self.current_doctype.public_id = Some(String::new());
                    self.state = TokenizerState::DoctypePublicIdentifierDoubleQuoted;
                }
                Some('\'') => {
                    tracing::warn!(
                        "HTML parse error: missing-whitespace-after-doctype-public-keyword"
                    );
                    self.current_doctype.public_id = Some(String::new());
                    self.state = TokenizerState::DoctypePublicIdentifierSingleQuoted;
                }
                Some('>') => {
                    tracing::warn!("HTML parse error: missing-doctype-public-identifier");
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
                Some(_) => {
                    tracing::warn!(
                        "HTML parse error: missing-quote-before-doctype-public-identifier"
                    );
                    self.current_doctype.force_quirks = true;
                    self.state = TokenizerState::BogusDoctype;
                }
            },
            TokenizerState::BeforeDoctypePublicIdentifier => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('"') => {
                        self.current_doctype.public_id = Some(String::new());
                        self.state = TokenizerState::DoctypePublicIdentifierDoubleQuoted;
                    }
                    Some('\'') => {
                        self.current_doctype.public_id = Some(String::new());
                        self.state = TokenizerState::DoctypePublicIdentifierSingleQuoted;
                    }
                    Some('>') => {
                        tracing::warn!("HTML parse error: missing-doctype-public-identifier");
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
                    Some(_) => {
                        tracing::warn!(
                            "HTML parse error: missing-quote-before-doctype-public-identifier"
                        );
                        self.current_doctype.force_quirks = true;
                        self.state = TokenizerState::BogusDoctype;
                    }
                }
            }
            TokenizerState::DoctypePublicIdentifierDoubleQuoted => match self.next_char() {
                Some('"') => {
                    self.state = TokenizerState::AfterDoctypePublicIdentifier;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    if let Some(id) = &mut self.current_doctype.public_id {
                        id.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    tracing::warn!("HTML parse error: abrupt-doctype-public-identifier");
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
                    if let Some(id) = &mut self.current_doctype.public_id {
                        id.push(c);
                    }
                }
            },
            TokenizerState::DoctypePublicIdentifierSingleQuoted => match self.next_char() {
                Some('\'') => {
                    self.state = TokenizerState::AfterDoctypePublicIdentifier;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    if let Some(id) = &mut self.current_doctype.public_id {
                        id.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    tracing::warn!("HTML parse error: abrupt-doctype-public-identifier");
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
                    if let Some(id) = &mut self.current_doctype.public_id {
                        id.push(c);
                    }
                }
            },
            TokenizerState::AfterDoctypePublicIdentifier => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::BetweenDoctypePublicAndSystemIdentifiers;
                }
                Some('>') => {
                    self.state = TokenizerState::Data;
                    self.emit_current_doctype();
                }
                Some('"') => {
                    tracing::warn!(
                        "HTML parse error: missing-whitespace-between-doctype-public-and-system-identifiers"
                    );
                    self.current_doctype.system_id = Some(String::new());
                    self.state = TokenizerState::DoctypeSystemIdentifierDoubleQuoted;
                }
                Some('\'') => {
                    tracing::warn!(
                        "HTML parse error: missing-whitespace-between-doctype-public-and-system-identifiers"
                    );
                    self.current_doctype.system_id = Some(String::new());
                    self.state = TokenizerState::DoctypeSystemIdentifierSingleQuoted;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-doctype");
                    self.current_doctype.force_quirks = true;
                    self.emit_current_doctype();
                    self.emit(Token::Eof);
                }
                Some(_) => {
                    tracing::warn!(
                        "HTML parse error: missing-quote-before-doctype-system-identifier"
                    );
                    self.current_doctype.force_quirks = true;
                    self.state = TokenizerState::BogusDoctype;
                }
            },
            TokenizerState::BetweenDoctypePublicAndSystemIdentifiers => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('>') => {
                        self.state = TokenizerState::Data;
                        self.emit_current_doctype();
                    }
                    Some('"') => {
                        self.current_doctype.system_id = Some(String::new());
                        self.state = TokenizerState::DoctypeSystemIdentifierDoubleQuoted;
                    }
                    Some('\'') => {
                        self.current_doctype.system_id = Some(String::new());
                        self.state = TokenizerState::DoctypeSystemIdentifierSingleQuoted;
                    }
                    None => {
                        tracing::warn!("HTML parse error: eof-in-doctype");
                        self.current_doctype.force_quirks = true;
                        self.emit_current_doctype();
                        self.emit(Token::Eof);
                    }
                    Some(_) => {
                        tracing::warn!(
                            "HTML parse error: missing-quote-before-doctype-system-identifier"
                        );
                        self.current_doctype.force_quirks = true;
                        self.state = TokenizerState::BogusDoctype;
                    }
                }
            }
            TokenizerState::AfterDoctypeSystemKeyword => match self.next_char() {
                Some('\t' | '\n' | '\x0C' | ' ') => {
                    self.state = TokenizerState::BeforeDoctypeSystemIdentifier;
                }
                Some('"') => {
                    tracing::warn!(
                        "HTML parse error: missing-whitespace-after-doctype-system-keyword"
                    );
                    self.current_doctype.system_id = Some(String::new());
                    self.state = TokenizerState::DoctypeSystemIdentifierDoubleQuoted;
                }
                Some('\'') => {
                    tracing::warn!(
                        "HTML parse error: missing-whitespace-after-doctype-system-keyword"
                    );
                    self.current_doctype.system_id = Some(String::new());
                    self.state = TokenizerState::DoctypeSystemIdentifierSingleQuoted;
                }
                Some('>') => {
                    tracing::warn!("HTML parse error: missing-doctype-system-identifier");
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
                Some(_) => {
                    tracing::warn!(
                        "HTML parse error: missing-quote-before-doctype-system-identifier"
                    );
                    self.current_doctype.force_quirks = true;
                    self.state = TokenizerState::BogusDoctype;
                }
            },
            TokenizerState::BeforeDoctypeSystemIdentifier => {
                match self.next_char() {
                    Some('\t' | '\n' | '\x0C' | ' ') => {} // ignore
                    Some('"') => {
                        self.current_doctype.system_id = Some(String::new());
                        self.state = TokenizerState::DoctypeSystemIdentifierDoubleQuoted;
                    }
                    Some('\'') => {
                        self.current_doctype.system_id = Some(String::new());
                        self.state = TokenizerState::DoctypeSystemIdentifierSingleQuoted;
                    }
                    Some('>') => {
                        tracing::warn!("HTML parse error: missing-doctype-system-identifier");
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
                    Some(_) => {
                        tracing::warn!(
                            "HTML parse error: missing-quote-before-doctype-system-identifier"
                        );
                        self.current_doctype.force_quirks = true;
                        self.state = TokenizerState::BogusDoctype;
                    }
                }
            }
            TokenizerState::DoctypeSystemIdentifierDoubleQuoted => match self.next_char() {
                Some('"') => {
                    self.state = TokenizerState::AfterDoctypeSystemIdentifier;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    if let Some(id) = &mut self.current_doctype.system_id {
                        id.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    tracing::warn!("HTML parse error: abrupt-doctype-system-identifier");
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
                    if let Some(id) = &mut self.current_doctype.system_id {
                        id.push(c);
                    }
                }
            },
            TokenizerState::DoctypeSystemIdentifierSingleQuoted => match self.next_char() {
                Some('\'') => {
                    self.state = TokenizerState::AfterDoctypeSystemIdentifier;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    if let Some(id) = &mut self.current_doctype.system_id {
                        id.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    tracing::warn!("HTML parse error: abrupt-doctype-system-identifier");
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
                    if let Some(id) = &mut self.current_doctype.system_id {
                        id.push(c);
                    }
                }
            },
            TokenizerState::AfterDoctypeSystemIdentifier => {
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
                        tracing::warn!(
                            "HTML parse error: unexpected-character-after-doctype-system-identifier"
                        );
                        // Do NOT set force_quirks per spec
                        self.state = TokenizerState::BogusDoctype;
                    }
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
            // RCDATA states
            TokenizerState::RcData => match self.next_char() {
                Some('&') => {
                    self.return_state = TokenizerState::RcData;
                    self.state = TokenizerState::CharacterReference;
                }
                Some('<') => self.state = TokenizerState::RcDataLessThanSign,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                }
                Some(c) => self.emit(Token::Character(c)),
                None => self.emit(Token::Eof),
            },
            TokenizerState::RcDataLessThanSign => match self.next_char() {
                Some('/') => {
                    self.temp_buffer.clear();
                    self.state = TokenizerState::RcDataEndTagOpen;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.reconsume();
                    self.state = TokenizerState::RcData;
                }
            },
            TokenizerState::RcDataEndTagOpen => match self.next_char() {
                Some(c) if c.is_ascii_alphabetic() => {
                    self.current_tag = Some(TagBuilder::new_end());
                    self.reconsume();
                    self.state = TokenizerState::RcDataEndTagName;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    self.reconsume();
                    self.state = TokenizerState::RcData;
                }
            },
            TokenizerState::RcDataEndTagName => match self.next_char() {
                Some(c @ ('\t' | '\n' | '\x0C' | ' ')) if self.is_appropriate_end_tag() => {
                    let _ = c;
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Some('/') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::SelfClosingStartTag;
                }
                Some('>') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    self.push_to_tag_name(c.to_ascii_lowercase());
                    self.temp_buffer.push(c);
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    for c in self.temp_buffer.drain(..).collect::<Vec<_>>() {
                        self.emit(Token::Character(c));
                    }
                    self.reconsume();
                    self.state = TokenizerState::RcData;
                }
            },

            // RAWTEXT states
            TokenizerState::RawText => match self.next_char() {
                Some('<') => self.state = TokenizerState::RawTextLessThanSign,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                }
                Some(c) => self.emit(Token::Character(c)),
                None => self.emit(Token::Eof),
            },
            TokenizerState::RawTextLessThanSign => match self.next_char() {
                Some('/') => {
                    self.temp_buffer.clear();
                    self.state = TokenizerState::RawTextEndTagOpen;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.reconsume();
                    self.state = TokenizerState::RawText;
                }
            },
            TokenizerState::RawTextEndTagOpen => match self.next_char() {
                Some(c) if c.is_ascii_alphabetic() => {
                    self.current_tag = Some(TagBuilder::new_end());
                    self.reconsume();
                    self.state = TokenizerState::RawTextEndTagName;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    self.reconsume();
                    self.state = TokenizerState::RawText;
                }
            },
            TokenizerState::RawTextEndTagName => match self.next_char() {
                Some(c @ ('\t' | '\n' | '\x0C' | ' ')) if self.is_appropriate_end_tag() => {
                    let _ = c;
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Some('/') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::SelfClosingStartTag;
                }
                Some('>') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    self.push_to_tag_name(c.to_ascii_lowercase());
                    self.temp_buffer.push(c);
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    for c in self.temp_buffer.drain(..).collect::<Vec<_>>() {
                        self.emit(Token::Character(c));
                    }
                    self.reconsume();
                    self.state = TokenizerState::RawText;
                }
            },

            // SCRIPT DATA states
            TokenizerState::ScriptData => match self.next_char() {
                Some('<') => self.state = TokenizerState::ScriptDataLessThanSign,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                }
                Some(c) => self.emit(Token::Character(c)),
                None => self.emit(Token::Eof),
            },
            TokenizerState::ScriptDataLessThanSign => match self.next_char() {
                Some('/') => {
                    self.temp_buffer.clear();
                    self.state = TokenizerState::ScriptDataEndTagOpen;
                }
                Some('!') => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('!'));
                    self.state = TokenizerState::ScriptDataEscapeStart;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.reconsume();
                    self.state = TokenizerState::ScriptData;
                }
            },
            TokenizerState::ScriptDataEndTagOpen => match self.next_char() {
                Some(c) if c.is_ascii_alphabetic() => {
                    self.current_tag = Some(TagBuilder::new_end());
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataEndTagName;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    self.reconsume();
                    self.state = TokenizerState::ScriptData;
                }
            },
            TokenizerState::ScriptDataEndTagName => match self.next_char() {
                Some(c @ ('\t' | '\n' | '\x0C' | ' ')) if self.is_appropriate_end_tag() => {
                    let _ = c;
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Some('/') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::SelfClosingStartTag;
                }
                Some('>') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    self.push_to_tag_name(c.to_ascii_lowercase());
                    self.temp_buffer.push(c);
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    for c in self.temp_buffer.drain(..).collect::<Vec<_>>() {
                        self.emit(Token::Character(c));
                    }
                    self.reconsume();
                    self.state = TokenizerState::ScriptData;
                }
            },
            TokenizerState::ScriptDataEscapeStart => match self.next_char() {
                Some('-') => {
                    self.emit(Token::Character('-'));
                    self.state = TokenizerState::ScriptDataEscapeStartDash;
                }
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::ScriptData;
                }
            },
            TokenizerState::ScriptDataEscapeStartDash => match self.next_char() {
                Some('-') => {
                    self.emit(Token::Character('-'));
                    self.state = TokenizerState::ScriptDataEscapedDashDash;
                }
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::ScriptData;
                }
            },
            TokenizerState::ScriptDataEscaped => match self.next_char() {
                Some('-') => {
                    self.emit(Token::Character('-'));
                    self.state = TokenizerState::ScriptDataEscapedDash;
                }
                Some('<') => self.state = TokenizerState::ScriptDataEscapedLessThanSign,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                }
                Some(c) => self.emit(Token::Character(c)),
                None => {
                    tracing::warn!("HTML parse error: eof-in-script-html-comment-like-text");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::ScriptDataEscapedDash => match self.next_char() {
                Some('-') => {
                    self.emit(Token::Character('-'));
                    self.state = TokenizerState::ScriptDataEscapedDashDash;
                }
                Some('<') => self.state = TokenizerState::ScriptDataEscapedLessThanSign,
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                    self.state = TokenizerState::ScriptDataEscaped;
                }
                Some(c) => {
                    self.emit(Token::Character(c));
                    self.state = TokenizerState::ScriptDataEscaped;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-script-html-comment-like-text");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::ScriptDataEscapedDashDash => match self.next_char() {
                Some('-') => self.emit(Token::Character('-')),
                Some('<') => self.state = TokenizerState::ScriptDataEscapedLessThanSign,
                Some('>') => {
                    self.emit(Token::Character('>'));
                    self.state = TokenizerState::ScriptData;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                    self.state = TokenizerState::ScriptDataEscaped;
                }
                Some(c) => {
                    self.emit(Token::Character(c));
                    self.state = TokenizerState::ScriptDataEscaped;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-script-html-comment-like-text");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::ScriptDataEscapedLessThanSign => match self.next_char() {
                Some('/') => {
                    self.temp_buffer.clear();
                    self.state = TokenizerState::ScriptDataEscapedEndTagOpen;
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    self.temp_buffer.clear();
                    self.emit(Token::Character('<'));
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataDoubleEscapeStart;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataEscaped;
                }
            },
            TokenizerState::ScriptDataEscapedEndTagOpen => match self.next_char() {
                Some(c) if c.is_ascii_alphabetic() => {
                    self.current_tag = Some(TagBuilder::new_end());
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataEscapedEndTagName;
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataEscaped;
                }
            },
            TokenizerState::ScriptDataEscapedEndTagName => match self.next_char() {
                Some(c @ ('\t' | '\n' | '\x0C' | ' ')) if self.is_appropriate_end_tag() => {
                    let _ = c;
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Some('/') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::SelfClosingStartTag;
                }
                Some('>') if self.is_appropriate_end_tag() => {
                    self.state = TokenizerState::Data;
                    self.emit_current_tag();
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    self.push_to_tag_name(c.to_ascii_lowercase());
                    self.temp_buffer.push(c);
                }
                _ => {
                    self.emit(Token::Character('<'));
                    self.emit(Token::Character('/'));
                    for c in self.temp_buffer.drain(..).collect::<Vec<_>>() {
                        self.emit(Token::Character(c));
                    }
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataEscaped;
                }
            },
            TokenizerState::ScriptDataDoubleEscapeStart => match self.next_char() {
                Some(c @ ('\t' | '\n' | '\x0C' | ' ' | '/' | '>')) => {
                    if self.temp_buffer.eq_ignore_ascii_case("script") {
                        self.state = TokenizerState::ScriptDataDoubleEscaped;
                    } else {
                        self.state = TokenizerState::ScriptDataEscaped;
                    }
                    self.emit(Token::Character(c));
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    self.temp_buffer.push(c.to_ascii_lowercase());
                    self.emit(Token::Character(c));
                }
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataEscaped;
                }
            },
            TokenizerState::ScriptDataDoubleEscaped => match self.next_char() {
                Some('-') => {
                    self.emit(Token::Character('-'));
                    self.state = TokenizerState::ScriptDataDoubleEscapedDash;
                }
                Some('<') => {
                    self.emit(Token::Character('<'));
                    self.state = TokenizerState::ScriptDataDoubleEscapedLessThanSign;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                }
                Some(c) => self.emit(Token::Character(c)),
                None => {
                    tracing::warn!("HTML parse error: eof-in-script-html-comment-like-text");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::ScriptDataDoubleEscapedDash => match self.next_char() {
                Some('-') => {
                    self.emit(Token::Character('-'));
                    self.state = TokenizerState::ScriptDataDoubleEscapedDashDash;
                }
                Some('<') => {
                    self.emit(Token::Character('<'));
                    self.state = TokenizerState::ScriptDataDoubleEscapedLessThanSign;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                    self.state = TokenizerState::ScriptDataDoubleEscaped;
                }
                Some(c) => {
                    self.emit(Token::Character(c));
                    self.state = TokenizerState::ScriptDataDoubleEscaped;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-script-html-comment-like-text");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::ScriptDataDoubleEscapedDashDash => match self.next_char() {
                Some('-') => self.emit(Token::Character('-')),
                Some('<') => {
                    self.emit(Token::Character('<'));
                    self.state = TokenizerState::ScriptDataDoubleEscapedLessThanSign;
                }
                Some('>') => {
                    self.emit(Token::Character('>'));
                    self.state = TokenizerState::ScriptData;
                }
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                    self.state = TokenizerState::ScriptDataDoubleEscaped;
                }
                Some(c) => {
                    self.emit(Token::Character(c));
                    self.state = TokenizerState::ScriptDataDoubleEscaped;
                }
                None => {
                    tracing::warn!("HTML parse error: eof-in-script-html-comment-like-text");
                    self.emit(Token::Eof);
                }
            },
            TokenizerState::ScriptDataDoubleEscapedLessThanSign => match self.next_char() {
                Some('/') => {
                    self.emit(Token::Character('/'));
                    self.temp_buffer.clear();
                    self.state = TokenizerState::ScriptDataDoubleEscapeEnd;
                }
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataDoubleEscaped;
                }
            },
            TokenizerState::ScriptDataDoubleEscapeEnd => match self.next_char() {
                Some(c @ ('\t' | '\n' | '\x0C' | ' ' | '/' | '>')) => {
                    if self.temp_buffer.eq_ignore_ascii_case("script") {
                        self.state = TokenizerState::ScriptDataEscaped;
                    } else {
                        self.state = TokenizerState::ScriptDataDoubleEscaped;
                    }
                    self.emit(Token::Character(c));
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    self.temp_buffer.push(c.to_ascii_lowercase());
                    self.emit(Token::Character(c));
                }
                _ => {
                    self.reconsume();
                    self.state = TokenizerState::ScriptDataDoubleEscaped;
                }
            },

            // PLAINTEXT state
            TokenizerState::PlainText => match self.next_char() {
                Some('\0') => {
                    tracing::warn!("HTML parse error: unexpected-null-character");
                    self.emit(Token::Character('\u{FFFD}'));
                }
                Some(c) => self.emit(Token::Character(c)),
                None => self.emit(Token::Eof),
            },

            // CHARACTER REFERENCE states
            TokenizerState::CharacterReference => {
                self.temp_buffer.clear();
                self.temp_buffer.push('&');
                match self.peek_char() {
                    Some(c) if c.is_ascii_alphanumeric() => {
                        self.state = TokenizerState::NamedCharacterReference;
                    }
                    Some('#') => {
                        self.temp_buffer.push('#');
                        self.pos += 1;
                        self.state = TokenizerState::NumericCharacterReference;
                    }
                    _ => {
                        self.flush_temp_buffer();
                        self.state = self.return_state;
                    }
                }
            }
            TokenizerState::NamedCharacterReference => {
                let remaining = &self.chars[self.pos..];
                if let Some((len, codepoints)) = entities::longest_match(remaining) {
                    let matched_ends_with_semicolon = remaining.get(len - 1).copied() == Some(';');
                    // Check if consumed as part of an attribute
                    if self.is_return_state_attr() {
                        let next_after = remaining.get(len).copied();
                        if !matched_ends_with_semicolon
                            && next_after.is_some_and(|c| c == '=' || c.is_ascii_alphanumeric())
                        {
                            // Treat as non-entity
                            self.flush_temp_buffer();
                            self.state = self.return_state;
                            return;
                        }
                    }
                    if !matched_ends_with_semicolon {
                        tracing::warn!(
                            "HTML parse error: missing-semicolon-after-character-reference"
                        );
                    }
                    self.temp_buffer.clear();
                    self.pos += len;
                    for &cp in codepoints {
                        if let Some(c) = char::from_u32(cp) {
                            self.emit_char_ref_result(c);
                        }
                    }
                    self.state = self.return_state;
                } else {
                    self.flush_temp_buffer();
                    self.state = TokenizerState::AmbiguousAmpersand;
                }
            }
            TokenizerState::AmbiguousAmpersand => match self.next_char() {
                Some(c) if c.is_ascii_alphanumeric() => {
                    if self.is_return_state_attr() {
                        self.push_to_attr_value(c);
                    } else {
                        self.emit(Token::Character(c));
                    }
                }
                Some(';') => {
                    tracing::warn!("HTML parse error: unknown-named-character-reference");
                    self.reconsume();
                    self.state = self.return_state;
                }
                None => {
                    self.state = self.return_state;
                }
                Some(_) => {
                    self.reconsume();
                    self.state = self.return_state;
                }
            },
            TokenizerState::NumericCharacterReference => {
                self.char_ref_code = 0;
                match self.peek_char() {
                    Some(c @ ('x' | 'X')) => {
                        self.pos += 1;
                        self.temp_buffer.push(c);
                        self.state = TokenizerState::HexadecimalCharacterReferenceStart;
                    }
                    _ => {
                        // Don't consume; DecimalCharacterReferenceStart will read next
                        self.state = TokenizerState::DecimalCharacterReferenceStart;
                    }
                }
            }
            TokenizerState::HexadecimalCharacterReferenceStart => match self.peek_char() {
                Some(c) if c.is_ascii_hexdigit() => {
                    self.state = TokenizerState::HexadecimalCharacterReference;
                }
                _ => {
                    tracing::warn!(
                        "HTML parse error: absence-of-digits-in-numeric-character-reference"
                    );
                    self.flush_temp_buffer();
                    self.state = self.return_state;
                }
            },
            TokenizerState::DecimalCharacterReferenceStart => match self.peek_char() {
                Some(c) if c.is_ascii_digit() => {
                    self.state = TokenizerState::DecimalCharacterReference;
                }
                _ => {
                    tracing::warn!(
                        "HTML parse error: absence-of-digits-in-numeric-character-reference"
                    );
                    self.flush_temp_buffer();
                    self.state = self.return_state;
                }
            },
            TokenizerState::HexadecimalCharacterReference => match self.next_char() {
                Some(c) if c.is_ascii_hexdigit() => {
                    let digit = c.to_digit(16).unwrap();
                    self.char_ref_code =
                        self.char_ref_code.saturating_mul(16).saturating_add(digit);
                    if self.char_ref_code > 0x10FFFF {
                        self.char_ref_code = 0x10FFFF + 1;
                    }
                }
                Some(';') => {
                    self.state = TokenizerState::NumericCharacterReferenceEnd;
                }
                _ => {
                    tracing::warn!("HTML parse error: missing-semicolon-after-character-reference");
                    self.reconsume();
                    self.state = TokenizerState::NumericCharacterReferenceEnd;
                }
            },
            TokenizerState::DecimalCharacterReference => match self.next_char() {
                Some(c) if c.is_ascii_digit() => {
                    let digit = c.to_digit(10).unwrap();
                    self.char_ref_code =
                        self.char_ref_code.saturating_mul(10).saturating_add(digit);
                    if self.char_ref_code > 0x10FFFF {
                        self.char_ref_code = 0x10FFFF + 1;
                    }
                }
                Some(';') => {
                    self.state = TokenizerState::NumericCharacterReferenceEnd;
                }
                _ => {
                    tracing::warn!("HTML parse error: missing-semicolon-after-character-reference");
                    self.reconsume();
                    self.state = TokenizerState::NumericCharacterReferenceEnd;
                }
            },
            TokenizerState::NumericCharacterReferenceEnd => {
                let c = if self.char_ref_code == 0 {
                    tracing::warn!("HTML parse error: null-character-reference");
                    '\u{FFFD}'
                } else if self.char_ref_code > 0x10FFFF {
                    tracing::warn!("HTML parse error: character-reference-outside-unicode-range");
                    '\u{FFFD}'
                } else if (0xD800..=0xDFFF).contains(&self.char_ref_code) {
                    tracing::warn!("HTML parse error: surrogate-character-reference");
                    '\u{FFFD}'
                } else if let Some(replacement) =
                    entities::windows_1252_replacement(self.char_ref_code)
                {
                    tracing::warn!("HTML parse error: character-reference-outside-unicode-range");
                    replacement
                } else {
                    // Check for noncharacter
                    let cp = self.char_ref_code;
                    if (0xFDD0..=0xFDEF).contains(&cp)
                        || matches!(
                            cp,
                            0xFFFE
                                | 0xFFFF
                                | 0x1FFFE
                                | 0x1FFFF
                                | 0x2FFFE
                                | 0x2FFFF
                                | 0x3FFFE
                                | 0x3FFFF
                                | 0x4FFFE
                                | 0x4FFFF
                                | 0x5FFFE
                                | 0x5FFFF
                                | 0x6FFFE
                                | 0x6FFFF
                                | 0x7FFFE
                                | 0x7FFFF
                                | 0x8FFFE
                                | 0x8FFFF
                                | 0x9FFFE
                                | 0x9FFFF
                                | 0xAFFFE
                                | 0xAFFFF
                                | 0xBFFFE
                                | 0xBFFFF
                                | 0xCFFFE
                                | 0xCFFFF
                                | 0xDFFFE
                                | 0xDFFFF
                                | 0xEFFFE
                                | 0xEFFFF
                                | 0xFFFFE
                                | 0xFFFFF
                                | 0x10FFFE
                                | 0x10FFFF
                        )
                    {
                        tracing::warn!("HTML parse error: noncharacter-character-reference");
                    }
                    // Control character check (0x01-0x1F except 0x09,0x0A,0x0C; 0x7F-0x9F)
                    if (cp <= 0x1F && !matches!(cp, 0x09 | 0x0A | 0x0C))
                        || (0x7F..=0x9F).contains(&cp)
                    {
                        tracing::warn!("HTML parse error: control-character-reference");
                    }
                    char::from_u32(cp).unwrap_or('\u{FFFD}')
                };
                self.emit_char_ref_result(c);
                self.state = self.return_state;
            }
        }
    }

    fn is_appropriate_end_tag(&self) -> bool {
        if let Some(tag) = &self.current_tag
            && tag.is_end_tag
            && let Some(ref last) = self.last_start_tag_name
        {
            return tag.name == *last;
        }
        false
    }

    fn is_return_state_attr(&self) -> bool {
        matches!(
            self.return_state,
            TokenizerState::AttributeValueDoubleQuoted
                | TokenizerState::AttributeValueSingleQuoted
                | TokenizerState::AttributeValueUnquoted
        )
    }

    fn flush_temp_buffer(&mut self) {
        let buf: Vec<char> = self.temp_buffer.drain(..).collect();
        if self.is_return_state_attr() {
            for c in buf {
                self.push_to_attr_value(c);
            }
        } else {
            for c in buf {
                self.emit(Token::Character(c));
            }
        }
    }

    fn emit_char_ref_result(&mut self, c: char) {
        if self.is_return_state_attr() {
            self.push_to_attr_value(c);
        } else {
            self.emit(Token::Character(c));
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

        let mut steps = 0;
        loop {
            if let Some(token) = self.pending_tokens.pop_front() {
                if token == Token::Eof {
                    self.finished = true;
                }
                return Some(token);
            }
            self.step();
            steps += 1;
            if steps > 10_000 {
                tracing::error!("tokenizer infinite loop detected, emitting EOF");
                self.finished = true;
                return Some(Token::Eof);
            }
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

    // --- Step 1b: Character reference tests ---

    #[test]
    fn named_entity_amp() {
        let tokens = tokenize("&amp;");
        assert_eq!(tokens, vec![Token::Character('&'), Token::Eof]);
    }

    #[test]
    fn named_entity_lt() {
        let tokens = tokenize("&lt;");
        assert_eq!(tokens, vec![Token::Character('<'), Token::Eof]);
    }

    #[test]
    fn numeric_entity_decimal() {
        let tokens = tokenize("&#60;");
        assert_eq!(tokens, vec![Token::Character('<'), Token::Eof]);
    }

    #[test]
    fn numeric_entity_hex() {
        let tokens = tokenize("&#x3C;");
        assert_eq!(tokens, vec![Token::Character('<'), Token::Eof]);
    }

    #[test]
    fn numeric_entity_hex_uppercase() {
        let tokens = tokenize("&#X3c;");
        assert_eq!(tokens, vec![Token::Character('<'), Token::Eof]);
    }

    #[test]
    fn entity_in_attribute() {
        let tokens = tokenize(r#"<a href="?a=1&amp;b=2">"#);
        assert_eq!(
            tokens,
            vec![
                Token::StartTag {
                    name: "a".to_string(),
                    attributes: vec![Attribute {
                        name: "href".to_string(),
                        value: "?a=1&b=2".to_string(),
                    }],
                    self_closing: false,
                },
                Token::Eof,
            ]
        );
    }

    #[test]
    fn entity_in_text() {
        let tokens = tokenize("a&lt;b");
        assert_eq!(
            tokens,
            vec![
                Token::Character('a'),
                Token::Character('<'),
                Token::Character('b'),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn bare_ampersand() {
        let tokens = tokenize("a&b");
        assert_eq!(
            tokens,
            vec![
                Token::Character('a'),
                Token::Character('&'),
                Token::Character('b'),
                Token::Eof,
            ]
        );
    }

    // --- Step 1b: RCDATA tests ---

    #[test]
    fn rcdata_title() {
        let mut tok = Tokenizer::new("<title>&amp; stuff</title>");
        // Consume the <title> start tag
        let start = tok.next().unwrap();
        assert!(start.is_start_tag("title"));
        // Switch to RCDATA as tree builder would
        tok.set_state(TokenizerState::RcData);
        let remaining: Vec<Token> = tok.collect();
        // Should resolve &amp; to & in RCDATA
        let text: String = remaining
            .iter()
            .filter_map(|t| match t {
                Token::Character(c) => Some(*c),
                _ => None,
            })
            .collect();
        assert_eq!(text, "& stuff");
        assert!(remaining.iter().any(|t| t.is_end_tag("title")));
    }

    // --- Step 1b: RawText tests ---

    #[test]
    fn rawtext_style() {
        let mut tok = Tokenizer::new("<style>.a { }</style>");
        let start = tok.next().unwrap();
        assert!(start.is_start_tag("style"));
        tok.set_state(TokenizerState::RawText);
        let remaining: Vec<Token> = tok.collect();
        let text: String = remaining
            .iter()
            .filter_map(|t| match t {
                Token::Character(c) => Some(*c),
                _ => None,
            })
            .collect();
        assert_eq!(text, ".a { }");
        assert!(remaining.iter().any(|t| t.is_end_tag("style")));
    }

    // --- Step 1b: ScriptData tests ---

    #[test]
    fn script_data_basic() {
        let mut tok = Tokenizer::new("<script>var x = 1 < 2;</script>");
        let start = tok.next().unwrap();
        assert!(start.is_start_tag("script"));
        tok.set_state(TokenizerState::ScriptData);
        let remaining: Vec<Token> = tok.collect();
        let text: String = remaining
            .iter()
            .filter_map(|t| match t {
                Token::Character(c) => Some(*c),
                _ => None,
            })
            .collect();
        assert_eq!(text, "var x = 1 < 2;");
        assert!(remaining.iter().any(|t| t.is_end_tag("script")));
    }

    // --- Step 1b: State switching test ---

    #[test]
    fn set_state_switches() {
        let mut tok = Tokenizer::new("content</textarea>");
        tok.set_state(TokenizerState::RcData);
        // In RCDATA, should accumulate text until </textarea> end tag
        // (last_start_tag_name would need to be set for appropriate end tag check)
        // Without last_start_tag_name set, end tag won't be recognized
        let tokens: Vec<Token> = tok.collect();
        // Should get characters since no appropriate end tag match
        assert!(tokens.iter().any(|t| matches!(t, Token::Character(_))));
    }

    #[test]
    fn numeric_entity_null_replacement() {
        let tokens = tokenize("&#0;");
        assert_eq!(tokens, vec![Token::Character('\u{FFFD}'), Token::Eof]);
    }

    #[test]
    fn multi_codepoint_entity() {
        // &nGt; → U+226B U+20D2
        let tokens = tokenize("&nGt;");
        let chars: Vec<char> = tokens
            .iter()
            .filter_map(|t| match t {
                Token::Character(c) => Some(*c),
                _ => None,
            })
            .collect();
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0], '\u{226B}');
        assert_eq!(chars[1], '\u{20D2}');
    }
}
