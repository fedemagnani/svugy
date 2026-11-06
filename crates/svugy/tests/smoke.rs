//! Prove each svugy dependency covers its intended use.

mod support;

use dagre_dgl_rs::{EdgeLabel, Graph, GraphLabel, NodeLabel, Point, layout};
use glam::{DMat3, DVec3};
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, Point as KurboPoint};
use taffy::prelude::*;
use ttf_parser::{Face, OutlineBuilder};

use support::{pixel_snapshot_test, rasterize_to_png};

const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
const INTER_BOLD: &[u8] = include_bytes!("../assets/fonts/Inter-Bold.ttf");

const ARC_LENGTH_ACCURACY: f64 = 1e-6;
const HALF_SQRT_3: f64 = 0.866_025_403_784_438_6;

// Plan (x, y, z) → screen: sx = (x − y)·√3/2, sy = (x + y)/2 − z.
const ISOMETRIC_PROJECTION: DMat3 = DMat3::from_cols(
    DVec3::new(HALF_SQRT_3, 0.5, 0.0),
    DVec3::new(-HALF_SQRT_3, 0.5, 0.0),
    DVec3::new(0.0, -1.0, 0.0),
);

#[derive(Default)]
struct OutlineCommandCounter {
    command_count: usize,
}

impl OutlineBuilder for OutlineCommandCounter {
    fn move_to(&mut self, _x: f32, _y: f32) {
        self.command_count += 1;
    }

    fn line_to(&mut self, _x: f32, _y: f32) {
        self.command_count += 1;
    }

    fn quad_to(&mut self, _x1: f32, _y1: f32, _x: f32, _y: f32) {
        self.command_count += 1;
    }

    fn curve_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) {
        self.command_count += 1;
    }

    fn close(&mut self) {
        self.command_count += 1;
    }
}

#[derive(Debug, PartialEq)]
struct DagreLayoutSnapshot {
    node_centers: Vec<(String, f64, f64)>,
    edge_points: Vec<Vec<Point>>,
}

fn lay_out_a_graph_with_long_edges_and_crossings() -> DagreLayoutSnapshot {
    let node_ids = [
        "gateway", "auth", "orders", "billing", "ledger", "db", "cache", "queue",
    ];
    let edge_pairs = [
        ("gateway", "auth"),
        ("gateway", "orders"),
        ("gateway", "db"),
        ("orders", "billing"),
        ("orders", "cache"),
        ("auth", "cache"),
        ("billing", "ledger"),
        ("billing", "queue"),
        ("ledger", "db"),
        ("queue", "auth"),
    ];
    let mut graph = Graph::with_options(true, true, false);
    let left_to_right = Some("LR".to_owned());
    graph.set_graph(GraphLabel {
        rankdir: left_to_right,
        ..Default::default()
    });
    for node_id in node_ids {
        let node_label = NodeLabel {
            width: 120.0,
            height: 48.0,
            ..Default::default()
        };
        graph.set_node(node_id, node_label);
    }
    for (from, to) in edge_pairs {
        graph.set_edge(from, to, EdgeLabel::default(), None);
    }

    layout(&mut graph);

    let node_centers = node_ids
        .iter()
        .map(|node_id| {
            let node = graph.node(node_id);
            let center_x = node.x.expect("layout sets x");
            let center_y = node.y.expect("layout sets y");
            (node_id.to_string(), center_x, center_y)
        })
        .collect();
    let edge_points = graph
        .edges()
        .iter()
        .map(|edge| {
            let edge_label = graph.edge(edge).expect("edge exists");
            edge_label.points.clone().expect("layout sets points")
        })
        .collect();
    DagreLayoutSnapshot {
        node_centers,
        edge_points,
    }
}

#[test]
fn fontdb_matches_a_bundled_face_by_weight() {
    let mut database = fontdb::Database::new();
    for face_bytes in [INTER_REGULAR, INTER_MEDIUM, INTER_SEMIBOLD, INTER_BOLD] {
        database.load_font_data(face_bytes.to_vec());
    }
    let matched_name_for_weight = |weight: u16| {
        let families = [fontdb::Family::Name("Inter")];
        let requested_weight = fontdb::Weight(weight);
        let query = fontdb::Query {
            families: &families,
            weight: requested_weight,
            ..Default::default()
        };
        let face_id = database.query(&query).expect("Inter matches");
        let face_info = database.face(face_id).expect("matched face exists");
        face_info.post_script_name.clone()
    };

    assert_eq!(matched_name_for_weight(400), "Inter-Regular");
    assert_eq!(matched_name_for_weight(700), "Inter-Bold");
    // fontdb picks Regular for 450 where CSS picks Medium, so measured and viewed weights can differ.
    assert_eq!(matched_name_for_weight(450), "Inter-Regular");
    assert_eq!(matched_name_for_weight(650), "Inter-Bold");
}

#[test]
fn ttf_parser_measures_advances_and_outlines_a_glyph() {
    let face = Face::parse(INTER_REGULAR, 0).expect("Inter parses");
    let font_size = 12.0_f64;
    let units_per_em = f64::from(face.units_per_em());
    let label_advance_units: u32 = "Order API"
        .chars()
        .map(|character| {
            let glyph_id = face.glyph_index(character).expect("Inter covers Latin");
            let advance = face
                .glyph_hor_advance(glyph_id)
                .expect("glyph has an advance");
            u32::from(advance)
        })
        .sum();
    let label_width = f64::from(label_advance_units) * font_size / units_per_em;

    let letter_a = face.glyph_index('A').expect("Inter covers A");
    let mut outline_counter = OutlineCommandCounter::default();
    let outline_bounds = face.outline_glyph(letter_a, &mut outline_counter);

    assert!(
        label_width > 40.0 && label_width < 80.0,
        "label width {label_width}"
    );
    assert!(outline_bounds.is_some());
    assert!(outline_counter.command_count > 3);
}

#[test]
fn dagre_lays_out_the_same_graph_identically_every_time() {
    let first_layout = lay_out_a_graph_with_long_edges_and_crossings();

    for _repeat_index in 0..20 {
        let repeated_layout = lay_out_a_graph_with_long_edges_and_crossings();
        assert_eq!(repeated_layout, first_layout);
    }
}

#[test]
fn taffy_measures_leaves_through_a_callback() {
    let mut tree: TaffyTree<&str> = TaffyTree::new();
    let short_label = tree
        .new_leaf_with_context(Style::default(), "API")
        .expect("leaf");
    let long_label = tree
        .new_leaf_with_context(Style::default(), "Payment ledger")
        .expect("leaf");
    let label_gap = length(10.0_f32);
    let row_style = Style {
        flex_direction: FlexDirection::Row,
        gap: label_gap,
        ..Default::default()
    };
    let row = tree
        .new_with_children(row_style, &[short_label, long_label])
        .expect("row");
    let mut measured_labels = Vec::new();

    tree.compute_layout_with_measure(
        row,
        Size::MAX_CONTENT,
        |_known_dimensions, _available_space, _node_id, label, _style| {
            let Some(label) = label else {
                return Size::ZERO;
            };
            measured_labels.push(label.to_string());
            let label_width = label.len() as f32 * 7.0;
            Size {
                width: label_width,
                height: 16.0,
            }
        },
    )
    .expect("layout succeeds");

    let long_label_layout = tree.layout(long_label).expect("laid out");
    assert_eq!(long_label_layout.location.x, 31.0);
    assert_eq!(long_label_layout.size.width, 98.0);
    assert!(measured_labels.contains(&"Payment ledger".to_owned()));
}

#[test]
#[allow(clippy::disallowed_methods)] // The smoke test proves the crate's arc-length API works; the pipeline never calls it.
fn kurbo_samples_a_path_by_arc_length() {
    let mut route = BezPath::new();
    route.move_to((0.0, 0.0));
    route.line_to((30.0, 40.0));
    route.curve_to((40.0, 40.0), (50.0, 40.0), (60.0, 40.0));

    let segments: Vec<_> = route.segments().collect();
    let segment_lengths: Vec<f64> = segments
        .iter()
        .map(|segment| segment.arclen(ARC_LENGTH_ACCURACY))
        .collect();
    let total_length: f64 = segment_lengths.iter().sum();
    let parameter_at_half_of_second = segments[1].inv_arclen(15.0, ARC_LENGTH_ACCURACY);
    let point_at_half_of_second = segments[1].eval(parameter_at_half_of_second);

    assert!(
        (total_length - 80.0).abs() < 1e-6,
        "total length {total_length}"
    );
    let expected_point = KurboPoint::new(45.0, 40.0);
    assert!(point_at_half_of_second.distance(expected_point) < 1e-6);
}

#[test]
fn glam_builds_a_const_projection_matrix() {
    let plan_corner = DVec3::new(10.0, 10.0, 0.0);
    let raised_origin = DVec3::new(0.0, 0.0, 5.0);

    let screen_corner = ISOMETRIC_PROJECTION * plan_corner;
    let screen_raised_origin = ISOMETRIC_PROJECTION * raised_origin;

    assert_eq!(screen_corner.x, 0.0);
    assert_eq!(screen_corner.y, 10.0);
    assert_eq!(screen_raised_origin.y, -5.0);
}

#[test]
fn libm_provides_the_transcendentals_clippy_bans_from_std() {
    let cube_root = libm::cbrt(27.0);
    let diagonal_angle = libm::atan2(1.0, 1.0);

    assert_eq!(cube_root, 3.0);
    assert_eq!(diagonal_angle, std::f64::consts::FRAC_PI_4);
}

#[test]
fn resvg_rasterizes_a_static_svg_without_fonts() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10">
        <rect x="0" y="0" width="10" height="10" fill="#3366cc"/>
        <text x="15" y="8" font-size="10" fill="#000000">A</text>
    </svg>"##;

    let png_bytes = rasterize_to_png(svg);
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png_bytes).expect("png decodes");

    let rect_pixel = pixmap.pixel(5, 5).expect("inside the pixmap");
    let text_area_pixels = (10..20).flat_map(|x| (0..10).map(move |y| (x, y)));
    let text_area_has_ink = text_area_pixels.into_iter().any(|(x, y)| {
        let pixel = pixmap.pixel(x, y).expect("inside the pixmap");
        pixel.alpha() > 0
    });
    assert_eq!(
        (rect_pixel.red(), rect_pixel.green(), rect_pixel.blue()),
        (0x33, 0x66, 0xcc)
    );
    assert!(
        !text_area_has_ink,
        "text must vanish without the text feature"
    );
}

const BLUE_SQUARE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#3366cc"/></svg>"##;

#[test]
fn insta_snapshots_svg_text() {
    insta::assert_snapshot!(BLUE_SQUARE_SVG, @r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#3366cc"/></svg>"##);
}

pixel_snapshot_test!(blue_square, BLUE_SQUARE_SVG);
