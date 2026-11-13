//! Name the v1 style keys and the kind of value each accepts, the one table every crate checks styles against.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use crate::color::Rgba;
use crate::model::{Edge, MAX_MAGNITUDE};

/// List the keywords a boolean style value spells.
const BOOLEAN_KEYWORDS: [&str; 2] = ["false", "true"];
/// Name the value kind `fill-pattern` accepts: one of its pattern keywords.
const FILL_PATTERN_KIND: StyleValueKind =
    StyleValueKind::Keyword(&["dots", "grain", "lines", "none", "paper"]);
/// Name the value kind `font` accepts: one of its font keywords.
const FONT_KIND: StyleValueKind = StyleValueKind::Keyword(&["mono"]);
/// Name the value kind `text-transform` accepts: one of its transform keywords.
const TEXT_TRANSFORM_KIND: StyleValueKind =
    StyleValueKind::Keyword(&["capitalize", "lowercase", "none", "uppercase"]);

/// Key a v1 style by its name, typed by the value `V` it reads, so a key can only be read as its own kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleKey<V> {
    name: &'static str,
    value_type: PhantomData<V>,
}

/// Key a v1 style holding a color.
pub type ColorKey = StyleKey<ColorValue>;

/// Key a v1 style holding a number.
pub type NumberKey = StyleKey<NumberValue>;

/// Type the value a typed style key reads: the kind validation checks it against and what it parses into.
pub trait StyleValue {
    /// Name the kind validation checks this value against.
    const KIND: StyleValueKind;
    /// Type what a validated value of this kind is parsed into.
    type Parsed;

    /// Parse a value of this kind, `None` when it is not one.
    fn parse(text: &str) -> Option<Self::Parsed>;
}

/// Read a style value as a color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorValue;

/// Read a style value as a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberValue;

/// Key a v1 style of any kind by its name, paired with the kind of value it accepts, for lookups by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnyStyleKey {
    name: &'static str,
    value_kind: StyleValueKind,
}

/// Classify the values a style key accepts, compared case-insensitively.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleValueKind {
    /// A hex color (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`), a CSS named color or `transparent`.
    Color,
    /// A non-negative plain decimal `digits(.digits)?` no greater than `MAX_MAGNITUDE`, with no exponent or unit.
    Number,
    Boolean,
    Keyword(&'static [&'static str]),
}

impl ColorKey {
    /// Base color of a solid's faces.
    pub const FILL: ColorKey = StyleKey::new("fill");
    /// Outline color of a solid's faces, or stroke color of a connection.
    pub const STROKE: ColorKey = StyleKey::new("stroke");
}

impl NumberKey {
    /// Dash length of a solid's outline, or of a connection's arc.
    pub const STROKE_DASH: NumberKey = StyleKey::new("stroke-dash");
}

impl<V: StyleValue> StyleKey<V> {
    /// Create a new key named `name`, reading values of kind `V`.
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            value_type: PhantomData,
        }
    }

    /// Erase the value type, keeping the name and the kind validation checks values against.
    pub const fn erased(self) -> AnyStyleKey {
        AnyStyleKey::new(self.name, V::KIND)
    }

    /// Read this key's value from `style`, already validated: `None` when unset.
    pub fn read_from(self, style: &BTreeMap<String, String>) -> Option<V::Parsed> {
        let value = style.get(self.name)?;
        V::parse(value)
    }

    /// Return the key's name, as a style entry spells it.
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl StyleValue for ColorValue {
    const KIND: StyleValueKind = StyleValueKind::Color;
    type Parsed = Rgba;

    fn parse(text: &str) -> Option<Rgba> {
        Rgba::parse(text)
    }
}

impl StyleValue for NumberValue {
    const KIND: StyleValueKind = StyleValueKind::Number;
    type Parsed = f64;

    /// Parse a `Number` value: a non-negative plain decimal, and only when it is no greater than `MAX_MAGNITUDE`.
    fn parse(text: &str) -> Option<f64> {
        let number: f64 = text.parse().ok()?;
        let is_plain_number = Self::is_plain_number(text);
        let is_bounded = number <= MAX_MAGNITUDE;
        let is_bounded_plain_number = is_plain_number && is_bounded;
        is_bounded_plain_number.then_some(number)
    }
}

impl NumberValue {
    /// Tell whether `text` is a plain decimal `digits(.digits)?`, with no sign, exponent or unit.
    fn is_plain_number(text: &str) -> bool {
        let Some((integer_digits, fraction_digits)) = text.split_once('.') else {
            return Self::is_ascii_digit_run(text);
        };
        let is_integer_part_valid = Self::is_ascii_digit_run(integer_digits);
        let is_fraction_part_valid = Self::is_ascii_digit_run(fraction_digits);
        is_integer_part_valid && is_fraction_part_valid
    }

    /// Tell whether `text` is a non-empty run of ASCII digits.
    fn is_ascii_digit_run(text: &str) -> bool {
        let has_digits = !text.is_empty();
        let is_all_digits = text.bytes().all(|digit| digit.is_ascii_digit());
        has_digits && is_all_digits
    }
}

impl AnyStyleKey {
    /// Create a new key named `name`, accepting values of `value_kind`.
    const fn new(name: &'static str, value_kind: StyleValueKind) -> Self {
        Self { name, value_kind }
    }

    /// Every v1 style key: d2's style keywords at v0.9.0, sorted by name.
    pub const ALL: [AnyStyleKey; 20] = [
        AnyStyleKey::new("3d", StyleValueKind::Boolean),
        AnyStyleKey::new("animated", StyleValueKind::Boolean),
        AnyStyleKey::new("bold", StyleValueKind::Boolean),
        AnyStyleKey::new("border-radius", StyleValueKind::Number),
        AnyStyleKey::new("double-border", StyleValueKind::Boolean),
        ColorKey::FILL.erased(),
        AnyStyleKey::new("fill-pattern", FILL_PATTERN_KIND),
        AnyStyleKey::new("filled", StyleValueKind::Boolean),
        AnyStyleKey::new("font", FONT_KIND),
        AnyStyleKey::new("font-color", StyleValueKind::Color),
        AnyStyleKey::new("font-size", StyleValueKind::Number),
        AnyStyleKey::new("italic", StyleValueKind::Boolean),
        AnyStyleKey::new("multiple", StyleValueKind::Boolean),
        AnyStyleKey::new("opacity", StyleValueKind::Number),
        AnyStyleKey::new("shadow", StyleValueKind::Boolean),
        ColorKey::STROKE.erased(),
        NumberKey::STROKE_DASH.erased(),
        AnyStyleKey::new("stroke-width", StyleValueKind::Number),
        AnyStyleKey::new("text-transform", TEXT_TRANSFORM_KIND),
        AnyStyleKey::new("underline", StyleValueKind::Boolean),
    ];

    /// Find the v1 style key spelled exactly `name`.
    pub fn find(name: &str) -> Option<AnyStyleKey> {
        let mut known_keys = Self::ALL.iter();
        let known_key = known_keys.find(|known_key| known_key.name == name);
        known_key.copied()
    }

    /// Return the key's name, as a style entry spells it.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Return the kind of value the key accepts.
    pub fn value_kind(&self) -> StyleValueKind {
        self.value_kind
    }
}

impl fmt::Display for AnyStyleKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name)
    }
}

impl Edge {
    /// Dash length an arc is drawn with when its edge sets no `stroke-dash`, the reference's flowing connections.
    pub const DEFAULT_STROKE_DASH: f64 = 5.0;

    /// Return the dash length the edge's arc is drawn with: its validated `stroke-dash`, else the default; 0 draws it solid.
    pub fn stroke_dash(&self) -> f64 {
        let style = self.style();
        let authored_dash = NumberKey::STROKE_DASH.read_from(style);
        authored_dash.unwrap_or(Self::DEFAULT_STROKE_DASH)
    }
}

impl StyleValueKind {
    /// Tell whether `value`, lowercased, is a value of this kind.
    pub fn accepts(&self, value: &str) -> bool {
        let lowercase_value = value.to_ascii_lowercase();
        let text = lowercase_value.as_str();
        match self {
            Self::Color => Rgba::parse(text).is_some(),
            Self::Number => NumberValue::parse(text).is_some(),
            Self::Boolean => BOOLEAN_KEYWORDS.contains(&text),
            Self::Keyword(keywords) => keywords.contains(&text),
        }
    }
}

impl fmt::Display for StyleValueKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Color => formatter.write_str("a color (hex, CSS name or `transparent`)"),
            Self::Number => write!(
                formatter,
                "a non-negative plain number no greater than {MAX_MAGNITUDE}"
            ),
            Self::Boolean => formatter.write_str("a boolean"),
            Self::Keyword(keywords) => {
                let quoted_keywords: Vec<String> = keywords
                    .iter()
                    .map(|keyword| format!("`{keyword}`"))
                    .collect();
                let keyword_list = quoted_keywords.join(", ");
                write!(formatter, "one of {keyword_list}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Element;

    #[test]
    fn style_keys_are_sorted_and_unique() {
        let is_sorted = AnyStyleKey::ALL
            .windows(2)
            .all(|pair| pair[0].name < pair[1].name);
        assert!(is_sorted);
    }

    #[test]
    fn typed_keys_read_their_values_and_leave_unset_keys_none() {
        let element = Element::new("api")
            .with_style("fill", "RED")
            .with_style("stroke-dash", "5")
            .with_style("stroke-width", "2");
        let red = Rgba::from_packed(0xff0000ff);
        let element_properties = element.properties();
        let element_style = element_properties.style();
        assert_eq!(ColorKey::FILL.read_from(element_style), Some(red));
        assert_eq!(ColorKey::STROKE.read_from(element_style), None);
        assert_eq!(NumberKey::STROKE_DASH.read_from(element_style), Some(5.0));
        let edge = Edge::new().with_style("stroke", "#14f195");
        let green = Rgba::from_packed(0x14f195ff);
        let edge_style = edge.style();
        assert_eq!(ColorKey::STROKE.read_from(edge_style), Some(green));
    }

    #[test]
    fn each_value_kind_accepts_only_its_own_values() {
        let overflowing_number = "9".repeat(400);
        let cases = [
            (StyleValueKind::Color, "#3478F6", true),
            (StyleValueKind::Color, "RebeccaPurple", true),
            (StyleValueKind::Color, "transparent", true),
            (StyleValueKind::Color, "12", false),
            (StyleValueKind::Color, "#12345", false),
            (StyleValueKind::Number, "1.5", true),
            (StyleValueKind::Number, "-1.5", false),
            (StyleValueKind::Number, "12px", false),
            (StyleValueKind::Number, ".5", false),
            (StyleValueKind::Number, overflowing_number.as_str(), false),
            (StyleValueKind::Number, "1000000", true),
            (StyleValueKind::Number, "1000000.5", false),
            (StyleValueKind::Number, "10000000000000", false),
            (StyleValueKind::Boolean, "TRUE", true),
            (StyleValueKind::Boolean, "yes", false),
            (FONT_KIND, "mono", true),
            (FONT_KIND, "serif", false),
            (TEXT_TRANSFORM_KIND, "Capitalize", true),
            (TEXT_TRANSFORM_KIND, "title", false),
        ];
        for (value_kind, value, is_expected_accepted) in cases {
            let is_accepted = value_kind.accepts(value);
            assert_eq!(is_accepted, is_expected_accepted, "{value_kind}: {value:?}");
        }
    }
}
