//! Snapshot the svugy golden, examples/svugy.md, as static and interactive SVG text and as its static raster, and guard its d2 and JSON twins.

mod support;

use serde_json::Value;
use support::pixel_snapshot_test;
use svugy::{InteractiveSvg, RenderSvg, StaticSvg};
use svugy_model::model::Model;
use svugy_parse::d2::D2Parse;
use svugy_parse::diagnostic::SourceLanguage;
use svugy_parse::json::{JsonLanguage, JsonPrinter};
use svugy_parse::markdown::MarkdownParse;

const SVUGY_MD: &str = include_str!("../../../examples/svugy.md");
const SVUGY_D2: &str = include_str!("../../../examples/svugy.d2");
const SVUGY_JSON: &str = include_str!("../../../examples/svugy.json");

const MARKUP_ONLY_LIVE_VIEWERS_RUN: [&str; 8] = [
    "<script",
    "<style",
    "<text",
    "var(",
    "<animate",
    "context-stroke",
    "data-svugy-traveler",
    "data-svugy-overlay",
];

#[test]
fn svugy_example_renders_as_stable_static_svg() {
    let static_svg = render_svugy_example();
    insta::assert_snapshot!(static_svg);
}

#[test]
fn svugy_example_renders_as_stable_interactive_svg() {
    let model = svugy_example_model();
    let interactive_svg = InteractiveSvg::render(&model);
    insta::assert_snapshot!(interactive_svg);
}

#[test]
fn svugy_example_renders_the_same_interactive_bytes_twice() {
    let model = svugy_example_model();
    let first_render = InteractiveSvg::render(&model);
    let second_render = InteractiveSvg::render(&model);
    assert!(first_render == second_render);
}

#[test]
fn svugy_example_interactive_svg_parses_as_xml() {
    let model = svugy_example_model();
    let interactive_svg = InteractiveSvg::render(&model);
    let options = usvg::Options::default();
    let parse_result = usvg::Tree::from_str(&interactive_svg, &options);
    assert!(parse_result.is_ok(), "{parse_result:?}");
}

#[test]
fn svugy_example_static_svg_needs_no_css_script_font_or_animation() {
    let static_svg = render_svugy_example();
    for live_only_markup in MARKUP_ONLY_LIVE_VIEWERS_RUN {
        let is_live_only_markup_present = static_svg.contains(live_only_markup);
        assert!(!is_live_only_markup_present, "{live_only_markup}");
    }
}

#[test]
fn svugy_example_twins_describe_the_same_model() {
    let markdown_model = svugy_example_model();
    let markdown_json = JsonPrinter::print(&markdown_model);
    let markdown_json_without_spans = without_spans(&markdown_json);

    let d2_parse = D2Parse::parse(SVUGY_D2);
    let d2_import = d2_parse.lower();
    let d2_model = d2_import
        .admitted_model()
        .expect("examples/svugy.d2 lowers into a sound model");
    let d2_json = JsonPrinter::print(d2_model);
    let d2_json_without_spans = without_spans(&d2_json);

    let json_import = JsonLanguage::import(SVUGY_JSON);
    let json_model = json_import
        .admitted_model()
        .expect("examples/svugy.json loads into a sound model");
    let json_json = JsonPrinter::print(json_model);
    let json_json_without_spans = without_spans(&json_json);

    let invariant = "examples/svugy.d2 and examples/svugy.json must describe the same model as examples/svugy.md, spans aside";
    assert_eq!(
        d2_json_without_spans, markdown_json_without_spans,
        "{invariant}: examples/svugy.d2 differs"
    );
    assert_eq!(
        json_json_without_spans, markdown_json_without_spans,
        "{invariant}: examples/svugy.json differs"
    );
}

pixel_snapshot_test!(svugy_example_static_raster, render_svugy_example());

fn render_svugy_example() -> String {
    let model = svugy_example_model();
    StaticSvg::render(&model)
}

/// Parse examples/svugy.md into its admitted model.
fn svugy_example_model() -> Model {
    let markdown_parse = MarkdownParse::parse(SVUGY_MD);
    let markdown_import = markdown_parse.lower();
    let import_parts = markdown_import.into_admitted_parts();
    import_parts
        .model
        .expect("examples/svugy.md lowers into a sound model")
}

/// Re-print model JSON without the `span` and `link_span` of each element and edge, source provenance the snapshot and twin comparison leave out.
fn without_spans(model_json: &str) -> String {
    let mut model: Value = serde_json::from_str(model_json).expect("a model saves as JSON");
    let elements = model["elements"]
        .as_object_mut()
        .expect("a model saves its elements as an object");
    for element in elements.values_mut() {
        remove_span(element);
    }
    let edges = model["edges"]
        .as_array_mut()
        .expect("a model saves its edges as an array");
    for edge in edges {
        remove_span(edge);
    }
    serde_json::to_string_pretty(&model).expect("a JSON value prints")
}

fn remove_span(record: &mut Value) {
    let Some(fields) = record.as_object_mut() else {
        return;
    };
    fields.remove("span");
    fields.remove("link_span");
}
