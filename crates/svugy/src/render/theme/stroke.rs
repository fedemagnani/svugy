//! Draw paths in a theme's hand.

use kurbo::{Affine, BezPath, RoundedRect};

use crate::geometry::ArrowheadMarker;
use crate::render::writer::css::CssValue;
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter};

/// Strokes every outline, connection and arrowhead a theme draws, in that theme's hand.
pub(crate) trait Stroke {
    /// Redraw `path`, a connection or its legend sample, in this hand.
    fn redraw_connection(path: &BezPath) -> BezPath;

    /// Widen an outline laid out `laid_out_width` wide to the width this hand strokes it at.
    fn outline_width(laid_out_width: f64) -> f64;

    /// Write an area and its outline: `write_area` writes the area carrying the presentation it is handed, `outline` is stroked in `outline_presentation`.
    fn write_outlined_area(
        writer: &mut SvgWriter,
        write_area: impl FnOnce(&mut SvgWriter, Attributes),
        outline: &BezPath,
        outline_presentation: Attributes,
    );

    /// Write a label plate and its outline: `write_plate` writes the plate, on `plane`, carrying the presentation it is handed, `plate`'s outline is stroked on `plane` in `outline_presentation`.
    fn write_outlined_plate(
        writer: &mut SvgWriter,
        write_plate: impl FnOnce(&mut SvgWriter, Attributes),
        plate: RoundedRect,
        plane: Affine,
        outline_presentation: Attributes,
    );

    /// Write an arrowhead in `paint`, in its marker's box.
    fn write_arrowhead(writer: &mut SvgWriter, paint: &CssValue);
}

/// Draws every path exactly as laid out.
pub(crate) struct Crisp;

impl Stroke for Crisp {
    fn redraw_connection(path: &BezPath) -> BezPath {
        path.clone()
    }

    fn outline_width(laid_out_width: f64) -> f64 {
        laid_out_width
    }

    /// Write the area alone, carrying its own outline: a crisp outline is the area's edge.
    fn write_outlined_area(
        writer: &mut SvgWriter,
        write_area: impl FnOnce(&mut SvgWriter, Attributes),
        _outline: &BezPath,
        outline_presentation: Attributes,
    ) {
        write_area(writer, outline_presentation);
    }

    /// Write the plate alone, carrying its own outline: a crisp outline is the plate's edge.
    fn write_outlined_plate(
        writer: &mut SvgWriter,
        write_plate: impl FnOnce(&mut SvgWriter, Attributes),
        _plate: RoundedRect,
        _plane: Affine,
        outline_presentation: Attributes,
    ) {
        write_plate(writer, outline_presentation);
    }

    /// Write a filled triangle, back top to tip to back bottom.
    fn write_arrowhead(writer: &mut SvgWriter, paint: &CssValue) {
        let [back_top, tip, back_bottom] = ArrowheadMarker::TRIANGLE;
        let mut triangle = BezPath::new();
        triangle.move_to(back_top);
        triangle.line_to(tip);
        triangle.line_to(back_bottom);
        triangle.close_path();
        let triangle_data = PathData(&triangle).to_string();
        let arrowhead_attributes = Attributes::new()
            .with_text("d", &triangle_data)
            .with_css_value("fill", paint);
        writer.empty_element("path", &arrowhead_attributes);
    }
}
