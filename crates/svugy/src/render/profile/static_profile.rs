//! Render for viewers without CSS, script or fonts: colors as palette literals, labels as glyph outlines, no overlays, dots or runtime.

use super::Profile;
use crate::geometry::Label;
use crate::metrics::{BundledFont, TextOutline};
use crate::render::theme::{DARK_PALETTE, Token};
use crate::render::writer::css::CssValue;
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter};

/// Renders the static profile: every token inlined as its dark palette literal, every label outlined in the font it was measured in, live-only content dropped.
pub(crate) struct Static<'f> {
    font: &'f BundledFont,
}

impl<'f> Static<'f> {
    /// Outline labels in `font`, the font they were measured in.
    pub(crate) fn new(font: &'f BundledFont) -> Self {
        Self { font }
    }
}

impl Profile for Static<'_> {
    fn token_value(&self, token: Token) -> CssValue {
        DARK_PALETTE.literal_value(token)
    }

    fn write_label(&self, writer: &mut SvgWriter, label: &Label, presentation: Attributes) {
        let spec = label.text_spec();
        let plane_outline = self
            .font
            .outline(&spec, label.baseline_origin, label.anchor);
        let is_blank = plane_outline.is_empty();
        if is_blank {
            return;
        }
        let screen_outline = label.plane * plane_outline;
        let path_data = PathData(&screen_outline).to_string();
        let outline_attributes = presentation.with_text("d", &path_data);
        writer.empty_element("path", &outline_attributes);
    }

    /// Drop live-only content: a static document has no view to enter and draws an animated dot at its un-animated origin.
    fn write_live_only(
        &self,
        _writer: &mut SvgWriter,
        _write_live_content: impl FnOnce(&mut SvgWriter),
    ) {
    }

    /// Write the content alone: no viewer reads the group's attributes.
    fn write_in_live_group(
        &self,
        writer: &mut SvgWriter,
        _attributes: &Attributes,
        write_content: impl FnOnce(&mut SvgWriter),
    ) {
        write_content(writer);
    }

    /// Write the content alone: a static document carries no style sheet and no script.
    fn embed_in_runtime(&self, writer: &mut SvgWriter, write_content: impl FnOnce(&mut SvgWriter)) {
        write_content(writer);
    }
}
