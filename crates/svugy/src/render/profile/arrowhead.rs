//! Define one arrowhead marker per connection paint, since a marker's fill cannot follow the stroke without context-stroke.

use std::collections::BTreeSet;

use kurbo::Rect;

use svugy_model::graph::WorkflowGraph;

use super::paint::GraphPaints;
use super::{Paint, Profile, ThemeStroke};
use crate::geometry::{ArrowheadMarker, Group};
use crate::render::theme::stroke::Stroke;
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
    /// Collect the paint of every connection of `drawn_layers` ending in an arrowhead, traces and arcs, interiors included, each painted from the graph its layer is drawn from.
    pub(crate) fn collect(drawn_layers: &[(&Group, &WorkflowGraph)]) -> Self {
        let mut connection_paints = BTreeSet::new();
        for &(root, graph) in drawn_layers {
            let paints = GraphPaints::new(graph);
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

    /// Write one marker per collected paint, its arrowhead drawn in the hand of `P`'s theme.
    pub(crate) fn write_markers<P: Profile>(&self, writer: &mut SvgWriter, profile: &P) {
        let view_box = Self::marker_view_box();
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
            let arrowhead_paint = profile.paint_value(paint);
            ThemeStroke::<P>::write_arrowhead(writer, &arrowhead_paint);
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

    /// Name the marker of the paint collected at `marker_index`.
    fn marker_id(marker_index: usize) -> String {
        format!("{MARKER_ID_PREFIX}{marker_index}")
    }
}
