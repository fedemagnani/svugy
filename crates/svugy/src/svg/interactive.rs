use svugy_model::disclose::DisclosedGraph;
use svugy_model::graph::WorkflowGraph;
use svugy_model::model::{ProjectedWorkflow, ShownWorkflow, ViewAngle};
use svugy_model::workflow::Workflow;

use super::RenderSvg;
use crate::arrange::ArrangementSearch;
use crate::geometry::{Camera, Isometric, Scene, Top};
use crate::layout::{Arrangements, LevelPlans};
use crate::metrics::{BundledFont, TextMeasure};
use crate::render::{
    CameraLayers, Interactive, InteractiveRenderer, LayerScene, Sketch, Standard, Theme,
    WorkflowScenes,
};

/// Renders a model as the interactive SVG, the product: themed for light and dark, animated, explorable by its embedded runtime, laid out in the most readable arrangement its search finds.
pub struct InteractiveSvg;

/// Renders a model as the interactive SVG laid out in the seed arrangement, skipping the search: a quick draft to show while the searched render runs.
pub struct DraftInteractiveSvg;

/// Renders a model as the interactive SVG in the hand-drawn sketch theme: hatched faces, wobbly strokes, labels in Excalifont.
pub struct SketchInteractiveSvg;

/// Renders a model as the sketch-themed interactive SVG laid out in the seed arrangement: the draft to show while the searched sketch render runs.
pub struct SketchDraftInteractiveSvg;

/// Picks the arrangements every layer of one workflow of an interactive document is laid out in, from the deepest layer's graph as camera `C` sees it.
trait ArrangementChoice {
    fn arrangements_of<C: Camera>(
        deepest_graph: &WorkflowGraph,
        font: &impl TextMeasure,
    ) -> Arrangements;
}

/// Picks the most readable arrangements the search finds.
struct Searched;

/// Picks the seed arrangements the search would start from.
struct Seeded;

impl RenderSvg for InteractiveSvg {
    fn render(shown: ShownWorkflow<'_>) -> String {
        InteractiveSvg::render_arranged_by::<Standard, Searched>(shown)
    }
}

impl RenderSvg for DraftInteractiveSvg {
    fn render(shown: ShownWorkflow<'_>) -> String {
        InteractiveSvg::render_arranged_by::<Standard, Seeded>(shown)
    }
}

impl RenderSvg for SketchInteractiveSvg {
    fn render(shown: ShownWorkflow<'_>) -> String {
        InteractiveSvg::render_arranged_by::<Sketch, Searched>(shown)
    }
}

impl RenderSvg for SketchDraftInteractiveSvg {
    fn render(shown: ShownWorkflow<'_>) -> String {
        InteractiveSvg::render_arranged_by::<Sketch, Seeded>(shown)
    }
}

impl InteractiveSvg {
    /// Project every workflow of `shown`'s model into its graph, disclose and lay out each on its own as `A` arranges it from the angle the author picks, project each layer from every angle in theme `T`'s font, then write them all as the interactive document in `T`, opening on the shown workflow from the author's angle.
    fn render_arranged_by<T: Theme, A: ArrangementChoice>(shown: ShownWorkflow<'_>) -> String {
        let model = shown.model();
        let shown_workflow = shown.workflow();
        let font = BundledFont::<T::Faces>::new();
        let shown_angle = model.view().unwrap_or_default();
        let projections: Vec<ProjectedWorkflow> = model
            .showing_each_workflow()
            .map(ShownWorkflow::projected)
            .collect();
        let disclosures: Vec<Vec<DisclosedGraph>> = projections
            .iter()
            .map(|projection| {
                let graph = projection.graph();
                DisclosedGraph::every_layer_of(graph)
            })
            .collect();
        let workflow_scenes: Vec<WorkflowScenes> = projections
            .iter()
            .zip(&disclosures)
            .map(|(projection, layers)| {
                let workflow = projection.workflow();
                let cameras = Self::project_every_camera::<A>(workflow, layers, shown_angle, &font);
                WorkflowScenes::new(projection, cameras)
            })
            .collect();
        let renderer = InteractiveRenderer::new(Interactive::<T>::new());
        let shown_workflow_id = shown_workflow.id();
        renderer.write_layered_document(&workflow_scenes, shown_workflow_id, shown_angle)
    }

    /// Let `A` pick the arrangements of `layers`, one workflow's disclosure layers, from its deepest layer as `shown_angle` sees it, lay out every layer in them, then project each, headed by `workflow`, from every angle.
    fn project_every_camera<'d, A: ArrangementChoice>(
        workflow: &Workflow,
        layers: &'d [DisclosedGraph],
        shown_angle: ViewAngle,
        font: &impl TextMeasure,
    ) -> Vec<CameraLayers<'d>> {
        let deepest_layer = layers.last().expect("disclosure yields layer 0 at least");
        let deepest_graph = deepest_layer.graph();
        let arrangements = match shown_angle {
            ViewAngle::Isometric => A::arrangements_of::<Isometric>(deepest_graph, font),
            ViewAngle::Top => A::arrangements_of::<Top>(deepest_graph, font),
        };
        let layer_plans = LevelPlans::lay_out_layers_with(layers, &arrangements, font);
        let isometric_layers =
            Self::project_every_layer::<Isometric>(workflow, layers, &layer_plans, font);
        let top_layers = Self::project_every_layer::<Top>(workflow, layers, &layer_plans, font);
        vec![isometric_layers, top_layers]
    }

    /// Project every disclosure layer of `layers`, laid out by its plans in `layer_plans`, as camera `C` sees it, labels measured by `font`, each headed by `workflow`.
    fn project_every_layer<'d, C: Camera>(
        workflow: &Workflow,
        layers: &'d [DisclosedGraph],
        layer_plans: &[LevelPlans],
        font: &impl TextMeasure,
    ) -> CameraLayers<'d> {
        let layer_scenes = layers
            .iter()
            .zip(layer_plans)
            .map(|(layer, plans)| {
                let unheaded_scene = Scene::project_disclosed::<C>(layer, plans, font);
                let headed_scene = unheaded_scene.headed_by(workflow, font);
                LayerScene::new(layer, headed_scene)
            })
            .collect();
        CameraLayers::seen_by::<C>(layer_scenes)
    }
}

impl ArrangementChoice for Searched {
    fn arrangements_of<C: Camera>(
        deepest_graph: &WorkflowGraph,
        font: &impl TextMeasure,
    ) -> Arrangements {
        let search = ArrangementSearch::<_, C>::new(deepest_graph, font);
        let (arrangements, _deepest_plans) = search.best();
        arrangements
    }
}

impl ArrangementChoice for Seeded {
    fn arrangements_of<C: Camera>(
        deepest_graph: &WorkflowGraph,
        font: &impl TextMeasure,
    ) -> Arrangements {
        let (_seed_plans, seed_arrangements) = LevelPlans::lay_out_seeded(deepest_graph, font);
        seed_arrangements
    }
}
