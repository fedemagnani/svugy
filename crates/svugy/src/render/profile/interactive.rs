//! Render for browsers: colors as custom properties of a light and a dark palette, labels as text in the viewer's font, the runtime embedded.

use kurbo::Affine;

use super::Profile;
use crate::geometry::Label;
use crate::metrics::{BundledFont, TextAnchor};
use crate::render::contract::RuntimeContract;
use crate::render::theme::{DARK_PALETTE, LIGHT_PALETTE, Token};
use crate::render::writer::css::{
    CssDeclaration, CssIdentifier, CssSelector, CssValue, StyleSheet,
};
use crate::render::writer::{Attributes, Matrix, SvgWriter};

const LABEL_FONT_FAMILY_STACK: &str =
    "Inter, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif";
const LIGHT_SCHEME_OPENING: &str = "@media (prefers-color-scheme: light){\n";
const MEDIA_CLOSING: &str = "}\n";

/// Writes tokens as custom properties, dark unless the viewer prefers light, sets labels as viewer-font text, and embeds the runtime that navigates levels.
#[derive(Default)]
pub(crate) struct Interactive;

impl Interactive {
    pub(crate) const RUNTIME_SCRIPT: &'static str = include_str!("../../../runtime/svugy.js");
    pub(crate) const RUNTIME_STYLE: &'static str = include_str!("../../../runtime/svugy.css");

    /// Build the style sheet: both palettes as custom properties of the root, the label font inherited from it, then the runtime's rules.
    fn style_sheet() -> StyleSheet {
        let root = CssSelector::attribute(RuntimeContract::CONTRACT);
        let font_family_property = CssIdentifier::escape("font-family");
        let font_family_stack = CssValue::from_static(LABEL_FONT_FAMILY_STACK);
        let font_family = CssDeclaration {
            property: font_family_property,
            value: font_family_stack,
        };
        let mut dark_declarations = DARK_PALETTE.custom_property_declarations();
        dark_declarations.push(font_family);
        let light_declarations = LIGHT_PALETTE.custom_property_declarations();
        let mut style_sheet = StyleSheet::new();
        style_sheet.push_rule(&root, &dark_declarations);
        style_sheet.push_static(LIGHT_SCHEME_OPENING);
        style_sheet.push_rule(&root, &light_declarations);
        style_sheet.push_static(MEDIA_CLOSING);
        style_sheet.push_static(Self::RUNTIME_STYLE);
        style_sheet
    }
}

impl Profile for Interactive {
    fn token_value(&self, token: Token) -> CssValue {
        let custom_property = token.css_custom_property();
        CssValue::custom_property_reference(&custom_property)
    }

    /// Set `label` as `<text>` in the font family it was measured in, its `textLength` pinned to the measured width so a drifting viewer font still fits.
    fn write_label(
        &self,
        writer: &mut SvgWriter,
        label: &Label,
        _font: &BundledFont,
        presentation: Attributes,
    ) {
        let spec = label.text_spec();
        let anchor_keyword = label.anchor.svg_keyword();
        let font_weight = f64::from(spec.weight);
        let mut text_attributes = presentation
            .with_number("x", label.baseline_origin.x)
            .with_number("y", label.baseline_origin.y)
            .with_text("text-anchor", anchor_keyword)
            .with_number("font-size", spec.size)
            .with_number("font-weight", font_weight)
            .with_number("textLength", label.size.width);
        let is_letter_spaced = spec.letter_spacing != 0.0;
        if is_letter_spaced {
            text_attributes = text_attributes.with_number("letter-spacing", spec.letter_spacing);
        }
        let is_on_projected_plane = label.plane != Affine::IDENTITY;
        if is_on_projected_plane {
            let plane_transform = Matrix(label.plane).to_string();
            text_attributes = text_attributes.with_text("transform", &plane_transform);
        }
        writer.open_element("text", &text_attributes);
        writer.text(&label.text);
        writer.close_element();
    }

    fn write_live_only(
        &self,
        writer: &mut SvgWriter,
        write_live_content: impl FnOnce(&mut SvgWriter),
    ) {
        write_live_content(writer);
    }

    /// Write the style sheet first and the runtime script last, so the script starts on a fully parsed document.
    fn embed_in_runtime(&self, writer: &mut SvgWriter, write_content: impl FnOnce(&mut SvgWriter)) {
        let style_sheet = Self::style_sheet();
        writer.style_element(&style_sheet);
        write_content(writer);
        writer.script_element(Self::RUNTIME_SCRIPT);
    }
}

impl TextAnchor {
    /// Spell the anchor as SVG's `text-anchor` keyword.
    fn svg_keyword(self) -> &'static str {
        match self {
            TextAnchor::Start => "start",
            TextAnchor::Middle => "middle",
        }
    }
}
