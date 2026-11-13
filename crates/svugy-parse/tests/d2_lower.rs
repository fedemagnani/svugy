//! Prove d2 lowering merges repeated keys, declares endpoints, lists every edge in one workflow and warns on every construct the model cannot hold.

use miette::SourceSpan;
use svugy_model::model::{EdgeId, Model, Shape};
use svugy_model::path::ElementPath;
use svugy_model::problem::Problem;
use svugy_model::workflow::{Workflow, WorkflowId};
use svugy_parse::d2::{Construct, D2Import, D2Parse, EdgeOperator};
use svugy_parse::json::JsonPrinter;

const NESTING: &str = include_str!("fixtures/d2/nesting.d2");
const EDGES: &str = include_str!("fixtures/d2/edges.d2");
const STYLES: &str = include_str!("fixtures/d2/styles.d2");
const MERGING: &str = include_str!("fixtures/d2/merging.d2");
const ENDPOINTS: &str = include_str!("fixtures/d2/endpoints.d2");
const UNMAPPED: &str = include_str!("fixtures/d2/unmapped.d2");

#[test]
fn nesting_lowers_into_a_model_snapshot() {
    let model = import_model(NESTING);

    let model_json = JsonPrinter::print(&model);
    insta::assert_snapshot!(model_json);
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
    let model_json = JsonPrinter::print(&model);
    insta::assert_snapshot!(model_json);
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

    let model_json = JsonPrinter::print(&model);
    insta::assert_snapshot!(model_json);
}

#[test]
fn repeated_keys_merge_into_a_model_snapshot() {
    let model = import_model(MERGING);

    let model_json = JsonPrinter::print(&model);
    insta::assert_snapshot!(model_json);
}

#[test]
fn edge_endpoints_declare_elements_into_a_model_snapshot() {
    let model = import_model(ENDPOINTS);

    let model_json = JsonPrinter::print(&model);
    insta::assert_snapshot!(model_json);
}

#[test]
fn unmapped_constructs_lower_into_a_model_snapshot() {
    let model = import_model(UNMAPPED);

    let model_json = JsonPrinter::print(&model);
    insta::assert_snapshot!(model_json);
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
fn lineage_edges_are_dropped_with_a_warning_and_their_untouched_leaf_is_still_listed() {
    let source = "a -> a.b\nb -> b\na.b -> c\n";

    let import = import(source);

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
        .expect("dropping lineage edges leaves a sound model");
    let main = main_workflow(&model);
    let edge_ids: Vec<String> = main.edges().keys().map(ToString::to_string).collect();
    let every_edge_step = &main.steps()[0];
    let listed_items: Vec<String> = every_edge_step.items().map(ToString::to_string).collect();
    assert_eq!(
        warnings,
        [
            (Construct::LineageEdge, "a -> a.b"),
            (Construct::LineageEdge, "b -> b"),
        ]
    );
    assert_eq!(edge_ids, ["(a.b -> c)[0]"]);
    assert_eq!(listed_items, ["edge `(a.b -> c)[0]`", "element `b`"]);
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
            (Construct::ContainerBadge, "true"),
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
            ("svugy::d2::unsupported".to_owned(), "a.b -> a"),
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
    let main = main_workflow(&model);
    let edge_ids: Vec<String> = main.edges().keys().map(ToString::to_string).collect();
    let f_fill = model
        .element(&ElementPath::from_segments(["a", "f"]))
        .map(|element| element.properties().style()["fill"].as_str());
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
fn the_svugy_variable_carries_caption_and_primitive_onto_the_element_and_position_onto_its_main_override()
 {
    let source = "bus: { vars: { svugy: { caption: \"a · b\"; shape: slab; position: { x: 150; y: 91.5 } } } }\nflat.vars: { svugy.caption: dotted; svugy.position.x: 0; svugy.position.y: 2 }\n";

    let model = import_model(source);

    let bus_path = ElementPath::from_segments(["bus"]);
    let flat_path = ElementPath::from_segments(["flat"]);
    let bus = model.element(&bus_path).expect("bus is declared");
    let flat = model.element(&flat_path).expect("flat is declared");
    let main = main_workflow(&model);
    let bus_override = main.override_of(&bus_path);
    let flat_override = main.override_of(&flat_path);
    let bus_position = bus_override
        .and_then(|bus_override| bus_override.position())
        .map(|position| (position.x, position.y));
    let flat_position = flat_override
        .and_then(|flat_override| flat_override.position())
        .map(|position| (position.x, position.y));
    assert_eq!(bus.properties().caption(), Some("a · b"));
    assert_eq!(bus.properties().shape(), Some(Shape::Slab));
    assert_eq!(bus_position, Some((150.0, 91.5)));
    assert_eq!(flat.properties().caption(), Some("dotted"));
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
            let site = impossible_height.site.to_string();
            let span = impossible_height
                .span
                .expect("lowered elements carry spans");
            let text = text_at(source, span);
            (site, impossible_height.height, text)
        })
        .collect();
    assert!(is_free_of_syntax_errors);
    assert!(admission.model.is_none());
    assert_eq!(
        impossible_heights,
        [("element `flat`".to_owned(), 0.0, "flat")]
    );
    let sound_source = source.replace("height: 0", "height: 7");
    let model = import_model(&sound_source);
    let heights: Vec<(String, Option<f64>)> = model
        .elements()
        .iter()
        .map(|(path, element)| (path.to_string(), element.properties().height()))
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
    assert_eq!(app.properties().caption(), Some("kept"));
    assert_eq!(app.properties().height(), None);
}

#[test]
fn the_root_label_titles_the_main_workflow_and_the_root_carrier_subtitles_it() {
    let source = "label: \"svugy · how it is built\"\nvars: { svugy: { subtitle: \"text in, SVG out\" } }\na: { _.label: \"last write wins\" }\n";

    let import = import(source);

    let diagnostic_count = import.diagnostics().len();
    let model = import
        .admission
        .model
        .expect("a titled workflow is a sound model");
    let element_paths: Vec<String> = model.elements().keys().map(ToString::to_string).collect();
    let main = main_workflow(&model);
    assert_eq!(diagnostic_count, 0);
    assert_eq!(main.title(), "last write wins");
    assert_eq!(main.subtitle(), Some("text in, SVG out"));
    assert_eq!(element_paths, ["a"]);
}

#[test]
fn a_diagram_declaring_no_shape_is_one_error_at_the_file_start_not_an_empty_main_step() {
    let label_only_source = "label: Hi\n";

    let import = import(label_only_source);

    let diagnostic_messages: Vec<String> = import
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect();
    let error_offsets: Vec<usize> = import
        .errors
        .iter()
        .map(|error| error.span.offset())
        .collect();
    let has_admitted_model = import.admitted_model().is_some();
    assert_eq!(
        diagnostic_messages,
        ["the diagram declares no shape, so there is nothing to draw"],
        "a shapeless diagram is the author's d2 error, never a step they did not write"
    );
    assert_eq!(error_offsets, [0]);
    assert!(!has_admitted_model);
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
    let main = main_workflow(&model);
    assert_eq!(
        errors,
        [
            (
                "`svugy.title` is not a svugy attribute of the root: expected `svugy.subtitle`, and the title is the root's own `label`",
                "title: nope"
            ),
            (
                "`svugy.height` is not a svugy attribute of the root: expected `svugy.subtitle`, and the title is the root's own `label`",
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
    assert_eq!(main.title(), "main");
    assert_eq!(main.subtitle(), None);
}

#[test]
fn a_svugy_legend_and_a_d2_legend_are_one_error_each_naming_where_their_meaning_goes() {
    let source = r#"vars: {
  svugy: {
    subtitle: kept
    legend: {
      title: Bet lifecycle
      dashes: { 5: HTTP }
    }
  }
  d2-legend: {
    a: { label: Microservice }
  }
}
quoter -> desk
"#;

    let import = import(source);

    let errors: Vec<(&str, &str)> = import
        .errors
        .iter()
        .map(|error| {
            let text = text_at(source, error.span);
            let first_line = text.lines().next().unwrap_or_default();
            (error.message.as_str(), first_line)
        })
        .collect();
    let model = import
        .admission
        .model
        .expect("a rejected legend leaves a sound model");
    let main = main_workflow(&model);
    assert_eq!(
        errors,
        [
            (
                "`svugy.legend` sets a legend, but svugy v4 has no legend: say what each entry meant in step prose or edge descriptions",
                "legend: {"
            ),
            (
                "`d2-legend` sets a legend, but svugy v4 has no legend: say what each entry meant in step prose or edge descriptions",
                "d2-legend: {"
            ),
        ]
    );
    assert_eq!(main.subtitle(), Some("kept"));
}

#[test]
fn the_carrier_primitive_outranks_the_d2_shape_whatever_their_order() {
    let source = "before: { vars: { svugy: { shape: disc } }; shape: cylinder }\nafter: { shape: cylinder; vars: { svugy: { shape: box } } }\n";

    let model = import_model(source);

    let shapes: Vec<Option<Shape>> = model
        .elements()
        .values()
        .map(|element| element.properties().shape())
        .collect();
    assert_eq!(shapes, [Some(Shape::Box), Some(Shape::Disc)]);
}

#[test]
fn a_badge_mark_badges_each_edge_from_a_leaf_into_the_marked_leaf_the_last_mark_winning() {
    let source = r#"desk: {
  sqlite: SQLite { vars: { svugy: { badge: true } } }
  cache: { vars: { svugy: { badge: TRUE } } }
  quoter
  quoter -> sqlite
  quoter -> cache
}
desk.cache.vars: { svugy.badge: false }
"#;

    let model = import_model(source);

    let main = main_workflow(&model);
    let quoter_path = ElementPath::from_segments(["desk", "quoter"]);
    let sqlite_path = ElementPath::from_segments(["desk", "sqlite"]);
    let cache_path = ElementPath::from_segments(["desk", "cache"]);
    let writes = EdgeId::new(quoter_path.clone(), sqlite_path, 0);
    let caches = EdgeId::new(quoter_path, cache_path, 0);
    let is_writes_badged = main.edge(&writes).is_some_and(|edge| edge.is_badge());
    let is_caches_badged = main.edge(&caches).is_some_and(|edge| edge.is_badge());
    assert!(is_writes_badged);
    assert!(!is_caches_badged);
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
      badge: yes
      height: [1; 2]
      position: { x: 1.5; y: north }
      x -> y
    }
  }
}
b.vars: { svugy: { shape: slab; position.x: 4 } }
vars: { svugy: { caption: nowhere } }
e -> f: { vars: { svugy: { traveler: envelope } } }
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
    let a_path = ElementPath::from_segments(["a"]);
    let b_path = ElementPath::from_segments(["b"]);
    let a = model.element(&a_path).expect("a is declared");
    let b = model.element(&b_path).expect("b is declared");
    let main = main_workflow(&model);
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
                "`yes` is not a boolean: `svugy.badge` is `true` or `false`",
                "yes"
            ),
            ("a svugy attribute takes one value, not a list", "[1; 2]"),
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
                "`svugy.caption` is not a svugy attribute of the root: expected `svugy.subtitle`, and the title is the root's own `label`",
                "caption: nowhere"
            ),
            (
                "d2 rejects a `vars` map in an edge map: set the edge's traveler in Markdown (`.md`)",
                "vars: { svugy: { traveler: envelope } }"
            ),
        ]
    );
    assert_eq!(warnings, [(Construct::Variables, "color: red")]);
    assert_eq!(main.title(), "main");
    assert_eq!(main.subtitle(), None);
    assert_eq!(a.properties().caption(), Some("kept"));
    assert_eq!(a.properties().shape(), Some(Shape::Disc));
    assert_eq!(a.properties().height(), None);
    assert_eq!(main.override_of(&a_path), None);
    assert_eq!(b.properties().shape(), Some(Shape::Slab));
    assert_eq!(main.override_of(&b_path), None);
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

fn main_workflow(model: &Model) -> &Workflow {
    let main_id = WorkflowId::new("main");
    model
        .workflow(&main_id)
        .expect("a d2 import yields the workflow `main`")
}

fn text_at(source: &str, span: SourceSpan) -> &str {
    let start = span.offset();
    let end = start + span.len();
    &source[start..end]
}
