//! Render a model as the SVG flavor the command line's profile and theme pick.

use clap::{Args, ValueEnum};
use svugy::{
    InteractiveSvg, RenderSvg, RenderSvgLayers, SketchInteractiveSvg, SketchStaticSvg, StaticSvg,
};
use svugy_model::model::ShownWorkflow;

use crate::workflow_choice::WORKFLOW_CHOICE_GROUP;

/// Render a model as the SVG of the chosen profile and theme.
#[derive(Args, Clone, Copy)]
pub struct SvgFlavor {
    /// Which SVG to write; static draws one workflow, so it needs --workflow or --all-workflows.
    #[arg(
        long,
        value_enum,
        default_value_t = RenderProfile::Interactive,
        requires_if("static", WORKFLOW_CHOICE_GROUP)
    )]
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
    /// Self-contained for <img> embeds and design tools: one workflow, no CSS, script, fonts or animation.
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

    /// Render the model of `shown` as the SVG of this flavor's profile and theme, showing its workflow.
    pub fn render(self, shown: ShownWorkflow<'_>) -> String {
        match (self.profile, self.theme) {
            (RenderProfile::Interactive, RenderTheme::Standard) => InteractiveSvg::render(shown),
            (RenderProfile::Interactive, RenderTheme::Sketch) => {
                SketchInteractiveSvg::render(shown)
            }
            (RenderProfile::Static, theme) => theme.render_static(shown),
        }
    }

    /// Return the theme this flavor draws in.
    pub fn theme(self) -> RenderTheme {
        self.theme
    }
}

impl RenderTheme {
    /// Render the shown workflow of `shown` as the static SVG drawn in this theme.
    pub fn render_static(self, shown: ShownWorkflow<'_>) -> String {
        match self {
            Self::Standard => StaticSvg::render(shown),
            Self::Sketch => SketchStaticSvg::render(shown),
        }
    }

    /// Render every disclosure layer of the shown workflow of `shown` as a static SVG drawn in this theme, layer 0 first.
    pub fn render_every_static_layer(self, shown: ShownWorkflow<'_>) -> Vec<String> {
        match self {
            Self::Standard => StaticSvg::render_every_layer(shown),
            Self::Sketch => SketchStaticSvg::render_every_layer(shown),
        }
    }
}
