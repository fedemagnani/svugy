use kurbo::{Affine, BezPath, Point};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use super::{TextAnchor, TextSize};

/// Stands in for every character the face has no glyph for.
const NOTDEF_GLYPH: GlyphId = GlyphId(0);

/// Places the glyphs of one face at one pixel size along their advances.
pub(super) struct SizedFace<'f> {
    face: &'f Face<'static>,
    pixels_per_unit: f64,
}

impl<'f> SizedFace<'f> {
    /// Size `face` at `size` pixels per em.
    pub(super) fn new(face: &'f Face<'static>, size: f64) -> Self {
        let raw_units_per_em = face.units_per_em();
        let units_per_em = f64::from(raw_units_per_em);
        Self {
            face,
            pixels_per_unit: size / units_per_em,
        }
    }

    /// Measure the line box of `text` set with `letter_spacing`.
    pub(super) fn measure(&self, text: &str, letter_spacing: f64) -> TextSize {
        let width = self.label_width(text, letter_spacing);
        let raw_ascender_units = self.face.ascender();
        let raw_descender_units = self.face.descender();
        let raw_line_gap_units = self.face.line_gap();
        let ascender_units = f64::from(raw_ascender_units);
        let descender_units = f64::from(raw_descender_units);
        let line_gap_units = f64::from(raw_line_gap_units);
        let line_height_units = ascender_units - descender_units + line_gap_units;
        let baseline_from_top_units = ascender_units + line_gap_units / 2.0;
        TextSize {
            width,
            line_height: line_height_units * self.pixels_per_unit,
            baseline_from_top: baseline_from_top_units * self.pixels_per_unit,
        }
    }

    /// Outline `text` set with `letter_spacing` as glyph paths, anchored at `baseline_origin` by `anchor`.
    pub(super) fn outline(
        &self,
        text: &str,
        letter_spacing: f64,
        baseline_origin: Point,
        anchor: TextAnchor,
    ) -> BezPath {
        let label_width = self.label_width(text, letter_spacing);
        let width_before_origin = anchor.width_share_before_origin() * label_width;
        let label_start_x = baseline_origin.x - width_before_origin;
        let mut label_path = BezPath::new();
        for placed_glyph in self.place_glyphs(text, letter_spacing) {
            let glyph_origin = Point::new(label_start_x + placed_glyph.pen_x, baseline_origin.y);
            let mut glyph_pen = GlyphPen::new(&mut label_path, glyph_origin, self.pixels_per_unit);
            let _glyph_bounds = self
                .face
                .outline_glyph(placed_glyph.glyph_id, &mut glyph_pen);
        }
        label_path
    }

    /// Return the width of `text` set with `letter_spacing`, from the first pen position to the last glyph's advance.
    fn label_width(&self, text: &str, letter_spacing: f64) -> f64 {
        let last_glyph = self.place_glyphs(text, letter_spacing).last();
        last_glyph.map_or(0.0, |glyph| glyph.pen_x + glyph.advance)
    }

    /// Place the glyphs of `text` along their advances, each spaced `letter_spacing` past the last.
    fn place_glyphs<'t>(
        &'t self,
        text: &'t str,
        letter_spacing: f64,
    ) -> impl Iterator<Item = PlacedGlyph> + 't {
        let characters = text.chars().enumerate();
        characters.scan(0_u32, move |pen_units, (character_position, character)| {
            let glyph_id = self.face.glyph_index(character).unwrap_or(NOTDEF_GLYPH);
            let advance_units = self.face.glyph_hor_advance(glyph_id).unwrap_or(0);
            let letter_spacing_before = character_position as f64 * letter_spacing;
            let pen_x = f64::from(*pen_units) * self.pixels_per_unit + letter_spacing_before;
            *pen_units += u32::from(advance_units);
            let advance = f64::from(advance_units) * self.pixels_per_unit;
            let placed_glyph = PlacedGlyph {
                glyph_id,
                pen_x,
                advance,
            };
            Some(placed_glyph)
        })
    }
}

/// Positions one glyph by its pen offset from the label start, in pixels.
struct PlacedGlyph {
    glyph_id: GlyphId,
    pen_x: f64,
    advance: f64,
}

/// Appends glyph outlines to a path, mapping y-up font units to y-down pixels.
struct GlyphPen<'p> {
    path: &'p mut BezPath,
    font_units_to_pixels: Affine,
}

impl<'p> GlyphPen<'p> {
    /// Map (x, y) font units to (origin.x + s·x, origin.y − s·y) pixels.
    fn new(path: &'p mut BezPath, glyph_origin: Point, pixels_per_unit: f64) -> Self {
        let font_units_to_pixels = Affine::new([
            pixels_per_unit,
            0.0,
            0.0,
            -pixels_per_unit,
            glyph_origin.x,
            glyph_origin.y,
        ]);
        Self {
            path,
            font_units_to_pixels,
        }
    }

    /// Map font units (x, y) to pixels.
    fn to_pixels(&self, x: f32, y: f32) -> Point {
        let font_x = f64::from(x);
        let font_y = f64::from(y);
        let font_point = Point::new(font_x, font_y);
        self.font_units_to_pixels * font_point
    }
}

impl OutlineBuilder for GlyphPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let point = self.to_pixels(x, y);
        self.path.move_to(point);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let point = self.to_pixels(x, y);
        self.path.line_to(point);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let control = self.to_pixels(x1, y1);
        let point = self.to_pixels(x, y);
        self.path.quad_to(control, point);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let first_control = self.to_pixels(x1, y1);
        let second_control = self.to_pixels(x2, y2);
        let point = self.to_pixels(x, y);
        self.path.curve_to(first_control, second_control, point);
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}
