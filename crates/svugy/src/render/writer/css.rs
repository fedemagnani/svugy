//! Build CSS text from static identifiers and values written from typed colors.

use svugy_model::color::Rgba;

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
