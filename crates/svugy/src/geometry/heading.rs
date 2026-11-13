//! Head the top-level view with the workflow's title over its subtitle.

use kurbo::{Point, Rect, Vec2};
use svugy_model::workflow::Workflow;

use super::{Heading, Label, Scene};
use crate::metrics::{LabelRole, TextAnchor, TextMeasure};

impl Heading {
    /// Drop from the title's baseline to the subtitle's.
    const SUBTITLE_LINE_OFFSET: f64 = 18.0;

    /// Set `workflow`'s title and subtitle just above `view`, from its left margin.
    pub(super) fn set_above(view: Rect, workflow: &Workflow, measure: &impl TextMeasure) -> Self {
        let unplaced_heading = Self::set_from(workflow, measure);
        let unplaced_bounds = unplaced_heading.bounds();
        let heading_left = view.x0 + Scene::VIEW_MARGIN;
        let heading_top = view.y0 - unplaced_bounds.height();
        let heading_offset = Vec2::new(heading_left, heading_top);
        unplaced_heading.translated(heading_offset)
    }

    /// Set `workflow`'s title with its line box's top-left at the origin and its subtitle, if any, one line below.
    fn set_from(workflow: &Workflow, measure: &impl TextMeasure) -> Self {
        let title_text = workflow.title();
        let title = Self::line_topped_at_origin(title_text, LabelRole::WorkflowTitle, measure);
        let subtitle_text = workflow.subtitle();
        let subtitle = subtitle_text.map(|subtitle_text| {
            let subtitle_size = LabelRole::WorkflowSubtitle.measure_text(subtitle_text, measure);
            let subtitle_line_offset = Vec2::new(0.0, Self::SUBTITLE_LINE_OFFSET);
            let subtitle_baseline_origin = title.baseline_origin + subtitle_line_offset;
            Label::new(
                subtitle_text,
                LabelRole::WorkflowSubtitle,
                TextAnchor::Start,
                subtitle_baseline_origin,
                subtitle_size,
            )
        });
        Self { title, subtitle }
    }

    /// Set `text` in `role`'s font, start-anchored, with its line box's top-left at the origin.
    fn line_topped_at_origin(text: &str, role: LabelRole, measure: &impl TextMeasure) -> Label {
        let size = role.measure_text(text, measure);
        let baseline_origin = Point::new(0.0, size.baseline_from_top);
        Label::new(text, role, TextAnchor::Start, baseline_origin, size)
    }

    /// Return the heading translated by `offset`.
    fn translated(self, offset: Vec2) -> Self {
        let title = self.title.translated(offset);
        let subtitle = self.subtitle.map(|subtitle| subtitle.translated(offset));
        Self { title, subtitle }
    }

    /// Return the union of the heading's line boxes.
    pub(crate) fn bounds(&self) -> Rect {
        let title_bounds = self.title.bounds();
        let subtitle_bounds = self.subtitle.as_ref().map(Label::bounds);
        subtitle_bounds.map_or(title_bounds, |subtitle_bounds| {
            title_bounds.union(subtitle_bounds)
        })
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::model::{Edge, Element};
    use svugy_model::workflow::WorkflowId;

    use super::*;
    use crate::geometry::Isometric;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::layout::{LevelPlan, LevelPlans};
    use crate::one_step::OneStepModelBuilder;

    #[test]
    fn the_title_stands_upright_above_the_content_from_its_left_margin_with_the_subtitle_below() {
        let checkout_id = WorkflowId::new("checkout");
        let checkout = Workflow::new(checkout_id, "A customer pays")
            .with_subtitle("from cart to charged card");
        let mut builder = OneStepModelBuilder::of_workflow(checkout.clone());
        let a = builder.add_top_level("a", Element::new("app"));
        let b = builder.add_top_level("b", Element::new("store"));
        builder.add_edge(&a, &b, Edge::new().with_label("calls"));
        let graph = builder.build_graph();
        let plan = LevelPlan::two_boxes_and_one_route();
        let plans = LevelPlans::root_only(plan);
        let unheaded_scene = Scene::project::<Isometric>(&graph, &plans, &FixedAdvanceMeasure);
        let content_view = unheaded_scene.root_view();

        let headed_scene = unheaded_scene.headed_by(&checkout, &FixedAdvanceMeasure);

        let heading = &headed_scene.heading;
        let title = &heading.title;
        let subtitle = heading.subtitle.as_ref().expect("the subtitle is set");
        let title_box = title.bounds();
        let heading_bounds = heading.bounds();
        assert_eq!(title.role, LabelRole::WorkflowTitle);
        assert_eq!(subtitle.role, LabelRole::WorkflowSubtitle);
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
        let headed_root_view = headed_scene.scene.root_view();
        assert_eq!(headed_root_view, expected_view);
    }
}
