use svugy_model::disclose::DisclosedModel;
use svugy_model::model::Model;

use super::RenderSvg;
use crate::geometry::Scene;
use crate::layout::LevelPlans;
use crate::metrics::BundledFont;
use crate::render::{Interactive, InteractiveRenderer, LayerScene};

/// Renders a model as the interactive SVG, the product: themed for light and dark, animated, explorable by its embedded runtime.
pub struct InteractiveSvg;

impl RenderSvg for InteractiveSvg {
    /// Disclose `model` layer by layer, lay out every layer consistently with the deepest one, project each in the bundled font, then write them all as the interactive document.
    fn render(model: &Model) -> String {
        let font = BundledFont::new();
        let layers = DisclosedModel::every_layer_of(model);
        let layer_plans = LevelPlans::lay_out_layers(&layers, &font);
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
