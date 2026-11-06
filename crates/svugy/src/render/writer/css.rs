//! Build CSS text from escaped identifiers and values written from typed colors.

use std::fmt::Write as _;

use svugy_model::color::Rgba;

use super::SvgWriter;

const NONE_KEYWORD: &str = "none";

/// Holds a CSS identifier escaped for a raw selector or property name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssIdentifier {
    escaped: String,
}

/// Holds a CSS value written only from a typed color, `none`, a custom-property reference or static text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssValue {
    text: String,
}

/// Holds a selector built only from static attribute names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssSelector {
    text: String,
}

/// Pairs an escaped property name with a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssDeclaration {
    pub(crate) property: CssIdentifier,
    pub(crate) value: CssValue,
}

/// Builds CSS text from static rules and sanitised selectors and declarations.
#[derive(Default)]
pub(crate) struct StyleSheet {
    css: String,
}

impl CssIdentifier {
    /// Escapes like CSSOM's "serialize an identifier", after stripping what XML 1.0 forbids.
    pub(crate) fn escape(raw: &str) -> Self {
        let characters: Vec<char> = raw.chars().filter(SvgWriter::is_xml_char).collect();
        let is_lone_hyphen = characters == ['-'];
        if is_lone_hyphen {
            let escaped = "\\-".to_owned();
            return CssIdentifier { escaped };
        }
        let starts_with_hyphen = characters.first() == Some(&'-');
        let mut escaped = String::new();
        for (index, &character) in characters.iter().enumerate() {
            let is_control = matches!(character, '\u{1}'..='\u{1F}' | '\u{7F}');
            let is_first_digit = index == 0 && character.is_ascii_digit();
            let is_digit_after_leading_hyphen =
                index == 1 && starts_with_hyphen && character.is_ascii_digit();
            if is_control || is_first_digit || is_digit_after_leading_hyphen {
                let _infallible = write!(escaped, "\\{:x} ", u32::from(character));
                continue;
            }
            let is_non_ascii = character >= '\u{80}';
            let is_name_punctuation = character == '-' || character == '_';
            let is_ascii_alphanumeric = character.is_ascii_alphanumeric();
            if is_non_ascii || is_name_punctuation || is_ascii_alphanumeric {
                escaped.push(character);
                continue;
            }
            escaped.push('\\');
            escaped.push(character);
        }
        CssIdentifier { escaped }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.escaped
    }
}

impl CssValue {
    /// Write `color` as `#rrggbb`, or as `#rrggbbaa` when it is translucent.
    pub(crate) fn color(color: Rgba) -> Self {
        let Rgba { rgb, alpha } = color;
        let is_opaque = color.is_opaque();
        let text = if is_opaque {
            format!("{rgb}")
        } else {
            format!("{rgb}{alpha:02x}")
        };
        CssValue { text }
    }

    pub(crate) fn none() -> Self {
        let text = NONE_KEYWORD.to_owned();
        CssValue { text }
    }

    /// Write a value spelled in the source, such as a font-family stack.
    pub(crate) fn from_static(value: &'static str) -> Self {
        let text = value.to_owned();
        CssValue { text }
    }

    pub(crate) fn custom_property_reference(custom_property: &CssIdentifier) -> Self {
        let text = format!("var({})", custom_property.as_str());
        CssValue { text }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }
}

impl CssSelector {
    /// Select the elements carrying attribute `name`, a static name so no input reaches the selector.
    pub(crate) fn attribute(name: &'static str) -> Self {
        let text = format!("[{name}]");
        CssSelector { text }
    }
}

impl StyleSheet {
    pub(crate) fn new() -> Self {
        StyleSheet::default()
    }

    pub(crate) fn push_static(&mut self, css: &'static str) {
        self.css.push_str(css);
    }

    pub(crate) fn push_rule(&mut self, selector: &CssSelector, declarations: &[CssDeclaration]) {
        self.css.push_str(&selector.text);
        self.css.push('{');
        for (declaration_index, declaration) in declarations.iter().enumerate() {
            if declaration_index > 0 {
                self.css.push(';');
            }
            self.css.push_str(declaration.property.as_str());
            self.css.push(':');
            self.css.push_str(declaration.value.as_str());
        }
        self.css.push_str("}\n");
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.css
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_escaped_for_selectors() {
        let cases = [
            ("block", "block"),
            ("--svugy-canvas", "--svugy-canvas"),
            ("1st", "\\31 st"),
            ("-1", "-\\31 "),
            ("-", "\\-"),
            ("a.b", "a\\.b"),
            ("a b", "a\\ b"),
            ("x{}</style>", "x\\{\\}\\<\\/style\\>"),
            ("tab\tnew\nline", "tab\\9 new\\a line"),
            ("drop\u{0}\u{FFFE}\u{FFFF}me", "dropme"),
            ("café", "café"),
            ("]]>", "\\]\\]\\>"),
        ];
        for (raw, expected) in cases {
            assert_eq!(CssIdentifier::escape(raw).as_str(), expected, "{raw:?}");
        }
    }

    #[test]
    fn colors_are_written_as_six_digit_hex_unless_translucent() {
        let opaque = Rgba::from_packed(0x3478f6ff);
        let translucent = Rgba::from_packed(0x3478f680);
        assert_eq!(CssValue::color(opaque).as_str(), "#3478f6");
        assert_eq!(CssValue::color(translucent).as_str(), "#3478f680");
        assert_eq!(CssValue::none().as_str(), "none");
    }

    #[test]
    fn style_sheet_joins_rules_from_sanitised_parts() {
        let mut style_sheet = StyleSheet::new();
        style_sheet.push_static(":root{color-scheme:light dark}\n");
        let selector = CssSelector::attribute("data-x");
        let fill_property = CssIdentifier::escape("fill");
        let canvas_property = CssIdentifier::escape("--svugy-canvas");
        let fill_value = CssValue::custom_property_reference(&canvas_property);
        let stroke_property = CssIdentifier::escape("stroke");
        let stroke_value = CssValue::none();
        let declarations = [
            CssDeclaration {
                property: fill_property,
                value: fill_value,
            },
            CssDeclaration {
                property: stroke_property,
                value: stroke_value,
            },
        ];
        style_sheet.push_rule(&selector, &declarations);
        let expected =
            ":root{color-scheme:light dark}\n[data-x]{fill:var(--svugy-canvas);stroke:none}\n";
        assert_eq!(style_sheet.as_str(), expected);
    }
}
