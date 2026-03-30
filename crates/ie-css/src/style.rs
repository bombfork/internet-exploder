use crate::values::{CssColor, CssValue, PropertyId};

/// Computed style for a DOM element.
/// Will be expanded when style resolution is wired up.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: CssColor,
    pub background_color: CssColor,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::default(),
            color: CssColor::rgb(0, 0, 0),
            background_color: CssColor::transparent(),
        }
    }
}

impl ComputedStyle {
    /// Apply a single declaration to this computed style
    pub fn apply(&mut self, property: PropertyId, value: &CssValue) {
        match property {
            PropertyId::Display => {
                if let CssValue::Keyword(kw) = value {
                    self.display = match kw.as_str() {
                        "block" => Display::Block,
                        "inline" => Display::Inline,
                        "flex" => Display::Flex,
                        "grid" => Display::Grid,
                        _ => return,
                    };
                } else if matches!(value, CssValue::None) {
                    self.display = Display::None;
                }
            }
            PropertyId::Color => {
                if let CssValue::Color(c) = value {
                    self.color = *c;
                }
            }
            PropertyId::BackgroundColor => {
                if let CssValue::Color(c) = value {
                    self.background_color = *c;
                }
            }
            _ => {} // Other properties handled when layout is wired up
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Display {
    #[default]
    Block,
    Inline,
    Flex,
    Grid,
    None,
}
