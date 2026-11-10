//! Shade face areas in a theme's manner.

use super::paint::Paint;
use crate::geometry::Face;
use crate::render::writer::css::CssValue;
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter};

/// Shades a face's area, and keeps the labels set on it legible.
pub(crate) trait Shading {
    /// Write `face`'s area shaded `shade`, the area itself carrying `area_presentation`, every paint written by `paint_value`.
    fn write_area(
        writer: &mut SvgWriter,
        face: &Face,
        shade: Paint,
        area_presentation: Attributes,
        paint_value: impl Fn(Paint) -> CssValue,
    );

    /// Ink a label lying on a shaded face, its solid inking it in `solid_ink`.
    fn label_ink(solid_ink: Paint) -> Paint;

    /// Halo a label lying on a shaded face, so nothing drawn on the face runs through its letters.
    fn halo_label(presentation: Attributes, paint_value: impl Fn(Paint) -> CssValue) -> Attributes;
}

/// Fills every face flat in its shade.
pub(crate) struct Flat;

impl Shading for Flat {
    fn write_area(
        writer: &mut SvgWriter,
        face: &Face,
        shade: Paint,
        area_presentation: Attributes,
        paint_value: impl Fn(Paint) -> CssValue,
    ) {
        let outline_data = PathData(&face.outline).to_string();
        let shade_fill = paint_value(shade);
        let area_attributes = area_presentation
            .with_text("d", &outline_data)
            .with_css_value("fill", &shade_fill);
        writer.empty_element("path", &area_attributes);
    }

    /// Keep the solid's ink, chosen for the very shade the face is filled with.
    fn label_ink(solid_ink: Paint) -> Paint {
        solid_ink
    }

    /// Leave the label bare: a flat face draws nothing under it.
    fn halo_label(
        presentation: Attributes,
        _paint_value: impl Fn(Paint) -> CssValue,
    ) -> Attributes {
        presentation
    }
}
