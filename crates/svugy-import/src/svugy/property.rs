//! Read a `key=value` entry into a typed property, the one place a property key decides how its value is read.

use std::fmt;

use chumsky::span::Spanned;
use svugy_model::color::Rgba;
use svugy_model::model::{PlacementHint, Position, Shape, Traveler};
use svugy_model::style::{NumberValue, StyleValue};

use super::ast::{Reference, Span, Value};
use super::diagnostic::ProseList;

/// Spell the prefix of a `meta.NAME` key.
const META_PREFIX: &str = "meta.";
/// Separate the two numbers of a `position` written `X;Y`.
const POSITION_SEMICOLON: char = ';';
/// Spell the dash length of `line=dashed`.
const DASHED_STROKE_DASH: f64 = 5.0;
/// Spell the dash length of `line=dotted`.
const DOTTED_STROKE_DASH: f64 = 2.0;

/// Name a property key: one of the spec's tables, `target` of a legend key row, or a `meta.NAME` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    Traveler,
    Target,
    Meta(String),
}

/// Pair a typed property with its key and the span it was written at, or the span of the style name that brought it.
#[derive(Debug, Clone)]
pub(crate) struct Property {
    pub(crate) key: PropertyKey,
    pub(crate) value: PropertyValue,
    pub(crate) span: Span,
}

/// Hold a property's value, read as its key decides.
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
    Traveler(Traveler),
    Target(Reference),
    Meta { name: String, value: String },
}

/// Name how a line is drawn: an outline or edge dash, or an edge drawn as a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    Badge,
}

/// Spell the kind and the keywords of a keyword-valued property, so one reader reads them all.
trait Keyword: Copy + 'static {
    /// Name the kind of value, as an error says what a value is not.
    const KIND: &'static str;
    /// Pair each keyword with what it reads as, in the spec's order.
    const KEYWORDS: &'static [(&'static str, Self)];
}

impl PropertyKey {
    /// Pair every key but `meta.NAME` with its spelling, in the spec's table order.
    pub(crate) const SPELLINGS: [(&'static str, Self); 12] = [
        ("caption", Self::Caption),
        ("color", Self::Color),
        ("stroke", Self::Stroke),
        ("line", Self::Line),
        ("shape", Self::Shape),
        ("height", Self::Height),
        ("position", Self::Position),
        ("layout", Self::Layout),
        ("columns", Self::Columns),
        ("link", Self::Link),
        ("traveler", Self::Traveler),
        ("target", Self::Target),
    ];

    /// Find the key spelled `spelling`, a `meta.NAME` key included.
    pub(crate) fn parse(spelling: &str) -> Option<Self> {
        let meta_name = spelling.strip_prefix(META_PREFIX);
        let named_meta_name = meta_name.filter(|name| !name.is_empty());
        if let Some(name) = named_meta_name {
            let name = name.to_owned();
            return Some(Self::Meta(name));
        }
        let mut spellings = Self::SPELLINGS.iter();
        let known_spelling = spellings.find(|(known, _key)| *known == spelling);
        known_spelling.map(|(_spelling, key)| key.clone())
    }

    /// Read `value` as this key decides, or return the error naming what the key accepts.
    pub(crate) fn read(&self, value: &Spanned<Value>) -> Result<PropertyValue, String> {
        let value_span = value.span;
        match self {
            Self::Caption => value.read_text().map(PropertyValue::Caption),
            Self::Color => value.read_colour().map(PropertyValue::Color),
            Self::Stroke => value.read_colour().map(PropertyValue::Stroke),
            Self::Line => value.read_keyword().map(PropertyValue::Line),
            Self::Shape => value.read_keyword().map(PropertyValue::Shape),
            Self::Height => value.read_positive_number(self).map(PropertyValue::Height),
            Self::Position => value.read_position(self).map(PropertyValue::Position),
            Self::Layout => value.read_keyword().map(PropertyValue::Layout),
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
            Self::Traveler => value.read_keyword().map(PropertyValue::Traveler),
            Self::Target => value.read_text().map(|target| {
                let reference = Reference::from_dotted(&target, value_span);
                PropertyValue::Target(reference)
            }),
            Self::Meta(name) => value.read_text().map(|text| {
                let name = name.clone();
                PropertyValue::Meta { name, value: text }
            }),
        }
    }
}

impl LineStyle {
    /// Return the dash length this line style draws, `None` for a solid line or a badge.
    pub(crate) fn stroke_dash(self) -> Option<f64> {
        match self {
            Self::Dashed => Some(DASHED_STROKE_DASH),
            Self::Dotted => Some(DOTTED_STROKE_DASH),
            Self::Solid | Self::Badge => None,
        }
    }
}

impl Value {
    /// Read the value as text: quoted, or one bare piece.
    fn read_text(&self) -> Result<String, String> {
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

    /// Read the value as one of `K`'s keywords.
    fn read_keyword<K: Keyword>(&self) -> Result<K, String> {
        let text = self.read_text()?;
        let mut keywords = K::KEYWORDS.iter();
        let keyword = keywords.find(|(spelling, _keyword)| *spelling == text);
        keyword.map(|(_spelling, keyword)| *keyword).ok_or_else(|| {
            let kind = K::KIND;
            let spellings: Vec<&str> = K::KEYWORDS.iter().map(|(spelling, _)| *spelling).collect();
            let expected = ProseList::join_quoted(&spellings, "or");
            format!("`{text}` is not a {kind}: expected {expected}")
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

impl Keyword for LineStyle {
    const KIND: &'static str = "line";
    const KEYWORDS: &'static [(&'static str, Self)] = &[
        ("solid", Self::Solid),
        ("dashed", Self::Dashed),
        ("dotted", Self::Dotted),
        ("badge", Self::Badge),
    ];
}

impl Keyword for Shape {
    const KIND: &'static str = "shape";
    const KEYWORDS: &'static [(&'static str, Self)] = &[
        ("box", Self::Box),
        ("slab", Self::Slab),
        ("cylinder", Self::Cylinder),
        ("disc", Self::Disc),
    ];
}

impl Keyword for PlacementHint {
    const KIND: &'static str = "layout";
    const KEYWORDS: &'static [(&'static str, Self)] = &[
        ("layered", Self::Layered),
        ("grid", Self::Grid { columns: 0 }),
        ("row", Self::Row),
        ("column", Self::Column),
    ];
}

impl Keyword for Traveler {
    const KIND: &'static str = "traveler";
    const KEYWORDS: &'static [(&'static str, Self)] = &[
        ("dot", Self::Dot),
        ("envelope", Self::Envelope),
        ("parcel", Self::Parcel),
        ("document", Self::Document),
        ("key", Self::Key),
    ];
}

impl fmt::Display for PropertyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Meta(name) = self {
            return write!(formatter, "{META_PREFIX}{name}");
        }
        let mut spellings = Self::SPELLINGS.iter();
        let spelling = spellings.find(|(_spelling, key)| key == self);
        let spelling = spelling.map_or("", |(spelling, _key)| spelling);
        formatter.write_str(spelling)
    }
}
