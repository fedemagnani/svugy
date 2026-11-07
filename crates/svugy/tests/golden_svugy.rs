//! Snapshot the svugy golden as JSON, as static and interactive SVG text and as its static raster, and guard its twins.

mod support;

#[path = "golden/svugy.rs"]
mod svugy_example;

use serde_json::Value;
use support::pixel_snapshot_test;
use svugy::{InteractiveSvg, StaticSvg};
use svugy_example::SvugyExample;
use svugy_parse::d2::D2Parse;
use svugy_parse::json::JsonPrinter;
use svugy_parse::markdown::MarkdownParse;

const SVUGY_MD: &str = include_str!("../../../examples/svugy.md");
const SVUGY_D2: &str = include_str!("../../../examples/svugy.d2");

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
fn svugy_example_model_saves_as_stable_json() {
    let model = SvugyExample::model();
    let saved = JsonPrinter::print(&model);
    insta::assert_snapshot!(saved);
}

#[test]
fn svugy_example_renders_as_stable_static_svg() {
    let static_svg = render_svugy_example();
    insta::assert_snapshot!(static_svg);
}

#[test]
fn svugy_example_renders_as_stable_interactive_svg() {
    let model = SvugyExample::model();
    let interactive_svg = InteractiveSvg::render(&model);
    insta::assert_snapshot!(interactive_svg);
}

#[test]
fn svugy_example_renders_the_same_interactive_bytes_twice() {
    let model = SvugyExample::model();
    let first_render = InteractiveSvg::render(&model);
    let second_render = InteractiveSvg::render(&model);
    assert!(first_render == second_render);
}

#[test]
fn svugy_example_interactive_svg_parses_as_xml() {
    let model = SvugyExample::model();
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
    let builder_model = SvugyExample::model();
    let builder_json = JsonPrinter::print(&builder_model);
    let builder_json_without_spans = without_spans(&builder_json);

    let markdown_parse = MarkdownParse::parse(SVUGY_MD);
    let markdown_import = markdown_parse.lower();
    let markdown_model = markdown_import
        .admitted_model()
        .expect("examples/svugy.md lowers into a sound model");
    let markdown_json = JsonPrinter::print(markdown_model);
    let markdown_json_without_spans = without_spans(&markdown_json);

    let d2_parse = D2Parse::parse(SVUGY_D2);
    let d2_import = d2_parse.lower();
    let d2_model = d2_import
        .admitted_model()
        .expect("examples/svugy.d2 lowers into a sound model");
    let d2_json = JsonPrinter::print(d2_model);
    let d2_json_without_spans = without_spans(&d2_json);

    let invariant = "examples/svugy.md, examples/svugy.d2 and golden/svugy.rs must describe the same model, spans aside";
    assert_eq!(
        markdown_json_without_spans, builder_json_without_spans,
        "{invariant}: examples/svugy.md differs"
    );
    assert_eq!(
        d2_json_without_spans, builder_json_without_spans,
        "{invariant}: examples/svugy.d2 differs"
    );
}

pixel_snapshot_test!(svugy_example_static_raster, render_svugy_example());

fn render_svugy_example() -> String {
    let model = SvugyExample::model();
    StaticSvg::render(&model)
}

/// Re-print model JSON without the `span` and `link_span` of each element and edge, the import provenance a builder never records.
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
