//! Render a svugy model as an isometric, explorable SVG.

mod geometry;
mod layout;
mod metrics;
mod render;
mod svg;
#[cfg(test)]
mod svugy_example;

pub use svg::{InteractiveSvg, RenderSvg, StaticSvg};
