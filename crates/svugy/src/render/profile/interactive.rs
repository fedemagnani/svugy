//! Render for browsers: colors as custom properties of a light and a dark palette, labels as text in the viewer's font, the runtime embedded.

use std::marker::PhantomData;

use super::Profile;
use crate::geometry::Label;
use crate::metrics::{FaceSet, TextAnchor};
use crate::render::contract::RuntimeContract;
use crate::render::theme::{Theme, Token};
use crate::render::writer::css::{
    CssDeclaration, CssIdentifier, CssSelector, CssValue, StyleSheet,
};
use crate::render::writer::{Attributes, SvgWriter};

/// Match viewers who prefer the light color scheme.
const LIGHT_SCHEME_QUERY: &str = "(prefers-color-scheme: light)";

/// Renders the interactive profile in theme `T`: tokens as custom properties, dark unless the viewer prefers light, labels as viewer-font text, the runtime that navigates levels embedded.
pub(crate) struct Interactive<T> {
    theme: PhantomData<T>,
}

/// Embeds the runtime that drives every interactive document: its script and its style sheet.
pub(crate) struct Runtime;

impl<T: Theme> Interactive<T> {
    /// Render the interactive profile in theme `T`.
    pub(crate) fn new() -> Self {
        let theme = PhantomData;
        Self { theme }
    }
}

impl<T: Theme> Profile for Interactive<T> {
    type Theme = T;

    fn token_value(&self, token: Token) -> CssValue {
        let custom_property = token.css_custom_property();
        CssValue::custom_property_reference(&custom_property)
    }

    /// Set `label` as `<text>` in the font family it was measured in, its `textLength` pinned to the measured width so a drifting viewer font still fits.
    fn write_label(&self, writer: &mut SvgWriter, label: &Label, presentation: Attributes) {
        let spec = label.text_spec();
        let anchor_keyword = label.anchor.svg_keyword();
        let font_weight = f64::from(spec.weight);
        let mut text_attributes = presentation
            .with_number("x", label.baseline_origin.x)
            .with_number("y", label.baseline_origin.y)
            .with_text("text-anchor", anchor_keyword)
            .with_number("font-size", spec.size)
            .with_number("font-weight", font_weight)
            .with_number("textLength", label.size.width)
            .with_transform(label.plane);
        let is_letter_spaced = spec.letter_spacing != 0.0;
        if is_letter_spaced {
            text_attributes = text_attributes.with_number("letter-spacing", spec.letter_spacing);
        }
        writer.text_element("text", &text_attributes, &label.text);
    }

    fn write_live_only(
        &self,
        writer: &mut SvgWriter,
        write_live_content: impl FnOnce(&mut SvgWriter),
    ) {
        write_live_content(writer);
    }

    fn write_in_live_group(
        &self,
        writer: &mut SvgWriter,
        attributes: &Attributes,
        write_content: impl FnOnce(&mut SvgWriter),
    ) {
        writer.open_element("g", attributes);
        write_content(writer);
        writer.close_element();
    }

    /// Write the style sheet first and the runtime script last, so the script starts on a fully parsed document.
    fn embed_in_runtime(&self, writer: &mut SvgWriter, write_content: impl FnOnce(&mut SvgWriter)) {
        let style_sheet = Self::style_sheet();
        writer.style_element(&style_sheet);
        write_content(writer);
        writer.script_element(Runtime::SCRIPT);
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

impl<T: Theme> Interactive<T> {
    /// Build the style sheet: `T`'s embedded label face if any, both of `T`'s palettes as custom properties of the root, `T`'s label font inherited from it, then the runtime's rules.
    fn style_sheet() -> StyleSheet {
        let root = CssSelector::attribute(RuntimeContract::CONTRACT);
        let font_family_property = CssIdentifier::from_static("font-family");
        let font_family_stack = CssValue::from_static(T::Faces::CSS_FONT_FAMILY_STACK);
        let font_family = CssDeclaration {
            property: font_family_property,
            value: font_family_stack,
        };
        let mut dark_declarations = T::DARK_PALETTE.custom_property_declarations();
        dark_declarations.push(font_family);
        let light_declarations = T::LIGHT_PALETTE.custom_property_declarations();
        let mut style_sheet = StyleSheet::new();
        if let Some(woff2) = T::Faces::EMBEDDED_WOFF2 {
            let embedded_family = T::Faces::first_css_family();
            style_sheet.push_font_face(embedded_family, woff2);
        }
        style_sheet.push_rule(&root, &dark_declarations);
        style_sheet.push_media_rule(LIGHT_SCHEME_QUERY, &root, &light_declarations);
        style_sheet.push_static(Runtime::STYLE);
        style_sheet
    }
}

impl Runtime {
    /// Embed the runtime script that navigates levels.
    pub(crate) const SCRIPT: &'static str = include_str!("../../../runtime/svugy.js");
    /// Embed the runtime style sheet.
    pub(crate) const STYLE: &'static str = include_str!("../../../runtime/svugy.css");
}
