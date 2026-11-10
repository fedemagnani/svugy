// Ported from rough.js (https://github.com/rough-stuff/rough) and roughr (https://github.com/orhanbalci/rough-rs).
//
// rough.js:
//
// MIT License
//
// Copyright (c) 2019 Preet Shihn
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//
// roughr:
//
// MIT License
//
// Copyright (c) 2022 Orhan Balci
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Sketch geometry as hand-drawn strokes and hachure fills; ported from rough.js / roughr (MIT).

pub(crate) use self::hachure::Hachure;
pub(crate) use self::pencil::Pencil;
pub(crate) use self::seed::Seed;

mod hachure;
mod pencil;
mod seed;

#[cfg(test)]
mod tests {
    use kurbo::{BezPath, Point, Rect};

    use super::{Hachure, Pencil, Seed};

    #[test]
    fn sketching_the_same_path_replays_the_same_strokes() {
        let pencil = Pencil::new(1.0, 1.0, 2.0);
        let mut outline = BezPath::new();
        outline.move_to((0.0, 0.0));
        outline.line_to((120.0, 0.0));
        outline.quad_to((160.0, 40.0), (120.0, 80.0));
        outline.curve_to((80.0, 110.0), (30.0, 60.0), (0.0, 80.0));
        outline.close_path();

        let first_seed = Seed::from_path(&outline);
        let first_sketch = pencil.sketch_path(&outline, first_seed);
        let replayed_outline = outline.clone();
        let replay_seed = Seed::from_path(&replayed_outline);
        let replay_sketch = pencil.sketch_path(&replayed_outline, replay_seed);

        assert_eq!(
            first_sketch, replay_sketch,
            "a sketch must depend only on the pencil and the path"
        );
    }

    #[test]
    fn hachure_fill_strokes_stay_within_the_polygon_bounds_widened_by_the_pencil_drift() {
        let roughness = 1.5;
        let bowing = 1.0;
        let max_randomness_offset = 2.0;
        let pencil = Pencil::new(roughness, bowing, max_randomness_offset);
        let hachure = Hachure::new(-41.0, 4.0);
        let rhombus = [
            Point::new(0.0, 50.0),
            Point::new(90.0, 0.0),
            Point::new(180.0, 50.0),
            Point::new(90.0, 100.0),
        ];
        let [first_vertex, other_vertices @ ..] = rhombus;
        let mut rhombus_outline = BezPath::new();
        rhombus_outline.move_to(first_vertex);
        for vertex in other_vertices {
            rhombus_outline.line_to(vertex);
        }
        rhombus_outline.close_path();
        let seed = Seed::from_path(&rhombus_outline).derived_for_stroke("hachure");

        let fill = pencil.sketch_hachure_fill(&rhombus, &hachure, seed);

        let rhombus_bounds = Rect::new(0.0, 0.0, 180.0, 100.0);
        let longest_hachure_line = rhombus_bounds.size().to_vec2().hypot2().sqrt();
        let max_bow = bowing * max_randomness_offset * longest_hachure_line / 200.0;
        let max_drift = roughness * (max_randomness_offset + max_bow);
        let allowed_bounds = rhombus_bounds.inflate(max_drift, max_drift);
        let fill_control_box = fill.control_box();
        assert!(!fill.is_empty(), "a hachure fill must draw strokes");
        assert!(
            allowed_bounds.contains_rect(fill_control_box),
            "every hachure point must stay within the polygon bounds widened by the pencil drift: {fill_control_box:?} escapes {allowed_bounds:?}"
        );
    }
}
