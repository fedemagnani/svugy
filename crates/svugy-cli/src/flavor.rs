//! Render a model as the SVG flavor the command line's profile and theme pick.

use clap::{Args, ValueEnum};
use svugy::{InteractiveSvg, RenderSvg, SketchInteractiveSvg, SketchStaticSvg, StaticSvg};
use svugy_model::model::Model;

/// Render a model as the SVG of the chosen profile and theme.
#[derive(Args, Clone, Copy)]
pub struct SvgFlavor {
    /// Which SVG to write.
    #[arg(long, value_enum, default_value_t = RenderProfile::Interactive)]
    profile: RenderProfile,
    /// How to draw the diagram.
    #[arg(long, value_enum, default_value_t = RenderTheme::Standard)]
    theme: RenderTheme,
}

/// Name the SVG profile `render` writes.
#[derive(Clone, Copy, ValueEnum)]
pub enum RenderProfile {
    /// Explorable in a browser: themed, animated, with the embedded runtime.
    Interactive,
    /// Self-contained for <img> embeds and design tools: no CSS, script, fonts or animation.
    Static,
}

/// Name how the diagram is drawn.
#[derive(Clone, Copy, ValueEnum)]
pub enum RenderTheme {
    /// Crisp strokes, solid faces, labels in Inter.
    Standard,
    /// Hand-drawn: wobbly strokes, hatched faces, labels in Excalifont.
    Sketch,
}

impl SvgFlavor {
    /// Create a new flavor of `profile` drawn in `theme`.
    pub fn new(profile: RenderProfile, theme: RenderTheme) -> Self {
        Self { profile, theme }
    }

    /// Render `model` as the SVG of this flavor's profile and theme.
    pub fn render(self, model: &Model) -> String {
        match (self.profile, self.theme) {
            (RenderProfile::Interactive, RenderTheme::Standard) => InteractiveSvg::render(model),
            (RenderProfile::Interactive, RenderTheme::Sketch) => {
                SketchInteractiveSvg::render(model)
            }
            (RenderProfile::Static, RenderTheme::Standard) => StaticSvg::render(model),
            (RenderProfile::Static, RenderTheme::Sketch) => SketchStaticSvg::render(model),
        }
    }
}
