//! Snapshot the svugy golden, examples/svugy/svugy.md, as static and interactive SVG text.

use svugy::{InteractiveSvg, RenderSvg, StaticSvg};
use svugy_model::model::Model;
use svugy_parse::markdown::MarkdownParse;

const SVUGY_MD: &str = include_str!("../../../examples/svugy/svugy.md");

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

fn render_svugy_example() -> String {
    let model = svugy_example_model();
    StaticSvg::render(&model)
}

/// Parse examples/svugy/svugy.md into its admitted model.
fn svugy_example_model() -> Model {
    let markdown_parse = MarkdownParse::parse(SVUGY_MD);
    let markdown_import = markdown_parse.lower();
    let import_parts = markdown_import.into_admitted_parts();
    import_parts
        .model
        .expect("examples/svugy/svugy.md lowers into a sound model")
}
