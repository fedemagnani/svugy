use kurbo::{BezPath, CubicBez, Line, PathSeg, Point, Vec2};

use super::hachure::Hachure;
use super::seed::{Seed, SplitMix64};

/// Redraws geometry as double, jittered strokes, like a hand-held pencil.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pencil {
    /// Scale of every random offset; 0 draws crisp strokes.
    roughness: f64,
    /// Sideways bow of a straight stroke, relative to its length.
    bowing: f64,
    /// Largest drift of a stroke end or control point, before roughness.
    max_randomness_offset: f64,
}

impl Pencil {
    /// Extra drift of a curve's second stroke, so it parts from the first.
    const SECOND_CURVE_STROKE_EXTRA_OFFSET: f64 = 0.3;

    /// Pencil drifting up to `max_randomness_offset`, scaled by `roughness`, bowing lines by `bowing`.
    pub(crate) const fn new(roughness: f64, bowing: f64, max_randomness_offset: f64) -> Self {
        Self {
            roughness,
            bowing,
            max_randomness_offset,
        }
    }

    /// Sketch every segment of `path` as two strokes jittered by `seed`.
    pub(crate) fn sketch_path(&self, path: &BezPath, seed: Seed) -> BezPath {
        let mut jitter = SplitMix64::new(seed);
        let mut sketched = BezPath::new();
        for segment in path.segments() {
            match segment {
                PathSeg::Line(line) if line.p0 == line.p1 => {}
                PathSeg::Line(line) => self.trace_double_line(line, &mut jitter, &mut sketched),
                PathSeg::Quad(quad) => {
                    let cubic = quad.raise();
                    self.trace_double_cubic(cubic, &mut jitter, &mut sketched);
                }
                PathSeg::Cubic(cubic) => {
                    self.trace_double_cubic(cubic, &mut jitter, &mut sketched);
                }
            }
        }
        sketched
    }

    /// Sketch every segment of `path` as one stroke jittered by `seed`, for dashed strokes a second stroke would garble.
    pub(crate) fn sketch_path_once(&self, path: &BezPath, seed: Seed) -> BezPath {
        let mut jitter = SplitMix64::new(seed);
        let mut sketched = BezPath::new();
        for segment in path.segments() {
            match segment {
                PathSeg::Line(line) if line.p0 == line.p1 => {}
                PathSeg::Line(line) => self.trace_single_line(line, &mut jitter, &mut sketched),
                PathSeg::Quad(quad) => {
                    let cubic = quad.raise();
                    let drift = self.max_randomness_offset;
                    self.trace_jittered_cubic(cubic.p0, cubic, drift, &mut jitter, &mut sketched);
                }
                PathSeg::Cubic(cubic) => {
                    let drift = self.max_randomness_offset;
                    self.trace_jittered_cubic(cubic.p0, cubic, drift, &mut jitter, &mut sketched);
                }
            }
        }
        sketched
    }

    /// Sketch `hachure`'s lines inside `polygon`, each as two strokes jittered by `seed`.
    pub(crate) fn sketch_hachure_fill(
        &self,
        polygon: &[Point],
        hachure: &Hachure,
        seed: Seed,
    ) -> BezPath {
        let hachure_lines = hachure.lines_inside(polygon);
        let mut jitter = SplitMix64::new(seed);
        let mut sketched = BezPath::new();
        for line in hachure_lines {
            self.trace_double_line(line, &mut jitter, &mut sketched);
        }
        sketched
    }

    /// Append two bowed strokes along `line` to `sketched`, the second drifting half as far.
    fn trace_double_line(&self, line: Line, jitter: &mut SplitMix64, sketched: &mut BezPath) {
        let length_sq = line.p0.distance_squared(line.p1);
        let length = length_sq.sqrt();
        let roughness_gain = Self::roughness_gain_for_length(length);
        let drift = self.max_randomness_offset.min(length / 10.0);
        let half_drift = drift / 2.0;
        self.trace_bowed_stroke(line, drift, roughness_gain, jitter, sketched);
        self.trace_bowed_stroke(line, half_drift, roughness_gain, jitter, sketched);
    }

    /// Append one bowed stroke along `line` to `sketched`.
    fn trace_single_line(&self, line: Line, jitter: &mut SplitMix64, sketched: &mut BezPath) {
        let length_sq = line.p0.distance_squared(line.p1);
        let length = length_sq.sqrt();
        let roughness_gain = Self::roughness_gain_for_length(length);
        let drift = self.max_randomness_offset.min(length / 10.0);
        self.trace_bowed_stroke(line, drift, roughness_gain, jitter, sketched);
    }

    /// Damp long strokes: 1 below length 200, 0.4 above 500, −0.0016668·length + 1.233334 between.
    fn roughness_gain_for_length(length: f64) -> f64 {
        if length < 200.0 {
            return 1.0;
        }
        if length > 500.0 {
            return 0.4;
        }
        -0.0016668 * length + 1.233334
    }

    /// Append one cubic stroke from near `line.p0` to near `line.p1`, bowed sideways and with ends and controls drifting by up to `drift`.
    fn trace_bowed_stroke(
        &self,
        line: Line,
        drift: f64,
        roughness_gain: f64,
        jitter: &mut SplitMix64,
        sketched: &mut BezPath,
    ) {
        let direction = line.p1 - line.p0;
        let diverge_point = 0.2 + jitter.next_unit() * 0.2;
        let bow_extent = self.bowing * self.max_randomness_offset / 200.0;
        let bow_normal = Vec2::new(direction.y, -direction.x) * bow_extent;
        let bow = self.random_shift(bow_normal, roughness_gain, jitter);
        let drift_extent = Vec2::new(drift, drift);
        let start_shift = self.random_shift(drift_extent, roughness_gain, jitter);
        let control1_shift = self.random_shift(drift_extent, roughness_gain, jitter);
        let control2_shift = self.random_shift(drift_extent, roughness_gain, jitter);
        let end_shift = self.random_shift(drift_extent, roughness_gain, jitter);
        let bowed_start = line.p0 + bow;
        let start = line.p0 + start_shift;
        let control1 = bowed_start + direction * diverge_point + control1_shift;
        let control2 = bowed_start + direction * (2.0 * diverge_point) + control2_shift;
        let end = line.p1 + end_shift;
        sketched.move_to(start);
        sketched.curve_to(control1, control2, end);
    }

    /// Append two strokes along `cubic` to `sketched`, the second starting off its start and drifting further.
    fn trace_double_cubic(&self, cubic: CubicBez, jitter: &mut SplitMix64, sketched: &mut BezPath) {
        let first_drift = self.max_randomness_offset;
        let second_drift = first_drift + Self::SECOND_CURVE_STROKE_EXTRA_OFFSET;
        self.trace_jittered_cubic(cubic.p0, cubic, first_drift, jitter, sketched);
        let first_drift_extent = Vec2::new(first_drift, first_drift);
        let second_start_shift = self.random_shift(first_drift_extent, 1.0, jitter);
        let second_start = cubic.p0 + second_start_shift;
        self.trace_jittered_cubic(second_start, cubic, second_drift, jitter, sketched);
    }

    /// Append one stroke from `start` along `cubic`, its controls and end drifting by up to `drift`.
    fn trace_jittered_cubic(
        &self,
        start: Point,
        cubic: CubicBez,
        drift: f64,
        jitter: &mut SplitMix64,
        sketched: &mut BezPath,
    ) {
        let drift_extent = Vec2::new(drift, drift);
        let end_shift = self.random_shift(drift_extent, 1.0, jitter);
        let control1_shift = self.random_shift(drift_extent, 1.0, jitter);
        let control2_shift = self.random_shift(drift_extent, 1.0, jitter);
        let control1 = cubic.p1 + control1_shift;
        let control2 = cubic.p2 + control2_shift;
        let end = cubic.p3 + end_shift;
        sketched.move_to(start);
        sketched.curve_to(control1, control2, end);
    }

    /// Draw a shift whose components are each roughness · gain · U(−extent, extent).
    fn random_shift(&self, extent: Vec2, roughness_gain: f64, jitter: &mut SplitMix64) -> Vec2 {
        let shift_x = self.random_offset(extent.x, roughness_gain, jitter);
        let shift_y = self.random_offset(extent.y, roughness_gain, jitter);
        Vec2::new(shift_x, shift_y)
    }

    /// Draw roughness · gain · U(−extent, extent).
    fn random_offset(&self, extent: f64, roughness_gain: f64, jitter: &mut SplitMix64) -> f64 {
        let unit = jitter.next_unit();
        let uniform_offset = unit * 2.0 * extent - extent;
        self.roughness * roughness_gain * uniform_offset
    }
}
