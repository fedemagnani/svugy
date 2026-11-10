//! Draw paths as a hand-held pencil would.

use kurbo::{Affine, BezPath, Point, RoundedRect};

use crate::render::theme::stroke::Stroke;
use crate::render::writer::css::CssValue;
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter};
use crate::sketch::{Pencil, Seed};

/// Draws every path as a hand-held pencil would: jittered, bowed, outlines traced twice.
pub(crate) struct Sketched;

impl Sketched {
    /// Trace face and sheet outlines.
    const OUTLINE_PENCIL: Pencil = Pencil::new(1.0, 1.0, 2.0);
    /// Trace connections, once so their dashes stay one train.
    const CONNECTION_PENCIL: Pencil = Pencil::new(0.8, 1.0, 2.0);
    /// Trace label plates, small enough that a full tremor would wreck them.
    const PLATE_PENCIL: Pencil = Pencil::new(0.6, 0.5, 2.0);
    /// Trace arrowheads, in marker units.
    const ARROWHEAD_PENCIL: Pencil = Pencil::new(0.5, 0.5, 2.0);
    /// Narrowest outline the pencil draws.
    const MIN_OUTLINE_WIDTH: f64 = 1.25;
    /// Width of an arrowhead's strokes, in marker units.
    const ARROWHEAD_STROKE_WIDTH: f64 = 1.4;
    /// Open chevron corners in marker units: the upper back end, the tip, the lower back end.
    const CHEVRON: [Point; 3] = [
        Point::new(1.0, 1.5),
        Point::new(8.5, 5.0),
        Point::new(1.0, 8.5),
    ];

    /// Write `sketched_outline` unfilled, stroked in `outline_presentation`, its joins and ends rounded like pencil marks.
    fn write_pencil_marks(
        writer: &mut SvgWriter,
        sketched_outline: &BezPath,
        outline_presentation: Attributes,
    ) {
        let outline_data = PathData(sketched_outline).to_string();
        let outline_attributes = outline_presentation
            .with_text("d", &outline_data)
            .with_text("fill", "none")
            .with_text("stroke-linecap", "round")
            .with_text("stroke-linejoin", "round");
        writer.empty_element("path", &outline_attributes);
    }

    /// Outline `plate` with straight sides and quadratic corners, so no arc reaches std trig.
    fn rounded_outline_of(plate: RoundedRect) -> BezPath {
        let rect = plate.rect();
        let radius = plate.radii().top_left;
        let mut outline = BezPath::new();
        outline.move_to((rect.x0 + radius, rect.y0));
        outline.line_to((rect.x1 - radius, rect.y0));
        outline.quad_to((rect.x1, rect.y0), (rect.x1, rect.y0 + radius));
        outline.line_to((rect.x1, rect.y1 - radius));
        outline.quad_to((rect.x1, rect.y1), (rect.x1 - radius, rect.y1));
        outline.line_to((rect.x0 + radius, rect.y1));
        outline.quad_to((rect.x0, rect.y1), (rect.x0, rect.y1 - radius));
        outline.line_to((rect.x0, rect.y0 + radius));
        outline.quad_to((rect.x0, rect.y0), (rect.x0 + radius, rect.y0));
        outline
    }
}

impl Stroke for Sketched {
    fn redraw_connection(path: &BezPath) -> BezPath {
        let seed = Seed::from_path(path);
        Self::CONNECTION_PENCIL.sketch_path_once(path, seed)
    }

    /// Widen to at least the narrowest pencil outline: max(laid_out_width, 1.25).
    fn outline_width(laid_out_width: f64) -> f64 {
        laid_out_width.max(Self::MIN_OUTLINE_WIDTH)
    }

    /// Write the area bare, then its outline traced twice over it.
    fn write_outlined_area(
        writer: &mut SvgWriter,
        write_area: impl FnOnce(&mut SvgWriter, Attributes),
        outline: &BezPath,
        outline_presentation: Attributes,
    ) {
        let bare_presentation = Attributes::new();
        write_area(writer, bare_presentation);
        let seed = Seed::from_path(outline);
        let sketched_outline = Self::OUTLINE_PENCIL.sketch_path(outline, seed);
        Self::write_pencil_marks(writer, &sketched_outline, outline_presentation);
    }

    /// Write the plate bare, then its rounded outline traced once over it.
    fn write_outlined_plate(
        writer: &mut SvgWriter,
        write_plate: impl FnOnce(&mut SvgWriter, Attributes),
        plate: RoundedRect,
        plane: Affine,
        outline_presentation: Attributes,
    ) {
        let bare_presentation = Attributes::new();
        write_plate(writer, bare_presentation);
        let outline = Self::rounded_outline_of(plate);
        let seed = Seed::from_path(&outline);
        let sketched_outline = Self::PLATE_PENCIL.sketch_path_once(&outline, seed);
        let planar_presentation = outline_presentation.with_transform(plane);
        Self::write_pencil_marks(writer, &sketched_outline, planar_presentation);
    }

    /// Write an open chevron traced once.
    fn write_arrowhead(writer: &mut SvgWriter, paint: &CssValue) {
        let [back_top, tip, back_bottom] = Self::CHEVRON;
        let mut chevron = BezPath::new();
        chevron.move_to(back_top);
        chevron.line_to(tip);
        chevron.line_to(back_bottom);
        let seed = Seed::from_path(&chevron);
        let sketched_chevron = Self::ARROWHEAD_PENCIL.sketch_path_once(&chevron, seed);
        let chevron_presentation = Attributes::new()
            .with_css_value("stroke", paint)
            .with_number("stroke-width", Self::ARROWHEAD_STROKE_WIDTH);
        Self::write_pencil_marks(writer, &sketched_chevron, chevron_presentation);
    }
}
