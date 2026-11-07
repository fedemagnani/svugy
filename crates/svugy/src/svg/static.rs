use svugy_model::model::Model;

use super::RenderSvg;
use crate::geometry::Scene;
use crate::layout::LevelPlans;
use crate::metrics::BundledFont;
use crate::render::{Static, StaticRenderer};

/// Renders a model as a static SVG: self-contained, for viewers without CSS, script or fonts.
pub struct StaticSvg;

impl RenderSvg for StaticSvg {
    /// Lay `model` out and project it in the bundled font, then write the scene as the static document, its labels outlined in that font.
    fn render(model: &Model) -> String {
        let font = BundledFont::new();
        let plans = LevelPlans::lay_out(model, &font);
        let scene = Scene::project(model, &plans, &font);
        let profile = Static::new(&font);
        let renderer = StaticRenderer::new(profile);
        renderer.write_document(model, &scene)
    }
}
