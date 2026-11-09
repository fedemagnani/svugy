//! Render a svugy model as an isometric, explorable SVG.

mod arrange;
mod geometry;
mod layout;
mod metrics;
mod render;
mod svg;
#[cfg(test)]
mod svugy_example;

pub use svg::{DraftInteractiveSvg, InteractiveSvg, RenderSvg, StaticSvg};
