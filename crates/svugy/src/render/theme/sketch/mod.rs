//! Look hand-drawn: the sketch theme, its pencil stroke and its hatched shading.

mod shading;
mod stroke;

use svugy_model::color::Rgb;

use super::{Palette, SurfaceColors, Theme};
use crate::metrics::ExcalifontFaces;
use shading::Hatched;
use stroke::Sketched;

/// Looks hand-drawn like Excalidraw: Excalifont, sketched strokes, hatched faces, dark unless the viewer prefers light, static in light.
pub(crate) struct Sketch;

impl Theme for Sketch {
    type Faces = ExcalifontFaces;
    type Stroke = Sketched;
    type Shading = Hatched;
    const DARK_PALETTE: Palette = SKETCH_DARK_PALETTE;
    const LIGHT_PALETTE: Palette = SKETCH_LIGHT_PALETTE;
    const STATIC_PALETTE: Palette = SKETCH_LIGHT_PALETTE;
}

/// Paint every token for the sketch theme's light canvas: paper white, pastel surfaces, outlines inked like the text.
const SKETCH_LIGHT_PALETTE: Palette = Palette {
    canvas: Rgb::from_packed(0xfbfbf9),
    grid_line: Rgb::from_packed(0xececf0),
    face_outline: Rgb::from_packed(0x1e1e1e),
    text: Rgb::from_packed(0x1e1e1e),
    text_muted: Rgb::from_packed(0x5c5f66),
    external_outline: Rgb::from_packed(0x868e96),
    neutral: SurfaceColors::new(0xced4da, 0x1e1e1e),
    container_blue: SurfaceColors::new(0xd0ebff, 0x1864ab),
};

/// Paint every token for the sketch theme's dark canvas: near-black paper, muted surfaces, outlines inked like the text.
const SKETCH_DARK_PALETTE: Palette = Palette {
    canvas: Rgb::from_packed(0x121212),
    grid_line: Rgb::from_packed(0x1c1c20),
    face_outline: Rgb::from_packed(0xe3e3e8),
    text: Rgb::from_packed(0xe3e3e8),
    text_muted: Rgb::from_packed(0x868e96),
    external_outline: Rgb::from_packed(0x868e96),
    neutral: SurfaceColors::new(0x343a40, 0xe3e3e8),
    container_blue: SurfaceColors::new(0x1c3a5a, 0x74c0fc),
};
