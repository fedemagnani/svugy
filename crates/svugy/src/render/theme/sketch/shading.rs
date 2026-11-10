//! Shade faces with a wash and hachure, as a pencil sketch does.

use kurbo::{BezPath, ParamCurve, PathSeg, Point, Shape as _};

use crate::geometry::{Face, FaceShade};
use crate::render::contract::RuntimeContract;
use crate::render::theme::Token;
use crate::render::theme::paint::Paint;
use crate::render::theme::shading::Shading;
use crate::render::writer::css::CssValue;
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter};
use crate::sketch::{Hachure, Pencil, Seed};

/// Washes every face in its shade over the canvas, then hatches it in that shade.
pub(crate) struct Hatched;

impl Hatched {
    /// Opacity of the shade washed over the canvas: the area reads as mix(canvas, shade, 0.25).
    const WASH_OPACITY: f64 = 0.25;
    /// Trace hachure lines.
    const HACHURE_PENCIL: Pencil = Pencil::new(1.0, 1.0, 2.0);
    /// Width of a hachure line.
    const HACHURE_STROKE_WIDTH: f64 = 0.8;
    /// Distance between hachure lines on a left face, the reference the other shades scale.
    const HACHURE_GAP: f64 = 4.0;
    /// Area above which a face is large enough that dense hachure turns into noise.
    const LARGE_FACE_AREA: f64 = 15_000.0;
    /// Widen the hachure gap of a large face by this much.
    const LARGE_FACE_GAP_GAIN: f64 = 2.5;
    /// Sample each curved outline segment at this many points, for the polygon hachure is clipped to.
    const CURVE_SAMPLE_COUNT: usize = 16;
    /// Width of the halo around a label set on a face.
    const HALO_WIDTH: f64 = 2.5;

    /// Write `face`'s hachure strokes in `shade_stroke`, unfilled and marked so they never take pointer events or highlights.
    fn write_hachure(writer: &mut SvgWriter, face: &Face, shade_stroke: &CssValue) {
        let hachure = Self::hachure_of(face);
        let polygon = Self::polygon_of(&face.outline);
        let outline_seed = Seed::from_path(&face.outline);
        let hachure_seed = outline_seed.salted("hachure");
        let hachure_strokes =
            Self::HACHURE_PENCIL.sketch_hachure_fill(&polygon, &hachure, hachure_seed);
        let hachure_data = PathData(&hachure_strokes).to_string();
        let hachure_attributes = Attributes::new()
            .with_text(RuntimeContract::HACHURE, "")
            .with_text("d", &hachure_data)
            .with_text("fill", "none")
            .with_css_value("stroke", shade_stroke)
            .with_number("stroke-width", Self::HACHURE_STROKE_WIDTH)
            .with_text("stroke-linecap", "round");
        writer.empty_element("path", &hachure_attributes);
    }

    /// Hatch by shade: top lines horizontal, left ones steep rising right, right ones steep falling right; gap = 4 · (1.25 top, 1 left, 0.8 right) · (2.5 when the face exceeds 15 000 px², else 1).
    fn hachure_of(face: &Face) -> Hachure {
        let (angle_degrees, shade_gap_gain) = match face.shade {
            FaceShade::Top => (-90.0, 1.25),
            FaceShade::Left => (-30.0, 1.0),
            FaceShade::Right => (30.0, 0.8),
        };
        let signed_area = face.outline.area();
        let is_large_face = signed_area.abs() > Self::LARGE_FACE_AREA;
        let size_gap_gain = if is_large_face {
            Self::LARGE_FACE_GAP_GAIN
        } else {
            1.0
        };
        let gap = Self::HACHURE_GAP * shade_gap_gain * size_gap_gain;
        Hachure::new(angle_degrees, gap)
    }

    /// Approximate `outline` by the vertex ring hachure is clipped to: each line's start, and each curve sampled at even parameter steps.
    fn polygon_of(outline: &BezPath) -> Vec<Point> {
        let mut polygon = Vec::new();
        for segment in outline.segments() {
            let is_line = matches!(segment, PathSeg::Line(_));
            let sample_count = if is_line { 1 } else { Self::CURVE_SAMPLE_COUNT };
            let samples = (0..sample_count).map(|sample_index| {
                let parameter = sample_index as f64 / sample_count as f64;
                segment.eval(parameter)
            });
            polygon.extend(samples);
        }
        polygon
    }
}

impl Shading for Hatched {
    /// Write an opaque canvas underlay, so the face still hides what lies behind it, the translucent shade wash over it, both marked as the face's fill, then the hachure; a face without shade draws no area.
    fn write_area(
        writer: &mut SvgWriter,
        face: &Face,
        shade: Paint,
        area_presentation: Attributes,
        paint_value: impl Fn(Paint) -> CssValue,
    ) {
        let is_unshaded = shade == Paint::None;
        if is_unshaded {
            return;
        }
        let outline_data = PathData(&face.outline).to_string();
        let canvas_paint = Paint::Token(Token::Canvas);
        let canvas_fill = paint_value(canvas_paint);
        let shade_value = paint_value(shade);
        let underlay_attributes = Attributes::new()
            .with_text(RuntimeContract::FACE_FILL, "")
            .with_text("d", &outline_data)
            .with_css_value("fill", &canvas_fill);
        writer.empty_element("path", &underlay_attributes);
        let wash_attributes = area_presentation
            .with_text(RuntimeContract::FACE_FILL, "")
            .with_text("d", &outline_data)
            .with_css_value("fill", &shade_value)
            .with_number("fill-opacity", Self::WASH_OPACITY);
        writer.empty_element("path", &wash_attributes);
        Self::write_hachure(writer, face, &shade_value);
    }

    /// Keep a palette's ink, chosen for its washed surfaces, but swap an ink derived from an authored base, chosen for the full base, for the canvas text the wash stays close to.
    fn label_ink(solid_ink: Paint) -> Paint {
        let is_derived_from_authored_base = matches!(solid_ink, Paint::Literal(_));
        if is_derived_from_authored_base {
            return Paint::Token(Token::Text);
        }
        solid_ink
    }

    /// Stroke the label's letters in the canvas under their fill, close to the washed face, so hachure stops short of them.
    fn halo_label(presentation: Attributes, paint_value: impl Fn(Paint) -> CssValue) -> Attributes {
        let canvas_paint = Paint::Token(Token::Canvas);
        let canvas_stroke = paint_value(canvas_paint);
        presentation
            .with_css_value("stroke", &canvas_stroke)
            .with_number("stroke-width", Self::HALO_WIDTH)
            .with_text("stroke-linejoin", "round")
            .with_text("paint-order", "stroke")
    }
}
