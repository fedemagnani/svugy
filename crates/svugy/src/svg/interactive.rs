use svugy_model::disclose::DisclosedModel;
use svugy_model::model::Model;

use super::RenderSvg;
use crate::arrange::ArrangementSearch;
use crate::geometry::Scene;
use crate::layout::{Arrangements, LevelPlans};
use crate::metrics::{BundledFont, TextMeasure};
use crate::render::{Interactive, InteractiveRenderer, LayerScene, Sketch, Standard, Theme};

/// Renders a model as the interactive SVG, the product: themed for light and dark, animated, explorable by its embedded runtime, laid out in the most readable arrangement its search finds.
pub struct InteractiveSvg;

/// Renders a model as the interactive SVG laid out in the seed arrangement, skipping the search: a quick draft to show while the searched render runs.
pub struct DraftInteractiveSvg;

/// Renders a model as the interactive SVG in the hand-drawn sketch theme: hatched faces, wobbly strokes, labels in Excalifont.
pub struct SketchInteractiveSvg;

/// Renders a model as the sketch-themed interactive SVG laid out in the seed arrangement: the draft to show while the searched sketch render runs.
pub struct SketchDraftInteractiveSvg;

/// Picks the arrangements every layer of an interactive document is laid out in, from the deepest layer's model.
trait ArrangementChoice {
    fn arrangements_of(deepest_model: &Model, font: &impl TextMeasure) -> Arrangements;
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
    /// Disclose `model` layer by layer, let `C` pick the deepest layer's arrangements, lay out every layer in them, project each in theme `T`'s font, then write them all as the interactive document in `T`.
    fn render_arranged_by<T: Theme, C: ArrangementChoice>(model: &Model) -> String {
        let font = BundledFont::<T::Faces>::new();
        let layers = DisclosedModel::every_layer_of(model);
        let Some(deepest_layer) = layers.last() else {
            let renderer = InteractiveRenderer::new(Interactive::<T>::new());
            return renderer.write_layered_document(model, &[]);
        };
        let deepest_model = deepest_layer.model();
        let arrangements = C::arrangements_of(deepest_model, &font);
        let layer_plans = LevelPlans::lay_out_layers_with(&layers, &arrangements, &font);
        let layer_scenes: Vec<LayerScene> = layers
            .iter()
            .zip(&layer_plans)
            .map(|(layer, plans)| {
                let scene = Scene::project_disclosed(layer, plans, &font);
                LayerScene::new(layer, scene)
            })
            .collect();
        let renderer = InteractiveRenderer::new(Interactive::<T>::new());
        renderer.write_layered_document(model, &layer_scenes)
    }
}

impl ArrangementChoice for Searched {
    fn arrangements_of(deepest_model: &Model, font: &impl TextMeasure) -> Arrangements {
        let search = ArrangementSearch::new(deepest_model, font);
        let (arrangements, _deepest_plans) = search.best();
        arrangements
    }
}

impl ArrangementChoice for Seeded {
    fn arrangements_of(deepest_model: &Model, font: &impl TextMeasure) -> Arrangements {
        let (_seed_plans, seed_arrangements) = LevelPlans::lay_out_seeded(deepest_model, font);
        seed_arrangements
    }
}
