use svugy_model::model::Model;

use super::RenderSvg;
use crate::arrange::ArrangementSearch;
use crate::geometry::Scene;
use crate::metrics::BundledFont;
use crate::render::{Sketch, Standard, Static, StaticRenderer, Theme};

/// Renders a model as a static SVG: self-contained, for viewers without CSS, script or fonts.
pub struct StaticSvg;

/// Renders a model as a static SVG in the hand-drawn sketch theme: self-contained, its labels outlined in Excalifont.
pub struct SketchStaticSvg;

impl RenderSvg for StaticSvg {
    fn render(model: &Model) -> String {
        StaticSvg::render_themed::<Standard>(model)
    }
}

impl RenderSvg for SketchStaticSvg {
    fn render(model: &Model) -> String {
        StaticSvg::render_themed::<Sketch>(model)
    }
}

impl StaticSvg {
    /// Lay `model` out in its most readable arrangement and project it in theme `T`'s font, then write the scene as the static document in `T`, its labels outlined in that font.
    fn render_themed<T: Theme>(model: &Model) -> String {
        let font = BundledFont::<T::Faces>::new();
        let search = ArrangementSearch::new(model, &font);
        let (_arrangements, plans) = search.best();
        let scene = Scene::project(model, &plans, &font);
        let profile = Static::<T>::new(&font);
        let renderer = StaticRenderer::new(profile);
        renderer.write_document(model, &scene)
    }
}
