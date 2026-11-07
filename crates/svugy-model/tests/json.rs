use miette::Diagnostic;
use svugy_model::builder::ModelBuilder;
use svugy_model::json::MalformedJson;
use svugy_model::legend::Legend;
use svugy_model::model::{ByteRange, Edge, Element, Model, PlacementHint, Position, Shape};
use svugy_model::path::ElementPath;
use svugy_model::problem::{Problem, Suggestion};
use svugy_model::validate::ModelReport;

fn parse(text: &str) -> ElementPath {
    text.parse().expect("well-formed path")
}

fn svugy_model() -> Model {
    let mut builder = ModelBuilder::new()
        .with_title("svugy · how it is built")
        .with_subtitle("text in, SVG out");
    let cli = builder.add_top_level(
        "cli",
        Element::new("svugy-cli")
            .with_caption("render · validate · import")
            .with_height(84.0)
            .with_style("fill", "#2563eb"),
    );
    let pipeline = builder.add_top_level(
        "pipeline",
        Element::new("pipeline")
            .with_caption("the pipeline")
            .with_shape(Shape::Slab)
            .with_child_placement(PlacementHint::Grid { columns: 2 })
            .with_metadata("owner", "backend team"),
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
            .with_shape(Shape::Disc)
            .with_position(Position { x: 640.0, y: 120.5 })
            .with_link("https://example.com/browser")
            .with_span(ByteRange::new(10, 16)),
    );
    let _model = builder.add_edge(&cli, &layout, Edge::new().with_label("① model"));
    let _boxes = builder.add_edge(&layout, &browser, Edge::new().with_label("boxes"));
    let _shapes = builder.add_edge(&layout, &browser, Edge::new().with_label("shapes"));
    let _markup = builder.add_edge(
        &render,
        &browser,
        Edge::new()
            .with_label("④ markup")
            .with_style("stroke-dash", "3"),
    );
    let _writes = builder.add_edge(
        &pipeline,
        &store,
        Edge::new().with_span(ByteRange::new(40, 52)),
    );
    let _retry = builder.add_edge(&render, &render, Edge::new().with_label("retry"));
    builder.build().model.expect("valid model")
}

fn load_rejected_report(json: &str) -> ModelReport {
    let admission = Model::load_json(json).expect("well-formed JSON");
    let is_admitted = admission.model.is_some();
    assert!(!is_admitted, "the model has errors");
    admission.report
}

fn load_admitted_model(json: &str) -> Model {
    let admission = Model::load_json(json).expect("well-formed JSON");
    admission.model.expect("the model has no error")
}

fn load_syntax_message(json: &str) -> String {
    let MalformedJson(syntax_error) = Model::load_json(json).expect_err("malformed JSON");
    syntax_error.to_string()
}

#[test]
fn saved_json_is_id_keyed_and_stable() {
    let model = svugy_model();

    let saved = model.to_json();

    insta::assert_snapshot!(saved);
}

#[test]
fn saved_json_loads_back_into_the_same_model() {
    let model = svugy_model();
    let saved = model.to_json();

    let loaded = load_admitted_model(&saved);
    let saved_again = loaded.to_json();

    assert_eq!(loaded, model);
    assert_eq!(saved_again, saved);
}

#[test]
fn a_legend_and_a_badge_mark_round_trip_through_json_keyed_by_dash_and_by_path() {
    let mut builder = ModelBuilder::new();
    let store_path = parse("pipeline.\"inter.v4\"");
    let legend = Legend::new()
        .with_title("Bet lifecycle")
        .with_entry("① quote")
        .with_entry("② open")
        .with_dash_description(5.0, "HTTP")
        .with_dash_description(1.5, "bus")
        .with_badge_description(store_path.clone(), "reads the bundled font");
    builder = builder.with_legend(legend);
    let pipeline = builder.add_top_level("pipeline", Element::new("pipeline"));
    let layout = builder.add_child(&pipeline, "layout", Element::new("layout"));
    let store_element = Element::new("Inter").with_badge_target();
    let store = builder.add_child(&pipeline, "inter.v4", store_element);
    let _writes = builder.add_edge(&layout, &store, Edge::new());
    let bus = Edge::new().with_style("stroke-dash", "1.5");
    let _bus = builder.add_edge(&layout, &layout, bus);
    let model = builder.build().model.expect("valid model");

    let saved = model.to_json();
    let loaded = load_admitted_model(&saved);

    assert_eq!(loaded, model);
    let saved_value: serde_json::Value = serde_json::from_str(&saved).expect("saved JSON parses");
    let saved_legend = &saved_value["legend"];
    assert_eq!(saved_legend["entries"][1], "② open");
    assert_eq!(saved_legend["dashes"]["1.5"], "bus");
    assert_eq!(
        saved_legend["badges"]["pipeline.\"inter.v4\""],
        "reads the bundled font"
    );
    assert_eq!(
        saved_value["elements"]["pipeline.\"inter.v4\""]["badge"],
        true
    );
    let has_unmarked_badge = saved_value["elements"]["pipeline.layout"]
        .as_object()
        .expect("an element saves as an object")
        .contains_key("badge");
    assert!(!has_unmarked_badge);
}

#[test]
fn a_legend_dash_key_must_be_a_dash_length_in_its_shortest_spelling() {
    let unspelled_dash = r#"{
        "legend": { "dashes": { "5.0": "HTTP", "dotted": "bus" } },
        "elements": { "a": { "label": "A" } }
    }"#;

    let report = load_rejected_report(unspelled_dash);

    let malformed_dashes: Vec<String> = report
        .problems()
        .iter()
        .filter_map(|problem| match problem {
            Problem::MalformedLegendDash(malformed) => Some(malformed.dash.to_string()),
            _other => None,
        })
        .collect();
    assert_eq!(malformed_dashes, ["5.0", "dotted"]);
    let unknown_legend_field = r#"{ "legend": { "rows": [] } }"#;
    let message = load_syntax_message(unknown_legend_field);
    assert!(message.contains("unknown field `rows`"), "{message}");
}

#[test]
fn hand_written_json_may_omit_ordinals_and_optional_fields() {
    let hand_written = r#"{
        "elements": {
            "a": { "label": "A" },
            "a.\"b.c\"": { "label": "dotted id" }
        },
        "edges": [
            { "from": "a", "to": "a.\"b.c\"" }
        ]
    }"#;

    let model = load_admitted_model(hand_written);

    let dotted = model
        .element(&parse(r#"a."b.c""#))
        .expect("dotted id is one element");
    assert_eq!(dotted.label(), "dotted id");
    assert_eq!(model.edges().len(), 1);
    insta::assert_snapshot!(model.to_json());
}

#[test]
fn hand_written_json_titles_the_diagram_at_its_root_and_raises_an_element() {
    let hand_written = r#"{
        "title": "svugy · how it is built",
        "subtitle": "text in, SVG out",
        "elements": { "cli": { "label": "svugy-cli", "height": 84 } }
    }"#;
    let untitled = r#"{ "elements": { "cli": { "label": "svugy-cli" } } }"#;

    let model = load_admitted_model(hand_written);
    let untitled_model = load_admitted_model(untitled);

    let cli = model.element(&parse("cli")).expect("cli is declared");
    let untitled_app = untitled_model
        .element(&parse("cli"))
        .expect("cli is declared");
    assert_eq!(model.title(), Some("svugy · how it is built"));
    assert_eq!(model.subtitle(), Some("text in, SVG out"));
    assert_eq!(cli.height(), Some(84.0));
    assert_eq!(untitled_model.title(), None);
    assert_eq!(untitled_model.subtitle(), None);
    assert_eq!(untitled_app.height(), None);
}

#[test]
fn a_negative_height_in_json_is_rejected_by_the_checks() {
    let sunken = r#"{ "elements": { "cli": { "label": "svugy-cli", "height": -3.5 } } }"#;

    let report = load_rejected_report(sunken);

    let [Problem::ImpossibleHeight(problem)] = report.problems() else {
        panic!("expected one impossible height, got {report:?}");
    };
    assert_eq!(problem.element, parse("cli"));
    assert_eq!(problem.height, -3.5);
}

#[test]
fn a_duplicate_element_key_is_rejected() {
    let duplicate =
        r#"{ "elements": { "api": { "label": "first" }, "api": { "label": "second" } } }"#;

    let message = load_syntax_message(duplicate);

    assert!(message.starts_with("duplicate key `api`"), "{message}");
}

#[test]
fn two_spellings_of_the_same_path_are_a_duplicate() {
    let duplicate =
        r#"{ "elements": { "api": { "label": "bare" }, "\"api\"": { "label": "quoted" } } }"#;

    let message = load_syntax_message(duplicate);

    assert!(message.starts_with("duplicate key `api`"), "{message}");
}

#[test]
fn a_duplicate_style_or_metadata_key_is_rejected() {
    let duplicate_style = r##"{ "elements": { "a": { "label": "A", "style": { "fill": "#fff", "fill": "#000" } } } }"##;
    let duplicate_metadata =
        r#"{ "elements": { "a": { "label": "A", "metadata": { "k": "1", "k": "2" } } } }"#;
    let duplicate_edge_style = r#"{
        "elements": { "a": { "label": "A" } },
        "edges": [ { "from": "a", "to": "a", "style": { "stroke": "red", "stroke": "blue" } } ]
    }"#;

    let style_message = load_syntax_message(duplicate_style);
    let metadata_message = load_syntax_message(duplicate_metadata);
    let edge_style_message = load_syntax_message(duplicate_edge_style);

    assert!(
        style_message.starts_with("duplicate key `fill`"),
        "{style_message}"
    );
    assert!(
        metadata_message.starts_with("duplicate key `k`"),
        "{metadata_message}"
    );
    assert!(
        edge_style_message.starts_with("duplicate key `stroke`"),
        "{edge_style_message}"
    );
}

#[test]
fn a_duplicate_edge_id_is_rejected() {
    let duplicate = r#"{
        "elements": { "a": { "label": "A" }, "b": { "label": "B" } },
        "edges": [ { "from": "a", "to": "b", "ordinal": 0 }, { "from": "a", "to": "b" } ]
    }"#;

    let message = load_syntax_message(duplicate);

    assert!(
        message.starts_with("duplicate edge `(a -> b)[0]`"),
        "{message}"
    );
}

#[test]
fn unknown_fields_and_malformed_paths_are_rejected() {
    let unknown_field = r#"{ "elements": { "a": { "label": "A", "lable": "typo" } } }"#;
    let malformed_path = r#"{ "elements": { "a..b": { "label": "A" } } }"#;

    let unknown_field_message = load_syntax_message(unknown_field);
    let malformed_path_message = load_syntax_message(malformed_path);

    assert!(
        unknown_field_message.contains("unknown field `lable`"),
        "{unknown_field_message}"
    );
    assert!(
        malformed_path_message.contains("empty segment at byte 2"),
        "{malformed_path_message}"
    );
}

#[test]
fn a_dangling_edge_endpoint_is_reported_with_a_suggestion() {
    let dangling = r#"{
        "elements": {
            "backend": { "label": "backend" },
            "backend.api": { "label": "API" },
            "backend.worker": { "label": "worker" }
        },
        "edges": [ { "from": "backend.worker", "to": "backend.apii", "span": { "start": 5, "end": 14 } } ]
    }"#;

    let report = load_rejected_report(dangling);

    let problems = report.problems();
    assert_eq!(problems.len(), 1);
    let Problem::DanglingEdgeEndpoint(problem) = &problems[0] else {
        panic!("expected a dangling endpoint, got {:?}", problems[0]);
    };
    let expected_suggestion = Suggestion(parse("backend.api"));
    let help = problem.help().map(|help| help.to_string());
    assert_eq!(problem.endpoint, parse("backend.apii"));
    assert_eq!(problem.suggestion, Some(expected_suggestion));
    assert_eq!(help.as_deref(), Some("did you mean `backend.api`?"));
    assert_eq!(problem.span, Some((5..14).into()));
}

#[test]
fn a_missing_ancestor_is_reported_once_with_a_suggestion() {
    let missing_ancestor = r#"{
        "elements": {
            "backend": { "label": "backend" },
            "bakend.api": { "label": "API" },
            "bakend.worker": { "label": "worker" }
        }
    }"#;

    let report = load_rejected_report(missing_ancestor);

    let problems = report.problems();
    assert_eq!(problems.len(), 1);
    let Problem::MissingAncestor(problem) = &problems[0] else {
        panic!("expected a missing ancestor, got {:?}", problems[0]);
    };
    let expected_suggestion = Suggestion(parse("backend"));
    assert_eq!(problem.element, parse("bakend.api"));
    assert_eq!(problem.ancestor, parse("bakend"));
    assert_eq!(problem.suggestion, Some(expected_suggestion));
}

#[test]
fn a_far_off_reference_gets_no_suggestion() {
    let dangling = r#"{
        "elements": { "backend": { "label": "backend" } },
        "edges": [ { "from": "backend", "to": "cache" } ]
    }"#;

    let report = load_rejected_report(dangling);

    let Problem::DanglingEdgeEndpoint(problem) = &report.problems()[0] else {
        panic!("expected a dangling endpoint");
    };
    assert_eq!(problem.suggestion, None);
}

#[test]
fn every_dangling_reference_reaches_the_machine_readable_report() {
    let dangling = r#"{
        "elements": {
            "db": { "label": "db" },
            "db.primary": { "label": "primary" },
            "cache.hot": { "label": "hot cache", "span": { "start": 30, "end": 39 } }
        },
        "edges": [
            { "from": "db.primary", "to": "db.primry", "span": { "start": 5, "end": 14 } },
            { "from": "ghost", "to": "ghost" }
        ]
    }"#;
    let report = load_rejected_report(dangling);

    let rendered = report.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("handler emits JSON");
    let pretty = serde_json::to_string_pretty(&parsed).expect("re-serializes");

    insta::assert_snapshot!(pretty);
}
