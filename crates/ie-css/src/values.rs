/// CSS property identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyId {
    Display,
    Width,
    Height,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BorderTopStyle,
    BorderRightStyle,
    BorderBottomStyle,
    BorderLeftStyle,
    BorderTopColor,
    BorderRightColor,
    BorderBottomColor,
    BorderLeftColor,
    BoxSizing,
    FontFamily,
    FontSize,
    FontWeight,
    FontStyle,
    LineHeight,
    TextAlign,
    TextDecoration,
    Color,
    WhiteSpace,
    Position,
    Top,
    Right,
    Bottom,
    Left,
    ZIndex,
    FlexDirection,
    FlexWrap,
    JustifyContent,
    AlignItems,
    AlignSelf,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    BackgroundColor,
    Overflow,
    Visibility,
    Opacity,
}

/// CSS value
#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    Keyword(String),
    Length(f64, LengthUnit),
    Percentage(f64),
    Number(f64),
    Color(CssColor),
    String(String),
    Auto,
    Inherit,
    Initial,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthUnit {
    Px,
    Em,
    Rem,
    Percent,
    Vw,
    Vh,
    Vmin,
    Vmax,
    Ch,
    Ex,
    Pt,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl CssColor {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }
}

/// Is this property inherited by default?
pub fn is_inherited(prop: PropertyId) -> bool {
    matches!(
        prop,
        PropertyId::Color
            | PropertyId::FontFamily
            | PropertyId::FontSize
            | PropertyId::FontWeight
            | PropertyId::FontStyle
            | PropertyId::LineHeight
            | PropertyId::TextAlign
            | PropertyId::TextDecoration
            | PropertyId::Visibility
            | PropertyId::WhiteSpace
    )
}

/// Initial value for a property
pub fn initial_value(prop: PropertyId) -> CssValue {
    match prop {
        PropertyId::Display => CssValue::Keyword("inline".into()),
        PropertyId::Position => CssValue::Keyword("static".into()),
        PropertyId::Color => CssValue::Color(CssColor::rgb(0, 0, 0)),
        PropertyId::BackgroundColor => CssValue::Color(CssColor::transparent()),
        PropertyId::FontSize => CssValue::Length(16.0, LengthUnit::Px),
        PropertyId::FontWeight => CssValue::Number(400.0),
        PropertyId::FontStyle => CssValue::Keyword("normal".into()),
        PropertyId::LineHeight => CssValue::Keyword("normal".into()),
        PropertyId::TextAlign => CssValue::Keyword("start".into()),
        PropertyId::Visibility => CssValue::Keyword("visible".into()),
        PropertyId::Opacity => CssValue::Number(1.0),
        PropertyId::FlexGrow => CssValue::Number(0.0),
        PropertyId::FlexShrink => CssValue::Number(1.0),
        PropertyId::ZIndex => CssValue::Auto,
        PropertyId::Overflow => CssValue::Keyword("visible".into()),
        PropertyId::BoxSizing => CssValue::Keyword("content-box".into()),
        _ => CssValue::Auto,
    }
}

/// Parse a property name to PropertyId
pub fn parse_property_id(name: &str) -> Option<PropertyId> {
    match name {
        "display" => Some(PropertyId::Display),
        "width" => Some(PropertyId::Width),
        "height" => Some(PropertyId::Height),
        "min-width" => Some(PropertyId::MinWidth),
        "max-width" => Some(PropertyId::MaxWidth),
        "min-height" => Some(PropertyId::MinHeight),
        "max-height" => Some(PropertyId::MaxHeight),
        "margin-top" => Some(PropertyId::MarginTop),
        "margin-right" => Some(PropertyId::MarginRight),
        "margin-bottom" => Some(PropertyId::MarginBottom),
        "margin-left" => Some(PropertyId::MarginLeft),
        "padding-top" => Some(PropertyId::PaddingTop),
        "padding-right" => Some(PropertyId::PaddingRight),
        "padding-bottom" => Some(PropertyId::PaddingBottom),
        "padding-left" => Some(PropertyId::PaddingLeft),
        "border-top-width" => Some(PropertyId::BorderTopWidth),
        "border-right-width" => Some(PropertyId::BorderRightWidth),
        "border-bottom-width" => Some(PropertyId::BorderBottomWidth),
        "border-left-width" => Some(PropertyId::BorderLeftWidth),
        "border-top-style" => Some(PropertyId::BorderTopStyle),
        "border-right-style" => Some(PropertyId::BorderRightStyle),
        "border-bottom-style" => Some(PropertyId::BorderBottomStyle),
        "border-left-style" => Some(PropertyId::BorderLeftStyle),
        "border-top-color" => Some(PropertyId::BorderTopColor),
        "border-right-color" => Some(PropertyId::BorderRightColor),
        "border-bottom-color" => Some(PropertyId::BorderBottomColor),
        "border-left-color" => Some(PropertyId::BorderLeftColor),
        "box-sizing" => Some(PropertyId::BoxSizing),
        "font-family" => Some(PropertyId::FontFamily),
        "font-size" => Some(PropertyId::FontSize),
        "font-weight" => Some(PropertyId::FontWeight),
        "font-style" => Some(PropertyId::FontStyle),
        "line-height" => Some(PropertyId::LineHeight),
        "text-align" => Some(PropertyId::TextAlign),
        "text-decoration" => Some(PropertyId::TextDecoration),
        "color" => Some(PropertyId::Color),
        "white-space" => Some(PropertyId::WhiteSpace),
        "position" => Some(PropertyId::Position),
        "top" => Some(PropertyId::Top),
        "right" => Some(PropertyId::Right),
        "bottom" => Some(PropertyId::Bottom),
        "left" => Some(PropertyId::Left),
        "z-index" => Some(PropertyId::ZIndex),
        "flex-direction" => Some(PropertyId::FlexDirection),
        "flex-wrap" => Some(PropertyId::FlexWrap),
        "justify-content" => Some(PropertyId::JustifyContent),
        "align-items" => Some(PropertyId::AlignItems),
        "align-self" => Some(PropertyId::AlignSelf),
        "flex-grow" => Some(PropertyId::FlexGrow),
        "flex-shrink" => Some(PropertyId::FlexShrink),
        "flex-basis" => Some(PropertyId::FlexBasis),
        "background-color" => Some(PropertyId::BackgroundColor),
        "overflow" => Some(PropertyId::Overflow),
        "visibility" => Some(PropertyId::Visibility),
        "opacity" => Some(PropertyId::Opacity),
        _ => None,
    }
}

/// Parse a CSS named color
pub fn parse_named_color(name: &str) -> Option<CssColor> {
    match name.to_ascii_lowercase().as_str() {
        "black" => Some(CssColor::rgb(0, 0, 0)),
        "white" => Some(CssColor::rgb(255, 255, 255)),
        "red" => Some(CssColor::rgb(255, 0, 0)),
        "green" => Some(CssColor::rgb(0, 128, 0)),
        "blue" => Some(CssColor::rgb(0, 0, 255)),
        "yellow" => Some(CssColor::rgb(255, 255, 0)),
        "cyan" | "aqua" => Some(CssColor::rgb(0, 255, 255)),
        "magenta" | "fuchsia" => Some(CssColor::rgb(255, 0, 255)),
        "gray" | "grey" => Some(CssColor::rgb(128, 128, 128)),
        "silver" => Some(CssColor::rgb(192, 192, 192)),
        "maroon" => Some(CssColor::rgb(128, 0, 0)),
        "olive" => Some(CssColor::rgb(128, 128, 0)),
        "navy" => Some(CssColor::rgb(0, 0, 128)),
        "purple" => Some(CssColor::rgb(128, 0, 128)),
        "teal" => Some(CssColor::rgb(0, 128, 128)),
        "orange" => Some(CssColor::rgb(255, 165, 0)),
        "transparent" => Some(CssColor::transparent()),
        _ => None,
    }
}

/// Parse a hex color (#rgb, #rrggbb, #rgba, #rrggbbaa)
pub fn parse_hex_color(hex: &str) -> Option<CssColor> {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(CssColor::rgb(r, g, b))
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            let a = u8::from_str_radix(&hex[3..4], 16).ok()? * 17;
            Some(CssColor::rgba(r, g, b, a))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(CssColor::rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(CssColor::rgba(r, g, b, a))
        }
        _ => None,
    }
}
