//! Render the scene graph as an SVG document.

mod contract;
mod profile;
mod theme;
mod writer;

pub(crate) use profile::{InteractiveRenderer, Profile, Renderer, StaticRenderer};
