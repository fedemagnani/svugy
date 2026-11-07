//! Define one arrowhead marker per connection paint, since a marker's fill cannot follow the stroke without context-stroke.

use std::collections::BTreeSet;

use kurbo::{BezPath, Rect};

use super::Profile;
use super::paint::{ModelPaints, Paint};
use crate::geometry::{ArrowheadMarker, Group};
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter, ViewBox};

/// Prefix every marker's id, followed by its paint's index.
const MARKER_ID_PREFIX: &str = "arrowhead-";
/// Orient each marker along its path, reversed at a path's start.
const MARKER_ORIENTATION: &str = "auto-start-reverse";

/// Defines the arrowhead markers of every paint a scene's connections are stroked with.
pub(crate) struct ArrowheadMarkers {
    paints: BTreeSet<Paint>,
}

impl ArrowheadMarkers {
    /// Collect the paint of every connection, interiors included, since each ends in an arrowhead.
    pub(crate) fn collect(root: &Group, paints: &ModelPaints) -> Self {
        let levels = root.this_and_nested_levels();
        let connections = levels.flat_map(|(group, _to_root)| &group.connections);
        let connection_paints = connections
            .map(|connection| paints.connection(&connection.edge))
            .collect();
        Self {
            paints: connection_paints,
        }
    }

    /// Write a `<defs>` holding one marker per collected paint, or nothing when none was collected.
    pub(crate) fn write_defs<P: Profile>(&self, writer: &mut SvgWriter, profile: &P) {
        let has_no_marker = self.paints.is_empty();
        if has_no_marker {
            return;
        }
        let no_attributes = Attributes::new();
        let view_box = Self::marker_view_box();
        let triangle_path_data = Self::triangle_path_data();
        writer.open_element("defs", &no_attributes);
        for (marker_index, &paint) in self.paints.iter().enumerate() {
            let marker_id = Self::marker_id(marker_index);
            let marker_attributes = Attributes::new()
                .with_text("id", &marker_id)
                .with_text("viewBox", &view_box)
                .with_number("refX", ArrowheadMarker::REFERENCE.x)
                .with_number("refY", ArrowheadMarker::REFERENCE.y)
                .with_number("markerWidth", ArrowheadMarker::SIZE)
                .with_number("markerHeight", ArrowheadMarker::SIZE)
                .with_text("orient", MARKER_ORIENTATION);
            writer.open_element("marker", &marker_attributes);
            let fill = profile.paint_value(paint);
            let arrowhead_attributes = Attributes::new()
                .with_text("d", &triangle_path_data)
                .with_css_value("fill", &fill);
            writer.empty_element("path", &arrowhead_attributes);
            writer.close_element();
        }
        writer.close_element();
    }

    /// Reference the marker of `paint` as a `marker-end` value.
    pub(crate) fn marker_reference(&self, paint: Paint) -> String {
        let collected_paints_before = self.paints.range(..paint);
        let marker_index = collected_paints_before.count();
        let is_collected = self.paints.contains(&paint);
        debug_assert!(is_collected, "uncollected paint {paint:?}");
        let marker_id = Self::marker_id(marker_index);
        format!("url(#{marker_id})")
    }

    /// Spell the marker's square viewBox, `0 0 size size`.
    fn marker_view_box() -> String {
        let size = ArrowheadMarker::VIEW_BOX_SIZE;
        let marker_square = Rect::new(0.0, 0.0, size, size);
        ViewBox(marker_square).to_string()
    }

    /// Spell the triangle as path data, back top to tip to back bottom, closed.
    fn triangle_path_data() -> String {
        let [back_top, tip, back_bottom] = ArrowheadMarker::TRIANGLE;
        let mut triangle = BezPath::new();
        triangle.move_to(back_top);
        triangle.line_to(tip);
        triangle.line_to(back_bottom);
        triangle.close_path();
        PathData(&triangle).to_string()
    }

    /// Name the marker of the paint collected at `marker_index`.
    fn marker_id(marker_index: usize) -> String {
        format!("{MARKER_ID_PREFIX}{marker_index}")
    }
}
