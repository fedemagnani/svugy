//! Tile the interactive document's ground without end.

use kurbo::{Rect, Shape as _};

use super::{Paint, Profile};
use crate::geometry::GroundPattern;
use crate::render::theme::Token;
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter};

/// Name the ground pattern, a synthetic `<defs>` target.
const GROUND_PATTERN_ID: &str = "ground-grid";
/// Reference the ground pattern as a fill.
const GROUND_PATTERN_FILL: &str = "url(#ground-grid)";
/// Reach this far from the origin on every side, beyond any view a reader pans or zooms to.
const GROUND_HALF_EXTENT: f64 = 1_000_000.0;
/// Draw grid lines this thick on the page, as thick as the finite grid's.
const GRID_LINE_PAGE_WIDTH: f64 = 1.0;
/// Flatten the cell outline within this tolerance; its edges are straight, so any tolerance draws them exactly.
const CELL_OUTLINE_TOLERANCE: f64 = 0.1;

/// Tiles the ground without end: one projected cell as a pattern among the shared definitions, painted over a square far larger than any view.
pub(super) struct EndlessGround {
    pattern: GroundPattern,
}

impl EndlessGround {
    /// Tile the grid's own cell, projected onto the ground plane.
    pub(super) fn new() -> Self {
        let pattern = GroundPattern::projected();
        Self { pattern }
    }

    /// Write the pattern: one cell with its four edges ruled, the tile clipping each to half its width so neighbouring tiles complete it.
    pub(super) fn write_pattern<P: Profile>(&self, writer: &mut SvgWriter, profile: &P) {
        let cell = self.pattern.cell;
        let pattern_attributes = Attributes::new()
            .with_text("id", GROUND_PATTERN_ID)
            .with_text("patternUnits", "userSpaceOnUse")
            .with_number("width", cell)
            .with_number("height", cell)
            .with_pattern_transform(self.pattern.plan_to_page);
        writer.open_element("pattern", &pattern_attributes);
        let cell_square = Rect::new(0.0, 0.0, cell, cell);
        let cell_outline = cell_square.to_path(CELL_OUTLINE_TOLERANCE);
        let outline_data = PathData(&cell_outline).to_string();
        let grid_paint = Paint::Token(Token::GridLine);
        let grid_stroke = profile.paint_value(grid_paint);
        let stroke_width = self.pattern.stroke_width_for(GRID_LINE_PAGE_WIDTH);
        let outline_attributes = Attributes::new()
            .with_text("d", &outline_data)
            .with_text("fill", "none")
            .with_css_value("stroke", &grid_stroke)
            .with_number("stroke-width", stroke_width);
        writer.empty_element("path", &outline_attributes);
        writer.close_element();
    }

    /// Paint the ground: a square reaching `GROUND_HALF_EXTENT` from the origin on every side, filled with the pattern.
    pub(super) fn write_plane(writer: &mut SvgWriter) {
        let side = 2.0 * GROUND_HALF_EXTENT;
        let plane_attributes = Attributes::new()
            .with_number("x", -GROUND_HALF_EXTENT)
            .with_number("y", -GROUND_HALF_EXTENT)
            .with_number("width", side)
            .with_number("height", side)
            .with_text("fill", GROUND_PATTERN_FILL);
        writer.empty_element("rect", &plane_attributes);
    }
}
