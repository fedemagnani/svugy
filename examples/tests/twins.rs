//! Guard the twins of each example folder: every format lowers to the model of its `.md`, and every committed `.svg` is the interactive render of its `.md`.

use serde_json::Value;
use svugy::{InteractiveSvg, RenderSvg};
use svugy_model::model::Model;
use svugy_parse::d2::D2Parse;
use svugy_parse::diagnostic::SourceLanguage;
use svugy_parse::json::{JsonLanguage, JsonPrinter};
use svugy_parse::markdown::MarkdownParse;

const WEBAPP_MD: &str = include_str!("../webapp/webapp.md");
const WEBAPP_D2: &str = include_str!("../webapp/webapp.d2");
const WEBAPP_SVG: &str = include_str!("../webapp/webapp.svg");
const SVUGY_MD: &str = include_str!("../svugy/svugy.md");
const SVUGY_D2: &str = include_str!("../svugy/svugy.d2");
const SVUGY_JSON: &str = include_str!("../svugy/svugy.json");
const SVUGY_SVG: &str = include_str!("../svugy/svugy.svg");
const RELEASE_MD: &str = include_str!("../release/release.md");
const RELEASE_SVG: &str = include_str!("../release/release.svg");
const EVENTS_MD: &str = include_str!("../events/events.md");
const EVENTS_SVG: &str = include_str!("../events/events.svg");
const CLUSTER_MD: &str = include_str!("../cluster/cluster.md");
const CLUSTER_SVG: &str = include_str!("../cluster/cluster.svg");

#[test]
fn every_twin_lowers_to_the_model_of_its_markdown_spans_aside() {
    let svugy_markdown_json = markdown_model_json(SVUGY_MD);
    let svugy_markdown_json_in_d2_reach = without_what_d2_cannot_say(&svugy_markdown_json);
    let svugy_d2_json = d2_model_json(SVUGY_D2);
    let svugy_json_json = json_model_json(SVUGY_JSON);
    let webapp_markdown_json = markdown_model_json(WEBAPP_MD);
    let webapp_markdown_json_in_d2_reach = without_what_d2_cannot_say(&webapp_markdown_json);
    let webapp_d2_json = d2_model_json(WEBAPP_D2);

    let twins = [
        (
            "examples/svugy/svugy.d2",
            svugy_d2_json,
            svugy_markdown_json_in_d2_reach,
        ),
        (
            "examples/svugy/svugy.json",
            svugy_json_json,
            svugy_markdown_json,
        ),
        (
            "examples/webapp/webapp.d2",
            webapp_d2_json,
            webapp_markdown_json_in_d2_reach,
        ),
    ];
    for (twin_path, twin_json, markdown_json) in twins {
        assert_eq!(
            twin_json, markdown_json,
            "{twin_path} must describe the model of the .md beside it, spans aside and, for d2, what d2 can't say aside"
        );
    }
}

#[test]
fn every_committed_svg_is_the_interactive_render_of_its_markdown() {
    let renders = [
        ("examples/svugy/svugy.svg", SVUGY_MD, SVUGY_SVG),
        ("examples/webapp/webapp.svg", WEBAPP_MD, WEBAPP_SVG),
        ("examples/release/release.svg", RELEASE_MD, RELEASE_SVG),
        ("examples/events/events.svg", EVENTS_MD, EVENTS_SVG),
        ("examples/cluster/cluster.svg", CLUSTER_MD, CLUSTER_SVG),
    ];
    for (svg_path, markdown_source, committed_svg) in renders {
        let model = markdown_model(markdown_source);
        let fresh_svg = InteractiveSvg::render(&model);
        let is_committed_svg_fresh = fresh_svg == committed_svg;
        assert!(
            is_committed_svg_fresh,
            "{svg_path} must equal the interactive render of the .md beside it: run `just examples`"
        );
    }
}

fn markdown_model(markdown_source: &str) -> Model {
    let markdown_parse = MarkdownParse::parse(markdown_source);
    let markdown_import = markdown_parse.lower();
    let import_parts = markdown_import.into_admitted_parts();
    import_parts
        .model
        .expect("an example .md lowers into a sound model")
}

fn markdown_model_json(markdown_source: &str) -> String {
    let model = markdown_model(markdown_source);
    let model_json = JsonPrinter::print(&model);
    without_spans(&model_json)
}

fn d2_model_json(d2_source: &str) -> String {
    let d2_parse = D2Parse::parse(d2_source);
    let d2_import = d2_parse.lower();
    let model = d2_import
        .admitted_model()
        .expect("an example .d2 lowers into a sound model");
    let model_json = JsonPrinter::print(model);
    without_spans(&model_json)
}

fn json_model_json(json_source: &str) -> String {
    let json_import = JsonLanguage::import(json_source);
    let model = json_import
        .admitted_model()
        .expect("an example .json loads into a sound model");
    let model_json = JsonPrinter::print(model);
    without_spans(&model_json)
}

/// Re-print model JSON without the `span` and `link_span` of each element and edge, source provenance no twin shares.
fn without_spans(model_json: &str) -> String {
    let mut model: Value = serde_json::from_str(model_json).expect("a model saves as JSON");
    let elements = model["elements"]
        .as_object_mut()
        .expect("a model saves its elements as an object");
    for element in elements.values_mut() {
        remove_fields(element, &["span", "link_span"]);
    }
    let edges = model["edges"]
        .as_array_mut()
        .expect("a model saves its edges as an array");
    for edge in edges {
        remove_fields(edge, &["span", "link_span"]);
    }
    serde_json::to_string_pretty(&model).expect("a JSON value prints")
}

/// Re-print model JSON without the diagram's description and the edges' travelers, which the d2 subset has no syntax for.
fn without_what_d2_cannot_say(model_json: &str) -> String {
    let mut model: Value = serde_json::from_str(model_json).expect("a model saves as JSON");
    remove_fields(&mut model, &["description"]);
    let edges = model["edges"]
        .as_array_mut()
        .expect("a model saves its edges as an array");
    for edge in edges {
        remove_fields(edge, &["traveler"]);
    }
    serde_json::to_string_pretty(&model).expect("a JSON value prints")
}

fn remove_fields(record: &mut Value, field_names: &[&str]) {
    let Some(fields) = record.as_object_mut() else {
        return;
    };
    for field_name in field_names {
        fields.remove(*field_name);
    }
}
