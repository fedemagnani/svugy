//! Set the diagram's legend top-right of the top-level view, its key rows sampled from what the diagram draws.

use kurbo::{Line, Point, Rect, Vec2};
use svugy_model::legend::Legend;
use svugy_model::model::{EdgeId, Model};
use svugy_model::path::ElementPath;

use super::badge::BadgeIcon;
use super::obstruction::Obstructions;
use super::{Connection, Face, Group, KeyRow, KeySample, Label, LegendPanel, Scene};
use crate::layout::LevelPlans;
use crate::metrics::{LabelRole, TextAnchor, TextMeasure};

impl Scene {
    /// Set `model`'s legend top-right of the root view, beside the heading, else just right of the content when drawables take that spot; the view grown to frame it.
    pub(super) fn add_legend(
        &mut self,
        model: &Model,
        plans: &LevelPlans,
        measure: &impl TextMeasure,
    ) {
        let Some(legend) = model.legend() else {
            return;
        };
        let drawn_keys = DrawnKeys::described_in(legend, model, plans);
        let unplaced_panel = LegendPanel::set_from(legend, &drawn_keys, Point::ORIGIN, measure);
        let Some(unplaced_bounds) = unplaced_panel.bounds() else {
            return;
        };
        let legend_offset = self.legend_offset(unplaced_bounds);
        let legend_top_left = legend_offset.to_point();
        let legend_bounds = unplaced_bounds + legend_offset;
        let framed_legend = legend_bounds.inflate(Group::VIEW_MARGIN, Group::VIEW_MARGIN);
        self.root.view = self.root.view.union(framed_legend);
        let legend_panel = LegendPanel::set_from(legend, &drawn_keys, legend_top_left, measure);
        self.legend = Some(legend_panel);
    }

    /// Move a panel bounded by `unplaced_bounds` to the root view's top-right corner inside its margin, or, when anything drawn there overlaps it, one margin right of the view.
    fn legend_offset(&self, unplaced_bounds: Rect) -> Vec2 {
        let view = self.root.view;
        let top = view.y0 + Group::VIEW_MARGIN;
        let right_aligned_left = view.x1 - Group::VIEW_MARGIN - unplaced_bounds.width();
        let top_right_offset = Vec2::new(
            right_aligned_left - unplaced_bounds.x0,
            top - unplaced_bounds.y0,
        );
        let top_right_bounds = unplaced_bounds + top_right_offset;
        let drawn_outlines = Obstructions::around_document(&self.root, &self.heading);
        let is_spot_taken = drawn_outlines.is_overlapped_by_rect(top_right_bounds);
        if is_spot_taken {
            let beside_content_left = view.x1 - unplaced_bounds.x0;
            return Vec2::new(beside_content_left, top_right_offset.y);
        }
        top_right_offset
    }
}

/// Lists the key rows a legend describes among those the diagram draws, each kind in first-use order: arc dashes, then badge targets.
struct DrawnKeys<'m> {
    dashes: Vec<DrawnDash<'m>>,
    badges: Vec<DrawnBadge<'m>>,
}

/// Holds one dash length some arcs are drawn with, those arcs and the legend's description of them.
struct DrawnDash<'m> {
    stroke_dash: f64,
    edges: Vec<EdgeId>,
    description: &'m str,
}

/// Holds one badge target, the icon its badges are drawn as and the legend's description of them.
struct DrawnBadge<'m> {
    target: &'m ElementPath,
    icon: BadgeIcon,
    description: &'m str,
}

impl<'m> DrawnKeys<'m> {
    /// Collect every dash `model`'s arcs are drawn with and every target its badges point to, keeping those `legend` describes.
    fn described_in(legend: &'m Legend, model: &'m Model, plans: &LevelPlans) -> Self {
        let mut dash_edges: Vec<(f64, Vec<EdgeId>)> = Vec::new();
        for (edge_id, edge) in model.arc_edges() {
            let stroke_dash = edge.stroke_dash();
            let same_dash = dash_edges
                .iter_mut()
                .find(|(dash, _edges)| *dash == stroke_dash);
            match same_dash {
                Some((_dash, edges)) => edges.push(edge_id.clone()),
                None => dash_edges.push((stroke_dash, vec![edge_id.clone()])),
            }
        }
        let dashes = dash_edges
            .into_iter()
            .filter_map(|(stroke_dash, edges)| {
                let description = legend.dash_description(stroke_dash)?;
                let drawn_dash = DrawnDash {
                    stroke_dash,
                    edges,
                    description,
                };
                Some(drawn_dash)
            })
            .collect();
        let mut targets: Vec<&'m ElementPath> = Vec::new();
        for edge_id in model.badge_edges() {
            let target = edge_id.to();
            let is_new_target = !targets.contains(&target);
            if is_new_target {
                targets.push(target);
            }
        }
        let badges = targets
            .into_iter()
            .filter_map(|target| {
                let description = legend.badge_description(target)?;
                let target_element = model.element(target)?;
                let target_footprint = plans.footprint_of(target)?;
                let icon = BadgeIcon::of_target(target_element, target_footprint);
                let drawn_badge = DrawnBadge {
                    target,
                    icon,
                    description,
                };
                Some(drawn_badge)
            })
            .collect();
        Self { dashes, badges }
    }
}

impl LegendPanel {
    /// Drop from the title's baseline to the first entry's.
    const TITLE_PITCH: f64 = 20.0;
    /// Drop from one entry or key row's baseline to the next.
    const LINE_PITCH: f64 = 17.0;
    /// Drop from the last authored line's baseline to the first key row's.
    const KEY_PITCH: f64 = 31.0;
    /// Length of a stroke sample, from the panel's left edge.
    const SAMPLE_LENGTH: f64 = 30.0;
    /// Rise of a sample's centre line above its row's baseline.
    const SAMPLE_RISE: f64 = 4.0;
    /// Indent of a key row's description from the panel's left edge, past its sample.
    const DESCRIPTION_INDENT: f64 = 40.0;
    /// Least gap between a badge sample and the row above.
    const SAMPLE_GAP: f64 = 2.0;

    /// Set `legend`'s title, entries and the rows of `drawn_keys`, one under the other from `top_left`.
    fn set_from(
        legend: &Legend,
        drawn_keys: &DrawnKeys,
        top_left: Point,
        measure: &impl TextMeasure,
    ) -> Self {
        let mut lines = LineStack::from(top_left);
        let title = legend.title().map(|title_text| {
            lines.set_line(title_text, LabelRole::LegendTitle, 0.0, 0.0, measure)
        });
        let first_entry_pitch = if title.is_some() {
            Self::TITLE_PITCH
        } else {
            Self::LINE_PITCH
        };
        let mut entries = Vec::new();
        for entry_text in legend.entries() {
            let is_first_entry = entries.is_empty();
            let pitch = if is_first_entry {
                first_entry_pitch
            } else {
                Self::LINE_PITCH
            };
            let entry = lines.set_line(entry_text, LabelRole::LegendLine, pitch, 0.0, measure);
            entries.push(entry);
        }
        let mut key_rows = Vec::new();
        for drawn_dash in &drawn_keys.dashes {
            let is_first_key_row = key_rows.is_empty();
            let key_row = KeyRow::sampling_dash(drawn_dash, is_first_key_row, &mut lines, measure);
            key_rows.push(key_row);
        }
        for drawn_badge in &drawn_keys.badges {
            let is_first_key_row = key_rows.is_empty();
            let key_row =
                KeyRow::sampling_badge(drawn_badge, is_first_key_row, &mut lines, measure);
            key_rows.push(key_row);
        }
        Self {
            title,
            entries,
            key_rows,
        }
    }

    /// Return the union of every line's box and every key row's bounds, or `None` for a legend with nothing to draw.
    pub(crate) fn bounds(&self) -> Option<Rect> {
        let labels = self.title.iter().chain(&self.entries);
        let label_bounds = labels.map(Label::bounds);
        let key_row_bounds = self.key_rows.iter().map(|key_row| key_row.bounds);
        let every_bounds = label_bounds.chain(key_row_bounds);
        every_bounds.reduce(|bounds, next_bounds| bounds.union(next_bounds))
    }
}

impl KeyRow {
    /// Set the next row of `lines`: a stroke sample dashed like `drawn_dash`'s arcs, then its description.
    fn sampling_dash(
        drawn_dash: &DrawnDash,
        is_first_key_row: bool,
        lines: &mut LineStack,
        measure: &impl TextMeasure,
    ) -> Self {
        let pitch = lines.key_row_pitch(0.0, is_first_key_row);
        let description = lines.set_description(drawn_dash.description, pitch, measure);
        let sample_start = lines.sample_start();
        let sample_length = Vec2::new(LegendPanel::SAMPLE_LENGTH, 0.0);
        let sample_end = sample_start + sample_length;
        let line = Line::new(sample_start, sample_end);
        let half_stroke_width = Connection::STROKE_WIDTH / 2.0;
        let line_bounds = Rect::from_points(sample_start, sample_end);
        let sample_bounds = line_bounds.inflate(half_stroke_width, half_stroke_width);
        let sample = KeySample::Stroke {
            line,
            stroke_dash: drawn_dash.stroke_dash,
            edges: drawn_dash.edges.clone(),
        };
        Self::new(sample, sample_bounds, description)
    }

    /// Set the next row of `lines`: `drawn_badge`'s icon centred on the sample, then its description.
    fn sampling_badge(
        drawn_badge: &DrawnBadge,
        is_first_key_row: bool,
        lines: &mut LineStack,
        measure: &impl TextMeasure,
    ) -> Self {
        let unplaced_faces = drawn_badge.icon.faces_around(Point::ORIGIN);
        let unplaced_icon_bounds = Face::bounds_of(&unplaced_faces).unwrap_or(Rect::ZERO);
        let icon_height = unplaced_icon_bounds.height();
        let pitch = lines.key_row_pitch(icon_height, is_first_key_row);
        let description = lines.set_description(drawn_badge.description, pitch, measure);
        let sample_start = lines.sample_start();
        let half_sample_length = Vec2::new(LegendPanel::SAMPLE_LENGTH / 2.0, 0.0);
        let sample_center = sample_start + half_sample_length;
        let faces = drawn_badge.icon.faces_around(sample_center);
        let sample_bounds = Face::bounds_of(&faces).unwrap_or(Rect::ZERO);
        let sample = KeySample::Badge {
            target: drawn_badge.target.clone(),
            faces,
        };
        Self::new(sample, sample_bounds, description)
    }

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
}

/// Stacks a legend's lines from its top-left corner, each baseline a pitch below the one before.
struct LineStack {
    top_left: Point,
    previous_baseline: Option<f64>,
}

impl LineStack {
    fn from(top_left: Point) -> Self {
        Self {
            top_left,
            previous_baseline: None,
        }
    }

    /// Set `text` in `role`'s font, start-anchored `indent` in from the left edge, `pitch` below the previous line, or with its box top on the top edge.
    fn set_line(
        &mut self,
        text: &str,
        role: LabelRole,
        pitch: f64,
        indent: f64,
        measure: &impl TextMeasure,
    ) -> Label {
        let size = role.measure_text(text, measure);
        let first_baseline = self.top_left.y + size.baseline_from_top;
        let baseline = self
            .previous_baseline
            .map_or(first_baseline, |previous| previous + pitch);
        self.previous_baseline = Some(baseline);
        let line_left = self.top_left.x + indent;
        let baseline_origin = Point::new(line_left, baseline);
        Label::new(text, role, TextAnchor::Start, baseline_origin, size)
    }

    /// Set a key row's description `pitch` below the previous line, indented past its sample.
    fn set_description(&mut self, text: &str, pitch: f64, measure: &impl TextMeasure) -> Label {
        let indent = LegendPanel::DESCRIPTION_INDENT;
        self.set_line(text, LabelRole::LegendLine, pitch, indent, measure)
    }

    /// Return where the last line's sample starts: on the left edge, risen above its baseline.
    fn sample_start(&self) -> Point {
        let baseline = self.previous_baseline.unwrap_or(self.top_left.y);
        let sample_y = baseline - LegendPanel::SAMPLE_RISE;
        Point::new(self.top_left.x, sample_y)
    }

    /// Return the drop to a key row whose sample stands `sample_height` tall: the key pitch below authored lines, else a line pitch grown so the sample clears the row above.
    fn key_row_pitch(&self, sample_height: f64, is_first_key_row: bool) -> f64 {
        let is_below_authored_lines = is_first_key_row && self.previous_baseline.is_some();
        if is_below_authored_lines {
            return LegendPanel::KEY_PITCH;
        }
        let clearing_pitch = sample_height + LegendPanel::SAMPLE_GAP;
        LegendPanel::LINE_PITCH.max(clearing_pitch)
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;

    /// Lay out and project `a -> b` flowing, `b -> c` dashed 2 and `a -> store` badged, under `builder`'s title and legend.
    fn keyed_scene(builder: ModelBuilder) -> Scene {
        let mut builder = builder;
        let a = builder.add_top_level("a", Element::new("app"));
        let b = builder.add_top_level("b", Element::new("bus"));
        let c = builder.add_top_level("c", Element::new("crank"));
        let store = builder.add_top_level("store", Element::new("store").with_badge());
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
        assert_eq!(entry_drop, LegendPanel::TITLE_PITCH);
        let first_row_drop =
            legend.key_rows[0].description.baseline_origin.y - legend.entries[0].baseline_origin.y;
        assert_eq!(first_row_drop, LegendPanel::KEY_PITCH);
        let description_indent =
            legend.key_rows[0].description.baseline_origin.x - title.baseline_origin.x;
        assert_eq!(description_indent, LegendPanel::DESCRIPTION_INDENT);
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
        let legend_bounds = legend.bounds().expect("the legend has a line");
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
        let legend_bounds = legend.bounds().expect("the legend has a line");
        assert!((legend_bounds.x0 - headed_view.x1).abs() < 1e-9);
        let heading_bounds = scene.heading.bounds().expect("the subtitle is set");
        assert!(!heading_bounds.overlaps(legend_bounds));
    }
}
