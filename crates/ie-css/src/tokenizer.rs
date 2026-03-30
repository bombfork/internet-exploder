/// CSS Syntax Module Level 3 tokenizer.
/// <https://www.w3.org/TR/css-syntax-3/#tokenization>

#[derive(Debug, Clone, PartialEq)]
pub enum CssToken {
    Ident(String),
    Function(String),
    AtKeyword(String),
    Hash(String, HashType),
    String(String),
    BadString,
    Url(String),
    BadUrl,
    Delim(char),
    Number(f64, NumType),
    Percentage(f64),
    Dimension(f64, String),
    Whitespace,
    /// `<!--`
    Cdo,
    /// `-->`
    Cdc,
    Colon,
    Semicolon,
    Comma,
    SquareBracketOpen,
    SquareBracketClose,
    ParenOpen,
    ParenClose,
    CurlyBracketOpen,
    CurlyBracketClose,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HashType {
    Id,
    Unrestricted,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumType {
    Integer,
    Number,
}

pub struct CssTokenizer<'a> {
    _input: &'a str,
    chars: Vec<char>,
    pos: usize,
    finished: bool,
}

impl<'a> CssTokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        // Preprocess: replace \r\n, \r, \f with \n (§3.3)
        let preprocessed: Vec<char> = {
            let raw: Vec<char> = input.chars().collect();
            let mut out = Vec::with_capacity(raw.len());
            let mut i = 0;
            while i < raw.len() {
                match raw[i] {
                    '\r' => {
                        out.push('\n');
                        if i + 1 < raw.len() && raw[i + 1] == '\n' {
                            i += 1;
                        }
                    }
                    '\x0C' => out.push('\n'),
                    '\0' => out.push('\u{FFFD}'),
                    c => out.push(c),
                }
                i += 1;
            }
            out
        };

        Self {
            _input: input,
            chars: preprocessed,
            pos: 0,
            finished: false,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn reconsume(&mut self) {
        if self.pos > 0 {
            self.pos -= 1;
        }
    }

    // §4.3.1 Consume a token
    fn consume_token(&mut self) -> CssToken {
        self.consume_comments();

        match self.advance() {
            None => CssToken::Eof,
            Some(c) => match c {
                c if is_whitespace(c) => {
                    self.consume_whitespace();
                    CssToken::Whitespace
                }
                '"' => self.consume_string_token('"'),
                '#' => {
                    let next = self.peek();
                    let next2 = self.peek_at(1);
                    if next.is_some_and(is_name_char)
                        || valid_escape(next.unwrap_or('\0'), next2.unwrap_or('\0'))
                    {
                        let hash_type =
                            if starts_ident(self.peek(), self.peek_at(1), self.peek_at(2)) {
                                HashType::Id
                            } else {
                                HashType::Unrestricted
                            };
                        let name = self.consume_name();
                        CssToken::Hash(name, hash_type)
                    } else {
                        CssToken::Delim('#')
                    }
                }
                '\'' => self.consume_string_token('\''),
                '(' => CssToken::ParenOpen,
                ')' => CssToken::ParenClose,
                '+' => {
                    if starts_number(Some('+'), self.peek(), self.peek_at(1)) {
                        self.reconsume();
                        self.consume_numeric_token()
                    } else {
                        CssToken::Delim('+')
                    }
                }
                ',' => CssToken::Comma,
                '-' => {
                    if starts_number(Some('-'), self.peek(), self.peek_at(1)) {
                        self.reconsume();
                        self.consume_numeric_token()
                    } else if self.peek() == Some('-') && self.peek_at(1) == Some('>') {
                        self.advance();
                        self.advance();
                        CssToken::Cdc
                    } else if starts_ident(Some('-'), self.peek(), self.peek_at(1)) {
                        self.reconsume();
                        self.consume_ident_like_token()
                    } else {
                        CssToken::Delim('-')
                    }
                }
                '.' => {
                    if starts_number(Some('.'), self.peek(), self.peek_at(1)) {
                        self.reconsume();
                        self.consume_numeric_token()
                    } else {
                        CssToken::Delim('.')
                    }
                }
                ':' => CssToken::Colon,
                ';' => CssToken::Semicolon,
                '<' => {
                    if self.peek() == Some('!')
                        && self.peek_at(1) == Some('-')
                        && self.peek_at(2) == Some('-')
                    {
                        self.advance();
                        self.advance();
                        self.advance();
                        CssToken::Cdo
                    } else {
                        CssToken::Delim('<')
                    }
                }
                '@' => {
                    if starts_ident(self.peek(), self.peek_at(1), self.peek_at(2)) {
                        let name = self.consume_name();
                        CssToken::AtKeyword(name)
                    } else {
                        CssToken::Delim('@')
                    }
                }
                '[' => CssToken::SquareBracketOpen,
                '\\' => {
                    if valid_escape('\\', self.peek().unwrap_or('\0')) {
                        self.reconsume();
                        self.consume_ident_like_token()
                    } else {
                        // Parse error
                        CssToken::Delim('\\')
                    }
                }
                ']' => CssToken::SquareBracketClose,
                '{' => CssToken::CurlyBracketOpen,
                '}' => CssToken::CurlyBracketClose,
                c if c.is_ascii_digit() => {
                    self.reconsume();
                    self.consume_numeric_token()
                }
                c if is_name_start_char(c) => {
                    self.reconsume();
                    self.consume_ident_like_token()
                }
                c => CssToken::Delim(c),
            },
        }
    }

    // §4.3.2 Consume comments
    fn consume_comments(&mut self) {
        loop {
            if self.peek() == Some('/') && self.peek_at(1) == Some('*') {
                self.advance();
                self.advance();
                loop {
                    match self.advance() {
                        Some('*') if self.peek() == Some('/') => {
                            self.advance();
                            break;
                        }
                        None => break, // EOF in comment
                        _ => {}
                    }
                }
            } else {
                break;
            }
        }
    }

    fn consume_whitespace(&mut self) {
        while self.peek().is_some_and(is_whitespace) {
            self.advance();
        }
    }

    // §4.3.4 Consume a string token
    fn consume_string_token(&mut self, ending: char) -> CssToken {
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return CssToken::String(s), // EOF → parse error, return what we have
                Some(c) if c == ending => return CssToken::String(s),
                Some('\n') => {
                    // Unescaped newline → BadString, reconsume the newline
                    self.reconsume();
                    return CssToken::BadString;
                }
                Some('\\') => match self.peek() {
                    None => {} // EOF after backslash, do nothing (spec says it's removed)
                    Some('\n') => {
                        self.advance(); // escaped newline, consume and continue
                    }
                    _ => {
                        let ch = self.consume_escape();
                        s.push(ch);
                    }
                },
                Some(c) => s.push(c),
            }
        }
    }

    // §4.3.3 Consume a numeric token
    fn consume_numeric_token(&mut self) -> CssToken {
        let (value, num_type) = self.consume_number();

        if starts_ident(self.peek(), self.peek_at(1), self.peek_at(2)) {
            let unit = self.consume_name();
            CssToken::Dimension(value, unit)
        } else if self.peek() == Some('%') {
            self.advance();
            CssToken::Percentage(value)
        } else {
            CssToken::Number(value, num_type)
        }
    }

    // §4.3.4 Consume an ident-like token
    fn consume_ident_like_token(&mut self) -> CssToken {
        let name = self.consume_name();

        if name.eq_ignore_ascii_case("url") && self.peek() == Some('(') {
            self.advance(); // consume '('

            // Skip whitespace and check what follows
            let saved = self.pos;
            self.consume_whitespace();

            match self.peek() {
                Some('"') | Some('\'') => {
                    // It's a function call like url("..."), not a url token
                    self.pos = saved; // restore to after '('
                    self.consume_whitespace();
                    CssToken::Function(name)
                }
                _ => {
                    self.pos = saved; // restore
                    self.consume_whitespace();
                    self.consume_url_token()
                }
            }
        } else if self.peek() == Some('(') {
            self.advance();
            CssToken::Function(name)
        } else {
            CssToken::Ident(name)
        }
    }

    // §4.3.6 Consume a URL token
    fn consume_url_token(&mut self) -> CssToken {
        let mut url = String::new();

        loop {
            match self.advance() {
                Some(')') => return CssToken::Url(url),
                None => return CssToken::Url(url), // EOF, parse error
                Some(c) if is_whitespace(c) => {
                    self.consume_whitespace();
                    match self.peek() {
                        Some(')') => {
                            self.advance();
                            return CssToken::Url(url);
                        }
                        None => return CssToken::Url(url), // EOF, parse error
                        _ => {
                            self.consume_bad_url_remnants();
                            return CssToken::BadUrl;
                        }
                    }
                }
                Some('"') | Some('\'') | Some('(') => {
                    // Parse error
                    self.consume_bad_url_remnants();
                    return CssToken::BadUrl;
                }
                Some('\\') => {
                    if valid_escape('\\', self.peek().unwrap_or('\0')) {
                        let ch = self.consume_escape();
                        url.push(ch);
                    } else {
                        // Parse error
                        self.consume_bad_url_remnants();
                        return CssToken::BadUrl;
                    }
                }
                Some(c) if is_non_printable(c) => {
                    self.consume_bad_url_remnants();
                    return CssToken::BadUrl;
                }
                Some(c) => url.push(c),
            }
        }
    }

    fn consume_bad_url_remnants(&mut self) {
        loop {
            match self.advance() {
                Some(')') | None => return,
                Some('\\') if valid_escape('\\', self.peek().unwrap_or('\0')) => {
                    self.consume_escape();
                }
                _ => {}
            }
        }
    }

    // §4.3.11 Consume a name
    fn consume_name(&mut self) -> String {
        let mut result = String::new();
        loop {
            match self.peek() {
                Some(c) if is_name_char(c) => {
                    self.advance();
                    result.push(c);
                }
                Some('\\') if valid_escape('\\', self.peek_at(1).unwrap_or('\0')) => {
                    self.advance(); // consume '\'
                    let ch = self.consume_escape();
                    result.push(ch);
                }
                _ => return result,
            }
        }
    }

    // §4.3.12 Consume a number
    fn consume_number(&mut self) -> (f64, NumType) {
        let mut repr = String::new();
        let mut num_type = NumType::Integer;

        // Optional sign
        if matches!(self.peek(), Some('+') | Some('-')) {
            repr.push(self.advance().unwrap());
        }

        // Digits
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            repr.push(self.advance().unwrap());
        }

        // Decimal part
        if self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            repr.push(self.advance().unwrap()); // '.'
            num_type = NumType::Number;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                repr.push(self.advance().unwrap());
            }
        }

        // Exponent part
        if matches!(self.peek(), Some('e') | Some('E')) {
            let next = self.peek_at(1);
            if next.is_some_and(|c| c.is_ascii_digit()) {
                repr.push(self.advance().unwrap()); // 'e'/'E'
                num_type = NumType::Number;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    repr.push(self.advance().unwrap());
                }
            } else if matches!(next, Some('+') | Some('-'))
                && self.peek_at(2).is_some_and(|c| c.is_ascii_digit())
            {
                repr.push(self.advance().unwrap()); // 'e'/'E'
                repr.push(self.advance().unwrap()); // sign
                num_type = NumType::Number;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    repr.push(self.advance().unwrap());
                }
            }
        }

        let value = repr.parse::<f64>().unwrap_or(0.0);
        (value, num_type)
    }

    // §4.3.7 Consume an escaped code point
    fn consume_escape(&mut self) -> char {
        match self.advance() {
            None => '\u{FFFD}', // EOF
            Some(c) if c.is_ascii_hexdigit() => {
                let mut hex = String::new();
                hex.push(c);
                for _ in 0..5 {
                    if self.peek().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                        hex.push(self.advance().unwrap());
                    } else {
                        break;
                    }
                }
                // Consume optional trailing whitespace
                if self.peek().is_some_and(is_whitespace) {
                    self.advance();
                }
                let cp = u32::from_str_radix(&hex, 16).unwrap_or(0);
                if cp == 0 || cp > 0x10FFFF || (0xD800..=0xDFFF).contains(&cp) {
                    '\u{FFFD}'
                } else {
                    char::from_u32(cp).unwrap_or('\u{FFFD}')
                }
            }
            Some(c) => c,
        }
    }
}

// Helpers

fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n')
}

fn is_name_start_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}

fn is_name_char(c: char) -> bool {
    is_name_start_char(c) || c.is_ascii_digit() || c == '-'
}

fn is_non_printable(c: char) -> bool {
    matches!(c, '\x00'..='\x08' | '\x0B' | '\x0E'..='\x1F' | '\x7F')
}

fn valid_escape(c1: char, c2: char) -> bool {
    c1 == '\\' && c2 != '\n'
}

fn starts_ident(c1: Option<char>, c2: Option<char>, c3: Option<char>) -> bool {
    match c1 {
        Some('-') => match c2 {
            Some(c) if is_name_start_char(c) => true,
            Some('-') => true,
            Some('\\') => valid_escape('\\', c3.unwrap_or('\n')),
            _ => false,
        },
        Some('\\') => valid_escape('\\', c2.unwrap_or('\n')),
        Some(c) if is_name_start_char(c) => true,
        _ => false,
    }
}

fn starts_number(c1: Option<char>, c2: Option<char>, c3: Option<char>) -> bool {
    match c1 {
        Some('+') | Some('-') => match c2 {
            Some(c) if c.is_ascii_digit() => true,
            Some('.') => c3.is_some_and(|c| c.is_ascii_digit()),
            _ => false,
        },
        Some('.') => c2.is_some_and(|c| c.is_ascii_digit()),
        Some(c) if c.is_ascii_digit() => true,
        _ => false,
    }
}

impl<'a> Iterator for CssTokenizer<'a> {
    type Item = CssToken;

    fn next(&mut self) -> Option<CssToken> {
        if self.finished {
            return None;
        }
        let token = self.consume_token();
        if token == CssToken::Eof {
            self.finished = true;
        }
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &str) -> Vec<CssToken> {
        CssTokenizer::new(input)
            .filter(|t| *t != CssToken::Eof)
            .collect()
    }

    #[test]
    fn simple_rule_tokens() {
        let tokens = tokenize("div { color: red; }");
        assert_eq!(
            tokens,
            vec![
                CssToken::Ident("div".into()),
                CssToken::Whitespace,
                CssToken::CurlyBracketOpen,
                CssToken::Whitespace,
                CssToken::Ident("color".into()),
                CssToken::Colon,
                CssToken::Whitespace,
                CssToken::Ident("red".into()),
                CssToken::Semicolon,
                CssToken::Whitespace,
                CssToken::CurlyBracketClose,
            ]
        );
    }

    #[test]
    fn string_token() {
        let tokens = tokenize(r#""hello world""#);
        assert_eq!(tokens, vec![CssToken::String("hello world".into())]);
    }

    #[test]
    fn string_with_escape() {
        let tokens = tokenize(r#""hello\"world""#);
        assert_eq!(tokens, vec![CssToken::String(r#"hello"world"#.into())]);
    }

    #[test]
    fn url_token() {
        let tokens = tokenize("url(https://example.com)");
        assert_eq!(tokens, vec![CssToken::Url("https://example.com".into())]);
    }

    #[test]
    fn dimension_token() {
        let tokens = tokenize("16px");
        assert_eq!(tokens, vec![CssToken::Dimension(16.0, "px".into())]);
    }

    #[test]
    fn percentage_token() {
        let tokens = tokenize("50%");
        assert_eq!(tokens, vec![CssToken::Percentage(50.0)]);
    }

    #[test]
    fn function_token() {
        let tokens = tokenize("rgb(");
        assert_eq!(tokens, vec![CssToken::Function("rgb".into())]);
    }

    #[test]
    fn hash_token() {
        let tokens = tokenize("#main");
        assert_eq!(tokens, vec![CssToken::Hash("main".into(), HashType::Id)]);
    }

    #[test]
    fn at_keyword() {
        let tokens = tokenize("@media");
        assert_eq!(tokens, vec![CssToken::AtKeyword("media".into())]);
    }

    #[test]
    fn number_tokens() {
        let tokens = tokenize("42");
        assert_eq!(tokens, vec![CssToken::Number(42.0, NumType::Integer)]);

        let tokens = tokenize("3.14");
        assert_eq!(tokens, vec![CssToken::Number(3.14, NumType::Number)]);
    }

    #[test]
    fn comments_stripped() {
        let tokens = tokenize("/* comment */div");
        assert_eq!(tokens, vec![CssToken::Ident("div".into())]);

        let tokens = tokenize("/* comment */ div");
        assert_eq!(
            tokens,
            vec![CssToken::Whitespace, CssToken::Ident("div".into())]
        );
    }

    #[test]
    fn cdo_cdc() {
        let tokens = tokenize("<!-- -->");
        assert_eq!(
            tokens,
            vec![CssToken::Cdo, CssToken::Whitespace, CssToken::Cdc]
        );
    }

    #[test]
    fn negative_number() {
        let tokens = tokenize("-5px");
        assert_eq!(tokens, vec![CssToken::Dimension(-5.0, "px".into())]);
    }

    #[test]
    fn single_char_tokens() {
        let tokens = tokenize("()[]{},:;");
        assert_eq!(
            tokens,
            vec![
                CssToken::ParenOpen,
                CssToken::ParenClose,
                CssToken::SquareBracketOpen,
                CssToken::SquareBracketClose,
                CssToken::CurlyBracketOpen,
                CssToken::CurlyBracketClose,
                CssToken::Comma,
                CssToken::Colon,
                CssToken::Semicolon,
            ]
        );
    }

    #[test]
    fn scientific_notation() {
        let tokens = tokenize("1e2");
        assert_eq!(tokens, vec![CssToken::Number(100.0, NumType::Number)]);

        let tokens = tokenize("1.5e+3");
        assert_eq!(tokens, vec![CssToken::Number(1500.0, NumType::Number)]);
    }

    #[test]
    fn bad_string() {
        // Unescaped newline in string
        let tokens = tokenize("\"hello\nworld\"");
        assert_eq!(tokens[0], CssToken::BadString,);
    }

    #[test]
    fn url_with_quotes_is_function() {
        let tokens = tokenize("url(\"foo.png\")");
        assert_eq!(tokens[0], CssToken::Function("url".into()));
    }

    #[test]
    fn hex_escape_in_string() {
        // \41 is 'A'
        let tokens = tokenize(r#""\41 bc""#);
        assert_eq!(tokens, vec![CssToken::String("Abc".into())]);
    }

    #[test]
    fn hash_unrestricted() {
        // #123 starts with a digit, not an ident start
        let tokens = tokenize("#123");
        assert_eq!(
            tokens,
            vec![CssToken::Hash("123".into(), HashType::Unrestricted)]
        );
    }

    #[test]
    fn unicode_ident() {
        let tokens = tokenize("café");
        assert_eq!(tokens, vec![CssToken::Ident("café".into())]);
    }

    #[test]
    fn empty_input() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn delim_tokens() {
        let tokens = tokenize("* > ~ |");
        assert_eq!(
            tokens,
            vec![
                CssToken::Delim('*'),
                CssToken::Whitespace,
                CssToken::Delim('>'),
                CssToken::Whitespace,
                CssToken::Delim('~'),
                CssToken::Whitespace,
                CssToken::Delim('|'),
            ]
        );
    }

    #[test]
    fn multiple_comments() {
        let tokens = tokenize("/* a *//* b */div");
        assert_eq!(tokens, vec![CssToken::Ident("div".into())]);
    }

    #[test]
    fn escaped_newline_in_string() {
        let tokens = tokenize("\"hello\\\nworld\"");
        assert_eq!(tokens, vec![CssToken::String("helloworld".into())]);
    }

    #[test]
    fn custom_property() {
        let tokens = tokenize("--my-var");
        assert_eq!(tokens, vec![CssToken::Ident("--my-var".into())]);
    }

    #[test]
    fn real_world_css() {
        let input = ".container { margin: 0 auto; width: 100%; }";
        let tokens = tokenize(input);
        assert_eq!(tokens[0], CssToken::Delim('.'));
        assert_eq!(tokens[1], CssToken::Ident("container".into()));
        // Just check it doesn't panic and produces reasonable output
        assert!(tokens.len() > 10);
    }
}
