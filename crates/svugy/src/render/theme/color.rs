//! Convert colors between 8-bit sRGB and Oklab.

use glam::DVec3;
use svugy_model::color::Rgb;

/// Bound an 8-bit channel from above.
const CHANNEL_MAX: f64 = 255.0;
/// Tolerate rounding this far outside the unit sRGB cube.
const GAMUT_TOLERANCE: f64 = 1e-6;
/// Count the bisection steps of the gamut search.
const GAMUT_SEARCH_ITERATIONS: usize = 24;
/// Bound the linear segment of the sRGB decode.
const SRGB_DECODE_THRESHOLD: f64 = 0.040_45;
/// Bound the linear segment of the sRGB encode.
const SRGB_ENCODE_THRESHOLD: f64 = 0.003_130_8;
/// Scale the linear segment of the sRGB transfer.
const SRGB_LINEAR_SLOPE: f64 = 12.92;
/// Raise the power segment of the sRGB transfer.
const SRGB_GAMMA: f64 = 2.4;
/// Offset the power segment of the sRGB transfer.
const SRGB_OFFSET: f64 = 0.055;
/// Bound from below the Oklab lightness of a stroke that reads on the canvas.
const READABLE_STROKE_LIGHTNESS: f64 = 0.5;
/// Map linear sRGB to LMS cone responses, Ottosson's M₁ by rows.
const LINEAR_SRGB_TO_LMS: [DVec3; 3] = [
    DVec3::new(0.412_221_470_8, 0.536_332_536_3, 0.051_445_992_9),
    DVec3::new(0.211_903_498_2, 0.680_699_545_1, 0.107_396_956_6),
    DVec3::new(0.088_302_461_9, 0.281_718_837_6, 0.629_978_700_5),
];
/// Map cube-rooted LMS to Oklab, Ottosson's M₂ by rows.
const LMS_CUBE_ROOT_TO_OKLAB: [DVec3; 3] = [
    DVec3::new(0.210_454_255_3, 0.793_617_785_0, -0.004_072_046_8),
    DVec3::new(1.977_998_495_1, -2.428_592_205_0, 0.450_593_709_9),
    DVec3::new(0.025_904_037_1, 0.782_771_766_2, -0.808_675_766_0),
];
/// Map Oklab to cube-rooted LMS, M₂⁻¹ by rows.
const OKLAB_TO_LMS_CUBE_ROOT: [DVec3; 3] = [
    DVec3::new(1.0, 0.396_337_777_4, 0.215_803_757_3),
    DVec3::new(1.0, -0.105_561_345_8, -0.063_854_172_8),
    DVec3::new(1.0, -0.089_484_177_5, -1.291_485_548_0),
];
/// Map LMS to linear sRGB, M₁⁻¹ by rows.
const LMS_TO_LINEAR_SRGB: [DVec3; 3] = [
    DVec3::new(4.076_741_662_1, -3.307_711_591_3, 0.230_969_929_2),
    DVec3::new(-1.268_438_004_6, 2.609_757_401_1, -0.341_319_396_5),
    DVec3::new(-0.004_196_086_3, -0.703_418_614_7, 1.707_614_701_0),
];

/// Holds a color in Oklab: perceptual lightness plus two opponent axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Oklab {
    pub(crate) lightness: f64,
    pub(crate) green_red: f64,
    pub(crate) blue_yellow: f64,
}

impl Oklab {
    /// Convert an 8-bit sRGB `Rgb` to Oklab = M₂ · ∛(M₁ · linear_srgb), with Ottosson's M₁ and M₂.
    pub(crate) fn from_rgb(color: Rgb) -> Self {
        let red = f64::from(color.red);
        let green = f64::from(color.green);
        let blue = f64::from(color.blue);
        let encoded = DVec3::new(red, green, blue) / CHANNEL_MAX;
        let linear_srgb = encoded.map(Self::decode_srgb_transfer);
        let lms = Self::multiply_rows(&LINEAR_SRGB_TO_LMS, linear_srgb);
        let lms_cube_root = lms.map(libm::cbrt);
        let oklab = Self::multiply_rows(&LMS_CUBE_ROOT_TO_OKLAB, lms_cube_root);
        Oklab {
            lightness: oklab.x,
            green_red: oklab.y,
            blue_yellow: oklab.z,
        }
    }

    /// Tell whether arcs stroked in this color read on the canvas: L ≥ 0.5.
    pub(crate) fn is_readable_as_stroke(self) -> bool {
        self.lightness >= READABLE_STROKE_LIGHTNESS
    }

    /// Dim to k·(L, a, b), the Oklab image of scaling linear sRGB by k³, so hue and chroma-to-lightness ratio stay.
    pub(crate) fn dimmed(self, factor: f64) -> Self {
        Oklab {
            lightness: self.lightness * factor,
            green_red: self.green_red * factor,
            blue_yellow: self.blue_yellow * factor,
        }
    }

    /// Quantise to an 8-bit sRGB `Rgb` after gamut-mapping by scaling chroma, never by clamping channels.
    pub(crate) fn to_rgb(self) -> Rgb {
        let chroma_scale = self.largest_in_gamut_chroma_scale();
        let gamut_mapped = self.chroma_scaled(chroma_scale);
        let linear_srgb = gamut_mapped.to_linear_srgb();
        let encoded = linear_srgb.map(Self::encode_srgb_transfer);
        let red = Self::quantise_channel(encoded.x);
        let green = Self::quantise_channel(encoded.y);
        let blue = Self::quantise_channel(encoded.z);
        Rgb { red, green, blue }
    }

    /// Return the largest in-gamut chroma scale, argmax s ∈ [0, 1] such that (L, s·a, s·b) lies in the sRGB cube, by bisection.
    fn largest_in_gamut_chroma_scale(self) -> f64 {
        let is_lightness_displayable = (0.0..=1.0).contains(&self.lightness);
        debug_assert!(is_lightness_displayable, "lightness {}", self.lightness);
        let is_in_gamut = self.is_in_srgb_gamut();
        if is_in_gamut {
            return 1.0;
        }
        let mut in_gamut_scale = 0.0;
        let mut out_of_gamut_scale = 1.0;
        for _iteration in 0..GAMUT_SEARCH_ITERATIONS {
            let middle_scale = (in_gamut_scale + out_of_gamut_scale) / 2.0;
            let candidate = self.chroma_scaled(middle_scale);
            let is_candidate_in_gamut = candidate.is_in_srgb_gamut();
            if is_candidate_in_gamut {
                in_gamut_scale = middle_scale;
            } else {
                out_of_gamut_scale = middle_scale;
            }
        }
        in_gamut_scale
    }

    /// Tell whether the color lies in the sRGB cube, within the gamut tolerance.
    fn is_in_srgb_gamut(self) -> bool {
        let linear_srgb = self.to_linear_srgb();
        let lowest_channel = linear_srgb.min_element();
        let highest_channel = linear_srgb.max_element();
        let is_above_black = lowest_channel >= -GAMUT_TOLERANCE;
        let is_below_white = highest_channel <= 1.0 + GAMUT_TOLERANCE;
        is_above_black && is_below_white
    }

    /// Scale the chroma to (L, s·a, s·b), lightness kept.
    fn chroma_scaled(self, scale: f64) -> Self {
        Oklab {
            lightness: self.lightness,
            green_red: self.green_red * scale,
            blue_yellow: self.blue_yellow * scale,
        }
    }

    /// Convert to linear_srgb = M₁⁻¹ · (M₂⁻¹ · (L, a, b))³, cubed per component.
    fn to_linear_srgb(self) -> DVec3 {
        let oklab = DVec3::new(self.lightness, self.green_red, self.blue_yellow);
        let lms_cube_root = Self::multiply_rows(&OKLAB_TO_LMS_CUBE_ROOT, oklab);
        let lms = lms_cube_root * lms_cube_root * lms_cube_root;
        Self::multiply_rows(&LMS_TO_LINEAR_SRGB, lms)
    }

    /// Decode the sRGB transfer: c ≤ 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055)^2.4.
    fn decode_srgb_transfer(encoded: f64) -> f64 {
        let is_linear_segment = encoded <= SRGB_DECODE_THRESHOLD;
        if is_linear_segment {
            return encoded / SRGB_LINEAR_SLOPE;
        }
        let shifted = (encoded + SRGB_OFFSET) / (1.0 + SRGB_OFFSET);
        libm::pow(shifted, SRGB_GAMMA)
    }

    /// Encode the sRGB transfer: c ≤ 0.0031308 ? 12.92 · c : 1.055 · c^(1/2.4) − 0.055.
    fn encode_srgb_transfer(linear: f64) -> f64 {
        let is_linear_segment = linear <= SRGB_ENCODE_THRESHOLD;
        if is_linear_segment {
            return linear * SRGB_LINEAR_SLOPE;
        }
        let powered = libm::pow(linear, 1.0 / SRGB_GAMMA);
        (1.0 + SRGB_OFFSET) * powered - SRGB_OFFSET
    }

    /// Multiply the matrix given by its `rows` with `vector`: (r₀·v, r₁·v, r₂·v).
    fn multiply_rows(rows: &[DVec3; 3], vector: DVec3) -> DVec3 {
        let first = rows[0].dot(vector);
        let second = rows[1].dot(vector);
        let third = rows[2].dot(vector);
        DVec3::new(first, second, third)
    }

    /// Quantise an encoded channel to 8 bits: round(255 · c).
    fn quantise_channel(encoded: f64) -> u8 {
        let scaled = (encoded * CHANNEL_MAX).round();
        let is_channel_in_range = (0.0..=CHANNEL_MAX).contains(&scaled);
        debug_assert!(
            is_channel_in_range,
            "channel {scaled} escaped the gamut map"
        );
        scaled as u8
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::color::NAMED_COLORS;

    use super::*;

    const ROUND_TRIP_STEP: usize = 5;

    fn largest_channel_distance(left: Rgb, right: Rgb) -> u8 {
        let red_distance = left.red.abs_diff(right.red);
        let green_distance = left.green.abs_diff(right.green);
        let blue_distance = left.blue.abs_diff(right.blue);
        red_distance.max(green_distance).max(blue_distance)
    }

    #[test]
    fn named_colors_match_what_usvg_paints() {
        let names_usvg_lacks = ["rebeccapurple"];
        for (name, rgb) in NAMED_COLORS {
            let is_unknown_to_usvg = names_usvg_lacks.contains(&name);
            if is_unknown_to_usvg {
                continue;
            }
            let markup = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><rect width=\"1\" height=\"1\" fill=\"{name}\"/></svg>"
            );
            let options = usvg::Options::default();
            let tree = usvg::Tree::from_str(&markup, &options).expect("parseable");
            let first_child = tree.root().children().first().expect("rect kept");
            let usvg::Node::Path(path) = first_child else {
                panic!("rect becomes a path");
            };
            let fill = path.fill().expect("filled");
            let usvg::Paint::Color(usvg_color) = fill.paint() else {
                panic!("{name} fills with a color");
            };
            let usvg_rgb = Rgb {
                red: usvg_color.red,
                green: usvg_color.green,
                blue: usvg_color.blue,
            };
            assert_eq!(usvg_rgb, rgb, "{name}");
        }
    }

    #[test]
    fn white_and_black_land_on_the_lightness_axis() {
        let white_rgb = Rgb::from_packed(0xffffff);
        let white = Oklab::from_rgb(white_rgb);
        assert!((white.lightness - 1.0).abs() < 1e-6, "{white:?}");
        assert!(white.green_red.abs() < 1e-6, "{white:?}");
        assert!(white.blue_yellow.abs() < 1e-6, "{white:?}");
        let black_rgb = Rgb::from_packed(0x000000);
        let black = Oklab::from_rgb(black_rgb);
        assert!(black.lightness.abs() < 1e-9, "{black:?}");
    }

    #[test]
    fn oklab_round_trips_within_one_unit_of_8_bit() {
        let channel_values: Vec<u8> = (0..=255).step_by(ROUND_TRIP_STEP).chain([1, 254]).collect();
        for &red in &channel_values {
            for &green in &channel_values {
                for &blue in &channel_values {
                    let original = Rgb { red, green, blue };
                    let original_oklab = Oklab::from_rgb(original);
                    let round_tripped = original_oklab.to_rgb();
                    let distance = largest_channel_distance(original, round_tripped);
                    assert!(distance <= 1, "{original} -> {round_tripped}");
                }
            }
        }
    }

    #[test]
    fn out_of_gamut_colors_keep_their_hue_by_losing_chroma() {
        let vivid = Oklab {
            lightness: 0.6,
            green_red: 0.3,
            blue_yellow: 0.1,
        };
        assert!(!vivid.is_in_srgb_gamut());
        let mapped_rgb = vivid.to_rgb();
        let mapped = Oklab::from_rgb(mapped_rgb);
        let original_hue_ratio = vivid.blue_yellow / vivid.green_red;
        let mapped_hue_ratio = mapped.blue_yellow / mapped.green_red;
        assert!(
            (original_hue_ratio - mapped_hue_ratio).abs() < 0.05,
            "{mapped:?}"
        );
        assert!(
            (mapped.lightness - vivid.lightness).abs() < 0.01,
            "{mapped:?}"
        );
        assert!(mapped.green_red < vivid.green_red, "{mapped:?}");
    }
}
