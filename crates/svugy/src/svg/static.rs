use svugy_model::disclose::DisclosedModel;
use svugy_model::model::{Model, ViewAngle};

use super::{RenderSvg, RenderSvgLayers};
use crate::arrange::ArrangementSearch;
use crate::geometry::{Camera, Isometric, Scene, Top};
use crate::layout::LevelPlans;
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

impl RenderSvgLayers for StaticSvg {
    fn render_every_layer(model: &Model) -> Vec<String> {
        StaticSvg::render_every_layer_themed::<Standard>(model)
    }
}

impl RenderSvgLayers for SketchStaticSvg {
    fn render_every_layer(model: &Model) -> Vec<String> {
        StaticSvg::render_every_layer_themed::<Sketch>(model)
    }
}

impl StaticSvg {
    /// Render `model` in theme `T` from the angle its author picks, isometric when none.
    fn render_themed<T: Theme>(model: &Model) -> String {
        let view = model.view().unwrap_or_default();
        match view {
            ViewAngle::Isometric => Self::render_seen_by::<T, Isometric>(model),
            ViewAngle::Top => Self::render_seen_by::<T, Top>(model),
        }
    }

    /// Lay `model` out in its most readable arrangement as camera `C` sees it and project it in theme `T`'s font, then write the scene as the static document in `T`, its labels outlined in that font.
    fn render_seen_by<T: Theme, C: Camera>(model: &Model) -> String {
        let font = BundledFont::<T::Faces>::new();
        let search = ArrangementSearch::<_, C>::new(model, &font);
        let (_arrangements, plans) = search.best();
        let scene = Scene::project::<C>(model, &plans, &font);
        let profile = Static::<T>::new(&font);
        let renderer = StaticRenderer::new(profile);
        renderer.write_document(model, &scene)
    }

    /// Render every disclosure layer of `model` in theme `T` from the angle its author picks, isometric when none.
    fn render_every_layer_themed<T: Theme>(model: &Model) -> Vec<String> {
        let view = model.view().unwrap_or_default();
        match view {
            ViewAngle::Isometric => Self::render_every_layer_seen_by::<T, Isometric>(model),
            ViewAngle::Top => Self::render_every_layer_seen_by::<T, Top>(model),
        }
    }

    /// Disclose `model` layer by layer, lay every layer out in the deepest layer's most readable arrangement as camera `C` sees it, as the interactive SVG does, project each in theme `T`'s font, then write each scene as its own static document in `T`.
    fn render_every_layer_seen_by<T: Theme, C: Camera>(model: &Model) -> Vec<String> {
        let font = BundledFont::<T::Faces>::new();
        let layers = DisclosedModel::every_layer_of(model);
        let deepest_layer = layers.last().expect("disclosure yields layer 0 at least");
        let deepest_model = deepest_layer.model();
        let search = ArrangementSearch::<_, C>::new(deepest_model, &font);
        let (arrangements, _deepest_plans) = search.best();
        let layer_plans = LevelPlans::lay_out_layers_with(&layers, &arrangements, &font);
        let profile = Static::<T>::new(&font);
        let renderer = StaticRenderer::new(profile);
        layers
            .iter()
            .zip(&layer_plans)
            .map(|(layer, plans)| {
                let scene = Scene::project_disclosed::<C>(layer, plans, &font);
                let layer_model = layer.model();
                renderer.write_document(layer_model, &scene)
            })
            .collect()
    }
}
