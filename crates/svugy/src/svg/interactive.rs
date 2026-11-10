use svugy_model::disclose::DisclosedModel;
use svugy_model::model::{Model, ViewAngle};

use super::RenderSvg;
use crate::arrange::ArrangementSearch;
use crate::geometry::{Camera, Isometric, Scene, Top};
use crate::layout::{Arrangements, LevelPlans};
use crate::metrics::{BundledFont, TextMeasure};
use crate::render::{
    CameraLayers, Interactive, InteractiveRenderer, LayerScene, Sketch, Standard, Theme,
};

/// Renders a model as the interactive SVG, the product: themed for light and dark, animated, explorable by its embedded runtime, laid out in the most readable arrangement its search finds.
pub struct InteractiveSvg;

/// Renders a model as the interactive SVG laid out in the seed arrangement, skipping the search: a quick draft to show while the searched render runs.
pub struct DraftInteractiveSvg;

/// Renders a model as the interactive SVG in the hand-drawn sketch theme: hatched faces, wobbly strokes, labels in Excalifont.
pub struct SketchInteractiveSvg;

/// Renders a model as the sketch-themed interactive SVG laid out in the seed arrangement: the draft to show while the searched sketch render runs.
pub struct SketchDraftInteractiveSvg;

/// Picks the arrangements every layer of an interactive document is laid out in, from the deepest layer's model as camera `C` sees it.
trait ArrangementChoice {
    fn arrangements_of<C: Camera>(deepest_model: &Model, font: &impl TextMeasure) -> Arrangements;
}

/// Picks the most readable arrangements the search finds.
struct Searched;

/// Picks the seed arrangements the search would start from.
struct Seeded;

impl RenderSvg for InteractiveSvg {
    fn render(model: &Model) -> String {
        InteractiveSvg::render_arranged_by::<Standard, Searched>(model)
    }
}

impl RenderSvg for DraftInteractiveSvg {
    fn render(model: &Model) -> String {
        InteractiveSvg::render_arranged_by::<Standard, Seeded>(model)
    }
}

impl RenderSvg for SketchInteractiveSvg {
    fn render(model: &Model) -> String {
        InteractiveSvg::render_arranged_by::<Sketch, Searched>(model)
    }
}

impl RenderSvg for SketchDraftInteractiveSvg {
    fn render(model: &Model) -> String {
        InteractiveSvg::render_arranged_by::<Sketch, Seeded>(model)
    }
}

impl InteractiveSvg {
    /// Disclose `model` layer by layer, let `A` pick the deepest layer's arrangements from the angle the author picks, lay out every layer in them, project each from every angle in theme `T`'s font, then write them all as the interactive document in `T`, the author's angle shown.
    fn render_arranged_by<T: Theme, A: ArrangementChoice>(model: &Model) -> String {
        let font = BundledFont::<T::Faces>::new();
        let layers = DisclosedModel::every_layer_of(model);
        let shown_angle = model.view().unwrap_or_default();
        let deepest_layer = layers.last().expect("disclosure yields layer 0 at least");
        let deepest_model = deepest_layer.model();
        let arrangements = match shown_angle {
            ViewAngle::Isometric => A::arrangements_of::<Isometric>(deepest_model, &font),
            ViewAngle::Top => A::arrangements_of::<Top>(deepest_model, &font),
        };
        let layer_plans = LevelPlans::lay_out_layers_with(&layers, &arrangements, &font);
        let isometric_layers = Self::project_every_layer::<Isometric>(&layers, &layer_plans, &font);
        let top_layers = Self::project_every_layer::<Top>(&layers, &layer_plans, &font);
        let camera_layers = [isometric_layers, top_layers];
        let renderer = InteractiveRenderer::new(Interactive::<T>::new());
        renderer.write_layered_document(model, shown_angle, &camera_layers)
    }

    /// Project every disclosure layer of `layers`, laid out by its plans in `layer_plans`, as camera `C` sees it, labels measured by `font`.
    fn project_every_layer<'d, C: Camera>(
        layers: &'d [DisclosedModel],
        layer_plans: &[LevelPlans],
        font: &impl TextMeasure,
    ) -> CameraLayers<'d> {
        let layer_scenes = layers
            .iter()
            .zip(layer_plans)
            .map(|(layer, plans)| {
                let scene = Scene::project_disclosed::<C>(layer, plans, font);
                LayerScene::new(layer, scene)
            })
            .collect();
        CameraLayers::seen_by::<C>(layer_scenes)
    }
}

impl ArrangementChoice for Searched {
    fn arrangements_of<C: Camera>(deepest_model: &Model, font: &impl TextMeasure) -> Arrangements {
        let search = ArrangementSearch::<_, C>::new(deepest_model, font);
        let (arrangements, _deepest_plans) = search.best();
        arrangements
    }
}

impl ArrangementChoice for Seeded {
    fn arrangements_of<C: Camera>(deepest_model: &Model, font: &impl TextMeasure) -> Arrangements {
        let (_seed_plans, seed_arrangements) = LevelPlans::lay_out_seeded(deepest_model, font);
        seed_arrangements
    }
}
