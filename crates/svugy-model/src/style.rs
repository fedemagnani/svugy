//! Name the v1 style keys and the kind of value each accepts, the one table every crate checks styles against.

use std::collections::BTreeMap;
use std::fmt;

use crate::color::Rgba;
use crate::model::{Edge, Element};
use crate::validate::MAX_MAGNITUDE;

const BOOLEAN_KEYWORDS: [&str; 2] = ["false", "true"];
const FILL_PATTERN_KEYWORDS: [&str; 5] = ["dots", "grain", "lines", "none", "paper"];
const FONT_KEYWORDS: [&str; 1] = ["mono"];
const TEXT_TRANSFORM_KEYWORDS: [&str; 4] = ["capitalize", "lowercase", "none", "uppercase"];

/// Key a v1 style by its name, paired with the kind of value it accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleKey {
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

impl StyleKey {
    /// Base color of a solid's faces.
    pub const FILL: StyleKey = StyleKey::new("fill", StyleValueKind::Color);
    /// Outline color of a solid's faces, or stroke color of a connection.
    pub const STROKE: StyleKey = StyleKey::new("stroke", StyleValueKind::Color);
    /// Dash length of a solid's outline, or of a connection's arc.
    pub const STROKE_DASH: StyleKey = StyleKey::new("stroke-dash", StyleValueKind::Number);

    /// Every v1 style key: d2's style keywords at v0.9.0, sorted by name.
    pub const ALL: [StyleKey; 20] = [
        StyleKey::new("3d", StyleValueKind::Boolean),
        StyleKey::new("animated", StyleValueKind::Boolean),
        StyleKey::new("bold", StyleValueKind::Boolean),
        StyleKey::new("border-radius", StyleValueKind::Number),
        StyleKey::new("double-border", StyleValueKind::Boolean),
        Self::FILL,
        StyleKey::new(
            "fill-pattern",
            StyleValueKind::Keyword(&FILL_PATTERN_KEYWORDS),
        ),
        StyleKey::new("filled", StyleValueKind::Boolean),
        StyleKey::new("font", StyleValueKind::Keyword(&FONT_KEYWORDS)),
        StyleKey::new("font-color", StyleValueKind::Color),
        StyleKey::new("font-size", StyleValueKind::Number),
        StyleKey::new("italic", StyleValueKind::Boolean),
        StyleKey::new("multiple", StyleValueKind::Boolean),
        StyleKey::new("opacity", StyleValueKind::Number),
        StyleKey::new("shadow", StyleValueKind::Boolean),
        Self::STROKE,
        Self::STROKE_DASH,
        StyleKey::new("stroke-width", StyleValueKind::Number),
        StyleKey::new(
            "text-transform",
            StyleValueKind::Keyword(&TEXT_TRANSFORM_KEYWORDS),
        ),
        StyleKey::new("underline", StyleValueKind::Boolean),
    ];

    const fn new(name: &'static str, value_kind: StyleValueKind) -> Self {
        Self { name, value_kind }
    }

    /// Find the v1 style key spelled exactly `name`.
    pub fn find(name: &str) -> Option<StyleKey> {
        let mut known_keys = Self::ALL.iter();
        let known_key = known_keys.find(|known_key| known_key.name == name);
        known_key.copied()
    }

    /// Read this key's color from `style`: `None` when unset, or when the key takes no color.
    fn color_in(self, style: &BTreeMap<String, String>) -> Option<Rgba> {
        let is_other_kind = self.value_kind != StyleValueKind::Color;
        if is_other_kind {
            return None;
        }
        let value = style.get(self.name)?;
        Rgba::parse(value)
    }

    /// Read this key's number from `style`: `None` when unset, or when the key takes no number.
    fn number_in(self, style: &BTreeMap<String, String>) -> Option<f64> {
        let is_other_kind = self.value_kind != StyleValueKind::Number;
        if is_other_kind {
            return None;
        }
        let value = style.get(self.name)?;
        StyleValueKind::parse_number(value)
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn value_kind(&self) -> StyleValueKind {
        self.value_kind
    }
}

impl Element {
    /// Read the color this element sets for `key`, already validated: `None` when unset or when `key` takes no color.
    pub fn style_color(&self, key: StyleKey) -> Option<Rgba> {
        key.color_in(self.style())
    }

    /// Read the number this element sets for `key`, already validated: `None` when unset or when `key` takes no number.
    pub fn style_number(&self, key: StyleKey) -> Option<f64> {
        key.number_in(self.style())
    }
}

impl Edge {
    /// Dash length an arc is drawn with when its edge sets no `stroke-dash`, the reference's flowing connections.
    pub const DEFAULT_STROKE_DASH: f64 = 5.0;

    /// Read the color this edge sets for `key`, already validated: `None` when unset or when `key` takes no color.
    pub fn style_color(&self, key: StyleKey) -> Option<Rgba> {
        key.color_in(self.style())
    }

    /// Return the dash length the edge's arc is drawn with: its validated `stroke-dash`, else the default; 0 draws it solid.
    pub fn stroke_dash(&self) -> f64 {
        let authored_dash = StyleKey::STROKE_DASH.number_in(self.style());
        authored_dash.unwrap_or(Self::DEFAULT_STROKE_DASH)
    }
}

impl fmt::Display for StyleKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name)
    }
}

impl StyleValueKind {
    /// Tell whether `value`, lowercased, is a value of this kind.
    pub fn accepts(&self, value: &str) -> bool {
        let lowercase_value = value.to_ascii_lowercase();
        let text = lowercase_value.as_str();
        match self {
            Self::Color => Rgba::parse(text).is_some(),
            Self::Number => Self::parse_number(text).is_some(),
            Self::Boolean => BOOLEAN_KEYWORDS.contains(&text),
            Self::Keyword(keywords) => keywords.contains(&text),
        }
    }

    /// Parse a `Number` value: a non-negative plain decimal, and only when it is no greater than `MAX_MAGNITUDE`.
    pub fn parse_number(text: &str) -> Option<f64> {
        let number: f64 = text.parse().ok()?;
        let is_plain_number = Self::is_plain_number(text);
        let is_bounded = number <= MAX_MAGNITUDE;
        let is_bounded_plain_number = is_plain_number && is_bounded;
        is_bounded_plain_number.then_some(number)
    }

    fn is_plain_number(text: &str) -> bool {
        let Some((integer_digits, fraction_digits)) = text.split_once('.') else {
            return Self::is_ascii_digit_run(text);
        };
        let is_integer_part_valid = Self::is_ascii_digit_run(integer_digits);
        let is_fraction_part_valid = Self::is_ascii_digit_run(fraction_digits);
        is_integer_part_valid && is_fraction_part_valid
    }

    fn is_ascii_digit_run(text: &str) -> bool {
        let has_digits = !text.is_empty();
        let is_all_digits = text.bytes().all(|digit| digit.is_ascii_digit());
        has_digits && is_all_digits
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

    #[test]
    fn style_keys_are_sorted_and_unique() {
        let is_sorted = StyleKey::ALL
            .windows(2)
            .all(|pair| pair[0].name < pair[1].name);
        assert!(is_sorted);
    }

    #[test]
    fn typed_accessors_read_only_keys_of_their_kind() {
        let element = Element::new("api")
            .with_style("fill", "RED")
            .with_style("stroke-dash", "5")
            .with_style("stroke-width", "2");
        let red = Rgba::from_packed(0xff0000ff);
        assert_eq!(element.style_color(StyleKey::FILL), Some(red));
        assert_eq!(element.style_color(StyleKey::STROKE), None);
        assert_eq!(element.style_color(StyleKey::STROKE_DASH), None);
        assert_eq!(element.style_number(StyleKey::STROKE_DASH), Some(5.0));
        assert_eq!(element.style_number(StyleKey::FILL), None);
        let edge = Edge::new().with_style("stroke", "#14f195");
        let green = Rgba::from_packed(0x14f195ff);
        assert_eq!(edge.style_color(StyleKey::STROKE), Some(green));
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
            (StyleValueKind::Keyword(&FONT_KEYWORDS), "mono", true),
            (StyleValueKind::Keyword(&FONT_KEYWORDS), "serif", false),
            (
                StyleValueKind::Keyword(&TEXT_TRANSFORM_KEYWORDS),
                "Capitalize",
                true,
            ),
            (
                StyleValueKind::Keyword(&TEXT_TRANSFORM_KEYWORDS),
                "title",
                false,
            ),
        ];
        for (value_kind, value, is_expected_accepted) in cases {
            let is_accepted = value_kind.accepts(value);
            assert_eq!(is_accepted, is_expected_accepted, "{value_kind}: {value:?}");
        }
    }
}
