//! Prove d2 lowering merges repeated keys, declares endpoints and warns on every construct the model cannot hold.

use miette::SourceSpan;
use svugy_import::d2::{Construct, D2Import, D2Parse, EdgeOperator};
use svugy_model::legend::DashKey;
use svugy_model::model::{Edge, EdgeId, Model, Shape, Traveler};
use svugy_model::path::ElementPath;
use svugy_model::problem::Problem;

const NESTING: &str = include_str!("fixtures/d2/nesting.d2");
const EDGES: &str = include_str!("fixtures/d2/edges.d2");
const STYLES: &str = include_str!("fixtures/d2/styles.d2");
const MERGING: &str = include_str!("fixtures/d2/merging.d2");
const ENDPOINTS: &str = include_str!("fixtures/d2/endpoints.d2");
const UNMAPPED: &str = include_str!("fixtures/d2/unmapped.d2");

#[test]
fn nesting_lowers_into_a_model_snapshot() {
    let model = import_model(NESTING);

    insta::assert_snapshot!(model.to_json());
}

#[test]
fn edges_lower_into_a_model_snapshot() {
    let import = import(EDGES);

    let warning_constructs: Vec<Construct> = import
        .warnings
        .iter()
        .map(|warning| warning.construct.clone())
        .collect();
    let model = import
        .admission
        .model
        .expect("the fixture lowers into a sound model");
    insta::assert_snapshot!(model.to_json());
    assert_eq!(
        warning_constructs,
        [
            Construct::ArrowheadForm(EdgeOperator::Bidirectional),
            Construct::ArrowheadForm(EdgeOperator::Undirected),
        ]
    );
}

#[test]
fn styles_lower_into_a_model_snapshot() {
    let model = import_model(STYLES);

    insta::assert_snapshot!(model.to_json());
}

#[test]
fn repeated_keys_merge_into_a_model_snapshot() {
    let model = import_model(MERGING);

    insta::assert_snapshot!(model.to_json());
}

#[test]
fn edge_endpoints_declare_elements_into_a_model_snapshot() {
    let model = import_model(ENDPOINTS);

    insta::assert_snapshot!(model.to_json());
}

#[test]
fn unmapped_constructs_lower_into_a_model_snapshot() {
    let model = import_model(UNMAPPED);

    insta::assert_snapshot!(model.to_json());
}

#[test]
fn fixtures_inside_the_mapped_subset_lower_without_diagnostics() {
    let clean_fixtures = [NESTING, STYLES, MERGING];

    let diagnostic_counts: Vec<(usize, bool)> = clean_fixtures
        .into_iter()
        .map(|source| {
            let import = import(source);
            let diagnostic_count = import.diagnostics().len();
            let is_model_sound = import.admission.model.is_some();
            (diagnostic_count, is_model_sound)
        })
        .collect();

    assert_eq!(diagnostic_counts, [(0, true); 3]);
}

#[test]
fn unmapped_constructs_warn_at_their_source_text() {
    let import = import(UNMAPPED);

    let warnings: Vec<(Construct, &str)> = import
        .warnings
        .iter()
        .map(|warning| {
            let text = text_at(UNMAPPED, warning.span);
            (warning.construct.clone(), text)
        })
        .collect();
    assert_eq!(
        warnings,
        [
            (Construct::UnmappedShape("queue".to_owned()), "queue"),
            (Construct::UnmappedDirection("left".to_owned()), "left"),
            (Construct::ArrowheadForm(EdgeOperator::Bidirectional), "<->"),
            (Construct::FieldOnEdge("shape"), "circle"),
            (Construct::FieldOnEdge("link"), "https://example.com"),
            (Construct::ArrowheadForm(EdgeOperator::Undirected), "--"),
            (Construct::UnmappedDirection("down".to_owned()), "down"),
            (Construct::SpecialShape("sql_table".to_owned()), "sql_table"),
        ]
    );
}

#[test]
fn parse_errors_and_warnings_survive_lowering_in_source_order() {
    let source = "a.shape: queue\nb ->\nc.icon: x.svg\n";

    let import = import(source);

    let error_texts: Vec<&str> = import
        .errors
        .iter()
        .map(|error| text_at(source, error.span))
        .collect();
    let warning_constructs: Vec<Construct> = import
        .warnings
        .iter()
        .map(|warning| warning.construct.clone())
        .collect();
    assert_eq!(error_texts, ["\n"]);
    assert_eq!(
        warning_constructs,
        [
            Construct::UnmappedShape("queue".to_owned()),
            Construct::ReservedField("icon"),
        ]
    );
}

#[test]
fn unknown_style_keys_warn_and_mistyped_style_values_reach_the_model_report() {
    let source = "a.style.foo: x\nb.style.fill: notacolor\na -> b\n";

    let import = import(source);

    let warnings: Vec<(Construct, &str)> = import
        .warnings
        .iter()
        .map(|warning| {
            let text = text_at(source, warning.span);
            (warning.construct.clone(), text)
        })
        .collect();
    let admission = import.admission;
    let report = &admission.report;
    let mismatches: Vec<(String, &str, &str)> = report
        .problems()
        .iter()
        .filter_map(|problem| match problem {
            Problem::MismatchedStyleValue(mismatch) => Some(mismatch),
            _other_problem => None,
        })
        .map(|mismatch| {
            let owner = mismatch.owner.to_string();
            let span = mismatch.span.expect("lowered elements carry spans");
            let text = text_at(source, span);
            (owner, mismatch.value.as_str(), text)
        })
        .collect();
    assert_eq!(
        warnings,
        [(
            Construct::UnknownStyleKey("foo".to_owned()),
            "a.style.foo: x"
        )]
    );
    assert!(admission.model.is_none());
    assert!(report.has_errors());
    assert_eq!(report.problems().len(), 1);
    assert_eq!(mismatches, [("element `b`".to_owned(), "notacolor", "b")]);
}

#[test]
fn model_warnings_reach_the_import_in_source_order_with_the_importer_warnings() {
    let source = "a.b -> a\nc.icon: x.svg\nd.link: a.missing\n";

    let import = import(source);

    let diagnostics: Vec<(String, &str)> = import
        .diagnostics()
        .into_iter()
        .map(|diagnostic| {
            let code = diagnostic.code().map(|code| code.to_string());
            let mut labels = diagnostic.labels().into_iter().flatten();
            let first_label = labels.next().expect("every diagnostic here is spanned");
            let text = text_at(source, *first_label.inner());
            (code.unwrap_or_default(), text)
        })
        .collect();
    assert!(import.admission.model.is_some());
    assert_eq!(
        diagnostics,
        [
            ("svugy::ancestor_descendant_edge".to_owned(), "a.b -> a"),
            ("svugy::d2::unsupported".to_owned(), "c.icon: x.svg"),
            ("svugy::unresolved_link".to_owned(), "a.missing"),
        ]
    );
}

#[test]
fn parent_references_climb_one_scope_each() {
    let source = "a: {\n  _.b -> c\n  d: { _._.e; _.f.style.fill: red }\n}\n";

    let model = import_model(source);

    let element_paths: Vec<String> = model.elements().keys().map(ToString::to_string).collect();
    let edge_ids: Vec<String> = model.edges().keys().map(ToString::to_string).collect();
    let f_fill = model
        .element(&ElementPath::from_segments(["a", "f"]))
        .map(|element| element.style()["fill"].as_str());
    assert_eq!(element_paths, ["a", "a.c", "a.d", "a.f", "b", "e"]);
    assert_eq!(edge_ids, ["(b -> a.c)[0]"]);
    assert_eq!(f_fill, Some("red"));
}

#[test]
fn misplaced_or_overreaching_parent_references_are_errors() {
    let source = "_.up\na: { _._.b -> c }\nd._: x\n_: y\ne -> _\n\"_\".quoted\n";

    let import = import(source);

    let syntax_errors = import.errors.to_vec();
    let errors: Vec<(&str, &str)> = syntax_errors
        .iter()
        .map(|error| {
            let text = text_at(source, error.span);
            (error.message.as_str(), text)
        })
        .collect();
    let model = import
        .admission
        .model
        .expect("the rejected statements leave a sound model");
    let element_paths: Vec<String> = model.elements().keys().map(ToString::to_string).collect();
    assert_eq!(
        errors,
        [
            (
                "`_` climbs above the root, which has no parent scope",
                "_.up"
            ),
            (
                "`_` climbs above the root, which has no parent scope",
                "_._.b"
            ),
            (
                "`_` names the parent scope only at the start of a key, e.g. `_.x`",
                "_"
            ),
            ("a key needs a name after its `_` parent references", "_"),
            ("a key needs a name after its `_` parent references", "_"),
        ]
    );
    assert_eq!(element_paths, ["_", "_.quoted", "a"]);
}

#[test]
fn the_svugy_variable_carries_caption_primitive_and_position() {
    let source = "bus: { vars: { svugy: { caption: \"a · b\"; shape: slab; position: { x: 150; y: 91.5 } } } }\nflat.vars: { svugy.caption: dotted; svugy.position.x: 0; svugy.position.y: 2 }\n";

    let model = import_model(source);

    let bus = model
        .element(&ElementPath::from_segments(["bus"]))
        .expect("bus is declared");
    let flat = model
        .element(&ElementPath::from_segments(["flat"]))
        .expect("flat is declared");
    let bus_position = bus.position().map(|position| (position.x, position.y));
    let flat_position = flat.position().map(|position| (position.x, position.y));
    assert_eq!(bus.caption(), Some("a · b"));
    assert_eq!(bus.shape(), Some(Shape::Slab));
    assert_eq!(bus_position, Some((150.0, 91.5)));
    assert_eq!(flat.caption(), Some("dotted"));
    assert_eq!(flat_position, Some((0.0, 2.0)));
}

#[test]
fn the_svugy_variable_carries_a_height_that_validation_then_bounds() {
    let source = "app: { vars: { svugy: { height: 84 } } }\nstore: { shape: cylinder; vars: { svugy.height: 40.5 } }\nflat: { vars: { svugy: { height: 0 } } }\n";

    let import = import(source);

    let is_free_of_syntax_errors = import.errors.is_empty();
    let admission = import.admission;
    let impossible_heights: Vec<(String, f64, &str)> = admission
        .report
        .problems()
        .iter()
        .filter_map(|problem| match problem {
            Problem::ImpossibleHeight(impossible_height) => Some(impossible_height),
            _other_problem => None,
        })
        .map(|impossible_height| {
            let element = impossible_height.element.to_string();
            let span = impossible_height
                .span
                .expect("lowered elements carry spans");
            let text = text_at(source, span);
            (element, impossible_height.height, text)
        })
        .collect();
    assert!(is_free_of_syntax_errors);
    assert!(admission.model.is_none());
    assert_eq!(impossible_heights, [("flat".to_owned(), 0.0, "flat")]);
    let sound_source = source.replace("height: 0", "height: 7");
    let model = import_model(&sound_source);
    let heights: Vec<(String, Option<f64>)> = model
        .elements()
        .iter()
        .map(|(path, element)| (path.to_string(), element.height()))
        .collect();
    assert_eq!(
        heights,
        [
            ("app".to_owned(), Some(84.0)),
            ("flat".to_owned(), Some(7.0)),
            ("store".to_owned(), Some(40.5)),
        ]
    );
}

#[test]
fn a_malformed_height_is_a_spanned_error_that_drops_only_the_height() {
    let source = "app: { vars: { svugy: { caption: kept; height: tall } } }\n";

    let import = import(source);

    let syntax_errors = import.errors.to_vec();
    let errors: Vec<(&str, &str)> = syntax_errors
        .iter()
        .map(|error| {
            let text = text_at(source, error.span);
            (error.message.as_str(), text)
        })
        .collect();
    let model = import
        .admission
        .model
        .expect("a malformed height leaves a sound model");
    let app = model
        .element(&ElementPath::from_segments(["app"]))
        .expect("app is declared");
    assert_eq!(
        errors,
        [(
            "`tall` is not a number: `svugy.height` is a plain decimal like `91.5`",
            "tall"
        )]
    );
    assert_eq!(app.caption(), Some("kept"));
    assert_eq!(app.height(), None);
}

#[test]
fn the_root_label_titles_the_diagram_and_the_root_carrier_subtitles_it() {
    let source = "label: \"svugy · how it is built\"\nvars: { svugy: { subtitle: \"text in, SVG out\" } }\na: { _.label: \"last write wins\" }\n";

    let import = import(source);

    let diagnostic_count = import.diagnostics().len();
    let model = import
        .admission
        .model
        .expect("a titled diagram is a sound model");
    let element_paths: Vec<String> = model.elements().keys().map(ToString::to_string).collect();
    assert_eq!(diagnostic_count, 0);
    assert_eq!(model.title(), Some("last write wins"));
    assert_eq!(model.subtitle(), Some("text in, SVG out"));
    assert_eq!(element_paths, ["a"]);
}

#[test]
fn the_root_carrier_rejects_element_attributes_and_elements_reject_a_subtitle() {
    let source = "vars: { svugy: { title: nope; height: 3 } }\na.vars: { svugy.subtitle: misplaced }\nshape: cylinder\n";

    let import = import(source);

    let syntax_errors = import.errors.to_vec();
    let errors: Vec<(&str, &str)> = syntax_errors
        .iter()
        .map(|error| {
            let text = text_at(source, error.span);
            (error.message.as_str(), text)
        })
        .collect();
    let warnings: Vec<(Construct, &str)> = import
        .warnings
        .iter()
        .map(|warning| {
            let text = text_at(source, warning.span);
            (warning.construct.clone(), text)
        })
        .collect();
    let model = import
        .admission
        .model
        .expect("rejected carrier attributes leave a sound model");
    assert_eq!(
        errors,
        [
            (
                "`svugy.title` is not a svugy attribute of the root: expected `svugy.subtitle` or `svugy.legend` with `title`, `entries`, `dashes` or `badges`, and the title is the root's own `label`",
                "title: nope"
            ),
            (
                "`svugy.height` is not a svugy attribute of the root: expected `svugy.subtitle` or `svugy.legend` with `title`, `entries`, `dashes` or `badges`, and the title is the root's own `label`",
                "height: 3"
            ),
            (
                "`svugy.subtitle` is not a svugy attribute: expected `svugy.caption`, `svugy.shape`, `svugy.height`, `svugy.badge` or `svugy.position` with `x` and `y`",
                "svugy.subtitle: misplaced"
            ),
        ]
    );
    assert_eq!(
        warnings,
        [(Construct::FieldOnRoot("shape"), "shape: cylinder")]
    );
    assert_eq!(model.title(), None);
    assert_eq!(model.subtitle(), None);
}

#[test]
fn the_carrier_primitive_outranks_the_d2_shape_whatever_their_order() {
    let source = "before: { vars: { svugy: { shape: disc } }; shape: cylinder }\nafter: { shape: cylinder; vars: { svugy: { shape: box } } }\n";

    let model = import_model(source);

    let shapes: Vec<Option<Shape>> = model
        .elements()
        .values()
        .map(|element| element.shape())
        .collect();
    assert_eq!(shapes, [Some(Shape::Box), Some(Shape::Disc)]);
}

#[test]
fn malformed_carrier_values_are_errors_and_the_rest_of_the_carrier_lands() {
    let source = "a: {
  shape: circle
  vars: {
    color: red
    svugy: {
      caption: kept
      shape: pyramid
      tilt: 3
      position: { x: 1.5; y: north }
      x -> y
    }
  }
}
b.vars: { svugy: { shape: slab; position.x: 4 } }
vars: { svugy: { caption: nowhere } }
e -> f: { vars: { svugy: { caption: edge; traveler: pigeon } } }
g -> h: { vars: { svugy: { traveler: Envelope } } }
";

    let import = import(source);

    let syntax_errors = import.errors.to_vec();
    let errors: Vec<(&str, &str)> = syntax_errors
        .iter()
        .map(|error| {
            let text = text_at(source, error.span);
            (error.message.as_str(), text)
        })
        .collect();
    let warnings: Vec<(Construct, &str)> = import
        .warnings
        .iter()
        .map(|warning| {
            let text = text_at(source, warning.span);
            (warning.construct.clone(), text)
        })
        .collect();
    let model = import
        .admission
        .model
        .expect("malformed carrier values leave a sound model");
    let a = model
        .element(&ElementPath::from_segments(["a"]))
        .expect("a is declared");
    let b = model
        .element(&ElementPath::from_segments(["b"]))
        .expect("b is declared");
    assert_eq!(
        errors,
        [
            (
                "`pyramid` is not a svugy primitive: expected `box`, `slab`, `cylinder` or `disc`",
                "pyramid"
            ),
            (
                "`svugy.tilt` is not a svugy attribute: expected `svugy.caption`, `svugy.shape`, `svugy.height`, `svugy.badge` or `svugy.position` with `x` and `y`",
                "tilt: 3"
            ),
            (
                "`north` is not a number: a `svugy.position` coordinate is a plain decimal like `91.5`",
                "north"
            ),
            (
                "the `svugy` variable holds only `key: value` entries",
                "x -> y"
            ),
            (
                "`svugy.position` needs both `x` and `y` in the same `vars` map",
                "b.vars: { svugy: { shape: slab; position.x: 4 } }"
            ),
            (
                "`svugy.caption` is not a svugy attribute of the root: expected `svugy.subtitle` or `svugy.legend` with `title`, `entries`, `dashes` or `badges`, and the title is the root's own `label`",
                "caption: nowhere"
            ),
            (
                "`svugy.caption` is not a svugy attribute of an edge: expected `svugy.traveler`",
                "caption: edge"
            ),
            (
                "`pigeon` is not a svugy traveler: expected `dot`, `envelope`, `parcel`, `document` or `key`",
                "pigeon"
            ),
        ]
    );
    assert_eq!(warnings, [(Construct::Variables, "color: red")]);
    assert_eq!(model.title(), None);
    assert_eq!(model.subtitle(), None);
    assert_eq!(a.caption(), Some("kept"));
    assert_eq!(a.shape(), Some(Shape::Disc));
    assert_eq!(a.position(), None);
    assert_eq!(b.shape(), Some(Shape::Slab));
    assert_eq!(b.position(), None);
    let e = ElementPath::from_segments(["e"]);
    let f = ElementPath::from_segments(["f"]);
    let g = ElementPath::from_segments(["g"]);
    let h = ElementPath::from_segments(["h"]);
    let e_to_f = EdgeId::new(e, f, 0);
    let g_to_h = EdgeId::new(g, h, 0);
    let e_to_f_traveler = model.edge(&e_to_f).and_then(Edge::traveler);
    let g_to_h_traveler = model.edge(&g_to_h).and_then(Edge::traveler);
    assert_eq!(e_to_f_traveler, None);
    assert_eq!(g_to_h_traveler, Some(Traveler::Envelope));
}

#[test]
fn ancestor_and_descendant_edges_stay_in_the_model_but_not_in_layout() {
    let model = import_model(ENDPOINTS);

    let cloud = ElementPath::from_segments(["cloud"]);
    let api = ElementPath::from_segments(["cloud", "vpc", "api"]);
    let edge_count = model.edges().len();
    let layoutable_edge_count = model.layoutable_edges().count();
    let has_ancestor_edge = model
        .edges()
        .keys()
        .any(|id| id.from() == &cloud && id.to() == &api);
    assert!(has_ancestor_edge);
    assert_eq!(layoutable_edge_count, edge_count - 1);
}

#[test]
fn the_root_legend_lowers_its_lines_dashes_and_badges_resolving_a_later_shape_case_insensitively() {
    let source = r#"vars: {
  svugy: {
    legend: {
      title: Bet lifecycle
      entries: [
        "① quote"
        "② open"
      ]
      dashes: { 5: HTTP; 2: in-process event bus }
      badges: { Desk.SQLite: writes to SQLite }
    }
  }
}
desk: {
  sqlite: SQLite { vars: { svugy: { badge: true } } }
  cache: { vars: { svugy: { badge: TRUE } } }
  quoter
  quoter -> sqlite
  quoter -> cache
}
desk.cache.vars: { svugy.badge: false }
"#;

    let import = import(source);

    assert!(import.errors.is_empty(), "{:?}", import.errors);
    let model = import
        .admission
        .model
        .expect("the legend lowers into a sound model");
    let legend = model.legend().expect("the root carries a legend");
    let sqlite_path = ElementPath::from_segments(["desk", "sqlite"]);
    let cache_path = ElementPath::from_segments(["desk", "cache"]);
    assert_eq!(legend.title(), Some("Bet lifecycle"));
    assert_eq!(legend.entries(), ["① quote", "② open"]);
    assert_eq!(legend.dash_description(5.0), Some("HTTP"));
    assert_eq!(legend.dash_description(2.0), Some("in-process event bus"));
    assert_eq!(
        legend.badge_description(&sqlite_path),
        Some("writes to SQLite")
    );
    let quoter_path = ElementPath::from_segments(["desk", "quoter"]);
    let writes = EdgeId::new(quoter_path.clone(), sqlite_path, 0);
    let caches = EdgeId::new(quoter_path, cache_path, 0);
    assert!(model.is_badge_edge(&writes));
    assert!(!model.is_badge_edge(&caches));
}

#[test]
fn malformed_legend_and_badge_values_are_spanned_errors_that_drop_only_themselves() {
    let source = r#"vars: { svugy: { legend: { entries: one line; dashes: { dotted: bus; 2: kept }; badges: { desk.nowhere: lost; desk.sqlite: kept }; rows: x } } }
desk.sqlite: { vars: { svugy: { badge: yes; caption: [a; b] } } }
vars: { svugy: { legend: { entries: ["kept"; [nested]; { map: 1 }] } } }
desk.quoter -> desk.sqlite
"#;

    let import = import(source);

    let errors: Vec<(&str, &str)> = import
        .errors
        .iter()
        .map(|error| {
            let text = text_at(source, error.span);
            (error.message.as_str(), text)
        })
        .collect();
    assert_eq!(
        errors,
        [
            (
                "`svugy.legend.entries` is a list of lines, e.g. `[\"① …\"; \"② …\"]`",
                "one line"
            ),
            (
                "`dotted` is not a dash length: a `svugy.legend.dashes` key is a plain number like `5`, an arc's `stroke-dash`",
                "dotted: bus"
            ),
            (
                "`desk.nowhere` names no shape: a `svugy.legend.badges` key is the path of a badge target, e.g. `pipeline.inter`",
                "desk.nowhere: lost"
            ),
            (
                "`svugy.legend.rows` is not a svugy attribute of the root: expected `svugy.subtitle` or `svugy.legend` with `title`, `entries`, `dashes` or `badges`, and the title is the root's own `label`",
                "rows: x"
            ),
            (
                "`yes` is not a boolean: `svugy.badge` is `true` or `false`",
                "yes"
            ),
            (
                "`svugy.caption` is not a svugy list attribute: a list carries only the root's `svugy.legend.entries`",
                "[a; b]"
            ),
            (
                "a list in the `svugy` variable holds only plain values",
                "[nested]"
            ),
            (
                "a list in the `svugy` variable holds only plain values",
                "{ map: 1 }"
            ),
        ]
    );
    let model = import
        .admission
        .model
        .expect("malformed legend values leave a sound model");
    let legend = model.legend().expect("the root carries a legend");
    let sqlite_path = ElementPath::from_segments(["desk", "sqlite"]);
    let sqlite = model.element(&sqlite_path).expect("sqlite is declared");
    let has_badge_edges = model.badge_edges().next().is_some();
    assert_eq!(legend.entries(), ["kept"]);
    let dash_keys: Vec<String> = legend.dashes().keys().map(DashKey::to_string).collect();
    assert_eq!(dash_keys, ["2"]);
    assert_eq!(legend.badge_description(&sqlite_path), Some("kept"));
    assert!(!has_badge_edges);
    assert_eq!(sqlite.caption(), None);
}

fn import(source: &str) -> D2Import {
    let parsed = D2Parse::parse(source);
    parsed.lower()
}

fn import_model(source: &str) -> Model {
    let import = import(source);
    import
        .admission
        .model
        .expect("the fixture lowers into a sound model")
}

fn text_at(source: &str, span: SourceSpan) -> &str {
    let start = span.offset();
    let end = start + span.len();
    &source[start..end]
}
