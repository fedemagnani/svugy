//! Define one arrowhead marker per connection paint, since a marker's fill cannot follow the stroke without context-stroke.

use std::collections::BTreeSet;

use kurbo::{BezPath, Rect};

use svugy_model::model::Model;

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
    /// Collect the paint of every connection of `drawn_layers` ending in an arrowhead, traces and arcs, interiors included, each painted from the model its layer is drawn from.
    pub(crate) fn collect(drawn_layers: &[(&Group, &Model)]) -> Self {
        let mut connection_paints = BTreeSet::new();
        for &(root, model) in drawn_layers {
            let paints = ModelPaints::new(model);
            let levels = root.this_and_nested_levels();
            let connections =
                levels.flat_map(|(group, _to_root)| group.traces.iter().chain(&group.connections));
            let headed_connections = connections.filter(|connection| connection.ends_in_arrowhead);
            let layer_paints =
                headed_connections.map(|connection| paints.connection(&connection.edge));
            connection_paints.extend(layer_paints);
        }
        Self {
            paints: connection_paints,
        }
    }

    /// Write one marker per collected paint.
    pub(crate) fn write_markers<P: Profile>(&self, writer: &mut SvgWriter, profile: &P) {
        let view_box = Self::marker_view_box();
        let triangle_path_data = Self::triangle_path_data();
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

    /// Tell whether no connection was collected.
    pub(crate) fn is_empty(&self) -> bool {
        self.paints.is_empty()
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
