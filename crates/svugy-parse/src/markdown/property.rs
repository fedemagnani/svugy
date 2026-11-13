//! Read a `key=value` entry into a typed property, the one place a property key decides how its value is read.

use std::fmt;
use std::str::FromStr;

use chumsky::span::Spanned;
use strum::{EnumIter, EnumString, IntoStaticStr, VariantNames};
use svugy_model::color::Rgba;
use svugy_model::model::{PlacementHint, Position, Shape, Traveler};
use svugy_model::style::{NumberValue, StyleValue};

use super::ast::{Span, Value};
use crate::prose::ProseList;

/// Spell the prefix of a `meta.NAME` key.
const META_PREFIX: &str = "meta.";
/// Separate the two numbers of a `position` written `X;Y`.
const POSITION_SEMICOLON: char = ';';
/// Dash length `line=solid` lowers to: no dash at all.
const SOLID_LENGTH: f64 = 0.0;
/// Dash length `line=dashed` lowers to.
const DASHED_LENGTH: f64 = 5.0;
/// Dash length `line=dotted` lowers to.
const DOTTED_LENGTH: f64 = 2.0;

/// Name a property key: one of the spec's tables in their order, or a `meta.NAME` entry.
#[derive(Debug, Clone, PartialEq, Eq, EnumString, EnumIter, IntoStaticStr)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum PropertyKey {
    Caption,
    Color,
    Stroke,
    Line,
    Shape,
    Height,
    Position,
    Layout,
    Columns,
    Link,
    Code,
    Traveler,
    #[strum(disabled)]
    Meta(String),
}

/// Pair a typed property with the span it was written at, or the span of the style name that brought it.
#[derive(Debug, Clone)]
pub(crate) struct Property {
    pub(crate) value: PropertyValue,
    pub(crate) span: Span,
}

/// Hold a property's value, read as its key decides, or the key alone when its value was unreadable and is already reported.
#[derive(Debug, Clone)]
pub(crate) enum PropertyValue {
    Caption(String),
    Color(String),
    Stroke(String),
    Line(LineStyle),
    Shape(Shape),
    Height(f64),
    Position(Position),
    Layout(PlacementHint),
    Columns(u32),
    Link(Spanned<String>),
    Code(Spanned<String>),
    Traveler(Traveler),
    Meta { name: String, value: String },
    Unreadable(PropertyKey),
}

/// Name how a `line` keyword draws: an outline or edge dash, or an edge drawn as a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    Badge,
}

impl PropertyKey {
    /// Find the key spelled `spelling`, a `meta.NAME` key included.
    pub(crate) fn parse(spelling: &str) -> Option<Self> {
        let meta_name = spelling.strip_prefix(META_PREFIX);
        let named_meta_name = meta_name.filter(|name| !name.is_empty());
        if let Some(name) = named_meta_name {
            let name = name.to_owned();
            return Some(Self::Meta(name));
        }
        let known_key = Self::from_str(spelling);
        known_key.ok()
    }

    /// Read `value` as this key decides, or return the error naming what the key accepts.
    pub(crate) fn read(&self, value: &Spanned<Value>) -> Result<PropertyValue, String> {
        let value_span = value.span;
        match self {
            Self::Caption => value.read_text().map(PropertyValue::Caption),
            Self::Color => value.read_colour().map(PropertyValue::Color),
            Self::Stroke => value.read_colour().map(PropertyValue::Stroke),
            Self::Line => value.read_keyword(self).map(PropertyValue::Line),
            Self::Shape => value.read_keyword(self).map(PropertyValue::Shape),
            Self::Height => value.read_positive_number(self).map(PropertyValue::Height),
            Self::Position => value.read_position(self).map(PropertyValue::Position),
            Self::Layout => value.read_keyword(self).map(PropertyValue::Layout),
            Self::Columns => value
                .read_positive_integer(self)
                .map(PropertyValue::Columns),
            Self::Link => value.read_text().map(|link| {
                let spanned_link = Spanned {
                    inner: link,
                    span: value_span,
                };
                PropertyValue::Link(spanned_link)
            }),
            Self::Code => value.read_text().map(|code| {
                let spanned_code = Spanned {
                    inner: code,
                    span: value_span,
                };
                PropertyValue::Code(spanned_code)
            }),
            Self::Traveler => value.read_keyword(self).map(PropertyValue::Traveler),
            Self::Meta(name) => value.read_text().map(|text| {
                let name = name.clone();
                PropertyValue::Meta { name, value: text }
            }),
        }
    }
}

impl PropertyValue {
    /// Return the key the value was written under.
    pub(crate) fn key(&self) -> PropertyKey {
        match self {
            Self::Caption(..) => PropertyKey::Caption,
            Self::Color(..) => PropertyKey::Color,
            Self::Stroke(..) => PropertyKey::Stroke,
            Self::Line(..) => PropertyKey::Line,
            Self::Shape(..) => PropertyKey::Shape,
            Self::Height(..) => PropertyKey::Height,
            Self::Position(..) => PropertyKey::Position,
            Self::Layout(..) => PropertyKey::Layout,
            Self::Columns(..) => PropertyKey::Columns,
            Self::Link(..) => PropertyKey::Link,
            Self::Code(..) => PropertyKey::Code,
            Self::Traveler(..) => PropertyKey::Traveler,
            Self::Meta { name, .. } => PropertyKey::Meta(name.clone()),
            Self::Unreadable(key) => key.clone(),
        }
    }
}

impl Value {
    /// Read the value as text: quoted, or one bare piece.
    pub(crate) fn read_text(&self) -> Result<String, String> {
        let pieces = match self {
            Self::Quoted(text) => return Ok(text.clone()),
            Self::Bare(pieces) => pieces,
        };
        let [piece] = pieces.as_slice() else {
            let joined_pieces = pieces.join(",");
            let message = format!("a bare value holds no commas: quote `{joined_pieces}`");
            return Err(message);
        };
        Ok(piece.clone())
    }

    /// Read the value as one of the keywords `K` names, spelled exactly.
    fn read_keyword<K: FromStr + VariantNames>(&self, key: &PropertyKey) -> Result<K, String> {
        let text = self.read_text()?;
        let keyword = K::from_str(&text);
        keyword.map_err(|_unknown_keyword| {
            let expected = ProseList::join_quoted(K::VARIANTS, "or");
            format!("`{text}` is not a {key}: expected {expected}")
        })
    }

    /// Read the value as a colour, kept as written for the model's style.
    fn read_colour(&self) -> Result<String, String> {
        let text = self.read_text()?;
        let is_colour = Rgba::parse(&text).is_some();
        if is_colour {
            return Ok(text);
        }
        let message = format!(
            "`{text}` is not a colour: expected `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, a CSS colour name or `transparent`"
        );
        Err(message)
    }

    /// Read the value as a plain number greater than 0.
    fn read_positive_number(&self, key: &PropertyKey) -> Result<f64, String> {
        let text = self.read_text()?;
        let number = NumberValue::parse(&text).filter(|number| *number > 0.0);
        number.ok_or_else(|| format!("`{key}` takes a number greater than 0"))
    }

    /// Read the value as a whole number greater than 0.
    fn read_positive_integer(&self, key: &PropertyKey) -> Result<u32, String> {
        let text = self.read_text()?;
        let parsed: Result<u32, _> = text.parse();
        let integer = parsed.ok().filter(|integer| *integer > 0);
        integer.ok_or_else(|| format!("`{key}` takes a whole number greater than 0"))
    }

    /// Read the value as two signed plain numbers, written `X,Y` or `X;Y`.
    fn read_position(&self, key: &PropertyKey) -> Result<Position, String> {
        let pieces: Vec<&str> = match self {
            Self::Bare(pieces) if pieces.len() == 2 => pieces.iter().map(String::as_str).collect(),
            Self::Bare(pieces) if pieces.len() == 1 => {
                pieces[0].split(POSITION_SEMICOLON).collect()
            }
            _other_value => Vec::new(),
        };
        let coordinates: Option<Vec<f64>> = pieces
            .iter()
            .map(|piece| Self::parse_signed_number(piece))
            .collect();
        let coordinates = coordinates.unwrap_or_default();
        let [x, y] = coordinates.as_slice() else {
            let message = format!("`{key}` takes two numbers, `X,Y`");
            return Err(message);
        };
        let position = Position { x: *x, y: *y };
        Ok(position)
    }

    /// Parse a plain number, optionally preceded by `-`.
    fn parse_signed_number(text: &str) -> Option<f64> {
        let Some(magnitude_text) = text.strip_prefix('-') else {
            return NumberValue::parse(text);
        };
        let magnitude = NumberValue::parse(magnitude_text)?;
        Some(-magnitude)
    }
}

impl fmt::Display for PropertyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Meta(name) = self {
            return write!(formatter, "{META_PREFIX}{name}");
        }
        let spelling: &str = self.into();
        formatter.write_str(spelling)
    }
}

impl LineStyle {
    /// Find the line style drawing `dash_length`, `None` for a length no keyword draws.
    pub(crate) fn drawing_dash(dash_length: f64) -> Option<Self> {
        let is_solid = dash_length == SOLID_LENGTH;
        if is_solid {
            return Some(Self::Solid);
        }
        let is_dashed = dash_length == DASHED_LENGTH;
        if is_dashed {
            return Some(Self::Dashed);
        }
        let is_dotted = dash_length == DOTTED_LENGTH;
        if is_dotted {
            return Some(Self::Dotted);
        }
        None
    }

    /// Return the dash length this line style draws, zero for a solid line, `None` for a badge.
    pub(crate) fn stroke_dash(self) -> Option<f64> {
        match self {
            Self::Solid => Some(SOLID_LENGTH),
            Self::Dashed => Some(DASHED_LENGTH),
            Self::Dotted => Some(DOTTED_LENGTH),
            Self::Badge => None,
        }
    }
}
