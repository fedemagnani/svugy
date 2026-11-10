use std::marker::PhantomData;

use kurbo::{BezPath, Point};
use ttf_parser::Face;

use super::sized_face::SizedFace;
use super::{TextAnchor, TextMeasure, TextOutline, TextSize, TextSpec};

/// Name the lightest weight of the middle band CSS matches within first, the normal weight.
const MIDDLE_BAND_LIGHTEST_WEIGHT: u16 = 400;
/// Name the heaviest weight of the middle band CSS matches within first.
const MIDDLE_BAND_HEAVIEST_WEIGHT: u16 = 500;

/// Sets every label in the bundled faces of the face set `S`.
pub(crate) struct BundledFont<S> {
    faces_by_ascending_weight: Vec<WeightedFace>,
    face_set: PhantomData<S>,
}

impl<S: FaceSet> BundledFont<S> {
    /// Parse every bundled face of `S`, by ascending weight.
    pub(crate) fn new() -> Self {
        let faces_by_ascending_weight = S::FACES_BY_ASCENDING_WEIGHT
            .iter()
            .map(|&(weight, face_bytes)| {
                let face = Face::parse(face_bytes, 0).expect("bundled face parses");
                WeightedFace { weight, face }
            })
            .collect();
        Self {
            faces_by_ascending_weight,
            face_set: PhantomData,
        }
    }
}

impl<S> TextMeasure for BundledFont<S> {
    fn measure(&self, spec: &TextSpec) -> TextSize {
        let sized_face = self.sized_face_matching(spec);
        sized_face.measure(spec.text, spec.letter_spacing)
    }
}

impl<S> TextOutline for BundledFont<S> {
    fn outline(&self, spec: &TextSpec, baseline_origin: Point, anchor: TextAnchor) -> BezPath {
        let sized_face = self.sized_face_matching(spec);
        sized_face.outline(spec.text, spec.letter_spacing, baseline_origin, anchor)
    }
}

impl<S> BundledFont<S> {
    /// Size the face matching `spec`'s weight at `spec`'s size.
    fn sized_face_matching(&self, spec: &TextSpec) -> SizedFace<'_> {
        let weighted_face = self.face_matching_css_weight(spec.weight);
        SizedFace::new(&weighted_face.face, spec.size)
    }

    /// Match a weight as CSS does: w<400 → max(a≤w) else min(a>w); 400≤w≤500 → min(a∈[w,500]) else max(a<w) else min(a>500); w>500 → min(a≥w) else max(a<w).
    fn face_matching_css_weight(&self, requested_weight: u16) -> &WeightedFace {
        let faces_by_ascending_weight = &self.faces_by_ascending_weight;
        let lighter_or_equal = || {
            let mut faces_by_descending_weight = faces_by_ascending_weight.iter().rev();
            faces_by_descending_weight.find(|face| face.weight <= requested_weight)
        };
        let heavier_or_equal = || {
            let mut faces = faces_by_ascending_weight.iter();
            faces.find(|face| face.weight >= requested_weight)
        };
        let middle_band_from_requested = requested_weight..=MIDDLE_BAND_HEAVIEST_WEIGHT;
        let within_middle_band = || {
            let mut faces = faces_by_ascending_weight.iter();
            faces.find(|face| middle_band_from_requested.contains(&face.weight))
        };
        let matched_face = match requested_weight {
            0..MIDDLE_BAND_LIGHTEST_WEIGHT => lighter_or_equal().or_else(heavier_or_equal),
            MIDDLE_BAND_LIGHTEST_WEIGHT..=MIDDLE_BAND_HEAVIEST_WEIGHT => within_middle_band()
                .or_else(lighter_or_equal)
                .or_else(heavier_or_equal),
            _heavier_than_middle_band => heavier_or_equal().or_else(lighter_or_equal),
        };
        matched_face.expect("a font has at least one face")
    }
}

/// Pairs a face with the CSS weight it is matched by.
struct WeightedFace {
    weight: u16,
    face: Face<'static>,
}

/// Bundles a font family's face set as TTF bytes paired with their CSS weight.
pub(crate) trait FaceSet {
    /// List every face of the set with its CSS weight, by ascending weight.
    const FACES_BY_ASCENDING_WEIGHT: &'static [(u16, &'static [u8])];
    /// Name the faces as a CSS font-family stack, falling back to the viewer's closest system faces.
    const CSS_FONT_FAMILY_STACK: &'static str;
    /// Embed the set's single face as WOFF2 in interactive documents, or `None` to leave viewers their installed faces.
    const EMBEDDED_WOFF2: Option<&'static [u8]>;

    /// Name the stack's first family, the one the embedded face is declared as, so the two never drift apart.
    fn first_css_family() -> &'static str {
        let stack = Self::CSS_FONT_FAMILY_STACK;
        let (first_family, _fallback_families) = stack.split_once(',').unwrap_or((stack, ""));
        first_family.trim()
    }
}

/// Bundles the four static Inter faces.
pub(crate) struct InterFaces;

impl FaceSet for InterFaces {
    const FACES_BY_ASCENDING_WEIGHT: &'static [(u16, &'static [u8])] = &[
        (400, include_bytes!("../../assets/fonts/Inter-Regular.ttf")),
        (500, include_bytes!("../../assets/fonts/Inter-Medium.ttf")),
        (600, include_bytes!("../../assets/fonts/Inter-SemiBold.ttf")),
        (700, include_bytes!("../../assets/fonts/Inter-Bold.ttf")),
    ];
    const CSS_FONT_FAMILY_STACK: &'static str =
        "Inter, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif";
    const EMBEDDED_WOFF2: Option<&'static [u8]> = None;
}

/// Bundles the single Excalifont face, merged from Excalidraw's latin and greek subsets.
pub(crate) struct ExcalifontFaces;

impl FaceSet for ExcalifontFaces {
    const FACES_BY_ASCENDING_WEIGHT: &'static [(u16, &'static [u8])] = &[(
        400,
        include_bytes!("../../assets/fonts/Excalifont-Regular.ttf"),
    )];
    const CSS_FONT_FAMILY_STACK: &'static str =
        "Excalifont, 'Segoe Print', 'Comic Sans MS', cursive";
    const EMBEDDED_WOFF2: Option<&'static [u8]> = Some(include_bytes!(
        "../../assets/fonts/Excalifont-Regular.woff2"
    ));
}

#[cfg(test)]
mod tests {
    use kurbo::Shape;

    use super::*;

    const LABELS: [&str; 6] = [
        "API",
        "Order API",
        "Payment ledger",
        "PostgreSQL 16",
        "WWW iii",
        "",
    ];

    fn width_table(sizes: &[f64]) -> String {
        let font = BundledFont::<InterFaces>::new();
        let mut table = String::new();
        for &size in sizes {
            for &(weight, _face_bytes) in InterFaces::FACES_BY_ASCENDING_WEIGHT {
                for label in LABELS {
                    let spec = TextSpec::new(label, weight, size);
                    let measured = font.measure(&spec);
                    let row = format!("{size:.0}px {weight} {label:?} {:.3}\n", measured.width);
                    table.push_str(&row);
                }
            }
        }
        table
    }

    #[test]
    fn excalifont_sets_every_weight_in_its_single_face_wider_than_inter() {
        let inter = BundledFont::<InterFaces>::new();
        let excalifont = BundledFont::<ExcalifontFaces>::new();
        let regular_label = TextSpec::new("Order API", 400, 14.0);
        let bold_label = TextSpec::new("Order API", 700, 14.0);

        let inter_regular_width = inter.measure(&regular_label).width;
        let excalifont_regular_width = excalifont.measure(&regular_label).width;
        let excalifont_bold_width = excalifont.measure(&bold_label).width;

        assert!(
            excalifont_regular_width > inter_regular_width,
            "hand-drawn Excalifont sets a label wider than Inter: {excalifont_regular_width} vs {inter_regular_width}"
        );
        assert_eq!(
            excalifont_bold_width, excalifont_regular_width,
            "a single-face set resolves every requested weight to its one face"
        );
    }

    #[test]
    fn widths_follow_advances_across_weights_and_sizes() {
        let table = width_table(&[12.0, 14.0]);

        insta::assert_snapshot!(table, @r#"
        12px 400 "API" 19.166
        12px 400 "Order API" 55.096
        12px 400 "Payment ledger" 89.168
        12px 400 "PostgreSQL 16" 83.350
        12px 400 "WWW iii" 47.566
        12px 400 "" 0.000
        12px 500 "API" 19.477
        12px 500 "Order API" 55.623
        12px 500 "Payment ledger" 90.240
        12px 500 "PostgreSQL 16" 84.117
        12px 500 "WWW iii" 48.375
        12px 500 "" 0.000
        12px 600 "API" 19.793
        12px 600 "Order API" 56.150
        12px 600 "Payment ledger" 91.283
        12px 600 "PostgreSQL 16" 84.879
        12px 600 "WWW iii" 49.166
        12px 600 "" 0.000
        12px 700 "API" 20.104
        12px 700 "Order API" 56.678
        12px 700 "Payment ledger" 92.355
        12px 700 "PostgreSQL 16" 85.641
        12px 700 "WWW iii" 49.951
        12px 700 "" 0.000
        14px 400 "API" 22.360
        14px 400 "Order API" 64.278
        14px 400 "Payment ledger" 104.029
        14px 400 "PostgreSQL 16" 97.241
        14px 400 "WWW iii" 55.494
        14px 400 "" 0.000
        14px 500 "API" 22.723
        14px 500 "Order API" 64.894
        14px 500 "Payment ledger" 105.280
        14px 500 "PostgreSQL 16" 98.137
        14px 500 "WWW iii" 56.438
        14px 500 "" 0.000
        14px 600 "API" 23.092
        14px 600 "Order API" 65.509
        14px 600 "Payment ledger" 106.497
        14px 600 "PostgreSQL 16" 99.025
        14px 600 "WWW iii" 57.360
        14px 600 "" 0.000
        14px 700 "API" 23.454
        14px 700 "Order API" 66.124
        14px 700 "Payment ledger" 107.748
        14px 700 "PostgreSQL 16" 99.914
        14px 700 "WWW iii" 58.276
        14px 700 "" 0.000
        "#);
    }

    #[test]
    fn weights_resolve_by_css_font_matching() {
        let font = BundledFont::<InterFaces>::new();
        let requested_weights = [
            100, 300, 399, 400, 420, 450, 500, 501, 550, 600, 650, 700, 800, 900,
        ];

        let resolved_weights: Vec<(u16, u16)> = requested_weights
            .iter()
            .map(|&requested_weight| {
                let matched_face = font.face_matching_css_weight(requested_weight);
                (requested_weight, matched_face.weight)
            })
            .collect();

        assert_eq!(
            resolved_weights,
            [
                (100, 400),
                (300, 400),
                (399, 400),
                (400, 400),
                (420, 500),
                (450, 500),
                (500, 500),
                (501, 600),
                (550, 600),
                (600, 600),
                (650, 700),
                (700, 700),
                (800, 700),
                (900, 700),
            ]
        );
    }

    #[test]
    fn bundled_faces_declare_the_weight_they_are_matched_by() {
        let font = BundledFont::<InterFaces>::new();

        for weighted_face in &font.faces_by_ascending_weight {
            let declared_weight = weighted_face.face.weight().to_number();
            assert_eq!(declared_weight, weighted_face.weight);
        }
    }

    #[test]
    fn letter_spacing_widens_each_gap_between_characters() {
        let font = BundledFont::<InterFaces>::new();
        let unspaced = TextSpec::new("Order API", 400, 12.0);
        let spaced = unspaced.with_letter_spacing(0.5);

        let unspaced_width = font.measure(&unspaced).width;
        let spaced_width = font.measure(&spaced).width;

        let gap_count = 8.0;
        let expected_width = unspaced_width + gap_count * 0.5;
        assert!((spaced_width - expected_width).abs() < 1e-9);
    }

    #[test]
    fn line_box_comes_from_the_face_metrics() {
        let font = BundledFont::<InterFaces>::new();
        let spec = TextSpec::new("Order API", 600, 12.0);

        let measured = font.measure(&spec);

        let line_box = format!(
            "line height {:.3}, baseline from top {:.3}",
            measured.line_height, measured.baseline_from_top
        );
        insta::assert_snapshot!(line_box, @"line height 14.520, baseline from top 11.625");
    }

    #[test]
    fn outline_spans_the_measured_width_around_its_anchor() {
        let font = BundledFont::<InterFaces>::new();
        let spec = TextSpec::new("Order API", 500, 14.0);
        let baseline_origin = Point::new(100.0, 50.0);

        let measured_width = font.measure(&spec).width;
        let outline = font.outline(&spec, baseline_origin, TextAnchor::Middle);

        let bounds = outline.bounding_box();
        let width_error = (bounds.width() - measured_width).abs();
        let center_error = (bounds.center().x - baseline_origin.x).abs();
        assert!(!outline.is_empty());
        assert!(
            width_error < 0.05 * measured_width,
            "width error {width_error}"
        );
        assert!(center_error < 1.0, "center error {center_error}");
        assert!(bounds.max_y() > baseline_origin.y && bounds.min_y() < baseline_origin.y);
    }
}
