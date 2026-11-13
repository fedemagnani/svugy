use svugy_model::model::{ProjectedWorkflow, ShownWorkflow, ViewAngle};

use super::RenderSvg;
use crate::arrange::ArrangementSearch;
use crate::geometry::{Camera, Isometric, Scene, Top};
use crate::metrics::BundledFont;
use crate::render::{Sketch, Standard, Static, StaticRenderer, Theme};

/// Renders one workflow of a model as a static SVG: self-contained, for viewers without CSS, script or fonts.
pub struct StaticSvg;

/// Renders one workflow of a model as a static SVG in the hand-drawn sketch theme: self-contained, its labels outlined in Excalifont.
pub struct SketchStaticSvg;

impl RenderSvg for StaticSvg {
    fn render(shown: ShownWorkflow<'_>) -> String {
        StaticSvg::render_themed::<Standard>(shown)
    }
}

impl RenderSvg for SketchStaticSvg {
    fn render(shown: ShownWorkflow<'_>) -> String {
        StaticSvg::render_themed::<Sketch>(shown)
    }
}

impl StaticSvg {
    /// Render the shown workflow of `shown` in theme `T` from the angle its author picks, isometric when none.
    fn render_themed<T: Theme>(shown: ShownWorkflow<'_>) -> String {
        let model = shown.model();
        let projection = shown.projected();
        let view = model.view().unwrap_or_default();
        match view {
            ViewAngle::Isometric => Self::render_seen_by::<T, Isometric>(&projection),
            ViewAngle::Top => Self::render_seen_by::<T, Top>(&projection),
        }
    }

    /// Lay the graph of `projection` out in its most readable arrangement as camera `C` sees it, project it in theme `T`'s font under its workflow's heading, then write the scene as the static document in `T`, its labels outlined in that font.
    fn render_seen_by<T: Theme, C: Camera>(projection: &ProjectedWorkflow) -> String {
        let workflow = projection.workflow();
        let graph = projection.graph();
        let font = BundledFont::<T::Faces>::new();
        let search = ArrangementSearch::<_, C>::new(graph, &font);
        let (_arrangements, plans) = search.best();
        let unheaded_scene = Scene::project::<C>(graph, &plans, &font);
        let headed_scene = unheaded_scene.headed_by(workflow, &font);
        let profile = Static::<T>::new(&font);
        let renderer = StaticRenderer::new(profile);
        renderer.write_document(projection, &headed_scene)
    }
}
