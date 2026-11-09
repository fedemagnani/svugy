use svugy_model::disclose::DisclosedModel;
use svugy_model::model::Model;

use super::RenderSvg;
use crate::arrange::ArrangementSearch;
use crate::geometry::Scene;
use crate::layout::LevelPlans;
use crate::metrics::BundledFont;
use crate::render::{Interactive, InteractiveRenderer, LayerScene};

/// Renders a model as the interactive SVG, the product: themed for light and dark, animated, explorable by its embedded runtime.
pub struct InteractiveSvg;

impl RenderSvg for InteractiveSvg {
    /// Disclose `model` layer by layer, search the deepest layer's most readable arrangement, lay out every layer in it, project each in the bundled font, then write them all as the interactive document.
    fn render(model: &Model) -> String {
        let font = BundledFont::new();
        let layers = DisclosedModel::every_layer_of(model);
        let Some(deepest_layer) = layers.last() else {
            let renderer = InteractiveRenderer::new(Interactive);
            return renderer.write_layered_document(model, &[]);
        };
        let deepest_model = deepest_layer.model();
        let search = ArrangementSearch::new(deepest_model, &font);
        let (arrangements, _deepest_plans) = search.best();
        let layer_plans = LevelPlans::lay_out_layers_with(&layers, &arrangements, &font);
        let layer_scenes: Vec<LayerScene> = layers
            .iter()
            .zip(&layer_plans)
            .map(|(layer, plans)| {
                let scene = Scene::project_disclosed(layer, plans, &font);
                LayerScene::new(layer, scene)
            })
            .collect();
        let renderer = InteractiveRenderer::new(Interactive);
        renderer.write_layered_document(model, &layer_scenes)
    }
}
