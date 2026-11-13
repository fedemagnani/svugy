use svugy_model::disclose::DisclosedGraph;
use svugy_model::model::{ProjectedWorkflow, ShownWorkflow, ViewAngle};

use super::{RenderSvg, RenderSvgLayers};
use crate::arrange::ArrangementSearch;
use crate::geometry::{Camera, Isometric, Scene, Top};
use crate::layout::LevelPlans;
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

impl RenderSvgLayers for StaticSvg {
    fn render_every_layer(shown: ShownWorkflow<'_>) -> Vec<String> {
        StaticSvg::render_every_layer_themed::<Standard>(shown)
    }
}

impl RenderSvgLayers for SketchStaticSvg {
    fn render_every_layer(shown: ShownWorkflow<'_>) -> Vec<String> {
        StaticSvg::render_every_layer_themed::<Sketch>(shown)
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

    /// Render every disclosure layer of the shown workflow of `shown` in theme `T` from the angle its author picks, isometric when none.
    fn render_every_layer_themed<T: Theme>(shown: ShownWorkflow<'_>) -> Vec<String> {
        let model = shown.model();
        let projection = shown.projected();
        let view = model.view().unwrap_or_default();
        match view {
            ViewAngle::Isometric => Self::render_every_layer_seen_by::<T, Isometric>(&projection),
            ViewAngle::Top => Self::render_every_layer_seen_by::<T, Top>(&projection),
        }
    }

    /// Disclose the graph of `projection` layer by layer, lay every layer out in the deepest layer's most readable arrangement as camera `C` sees it, as the interactive SVG does, project each in theme `T`'s font under its workflow's heading, then write each scene as its own static document in `T`.
    fn render_every_layer_seen_by<T: Theme, C: Camera>(
        projection: &ProjectedWorkflow,
    ) -> Vec<String> {
        let workflow = projection.workflow();
        let graph = projection.graph();
        let font = BundledFont::<T::Faces>::new();
        let layers = DisclosedGraph::every_layer_of(graph);
        let deepest_layer = layers.last().expect("disclosure yields layer 0 at least");
        let deepest_graph = deepest_layer.graph();
        let search = ArrangementSearch::<_, C>::new(deepest_graph, &font);
        let (arrangements, _deepest_plans) = search.best();
        let layer_plans = LevelPlans::lay_out_layers_with(&layers, &arrangements, &font);
        let profile = Static::<T>::new(&font);
        let renderer = StaticRenderer::new(profile);
        layers
            .iter()
            .zip(&layer_plans)
            .map(|(layer, plans)| {
                let unheaded_scene = Scene::project_disclosed::<C>(layer, plans, &font);
                let headed_scene = unheaded_scene.headed_by(workflow, &font);
                renderer.write_layer_document(projection, layer, &headed_scene)
            })
            .collect()
    }
}
