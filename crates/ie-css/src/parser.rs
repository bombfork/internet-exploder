use crate::tokenizer::{CssToken, CssTokenizer};
use crate::values::{
    CssColor, CssValue, LengthUnit, PropertyId, parse_hex_color, parse_named_color,
    parse_property_id,
};

#[derive(Debug)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug)]
pub struct Rule {
    /// Raw selector strings — parsed selector objects deferred to Step 3c
    pub selectors: Vec<String>,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug)]
pub struct Declaration {
    pub property: PropertyId,
    pub value: CssValue,
    pub important: bool,
}

pub fn parse_stylesheet(css: &str) -> Stylesheet {
    let mut parser = CssParser::new(css);
    parser.parse_stylesheet()
}

pub fn parse_declarations(css: &str) -> Vec<Declaration> {
    let mut parser = CssParser::new(css);
    parser.parse_declaration_list()
}

struct CssParser {
    tokens: Vec<CssToken>,
    pos: usize,
}

impl CssParser {
    fn new(css: &str) -> Self {
        let tokens: Vec<CssToken> = CssTokenizer::new(css).collect();
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &CssToken {
        self.tokens.get(self.pos).unwrap_or(&CssToken::Eof)
    }

    fn advance(&mut self) -> &CssToken {
        let tok = self.tokens.get(self.pos).unwrap_or(&CssToken::Eof);
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), CssToken::Whitespace) {
            self.advance();
        }
    }

    fn parse_stylesheet(&mut self) -> Stylesheet {
        let mut rules = Vec::new();

        loop {
            self.skip_whitespace();
            match self.peek() {
                CssToken::Eof => break,
                CssToken::Cdo | CssToken::Cdc => {
                    self.advance();
                }
                CssToken::AtKeyword(_) => {
                    self.consume_at_rule();
                }
                _ => {
                    if let Some(rule) = self.consume_qualified_rule() {
                        rules.push(rule);
                    }
                }
            }
        }

        Stylesheet { rules }
    }

    /// Skip an at-rule (consume until semicolon or matching braces)
    fn consume_at_rule(&mut self) {
        self.advance(); // consume AtKeyword
        let mut brace_depth = 0;
        loop {
            match self.advance() {
                CssToken::Semicolon if brace_depth == 0 => return,
                CssToken::CurlyBracketOpen => brace_depth += 1,
                CssToken::CurlyBracketClose => {
                    if brace_depth <= 1 {
                        return;
                    }
                    brace_depth -= 1;
                }
                CssToken::Eof => return,
                _ => {}
            }
        }
    }

    /// Consume a qualified rule: selectors + declaration block
    fn consume_qualified_rule(&mut self) -> Option<Rule> {
        // Collect tokens as raw selector text until `{`
        let mut selector_parts: Vec<String> = Vec::new();
        let mut current = String::new();

        loop {
            match self.peek() {
                CssToken::CurlyBracketOpen | CssToken::Eof => break,
                CssToken::Comma => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        selector_parts.push(trimmed);
                    }
                    current.clear();
                    self.advance();
                }
                _ => {
                    current.push_str(&self.token_to_string(self.pos));
                    self.advance();
                }
            }
        }

        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            selector_parts.push(trimmed);
        }

        if matches!(self.peek(), CssToken::Eof) {
            return None;
        }

        // Consume '{'
        self.advance();

        let declarations = self.parse_declaration_list();

        // Consume '}' if present
        if matches!(self.peek(), CssToken::CurlyBracketClose) {
            self.advance();
        }

        if selector_parts.is_empty() {
            return None;
        }

        Some(Rule {
            selectors: selector_parts,
            declarations,
        })
    }

    /// Reconstruct approximate text from a token at a given position
    fn token_to_string(&self, pos: usize) -> String {
        match self.tokens.get(pos).unwrap_or(&CssToken::Eof) {
            CssToken::Ident(s) => s.clone(),
            CssToken::Hash(s, _) => format!("#{s}"),
            CssToken::Delim(c) => c.to_string(),
            CssToken::Whitespace => " ".into(),
            CssToken::Colon => ":".into(),
            CssToken::Number(n, _) => format!("{n}"),
            CssToken::SquareBracketOpen => "[".into(),
            CssToken::SquareBracketClose => "]".into(),
            CssToken::ParenOpen => "(".into(),
            CssToken::ParenClose => ")".into(),
            CssToken::String(s) => format!("\"{s}\""),
            CssToken::Function(s) => format!("{s}("),
            CssToken::Dimension(n, u) => format!("{n}{u}"),
            CssToken::Percentage(n) => format!("{n}%"),
            CssToken::Comma => ",".into(),
            _ => String::new(),
        }
    }

    /// Parse declarations between { and }
    fn parse_declaration_list(&mut self) -> Vec<Declaration> {
        let mut declarations = Vec::new();

        loop {
            self.skip_whitespace();
            match self.peek() {
                CssToken::CurlyBracketClose | CssToken::Eof => break,
                CssToken::Semicolon => {
                    self.advance();
                }
                CssToken::Ident(_) => {
                    self.consume_declaration(&mut declarations);
                }
                _ => {
                    // Error recovery: skip token
                    self.advance();
                }
            }
        }

        declarations
    }

    /// Consume a single declaration and expand shorthands
    fn consume_declaration(&mut self, declarations: &mut Vec<Declaration>) {
        // Read property name
        let property_name = match self.advance().clone() {
            CssToken::Ident(name) => name,
            _ => return,
        };

        self.skip_whitespace();

        // Expect colon
        if !matches!(self.peek(), CssToken::Colon) {
            // Error recovery: skip to semicolon
            self.skip_to_semicolon();
            return;
        }
        self.advance();
        self.skip_whitespace();

        // Collect value tokens until ';' or '}' or EOF
        let mut value_tokens = Vec::new();
        loop {
            match self.peek() {
                CssToken::Semicolon | CssToken::CurlyBracketClose | CssToken::Eof => break,
                _ => {
                    value_tokens.push(self.advance().clone());
                }
            }
        }

        // Check for !important
        let important = check_important(&mut value_tokens);

        // Trim trailing whitespace tokens
        while matches!(value_tokens.last(), Some(CssToken::Whitespace)) {
            value_tokens.pop();
        }

        let prop_lower = property_name.to_ascii_lowercase();

        // Try shorthand expansion first
        if let Some(expanded) = self.expand_shorthand(&prop_lower, &value_tokens) {
            for (pid, val, imp) in expanded {
                declarations.push(Declaration {
                    property: pid,
                    value: val,
                    important: imp || important,
                });
            }
            return;
        }

        // Single property
        if let Some(property) = parse_property_id(&prop_lower)
            && let Some(value) = parse_value(&value_tokens)
        {
            declarations.push(Declaration {
                property,
                value,
                important,
            });
        }
    }

    fn skip_to_semicolon(&mut self) {
        loop {
            match self.peek() {
                CssToken::Semicolon | CssToken::CurlyBracketClose | CssToken::Eof => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Expand shorthand properties. Returns None if not a shorthand.
    fn expand_shorthand(
        &self,
        property: &str,
        tokens: &[CssToken],
    ) -> Option<Vec<(PropertyId, CssValue, bool)>> {
        match property {
            "margin" => {
                let values = split_shorthand_values(tokens);
                Some(expand_box_shorthand(
                    &values,
                    PropertyId::MarginTop,
                    PropertyId::MarginRight,
                    PropertyId::MarginBottom,
                    PropertyId::MarginLeft,
                ))
            }
            "padding" => {
                let values = split_shorthand_values(tokens);
                Some(expand_box_shorthand(
                    &values,
                    PropertyId::PaddingTop,
                    PropertyId::PaddingRight,
                    PropertyId::PaddingBottom,
                    PropertyId::PaddingLeft,
                ))
            }
            "border-width" => {
                let values = split_shorthand_values(tokens);
                Some(expand_box_shorthand(
                    &values,
                    PropertyId::BorderTopWidth,
                    PropertyId::BorderRightWidth,
                    PropertyId::BorderBottomWidth,
                    PropertyId::BorderLeftWidth,
                ))
            }
            "border-style" => {
                let values = split_shorthand_values(tokens);
                Some(expand_box_shorthand(
                    &values,
                    PropertyId::BorderTopStyle,
                    PropertyId::BorderRightStyle,
                    PropertyId::BorderBottomStyle,
                    PropertyId::BorderLeftStyle,
                ))
            }
            "border-color" => {
                let values = split_shorthand_values(tokens);
                Some(expand_box_shorthand(
                    &values,
                    PropertyId::BorderTopColor,
                    PropertyId::BorderRightColor,
                    PropertyId::BorderBottomColor,
                    PropertyId::BorderLeftColor,
                ))
            }
            "border" => Some(expand_border_shorthand(tokens)),
            _ => None,
        }
    }
}

/// Check and remove !important from value tokens
fn check_important(tokens: &mut Vec<CssToken>) -> bool {
    // Look for Delim('!') followed by Ident("important"), possibly with whitespace
    let len = tokens.len();
    if len < 2 {
        return false;
    }

    // Find !important from the end
    let mut i = len;
    // skip trailing whitespace
    while i > 0 && matches!(tokens[i - 1], CssToken::Whitespace) {
        i -= 1;
    }

    if i < 2 {
        return false;
    }

    let important_pos = i - 1;
    let is_important =
        matches!(&tokens[important_pos], CssToken::Ident(s) if s.eq_ignore_ascii_case("important"));
    if !is_important {
        return false;
    }

    // Skip whitespace before "important"
    let mut j = important_pos;
    while j > 0 && matches!(tokens[j - 1], CssToken::Whitespace) {
        j -= 1;
    }

    if j == 0 {
        return false;
    }

    let bang_pos = j - 1;
    if !matches!(tokens[bang_pos], CssToken::Delim('!')) {
        return false;
    }

    // Remove everything from bang_pos onwards
    tokens.truncate(bang_pos);
    true
}

/// Parse a CSS value from tokens
fn parse_value(tokens: &[CssToken]) -> Option<CssValue> {
    // Filter whitespace for single-token values
    let non_ws: Vec<&CssToken> = tokens
        .iter()
        .filter(|t| !matches!(t, CssToken::Whitespace))
        .collect();

    if non_ws.is_empty() {
        return None;
    }

    // Single token values
    if non_ws.len() == 1 {
        return parse_single_token(non_ws[0]);
    }

    // Function calls: rgb(...) / rgba(...)
    if let Some(CssToken::Function(name)) = non_ws.first() {
        let name_lower = name.to_ascii_lowercase();
        if name_lower == "rgb" || name_lower == "rgba" {
            return parse_rgb_function(&non_ws[1..]);
        }
    }

    // Fallback: try the first non-ws token
    parse_single_token(non_ws[0])
}

fn parse_single_token(token: &CssToken) -> Option<CssValue> {
    match token {
        CssToken::Dimension(v, unit) => {
            let lu = parse_length_unit(unit)?;
            Some(CssValue::Length(*v, lu))
        }
        CssToken::Percentage(v) => Some(CssValue::Percentage(*v)),
        CssToken::Number(v, _) => Some(CssValue::Number(*v)),
        CssToken::Hash(hex, _) => {
            let color = parse_hex_color(hex)?;
            Some(CssValue::Color(color))
        }
        CssToken::Ident(name) => {
            let lower = name.to_ascii_lowercase();
            match lower.as_str() {
                "auto" => Some(CssValue::Auto),
                "none" => Some(CssValue::None),
                "inherit" => Some(CssValue::Inherit),
                "initial" => Some(CssValue::Initial),
                _ => {
                    if let Some(color) = parse_named_color(&lower) {
                        Some(CssValue::Color(color))
                    } else {
                        Some(CssValue::Keyword(lower))
                    }
                }
            }
        }
        CssToken::String(s) => Some(CssValue::String(s.clone())),
        _ => None,
    }
}

fn parse_length_unit(unit: &str) -> Option<LengthUnit> {
    match unit.to_ascii_lowercase().as_str() {
        "px" => Some(LengthUnit::Px),
        "em" => Some(LengthUnit::Em),
        "rem" => Some(LengthUnit::Rem),
        "vw" => Some(LengthUnit::Vw),
        "vh" => Some(LengthUnit::Vh),
        "vmin" => Some(LengthUnit::Vmin),
        "vmax" => Some(LengthUnit::Vmax),
        "ch" => Some(LengthUnit::Ch),
        "ex" => Some(LengthUnit::Ex),
        "pt" => Some(LengthUnit::Pt),
        _ => None,
    }
}

/// Parse rgb(r, g, b) or rgba(r, g, b, a) arguments
fn parse_rgb_function(tokens: &[&CssToken]) -> Option<CssValue> {
    // Collect numeric values, ignoring commas and parens
    let mut nums: Vec<f64> = Vec::new();
    for tok in tokens {
        match tok {
            CssToken::Number(v, _) => nums.push(*v),
            CssToken::Percentage(v) => nums.push(*v * 255.0 / 100.0),
            CssToken::ParenClose => break,
            _ => {}
        }
    }

    if nums.len() == 3 {
        Some(CssValue::Color(CssColor::rgb(
            nums[0].clamp(0.0, 255.0) as u8,
            nums[1].clamp(0.0, 255.0) as u8,
            nums[2].clamp(0.0, 255.0) as u8,
        )))
    } else if nums.len() >= 4 {
        // For rgba, 4th arg is 0-1 float or 0-255 int
        let a = if nums[3] <= 1.0 {
            (nums[3] * 255.0) as u8
        } else {
            nums[3].clamp(0.0, 255.0) as u8
        };
        Some(CssValue::Color(CssColor::rgba(
            nums[0].clamp(0.0, 255.0) as u8,
            nums[1].clamp(0.0, 255.0) as u8,
            nums[2].clamp(0.0, 255.0) as u8,
            a,
        )))
    } else {
        None
    }
}

/// Split shorthand value tokens into individual value groups (by whitespace)
fn split_shorthand_values(tokens: &[CssToken]) -> Vec<Vec<CssToken>> {
    let mut result = Vec::new();
    let mut current = Vec::new();

    for token in tokens {
        match token {
            CssToken::Whitespace => {
                if !current.is_empty() {
                    result.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(token.clone());
            }
        }
    }

    if !current.is_empty() {
        result.push(current);
    }

    result
}

/// Expand a box shorthand (margin, padding, border-width, etc.) with 1-4 values
fn expand_box_shorthand(
    values: &[Vec<CssToken>],
    top: PropertyId,
    right: PropertyId,
    bottom: PropertyId,
    left: PropertyId,
) -> Vec<(PropertyId, CssValue, bool)> {
    let parsed: Vec<CssValue> = values
        .iter()
        .filter_map(|tokens| parse_value(tokens))
        .collect();

    let (t, r, b, l) = match parsed.len() {
        1 => (
            parsed[0].clone(),
            parsed[0].clone(),
            parsed[0].clone(),
            parsed[0].clone(),
        ),
        2 => (
            parsed[0].clone(),
            parsed[1].clone(),
            parsed[0].clone(),
            parsed[1].clone(),
        ),
        3 => (
            parsed[0].clone(),
            parsed[1].clone(),
            parsed[2].clone(),
            parsed[1].clone(),
        ),
        4 => (
            parsed[0].clone(),
            parsed[1].clone(),
            parsed[2].clone(),
            parsed[3].clone(),
        ),
        _ => return Vec::new(),
    };

    vec![
        (top, t, false),
        (right, r, false),
        (bottom, b, false),
        (left, l, false),
    ]
}

/// Expand border shorthand: border: <width> <style> <color>
fn expand_border_shorthand(tokens: &[CssToken]) -> Vec<(PropertyId, CssValue, bool)> {
    let parts = split_shorthand_values(tokens);
    let mut width = None;
    let mut style = None;
    let mut color = None;

    for part in &parts {
        if let Some(val) = parse_value(part) {
            match &val {
                CssValue::Length(_, _) | CssValue::Number(_) => {
                    if width.is_none() {
                        width = Some(val);
                    }
                }
                CssValue::Keyword(kw) => {
                    let is_style = matches!(
                        kw.as_str(),
                        "none"
                            | "hidden"
                            | "dotted"
                            | "dashed"
                            | "solid"
                            | "double"
                            | "groove"
                            | "ridge"
                            | "inset"
                            | "outset"
                    );
                    if is_style && style.is_none() {
                        style = Some(val);
                    } else if color.is_none() {
                        color = Some(val);
                    }
                }
                CssValue::Color(_) => {
                    if color.is_none() {
                        color = Some(val);
                    }
                }
                CssValue::None => {
                    if style.is_none() {
                        style = Some(CssValue::Keyword("none".into()));
                    }
                }
                _ => {}
            }
        }
    }

    let w = width.unwrap_or(CssValue::Length(3.0, LengthUnit::Px));
    let s = style.unwrap_or(CssValue::Keyword("none".into()));
    let c = color.unwrap_or(CssValue::Keyword("currentcolor".into()));

    vec![
        (PropertyId::BorderTopWidth, w.clone(), false),
        (PropertyId::BorderRightWidth, w.clone(), false),
        (PropertyId::BorderBottomWidth, w.clone(), false),
        (PropertyId::BorderLeftWidth, w, false),
        (PropertyId::BorderTopStyle, s.clone(), false),
        (PropertyId::BorderRightStyle, s.clone(), false),
        (PropertyId::BorderBottomStyle, s.clone(), false),
        (PropertyId::BorderLeftStyle, s, false),
        (PropertyId::BorderTopColor, c.clone(), false),
        (PropertyId::BorderRightColor, c.clone(), false),
        (PropertyId::BorderBottomColor, c.clone(), false),
        (PropertyId::BorderLeftColor, c, false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::{CssColor, CssValue, LengthUnit, PropertyId};

    #[test]
    fn parse_simple_rule() {
        let ss = parse_stylesheet("p { color: red; }");
        assert_eq!(ss.rules.len(), 1);
        assert_eq!(ss.rules[0].selectors, vec!["p"]);
        assert_eq!(ss.rules[0].declarations.len(), 1);
        assert_eq!(ss.rules[0].declarations[0].property, PropertyId::Color);
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Color(CssColor::rgb(255, 0, 0))
        );
    }

    #[test]
    fn parse_multiple_declarations() {
        let ss = parse_stylesheet("div { color: blue; font-size: 16px; }");
        assert_eq!(ss.rules.len(), 1);
        assert_eq!(ss.rules[0].declarations.len(), 2);
        assert_eq!(ss.rules[0].declarations[0].property, PropertyId::Color);
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Color(CssColor::rgb(0, 0, 255))
        );
        assert_eq!(ss.rules[0].declarations[1].property, PropertyId::FontSize);
        assert_eq!(
            ss.rules[0].declarations[1].value,
            CssValue::Length(16.0, LengthUnit::Px)
        );
    }

    #[test]
    fn parse_important() {
        let ss = parse_stylesheet("p { color: red !important; }");
        assert_eq!(ss.rules[0].declarations[0].important, true);
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Color(CssColor::rgb(255, 0, 0))
        );
    }

    #[test]
    fn parse_hex_color_value() {
        let ss = parse_stylesheet("div { color: #ff0000; }");
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Color(CssColor::rgb(255, 0, 0))
        );
    }

    #[test]
    fn parse_rgb_function() {
        let ss = parse_stylesheet("div { color: rgb(255, 0, 0); }");
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Color(CssColor::rgb(255, 0, 0))
        );
    }

    #[test]
    fn parse_length_values() {
        let ss = parse_stylesheet("div { width: 100px; height: 50%; margin-top: 2em; }");
        assert_eq!(ss.rules[0].declarations.len(), 3);
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Length(100.0, LengthUnit::Px)
        );
        assert_eq!(
            ss.rules[0].declarations[1].value,
            CssValue::Percentage(50.0)
        );
        assert_eq!(
            ss.rules[0].declarations[2].value,
            CssValue::Length(2.0, LengthUnit::Em)
        );
    }

    #[test]
    fn parse_margin_shorthand() {
        let ss = parse_stylesheet("div { margin: 10px 20px; }");
        assert_eq!(ss.rules[0].declarations.len(), 4);
        assert_eq!(ss.rules[0].declarations[0].property, PropertyId::MarginTop);
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Length(10.0, LengthUnit::Px)
        );
        assert_eq!(
            ss.rules[0].declarations[1].property,
            PropertyId::MarginRight
        );
        assert_eq!(
            ss.rules[0].declarations[1].value,
            CssValue::Length(20.0, LengthUnit::Px)
        );
        assert_eq!(
            ss.rules[0].declarations[2].property,
            PropertyId::MarginBottom
        );
        assert_eq!(
            ss.rules[0].declarations[2].value,
            CssValue::Length(10.0, LengthUnit::Px)
        );
        assert_eq!(ss.rules[0].declarations[3].property, PropertyId::MarginLeft);
        assert_eq!(
            ss.rules[0].declarations[3].value,
            CssValue::Length(20.0, LengthUnit::Px)
        );
    }

    #[test]
    fn parse_padding_shorthand() {
        let ss = parse_stylesheet("div { padding: 5px; }");
        assert_eq!(ss.rules[0].declarations.len(), 4);
        for decl in &ss.rules[0].declarations {
            assert_eq!(decl.value, CssValue::Length(5.0, LengthUnit::Px));
        }
    }

    #[test]
    fn parse_multiple_rules() {
        let ss = parse_stylesheet("p { color: red; } div { color: blue; }");
        assert_eq!(ss.rules.len(), 2);
        assert_eq!(ss.rules[0].selectors, vec!["p"]);
        assert_eq!(ss.rules[1].selectors, vec!["div"]);
    }

    #[test]
    fn parse_selector_list() {
        let ss = parse_stylesheet("h1, h2 { color: red; }");
        assert_eq!(ss.rules[0].selectors, vec!["h1", "h2"]);
    }

    #[test]
    fn parse_named_colors() {
        let ss = parse_stylesheet("div { color: navy; background-color: transparent; }");
        assert_eq!(
            ss.rules[0].declarations[0].value,
            CssValue::Color(CssColor::rgb(0, 0, 128))
        );
        assert_eq!(
            ss.rules[0].declarations[1].value,
            CssValue::Color(CssColor::transparent())
        );
    }

    #[test]
    fn parse_auto_none() {
        let ss = parse_stylesheet("div { width: auto; display: none; }");
        assert_eq!(ss.rules[0].declarations[0].value, CssValue::Auto);
        assert_eq!(ss.rules[0].declarations[1].value, CssValue::None);
    }

    #[test]
    fn parse_inline_style() {
        let decls = parse_declarations("color: red; font-size: 14px;");
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0].property, PropertyId::Color);
        assert_eq!(decls[0].value, CssValue::Color(CssColor::rgb(255, 0, 0)));
        assert_eq!(decls[1].property, PropertyId::FontSize);
        assert_eq!(decls[1].value, CssValue::Length(14.0, LengthUnit::Px));
    }

    #[test]
    fn parse_at_import_skipped() {
        let ss = parse_stylesheet("@import url('x'); p { color: red; }");
        assert_eq!(ss.rules.len(), 1);
        assert_eq!(ss.rules[0].selectors, vec!["p"]);
    }
}
