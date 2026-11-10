//! Build CSS text from static identifiers and values written from typed colors.

use svugy_model::color::Rgba;

/// Spell each six-bit value as its standard base64 digit.
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Builds a style sheet from static rules and sanitised selectors and declarations.
#[derive(Default)]
pub(crate) struct StyleSheet {
    css: String,
}

impl StyleSheet {
    /// Start an empty style sheet.
    pub(crate) fn new() -> Self {
        StyleSheet::default()
    }

    /// Push `css` spelled in the source, as is.
    pub(crate) fn push_static(&mut self, css: &'static str) {
        self.css.push_str(css);
    }

    /// Push one rule: `selector`, then its `declarations`.
    pub(crate) fn push_rule(&mut self, selector: &CssSelector, declarations: &[CssDeclaration]) {
        self.css.push_str(&selector.text);
        self.css.push('{');
        for (declaration_index, declaration) in declarations.iter().enumerate() {
            if declaration_index > 0 {
                self.css.push(';');
            }
            let property = declaration.property.as_str();
            let value = declaration.value.as_str();
            self.css.push_str(property);
            self.css.push(':');
            self.css.push_str(value);
        }
        self.css.push_str("}\n");
    }

    /// Push one rule applied only while media `query` matches, as `@media query{selector{…}}`.
    pub(crate) fn push_media_rule(
        &mut self,
        query: &'static str,
        selector: &CssSelector,
        declarations: &[CssDeclaration],
    ) {
        self.css.push_str("@media ");
        self.css.push_str(query);
        self.css.push_str("{\n");
        self.push_rule(selector, declarations);
        self.css.push_str("}\n");
    }

    /// Push an `@font-face` rule declaring `woff2` as `family` at every weight, so the browser never synthesizes bold from the single face.
    pub(crate) fn push_font_face(&mut self, family: &'static str, woff2: &[u8]) {
        self.css.push_str("@font-face{font-family:");
        self.css.push_str(family);
        self.css.push_str(";src:url(data:font/woff2;base64,");
        self.push_base64(woff2);
        self.css
            .push_str(") format(\"woff2\");font-weight:100 900;font-display:block}\n");
    }

    /// Push `bytes` as padded base64: every 3 bytes as 4 six-bit digits, a short last group padded with `=`.
    fn push_base64(&mut self, bytes: &[u8]) {
        let digit_count = bytes.len().div_ceil(3) * 4;
        self.css.reserve(digit_count);
        for group in bytes.chunks(3) {
            let mut padded_group = [0u8; 3];
            padded_group[..group.len()].copy_from_slice(group);
            let [first_byte, second_byte, third_byte] = padded_group;
            let packed_group = u32::from_be_bytes([0, first_byte, second_byte, third_byte]);
            let significant_digit_count = group.len() + 1;
            for digit_index in 0..4 {
                let is_padding = digit_index >= significant_digit_count;
                if is_padding {
                    self.css.push('=');
                    continue;
                }
                let shift = 18 - 6 * digit_index;
                let sextet = (packed_group >> shift) & 0x3f;
                let digit = BASE64_ALPHABET[sextet as usize];
                self.css.push(char::from(digit));
            }
        }
    }

    /// Borrow the style sheet's CSS as a str.
    pub(crate) fn as_str(&self) -> &str {
        &self.css
    }
}

/// Holds a selector built only from static attribute names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssSelector {
    text: String,
}

impl CssSelector {
    /// Select the elements carrying attribute `name`, a static name so no input reaches the selector.
    pub(crate) fn attribute(name: &'static str) -> Self {
        let text = format!("[{name}]");
        CssSelector { text }
    }
}

/// Pairs a static property name with a value as one declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssDeclaration {
    pub(crate) property: CssIdentifier,
    pub(crate) value: CssValue,
}

/// Holds a CSS identifier spelled in the source, so no input reaches a selector or property name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssIdentifier {
    name: &'static str,
}

impl CssIdentifier {
    /// Take identifier `name`, a static name so no input reaches the CSS.
    pub(crate) fn from_static(name: &'static str) -> Self {
        CssIdentifier { name }
    }

    /// Borrow the identifier as a str.
    pub(crate) fn as_str(&self) -> &str {
        self.name
    }
}

/// Holds a CSS value written only from a typed color, a custom-property reference or static text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssValue {
    text: String,
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

    /// Write a value spelled in the source, such as a font-family stack.
    pub(crate) fn from_static(value: &'static str) -> Self {
        let text = value.to_owned();
        CssValue { text }
    }

    /// Reference `custom_property` as `var(..)`.
    pub(crate) fn custom_property_reference(custom_property: &CssIdentifier) -> Self {
        let custom_property_name = custom_property.as_str();
        let text = format!("var({custom_property_name})");
        CssValue { text }
    }

    /// Borrow the value's CSS text as a str.
    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_face_embeds_woff2_as_padded_base64_for_every_weight() {
        let woff2_magic_and_one_byte = [b'w', b'O', b'F', b'2', 0xff];
        let mut style_sheet = StyleSheet::new();

        style_sheet.push_font_face("Excalifont", &woff2_magic_and_one_byte);

        let expected = "@font-face{font-family:Excalifont;src:url(data:font/woff2;base64,d09GMv8=) format(\"woff2\");font-weight:100 900;font-display:block}\n";
        assert_eq!(style_sheet.as_str(), expected);
    }

    #[test]
    fn colors_are_written_as_six_digit_hex_unless_translucent() {
        let opaque = Rgba::from_packed(0x3478f6ff);
        let translucent = Rgba::from_packed(0x3478f680);
        assert_eq!(CssValue::color(opaque).as_str(), "#3478f6");
        assert_eq!(CssValue::color(translucent).as_str(), "#3478f680");
    }

    #[test]
    fn style_sheet_joins_rules_from_sanitised_parts() {
        let mut style_sheet = StyleSheet::new();
        style_sheet.push_static(":root{color-scheme:light dark}\n");
        let selector = CssSelector::attribute("data-x");
        let fill_property = CssIdentifier::from_static("fill");
        let canvas_property = CssIdentifier::from_static("--svugy-canvas");
        let fill_value = CssValue::custom_property_reference(&canvas_property);
        let stroke_property = CssIdentifier::from_static("stroke");
        let stroke_value = CssValue::from_static("none");
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
