//! Head the top-level view with the diagram's title over its subtitle.

use std::iter;

use kurbo::{Point, Rect, Vec2};
use svugy_model::model::Model;

use super::{Heading, Label, Scene};
use crate::metrics::{LabelRole, TextAnchor, TextMeasure};

impl Heading {
    /// Drop from the title's baseline to the subtitle's.
    const SUBTITLE_LINE_OFFSET: f64 = 18.0;

    /// Set `model`'s title and subtitle just above `view`, from its left margin, or `None` when it has neither.
    pub(super) fn set_above(view: Rect, model: &Model, measure: &impl TextMeasure) -> Option<Self> {
        let unplaced_heading = Self::set_from(model, measure)?;
        let unplaced_bounds = unplaced_heading.bounds();
        let heading_left = view.x0 + Scene::VIEW_MARGIN;
        let heading_top = view.y0 - unplaced_bounds.height();
        let heading_offset = Vec2::new(heading_left, heading_top);
        let heading = unplaced_heading.translated(heading_offset);
        Some(heading)
    }

    /// Set `model`'s title with its line box's top-left at the origin and its subtitle one line below, or the subtitle alone at the origin, or `None` when it has neither.
    fn set_from(model: &Model, measure: &impl TextMeasure) -> Option<Self> {
        let subtitle_text = model.subtitle();
        let Some(title_text) = model.title() else {
            let subtitle_text = subtitle_text?;
            let subtitle =
                Self::line_topped_at_origin(subtitle_text, LabelRole::DiagramSubtitle, measure);
            let subtitled = Self::Subtitled(subtitle);
            return Some(subtitled);
        };
        let title = Self::line_topped_at_origin(title_text, LabelRole::DiagramTitle, measure);
        let subtitle = subtitle_text.map(|subtitle_text| {
            let subtitle_size = LabelRole::DiagramSubtitle.measure_text(subtitle_text, measure);
            let subtitle_line_offset = Vec2::new(0.0, Self::SUBTITLE_LINE_OFFSET);
            let subtitle_baseline_origin = title.baseline_origin + subtitle_line_offset;
            Label::new(
                subtitle_text,
                LabelRole::DiagramSubtitle,
                TextAnchor::Start,
                subtitle_baseline_origin,
                subtitle_size,
            )
        });
        let titled = Self::Titled { title, subtitle };
        Some(titled)
    }

    /// Set `text` in `role`'s font, start-anchored, with its line box's top-left at the origin.
    fn line_topped_at_origin(text: &str, role: LabelRole, measure: &impl TextMeasure) -> Label {
        let size = role.measure_text(text, measure);
        let baseline_origin = Point::new(0.0, size.baseline_from_top);
        Label::new(text, role, TextAnchor::Start, baseline_origin, size)
    }

    /// Return the heading translated by `offset`.
    fn translated(self, offset: Vec2) -> Self {
        match self {
            Self::Titled { title, subtitle } => {
                let title = title.translated(offset);
                let subtitle = subtitle.map(|subtitle| subtitle.translated(offset));
                Self::Titled { title, subtitle }
            }
            Self::Subtitled(subtitle) => {
                let subtitle = subtitle.translated(offset);
                Self::Subtitled(subtitle)
            }
        }
    }

    /// Return the union of the heading's line boxes.
    pub(crate) fn bounds(&self) -> Rect {
        let (first_line, second_line) = self.first_and_second_line();
        let first_line_bounds = first_line.bounds();
        let second_line_bounds = second_line.map(Label::bounds);
        second_line_bounds.map_or(first_line_bounds, |second_line_bounds| {
            first_line_bounds.union(second_line_bounds)
        })
    }

    /// Yield the title, then the subtitle, whichever is set.
    pub(super) fn lines(&self) -> impl Iterator<Item = &Label> {
        let (first_line, second_line) = self.first_and_second_line();
        let first_line_only = iter::once(first_line);
        first_line_only.chain(second_line)
    }

    /// Return the heading's first line and the line below it, if any.
    fn first_and_second_line(&self) -> (&Label, Option<&Label>) {
        match self {
            Self::Titled { title, subtitle } => (title, subtitle.as_ref()),
            Self::Subtitled(subtitle) => (subtitle, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element};

    use super::*;
    use crate::geometry::Scene;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::layout::{LevelPlan, LevelPlans};

    const TITLE: &str = "svugy · how it is built";
    const SUBTITLE: &str = "text in → model → layout → geometry → SVG out";

    /// Build the fixture plan's boxes `a` and `b` and their edge, headed by `builder`'s title and subtitle.
    fn headed_two_boxes_scene(builder: ModelBuilder) -> Scene {
        let mut builder = builder;
        let a = builder.add_top_level("a", Element::new("app"));
        let b = builder.add_top_level("b", Element::new("store"));
        builder.add_edge(&a, &b, Edge::new().with_label("calls"));
        let model = builder.build().model.expect("fixture model is valid");
        let plan = LevelPlan::two_boxes_and_one_route();
        let plans = LevelPlans::root_only(plan);
        Scene::project(&model, &plans, &FixedAdvanceMeasure)
    }

    fn untitled_scene() -> Scene {
        let builder = ModelBuilder::new();
        headed_two_boxes_scene(builder)
    }

    #[test]
    fn an_untitled_diagram_has_no_heading_to_frame() {
        let scene = untitled_scene();

        assert_eq!(scene.heading, None);
    }

    #[test]
    fn the_title_stands_upright_above_the_content_from_its_left_margin_with_the_subtitle_below() {
        let untitled = untitled_scene();
        let builder = ModelBuilder::new()
            .with_title(TITLE)
            .with_subtitle(SUBTITLE);

        let scene = headed_two_boxes_scene(builder);

        let heading = scene.heading.as_ref().expect("the heading is set");
        let Heading::Titled {
            title,
            subtitle: Some(subtitle),
        } = heading
        else {
            panic!("the title and subtitle are set: {heading:?}");
        };
        let title_box = title.bounds();
        let heading_bounds = heading.bounds();
        let content_view = untitled.root_view();
        assert_eq!(title.role, LabelRole::DiagramTitle);
        assert_eq!(subtitle.role, LabelRole::DiagramSubtitle);
        assert_eq!(title.anchor, TextAnchor::Start);
        assert_eq!(title.plane, kurbo::Affine::IDENTITY);
        assert_eq!(subtitle.plane, kurbo::Affine::IDENTITY);
        assert_eq!(title_box.x0, content_view.x0 + Scene::VIEW_MARGIN);
        let subtitle_drop = subtitle.baseline_origin.y - title.baseline_origin.y;
        assert_eq!(subtitle_drop, Heading::SUBTITLE_LINE_OFFSET);
        assert_eq!(subtitle.baseline_origin.x, title.baseline_origin.x);
        assert!((heading_bounds.y1 - content_view.y0).abs() < 1e-9);
        let framed_heading = heading_bounds.inflate(Scene::VIEW_MARGIN, Scene::VIEW_MARGIN);
        let expected_view = content_view.union(framed_heading);
        assert_eq!(scene.root_view(), expected_view);
        assert!(scene.root_view().width() > content_view.width());
    }

    #[test]
    fn a_subtitle_alone_takes_the_title_line() {
        let untitled = untitled_scene();
        let builder = ModelBuilder::new().with_subtitle("subtitle only");

        let scene = headed_two_boxes_scene(builder);

        let heading = scene.heading.as_ref().expect("the heading is set");
        let Heading::Subtitled(subtitle) = heading else {
            panic!("the subtitle alone is set: {heading:?}");
        };
        let subtitle_box = subtitle.bounds();
        assert!((subtitle_box.y1 - untitled.root_view().y0).abs() < 1e-9);
        assert_eq!(scene.root_view().y0, subtitle_box.y0 - Scene::VIEW_MARGIN);
    }
}
