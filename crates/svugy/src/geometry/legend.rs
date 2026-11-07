//! Set the diagram's legend top-right of the top-level view, its key rows sampled from what the diagram draws.

use kurbo::{Affine, Line, Point, Rect, Vec2};
use svugy_model::legend::Legend;
use svugy_model::model::{EdgeId, Model};
use svugy_model::path::ElementPath;

use super::badge::BadgeIcon;
use super::obstruction::Obstructions;
use super::{Connection, Face, Group, Heading, KeyRow, KeySample, Label, LegendPanel};
use crate::layout::LevelPlans;
use crate::metrics::{LabelRole, TextAnchor, TextMeasure};

impl LegendPanel {
    /// Length of a stroke sample, from the panel's left edge.
    const SAMPLE_LENGTH: f64 = 30.0;

    /// Set `model`'s legend top-right of `root`'s view, beside `heading`, else just right of the content when drawables take that spot, or `None` when it has nothing to draw.
    pub(super) fn set_top_right_of(
        root: &Group,
        heading: Option<&Heading>,
        model: &Model,
        plans: &LevelPlans,
        measure: &impl TextMeasure,
    ) -> Option<Self> {
        let legend = model.legend()?;
        let drawn_dashes = DrawnDash::all_described_in(legend, model);
        let drawn_badges = DrawnBadge::all_described_in(legend, model, plans);
        let unplaced_panel = Self::set_from(legend, &drawn_dashes, &drawn_badges, measure)?;
        let unplaced_bounds = unplaced_panel.bounds;
        let legend_offset = Self::offset_to_top_right_of(root, heading, unplaced_bounds);
        unplaced_panel.translated(legend_offset)
    }

    /// Set `legend`'s title, entries and the rows of `drawn_dashes` then `drawn_badges`, one under the other from the origin, or `None` when they draw nothing.
    fn set_from(
        legend: &Legend,
        drawn_dashes: &[DrawnDash],
        drawn_badges: &[DrawnBadge],
        measure: &impl TextMeasure,
    ) -> Option<Self> {
        let mut lines = LineStack::new();
        let mut title = None;
        if let Some(title_text) = legend.title() {
            let title_line = lines.set_title(title_text, measure);
            title = Some(title_line);
        }
        let mut entries = Vec::new();
        for entry_text in legend.entries() {
            let entry = lines.set_entry(entry_text, measure);
            entries.push(entry);
        }
        let mut key_rows = KeyRow::sampling(drawn_dashes, &mut lines, measure);
        let badge_key_rows = KeyRow::sampling(drawn_badges, &mut lines, measure);
        key_rows.extend(badge_key_rows);
        Self::bounding(title, entries, key_rows)
    }

    /// Bound `title`, `entries` and `key_rows` as one panel by the union of every line's box and every key row's bounds, or `None` when they draw nothing.
    fn bounding(title: Option<Label>, entries: Vec<Label>, key_rows: Vec<KeyRow>) -> Option<Self> {
        let labels = title.iter().chain(&entries);
        let label_bounds = labels.map(Label::bounds);
        let key_row_bounds = key_rows.iter().map(|key_row| key_row.bounds);
        let every_bounds = label_bounds.chain(key_row_bounds);
        let bounds = every_bounds.reduce(|bounds, next_bounds| bounds.union(next_bounds))?;
        let panel = Self {
            title,
            entries,
            key_rows,
            bounds,
        };
        Some(panel)
    }

    /// Return the offset moving a panel bounded by `unplaced_bounds` to `root`'s view's top-right corner inside its margin, or, when anything drawn there or `heading` overlaps it, one margin right of the view.
    fn offset_to_top_right_of(
        root: &Group,
        heading: Option<&Heading>,
        unplaced_bounds: Rect,
    ) -> Vec2 {
        let view = root.view;
        let top = view.y0 + Group::VIEW_MARGIN;
        let right_aligned_left = view.x1 - Group::VIEW_MARGIN - unplaced_bounds.width();
        let top_right_offset = Vec2::new(
            right_aligned_left - unplaced_bounds.x0,
            top - unplaced_bounds.y0,
        );
        let top_right_bounds = unplaced_bounds + top_right_offset;
        let drawn_outlines = Obstructions::around_document(root, heading);
        let overlap_count = drawn_outlines.count_overlapped_by(&top_right_bounds);
        let is_spot_taken = overlap_count > 0;
        if is_spot_taken {
            let beside_content_left = view.x1 - unplaced_bounds.x0;
            return Vec2::new(beside_content_left, top_right_offset.y);
        }
        top_right_offset
    }

    /// Return the panel translated by `offset`, bounded again around its translated lines and rows, or `None` when it draws nothing.
    fn translated(self, offset: Vec2) -> Option<Self> {
        let title = self.title.map(|title| title.translated(offset));
        let entries = self
            .entries
            .into_iter()
            .map(|entry| entry.translated(offset))
            .collect();
        let key_rows = self
            .key_rows
            .into_iter()
            .map(|key_row| key_row.translated(offset))
            .collect();
        Self::bounding(title, entries, key_rows)
    }
}

/// Samples one way the diagram has drawn some edges, for the key row describing it.
trait DrawnKey {
    /// Return the legend's description of the sampled edges.
    fn description(&self) -> &str;

    /// Return the sample height the row's pitch must clear above its centre line.
    fn height_to_clear(&self) -> f64;

    /// Return the sample starting at `sample_start`, with its screen bounds.
    fn sample_from(&self, sample_start: Point) -> (KeySample, Rect);
}

/// Holds one dash length some arcs are drawn with, those arcs and the legend's description of them.
struct DrawnDash<'m> {
    stroke_dash: f64,
    edges: Vec<EdgeId>,
    description: &'m str,
}

impl<'m> DrawnDash<'m> {
    /// Collect every dash `model`'s arcs are drawn with, in first-use order, keeping all those `legend` has described.
    fn all_described_in(legend: &'m Legend, model: &Model) -> Vec<Self> {
        let mut dash_edges: Vec<(f64, Vec<EdgeId>)> = Vec::new();
        for (edge_id, edge) in model.arc_edges() {
            let stroke_dash = edge.stroke_dash();
            let same_dash = dash_edges
                .iter_mut()
                .find(|(dash, _edges)| *dash == stroke_dash);
            let edge_id = edge_id.clone();
            let Some((_dash, edges)) = same_dash else {
                let first_edges = vec![edge_id];
                let new_dash_entry = (stroke_dash, first_edges);
                dash_edges.push(new_dash_entry);
                continue;
            };
            edges.push(edge_id);
        }
        dash_edges
            .into_iter()
            .filter_map(|(stroke_dash, edges)| {
                let description = legend.dash_description(stroke_dash)?;
                let drawn_dash = Self {
                    stroke_dash,
                    edges,
                    description,
                };
                Some(drawn_dash)
            })
            .collect()
    }
}

impl DrawnKey for DrawnDash<'_> {
    fn description(&self) -> &str {
        self.description
    }

    /// Return nothing to clear, since a stroke sample lies on its row's centre line.
    fn height_to_clear(&self) -> f64 {
        0.0
    }

    /// Return a stroke dashed like the arcs, one sample length long.
    fn sample_from(&self, sample_start: Point) -> (KeySample, Rect) {
        let sample_length = Vec2::new(LegendPanel::SAMPLE_LENGTH, 0.0);
        let sample_end = sample_start + sample_length;
        let line = Line::new(sample_start, sample_end);
        let half_stroke_width = Connection::STROKE_WIDTH / 2.0;
        let line_bounds = Rect::from_points(sample_start, sample_end);
        let sample_bounds = line_bounds.inflate(half_stroke_width, half_stroke_width);
        let edges = self.edges.clone();
        let sample = KeySample::Stroke {
            line,
            stroke_dash: self.stroke_dash,
            edges,
        };
        (sample, sample_bounds)
    }
}

/// Holds one badge target, the upright faces its badges' icon is drawn as with their screen bounds, and the legend's description of them.
struct DrawnBadge<'m> {
    target: &'m ElementPath,
    icon_faces: Vec<Face>,
    icon_bounds: Rect,
    description: &'m str,
}

impl<'m> DrawnBadge<'m> {
    /// Collect every target `model`'s badges point to, in first-use order, keeping all those `legend` has described.
    fn all_described_in(legend: &'m Legend, model: &'m Model, plans: &LevelPlans) -> Vec<Self> {
        let mut targets: Vec<&'m ElementPath> = Vec::new();
        for edge_id in model.badge_edges() {
            let target = edge_id.to();
            let is_new_target = !targets.contains(&target);
            if is_new_target {
                targets.push(target);
            }
        }
        targets
            .into_iter()
            .filter_map(|target| {
                let description = legend.badge_description(target)?;
                let icon = BadgeIcon::of_target_in(model, plans, target)?;
                let (icon_faces, icon_bounds) = icon.upright_faces_with_bounds();
                let drawn_badge = Self {
                    target,
                    icon_faces,
                    icon_bounds,
                    description,
                };
                Some(drawn_badge)
            })
            .collect()
    }
}

impl DrawnKey for DrawnBadge<'_> {
    fn description(&self) -> &str {
        self.description
    }

    fn height_to_clear(&self) -> f64 {
        self.icon_bounds.height()
    }

    /// Return the icon with its screen bounding box centred half a sample length along.
    fn sample_from(&self, sample_start: Point) -> (KeySample, Rect) {
        let half_sample_length = Vec2::new(LegendPanel::SAMPLE_LENGTH / 2.0, 0.0);
        let sample_center = sample_start + half_sample_length;
        let icon_center = self.icon_bounds.center();
        let offset = sample_center - icon_center;
        let shift = Affine::translate(offset);
        let faces: Vec<Face> = self
            .icon_faces
            .iter()
            .map(|face| face.transformed(shift))
            .collect();
        let face_bounds = Face::bounds_of(&faces);
        let sample_bounds = face_bounds.unwrap_or(Rect::ZERO);
        let target = self.target.clone();
        let sample = KeySample::Badge { target, faces };
        (sample, sample_bounds)
    }
}

impl KeyRow {
    /// Pair `sample`, bounded on screen by `sample_bounds`, with `description`.
    fn new(sample: KeySample, sample_bounds: Rect, description: Label) -> Self {
        let description_bounds = description.bounds();
        let bounds = sample_bounds.union(description_bounds);
        Self {
            sample,
            description,
            bounds,
        }
    }

    /// Return one key row sampling each key of `drawn_keys`, each set as the next key row of `lines`.
    fn sampling<K: DrawnKey>(
        drawn_keys: &[K],
        lines: &mut LineStack,
        measure: &impl TextMeasure,
    ) -> Vec<Self> {
        let mut key_rows = Vec::new();
        for drawn_key in drawn_keys {
            let description_text = drawn_key.description();
            let height_to_clear = drawn_key.height_to_clear();
            let (description, sample_start) =
                lines.set_key_row(description_text, height_to_clear, measure);
            let (sample, sample_bounds) = drawn_key.sample_from(sample_start);
            let key_row = Self::new(sample, sample_bounds, description);
            key_rows.push(key_row);
        }
        key_rows
    }

    /// Return the row translated by `offset`.
    fn translated(self, offset: Vec2) -> Self {
        let sample = self.sample.translated(offset);
        let description = self.description.translated(offset);
        let bounds = self.bounds + offset;
        Self {
            sample,
            description,
            bounds,
        }
    }
}

impl KeySample {
    /// Return the sample translated by `offset`.
    fn translated(self, offset: Vec2) -> Self {
        let shift = Affine::translate(offset);
        match self {
            Self::Stroke {
                line,
                stroke_dash,
                edges,
            } => Self::Stroke {
                line: shift * line,
                stroke_dash,
                edges,
            },
            Self::Badge { target, faces } => {
                let shifted_faces = faces.iter().map(|face| face.transformed(shift)).collect();
                Self::Badge {
                    target,
                    faces: shifted_faces,
                }
            }
        }
    }
}

/// Stacks a legend's lines from the origin, each baseline a pitch below the one before, the pitch set by the part of the legend each of the two lines belongs to.
struct LineStack {
    previous_line: Option<(LegendPart, f64)>,
}

/// Names the part of a legend one stacked line belongs to.
#[derive(Clone, Copy)]
enum LegendPart {
    Title,
    Entry,
    KeyRow,
}

impl LineStack {
    /// Drop from the title's baseline to the first entry's.
    const TITLE_PITCH: f64 = 20.0;
    /// Drop from one entry or key row's baseline to the next.
    const LINE_PITCH: f64 = 17.0;
    /// Drop from the last authored line's baseline to the first key row's.
    const KEY_PITCH: f64 = 31.0;
    /// Least gap between a badge sample and the row above.
    const SAMPLE_GAP: f64 = 2.0;
    /// Indent of a key row's description from the panel's left edge, past its sample.
    const DESCRIPTION_INDENT: f64 = 40.0;
    /// Rise of a sample's centre line above its row's baseline.
    const SAMPLE_RISE: f64 = 4.0;

    /// Start a stack of no line yet, whose first line's box top lies on the top edge.
    fn new() -> Self {
        Self {
            previous_line: None,
        }
    }

    /// Set the legend's title flush left, as the stack's first line.
    fn set_title(&mut self, text: &str, measure: &impl TextMeasure) -> Label {
        let top_line_pitch = 0.0;
        let flush_left = 0.0;
        let role = LabelRole::LegendTitle;
        self.set_line(
            text,
            role,
            LegendPart::Title,
            top_line_pitch,
            flush_left,
            measure,
        )
    }

    /// Set an authored entry flush left, a title pitch below the title, else a line pitch below the entry before.
    fn set_entry(&mut self, text: &str, measure: &impl TextMeasure) -> Label {
        let is_below_title = matches!(self.previous_line, Some((LegendPart::Title, _baseline)));
        let pitch = if is_below_title {
            Self::TITLE_PITCH
        } else {
            Self::LINE_PITCH
        };
        let flush_left = 0.0;
        let role = LabelRole::LegendLine;
        self.set_line(text, role, LegendPart::Entry, pitch, flush_left, measure)
    }

    /// Set a key row's description indented past its sample, and return it with where the sample starts: on the left edge, risen above the row's baseline, a key pitch below authored lines, else a line pitch grown so a sample `sample_height` tall clears the row above.
    fn set_key_row(
        &mut self,
        text: &str,
        sample_height: f64,
        measure: &impl TextMeasure,
    ) -> (Label, Point) {
        let is_below_key_row = matches!(self.previous_line, Some((LegendPart::KeyRow, _baseline)));
        let clearing_pitch = sample_height + Self::SAMPLE_GAP;
        let pitch = if is_below_key_row {
            Self::LINE_PITCH.max(clearing_pitch)
        } else {
            Self::KEY_PITCH
        };
        let indent = Self::DESCRIPTION_INDENT;
        let role = LabelRole::LegendLine;
        let description = self.set_line(text, role, LegendPart::KeyRow, pitch, indent, measure);
        let sample_y = description.baseline_origin.y - Self::SAMPLE_RISE;
        let left_edge = 0.0;
        let sample_start = Point::new(left_edge, sample_y);
        (description, sample_start)
    }

    /// Set `text` in `role`'s font as the stack's next line, of the legend's `part`, start-anchored `indent` in from the left edge, `pitch` below the previous line, or with its box top on the top edge.
    fn set_line(
        &mut self,
        text: &str,
        role: LabelRole,
        part: LegendPart,
        pitch: f64,
        indent: f64,
        measure: &impl TextMeasure,
    ) -> Label {
        let size = role.measure_text(text, measure);
        let first_baseline = size.baseline_from_top;
        let previous_baseline = self
            .previous_line
            .map(|(_previous_part, baseline)| baseline);
        let baseline = previous_baseline.map_or(first_baseline, |previous| previous + pitch);
        self.previous_line = Some((part, baseline));
        let baseline_origin = Point::new(indent, baseline);
        Label::new(text, role, TextAnchor::Start, baseline_origin, size)
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element};

    use super::*;
    use crate::geometry::Scene;
    use crate::geometry::fixtures::FixedAdvanceMeasure;

    /// Lay out and project `a -> b` flowing, `b -> c` dashed 2 and `a -> store` badged, under `builder`'s title and legend.
    fn keyed_scene(builder: ModelBuilder) -> Scene {
        let mut builder = builder;
        let a = builder.add_top_level("a", Element::new("app"));
        let b = builder.add_top_level("b", Element::new("bus"));
        let c = builder.add_top_level("c", Element::new("crank"));
        let store = builder.add_top_level("store", Element::new("store").with_badge_target());
        builder.add_edge(&a, &b, Edge::new());
        builder.add_edge(&b, &c, Edge::new().with_style("stroke-dash", "2"));
        builder.add_edge(&a, &store, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");
        let plans = LevelPlans::lay_out(&model, &FixedAdvanceMeasure);
        Scene::project(&model, &plans, &FixedAdvanceMeasure)
    }

    fn store_path() -> ElementPath {
        ElementPath::from_segments(["store"])
    }

    /// Name each key row by its sample and its description, e.g. `dash 5: flows`.
    fn key_row_names(legend: &LegendPanel) -> Vec<String> {
        legend
            .key_rows
            .iter()
            .map(|key_row| {
                let description = &key_row.description.text;
                match &key_row.sample {
                    KeySample::Stroke {
                        stroke_dash, edges, ..
                    } => format!("dash {stroke_dash} of {}: {description}", edges.len()),
                    KeySample::Badge { target, .. } => format!("badge {target}: {description}"),
                }
            })
            .collect()
    }

    #[test]
    fn the_key_samples_only_described_dashes_and_badges_the_diagram_draws_in_first_use_order() {
        let legend = Legend::new()
            .with_title("Bet lifecycle")
            .with_entry("① quote")
            .with_dash_description(3.0, "never drawn")
            .with_dash_description(2.0, "bus")
            .with_dash_description(5.0, "flows")
            .with_badge_description(store_path(), "writes")
            .with_badge_description(ElementPath::from_segments(["a"]), "not a badge target");
        let builder = ModelBuilder::new().with_legend(legend);

        let scene = keyed_scene(builder);

        let legend = scene
            .legend
            .expect("the legend draws its title, entry and rows");
        let title = legend.title.as_ref().expect("the title is set");
        assert_eq!(title.role, LabelRole::LegendTitle);
        assert_eq!(legend.entries[0].role, LabelRole::LegendLine);
        let names = key_row_names(&legend);
        assert_eq!(
            names,
            [
                "dash 5 of 1: flows",
                "dash 2 of 1: bus",
                "badge store: writes"
            ]
        );
        let entry_drop = legend.entries[0].baseline_origin.y - title.baseline_origin.y;
        assert_eq!(entry_drop, LineStack::TITLE_PITCH);
        let first_row_drop =
            legend.key_rows[0].description.baseline_origin.y - legend.entries[0].baseline_origin.y;
        assert_eq!(first_row_drop, LineStack::KEY_PITCH);
        let description_indent =
            legend.key_rows[0].description.baseline_origin.x - title.baseline_origin.x;
        assert_eq!(description_indent, LineStack::DESCRIPTION_INDENT);
    }

    #[test]
    fn a_legend_describing_nothing_drawn_draws_nothing_and_frames_nothing() {
        let unlegended_scene = keyed_scene(ModelBuilder::new());
        let legend = Legend::new().with_dash_description(3.0, "never drawn");
        let builder = ModelBuilder::new().with_legend(legend);

        let scene = keyed_scene(builder);

        assert_eq!(scene.legend, None);
        assert_eq!(scene.root.view, unlegended_scene.root.view);
    }

    #[test]
    fn the_legend_stands_upright_top_right_inside_the_view_grown_to_frame_it() {
        let unlegended_view = keyed_scene(ModelBuilder::new()).root.view;
        let legend = Legend::new().with_title("Key");
        let builder = ModelBuilder::new().with_legend(legend);

        let scene = keyed_scene(builder);

        let legend = scene.legend.expect("the title is drawn");
        let title = legend.title.as_ref().expect("the title is set");
        let legend_bounds = legend.bounds;
        assert_eq!(title.plane, kurbo::Affine::IDENTITY);
        assert!((legend_bounds.x1 - (unlegended_view.x1 - Group::VIEW_MARGIN)).abs() < 1e-9);
        assert!((legend_bounds.y0 - (unlegended_view.y0 + Group::VIEW_MARGIN)).abs() < 1e-9);
        let framed_legend = legend_bounds.inflate(Group::VIEW_MARGIN, Group::VIEW_MARGIN);
        assert_eq!(scene.root.view, unlegended_view.union(framed_legend));
    }

    #[test]
    fn a_heading_across_the_top_right_spot_moves_the_legend_beside_the_content() {
        let long_subtitle =
            "a subtitle long enough to run under the whole top edge of this small diagram";
        let headed_builder = ModelBuilder::new().with_subtitle(long_subtitle);
        let headed_view = keyed_scene(headed_builder).root.view;
        let legend = Legend::new().with_title("Key");
        let builder = ModelBuilder::new()
            .with_subtitle(long_subtitle)
            .with_legend(legend);

        let scene = keyed_scene(builder);

        let legend = scene.legend.expect("the title is drawn");
        let legend_bounds = legend.bounds;
        assert!((legend_bounds.x0 - headed_view.x1).abs() < 1e-9);
        let heading = scene.heading.expect("the subtitle is set");
        let heading_bounds = heading.bounds();
        assert!(!heading_bounds.overlaps(legend_bounds));
    }
}
