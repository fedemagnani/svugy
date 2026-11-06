//! Head the top-level view with the diagram's title over its subtitle.

use kurbo::{Point, Rect, Vec2};
use svugy_model::model::Model;

use super::{Group, Heading, Label, Scene};
use crate::metrics::{LabelRole, TextAnchor, TextMeasure};

impl Scene {
    /// Head the root view with `model`'s title and subtitle, set just above the framed content from its left margin, the view grown to frame them too.
    pub(super) fn head_with_title_and_subtitle(
        &mut self,
        model: &Model,
        measure: &impl TextMeasure,
    ) {
        let unplaced_heading = Heading::set_from(model, Point::ORIGIN, measure);
        let Some(unplaced_bounds) = unplaced_heading.bounds() else {
            return;
        };
        let root_view = self.root.view;
        let heading_left = root_view.x0 + Group::VIEW_MARGIN;
        let heading_top = root_view.y0 - unplaced_bounds.height();
        let heading_offset = Vec2::new(heading_left, heading_top);
        let heading_top_left = heading_offset.to_point();
        let heading_bounds = unplaced_bounds + heading_offset;
        let framed_heading = heading_bounds.inflate(Group::VIEW_MARGIN, Group::VIEW_MARGIN);
        self.root.view = root_view.union(framed_heading);
        self.heading = Heading::set_from(model, heading_top_left, measure);
    }
}

impl Heading {
    /// Drop from the title's baseline to the subtitle's.
    const SUBTITLE_LINE_OFFSET: f64 = 18.0;

    /// Set `model`'s title with its line box's top-left at `top_left` and its subtitle one line below, or the subtitle alone at `top_left`.
    fn set_from(model: &Model, top_left: Point, measure: &impl TextMeasure) -> Self {
        let title = model.title().map(|title_text| {
            let title_size = LabelRole::DiagramTitle.measure_text(title_text, measure);
            let title_baseline_drop = Vec2::new(0.0, title_size.baseline_from_top);
            let title_baseline_origin = top_left + title_baseline_drop;
            Label::new(
                title_text,
                LabelRole::DiagramTitle,
                TextAnchor::Start,
                title_baseline_origin,
                title_size,
            )
        });
        let subtitle = model.subtitle().map(|subtitle_text| {
            let subtitle_size = LabelRole::DiagramSubtitle.measure_text(subtitle_text, measure);
            let subtitle_baseline_drop = Vec2::new(0.0, subtitle_size.baseline_from_top);
            let untitled_baseline_origin = top_left + subtitle_baseline_drop;
            let subtitle_line_offset = Vec2::new(0.0, Self::SUBTITLE_LINE_OFFSET);
            let subtitle_baseline_origin = match &title {
                Some(title) => title.baseline_origin + subtitle_line_offset,
                None => untitled_baseline_origin,
            };
            Label::new(
                subtitle_text,
                LabelRole::DiagramSubtitle,
                TextAnchor::Start,
                subtitle_baseline_origin,
                subtitle_size,
            )
        });
        Self { title, subtitle }
    }

    /// Return the union of both lines' boxes, or `None` for a diagram with neither title nor subtitle.
    pub(crate) fn bounds(&self) -> Option<Rect> {
        let lines = self.title.iter().chain(&self.subtitle);
        let line_bounds = lines.map(Label::bounds);
        line_bounds.reduce(|bounds, next_bounds| bounds.union(next_bounds))
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

        assert_eq!(scene.heading, Heading::default());
        assert_eq!(scene.heading.bounds(), None);
    }

    #[test]
    fn the_title_stands_upright_above_the_content_from_its_left_margin_with_the_subtitle_below() {
        let untitled = untitled_scene();
        let builder = ModelBuilder::new()
            .with_title(TITLE)
            .with_subtitle(SUBTITLE);

        let scene = headed_two_boxes_scene(builder);

        let title = scene.heading.title.as_ref().expect("the title is set");
        let subtitle = scene
            .heading
            .subtitle
            .as_ref()
            .expect("the subtitle is set");
        let title_box = title.bounds();
        let heading_bounds = scene.heading.bounds().expect("the heading has lines");
        let content_view = untitled.root.view;
        assert_eq!(title.role, LabelRole::DiagramTitle);
        assert_eq!(subtitle.role, LabelRole::DiagramSubtitle);
        assert_eq!(title.anchor, TextAnchor::Start);
        assert_eq!(title.plane, kurbo::Affine::IDENTITY);
        assert_eq!(subtitle.plane, kurbo::Affine::IDENTITY);
        assert_eq!(title_box.x0, content_view.x0 + Group::VIEW_MARGIN);
        let subtitle_drop = subtitle.baseline_origin.y - title.baseline_origin.y;
        assert_eq!(subtitle_drop, Heading::SUBTITLE_LINE_OFFSET);
        assert_eq!(subtitle.baseline_origin.x, title.baseline_origin.x);
        assert!((heading_bounds.y1 - content_view.y0).abs() < 1e-9);
        let framed_heading = heading_bounds.inflate(Group::VIEW_MARGIN, Group::VIEW_MARGIN);
        let expected_view = content_view.union(framed_heading);
        assert_eq!(scene.root.view, expected_view);
        assert!(scene.root.view.width() > content_view.width());
    }

    #[test]
    fn a_subtitle_alone_takes_the_title_line() {
        let untitled = untitled_scene();
        let builder = ModelBuilder::new().with_subtitle("subtitle only");

        let scene = headed_two_boxes_scene(builder);

        let subtitle = scene
            .heading
            .subtitle
            .as_ref()
            .expect("the subtitle is set");
        let subtitle_box = subtitle.bounds();
        assert!(scene.heading.title.is_none());
        assert!((subtitle_box.y1 - untitled.root.view.y0).abs() < 1e-9);
        assert_eq!(scene.root.view.y0, subtitle_box.y0 - Group::VIEW_MARGIN);
    }
}
