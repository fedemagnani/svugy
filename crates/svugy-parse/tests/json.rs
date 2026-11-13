//! Pin how a model loads from and prints back as JSON: duplicate and v3 keys rejected, the model admitted through every check, the save stable.

use miette::Diagnostic;
use svugy_model::builder::ModelBuilder;
use svugy_model::disclose::DisclosedGraph;
use svugy_model::model::{
    ByteRange, Edge, Element, Model, PlacementHint, Position, Shape, Traveler,
};
use svugy_model::path::ElementPath;
use svugy_model::problem::{Problem, Suggestion};
use svugy_model::properties::ComponentProperties;
use svugy_model::validate::ModelReport;
use svugy_model::workflow::{ElementOverride, Step, StepId, Workflow, WorkflowId};
use svugy_parse::diagnostic::SourceLanguage;
use svugy_parse::json::{JsonImport, JsonLanguage, JsonPrinter};

#[test]
fn saved_json_is_id_keyed_and_stable() {
    let model = svugy_model();

    let saved = JsonPrinter::print(&model);

    insta::assert_snapshot!(saved);
}

#[test]
fn saved_json_loads_back_into_the_same_model() {
    let model = svugy_model();
    let saved = JsonPrinter::print(&model);

    let loaded = load_admitted_model(&saved);
    let saved_again = JsonPrinter::print(&loaded);

    assert_eq!(loaded, model);
    assert_eq!(saved_again, saved);
}

#[test]
fn hand_written_json_may_omit_ordinals_and_optional_fields() {
    let hand_written = r#"{
        "elements": {
            "a": { "label": "A" },
            "a.\"b.c\"": { "label": "dotted id" },
            "c": { "label": "C" }
        },
        "workflows": [ {
            "id": "main",
            "title": "Main",
            "edges": [ { "from": "a", "to": "c" } ],
            "steps": [ { "id": "all", "title": "All", "items": [
                { "edge": { "from": "a", "to": "c" } },
                { "element": "a.\"b.c\"" }
            ] } ]
        } ]
    }"#;

    let model = load_admitted_model(hand_written);

    let dotted = model
        .element(&parse(r#"a."b.c""#))
        .expect("dotted id is one element");
    let [main_workflow] = model.workflows() else {
        panic!("one workflow was written");
    };
    let edge_ids: Vec<String> = main_workflow
        .edges()
        .keys()
        .map(ToString::to_string)
        .collect();
    assert_eq!(dotted.label(), "dotted id");
    assert_eq!(
        edge_ids,
        ["(a -> c)[0]"],
        "an omitted ordinal is 0, in the edge and in the item naming it"
    );
    let model_json = JsonPrinter::print(&model);
    insta::assert_snapshot!(model_json);
}

#[test]
fn v3_keys_are_rejected_naming_their_fix() {
    let cases = [
        (
            r#"{ "title": "T" }"#,
            "`title` moved from the root into workflows",
        ),
        (
            r#"{ "subtitle": "S" }"#,
            "`subtitle` moved from the root into workflows",
        ),
        (
            r#"{ "description": "D" }"#,
            "`description` moved from the root into workflows",
        ),
        (r#"{ "legend": {} }"#, "`legend` was removed"),
        (
            r#"{ "edges": [] }"#,
            "`edges` moved from the root into workflows",
        ),
        (
            r#"{ "elements": { "a": { "label": "A", "position": { "x": 0, "y": 0 } } } }"#,
            "`position` moved from elements to workflows",
        ),
        (
            r#"{ "elements": { "a": { "label": "A", "child_placement": "row" } } }"#,
            "`child_placement` moved from elements to workflows",
        ),
        (
            r#"{ "elements": { "a": { "label": "A", "badge": true } } }"#,
            "`badge` moved from elements to edges",
        ),
    ];
    for (v3_json, expected_start) in cases {
        let message = load_syntax_message(v3_json);
        assert!(
            message.starts_with(expected_start),
            "a v3 key is an error naming its v4 home: {message}"
        );
    }
}

#[test]
fn duplicate_keys_unknown_fields_missing_root_keys_and_malformed_paths_are_rejected() {
    let edges_with_style = r#""edges": [ { "from": "a", "to": "b", "style": { "stroke": "red", "stroke": "blue" } } ]"#;
    let duplicate_edges =
        r#""edges": [ { "from": "a", "to": "b", "ordinal": 0 }, { "from": "a", "to": "b" } ]"#;
    let duplicate_overrides =
        r#""overrides": { "a": { "caption": "x" }, "\"a\"": { "caption": "y" } }"#;
    let cases = [
        (r#"{ "elements": { "api": { "label": "first" }, "api": { "label": "second" } } }"#.to_owned(), "duplicate key `api`"),
        (r#"{ "elements": { "api": { "label": "bare" }, "\"api\"": { "label": "quoted" } } }"#.to_owned(), "duplicate key `api`"),
        (r##"{ "elements": { "a": { "label": "A", "style": { "fill": "#fff", "fill": "#000" } } } }"##.to_owned(), "duplicate key `fill`"),
        (r#"{ "elements": { "a": { "label": "A", "metadata": { "k": "1", "k": "2" } } } }"#.to_owned(), "duplicate key `k`"),
        (workflow_json(edges_with_style), "duplicate key `stroke`"),
        (workflow_json(duplicate_edges), "duplicate edge `(a -> b)[0]`"),
        (workflow_json(duplicate_overrides), "duplicate key `a`"),
        (r#"{ "elements": { "a": { "label": "A", "lable": "typo" } } }"#.to_owned(), "unknown field `lable`"),
        (r#"{ "elements": { "a..b": { "label": "A" } } }"#.to_owned(), "empty segment at byte 2"),
        (r#"{ "workflows": [] }"#.to_owned(), "missing field `elements`"),
        (r#"{ "elements": {} }"#.to_owned(), "missing field `workflows`"),
    ];
    for (json, expected_message) in cases {
        let message = load_syntax_message(&json);
        assert!(message.contains(expected_message), "{message}");
    }
}

#[test]
fn a_dangling_edge_endpoint_is_reported_with_a_suggestion() {
    let dangling = r#"{
        "elements": {
            "backend": { "label": "backend" },
            "backend.api": { "label": "API" },
            "backend.worker": { "label": "worker" }
        },
        "workflows": [ {
            "id": "main",
            "title": "Main",
            "edges": [ { "from": "backend.worker", "to": "backend.apii", "span": { "start": 5, "end": 14 } } ],
            "steps": [ { "id": "all", "title": "All", "items": [
                { "edge": { "from": "backend.worker", "to": "backend.apii" } },
                { "element": "backend.api" }
            ] } ]
        } ]
    }"#;

    let report = load_rejected_report(dangling);

    let [Problem::DanglingEdgeEndpoint(problem)] = report.problems() else {
        panic!("expected one dangling endpoint, got {report:?}");
    };
    let expected_suggestion = Suggestion(parse("backend.api"));
    let help = problem.help().map(|help| help.to_string());
    assert_eq!(problem.endpoint, parse("backend.apii"));
    assert_eq!(problem.suggestion, Some(expected_suggestion));
    assert_eq!(help.as_deref(), Some("did you mean `backend.api`?"));
    assert_eq!(problem.span, Some((5..14).into()));
}

#[test]
fn every_dangling_reference_reaches_the_machine_readable_report() {
    let dangling = r#"{
        "elements": {
            "db": { "label": "db", "description": "Reads go to [the primary](#db.primry)." },
            "db.primary": { "label": "primary" },
            "cache.hot": { "label": "hot cache", "span": { "start": 30, "end": 39 } }
        },
        "workflows": [ {
            "id": "main",
            "title": "Main",
            "edges": [
                { "from": "db.primary", "to": "db.primry", "span": { "start": 5, "end": 14 } },
                { "from": "ghost", "to": "ghost" }
            ],
            "steps": [ { "id": "all", "title": "All", "items": [
                { "edge": { "from": "db.primary", "to": "db.primry" } },
                { "edge": { "from": "db.primary", "to": "db.primry", "ordinal": 1 } },
                { "element": "cache.hot" }
            ] } ]
        } ]
    }"#;
    let report = load_rejected_report(dangling);

    let rendered = report.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("handler emits JSON");
    let pretty = serde_json::to_string_pretty(&parsed).expect("re-serializes");

    insta::assert_snapshot!(pretty);
}

#[test]
fn a_malformed_json_error_is_labelled_where_serde_stopped_reading() {
    let trailing_garbage = "{\n  \"elements\": {},\n  \"workflows\": []\n} x";

    let import = JsonLanguage::import(trailing_garbage);

    let [syntax_error] = import.errors.as_slice() else {
        panic!("trailing garbage raises one error, got {:?}", import.errors);
    };
    let garbage_offset = trailing_garbage.find('x');
    let label_offset = Some(syntax_error.span.offset());
    let has_admitted_model = import.admitted_model().is_some();
    let model_problems = import.admission.report.problems();
    assert_eq!(
        label_offset, garbage_offset,
        "serde's line and column must label the byte it stopped at: {syntax_error}"
    );
    assert!(!has_admitted_model);
    assert!(
        model_problems.is_empty(),
        "malformed JSON reports its one error and no model problem: {model_problems:?}"
    );
}

#[test]
fn the_deepest_disclosed_layer_is_the_workflow_graph_itself_even_when_edge_ordinals_skip() {
    let json_with_skipped_ordinal = r#"{
        "elements": {
            "a": { "label": "A" },
            "a.b": { "label": "B" },
            "c": { "label": "C" }
        },
        "workflows": [ {
            "id": "main",
            "title": "Main",
            "edges": [
                { "from": "a.b", "to": "c", "ordinal": 0 },
                { "from": "a.b", "to": "c", "ordinal": 2 }
            ],
            "steps": [ { "id": "all", "title": "All", "items": [
                { "edge": { "from": "a.b", "to": "c", "ordinal": 0 } },
                { "edge": { "from": "a.b", "to": "c", "ordinal": 2 } }
            ] } ]
        } ]
    }"#;
    let model = load_admitted_model(json_with_skipped_ordinal);
    let main_workflow = model.showing_first_workflow();
    let main_projection = main_workflow.projected();
    let main_graph = main_projection.graph();

    let layers = DisclosedGraph::every_layer_of(main_graph);

    let deepest_layer = layers.last().expect("every graph has layer 0");
    assert!(
        deepest_layer.graph() == main_graph,
        "the deepest layer must be the workflow graph itself, written edge ids included, so the static profile draws what the workflow lists"
    );
}

fn svugy_model() -> Model {
    let mut builder = ModelBuilder::new();
    let cli = builder.add_top_level(
        "cli",
        Element::new("svugy-cli")
            .with_caption("render · validate · import")
            .with_height(84.0)
            .with_style("fill", "#2563eb")
            .with_description("Reads [the layout](#pipeline.layout) back through [its edge](#cli->pipeline.layout)."),
    );
    let pipeline = builder.add_top_level(
        "pipeline",
        Element::new("pipeline").with_properties(
            ComponentProperties::new()
                .with_caption("the pipeline")
                .with_shape(Shape::Slab)
                .with_metadata("owner", "backend team"),
        ),
    );
    let layout = builder.add_child(
        &pipeline,
        "layout",
        Element::new("Layout").with_caption("plan space"),
    );
    let render = builder.add_child(
        &pipeline,
        "render",
        Element::new("Render").with_caption("markup"),
    );
    let store = builder.add_child(
        &pipeline,
        "inter.v4",
        Element::new("Inter").with_shape(Shape::Cylinder),
    );
    let browser = builder.add_top_level(
        "browser",
        Element::new("Browser")
            .with_properties(
                ComponentProperties::new()
                    .with_shape(Shape::Disc)
                    .with_link("https://example.com/browser"),
            )
            .with_span(ByteRange::new(10, 16)),
    );
    let build_workflow = Workflow::new(WorkflowId::new("build"), "How it is built")
        .with_subtitle("text in, SVG out")
        .with_span(ByteRange::new(60, 90));
    let build = builder.add_workflow(build_workflow);
    let grid = ElementOverride::new().with_child_placement(PlacementHint::Grid { columns: 2 });
    builder.override_element(&build, &pipeline, grid);
    let any_browser = ComponentProperties::new().with_caption("any browser");
    let pinned_browser = ElementOverride::new()
        .with_position(Position { x: 640.0, y: 120.5 })
        .with_properties(any_browser);
    builder.override_element(&build, &browser, pinned_browser);
    let lay_out_step = Step::new(StepId::new("lay_out"), "The CLI lays the model out")
        .with_span(ByteRange::new(92, 120));
    let lay_out = builder.add_step(&build, lay_out_step);
    let model_edge = builder.add_edge(&lay_out, &cli, &layout, Edge::new().with_label("① model"));
    let _boxes = builder.add_edge(&lay_out, &layout, &browser, Edge::new().with_label("boxes"));
    let shapes = Edge::new()
        .with_label("shapes")
        .with_traveler(Traveler::Parcel);
    let _shapes = builder.add_edge(&lay_out, &layout, &browser, shapes);
    let draw_step = Step::new(StepId::new("draw"), "The renderer draws it");
    let draw = builder.add_step(&build, draw_step);
    let markup = Edge::new()
        .with_label("④ markup")
        .with_style("stroke-dash", "3");
    let _markup = builder.add_edge(&draw, &render, &browser, markup);
    let font = Edge::new().with_badge().with_span(ByteRange::new(40, 52));
    let _font = builder.add_edge(&draw, &render, &store, font);
    builder.list_edge(&draw, model_edge.id().clone());
    let draw_description = "[The renderer](#pipeline.render) reads [the font](#build/draw).";
    let draw_owner = svugy_model::builder::DescribedRef::Step(draw);
    builder.describe(&draw_owner, draw_description);
    builder.build().model.expect("valid model")
}

fn workflow_json(workflow_field: &str) -> String {
    format!(
        r#"{{ "elements": {{ "a": {{ "label": "A" }}, "b": {{ "label": "B" }} }}, "workflows": [ {{ "id": "main", "title": "Main", {workflow_field}, "steps": [] }} ] }}"#
    )
}

fn load_well_formed(json: &str) -> JsonImport {
    let import = JsonLanguage::import(json);
    let syntax_messages: Vec<String> = import.errors.iter().map(ToString::to_string).collect();
    assert!(
        syntax_messages.is_empty(),
        "well-formed JSON: {syntax_messages:?}"
    );
    import
}

fn load_rejected_report(json: &str) -> ModelReport {
    let import = load_well_formed(json);
    let admission = import.admission;
    let is_admitted = admission.model.is_some();
    assert!(!is_admitted, "the model has errors");
    admission.report
}

fn load_admitted_model(json: &str) -> Model {
    let import = load_well_formed(json);
    let admission = import.admission;
    admission.model.expect("the model has no error")
}

fn load_syntax_message(json: &str) -> String {
    let import = JsonLanguage::import(json);
    let [syntax_error] = import.errors.as_slice() else {
        panic!("malformed JSON raises one error, got {:?}", import.errors);
    };
    syntax_error.to_string()
}

fn parse(text: &str) -> ElementPath {
    text.parse().expect("well-formed path")
}
