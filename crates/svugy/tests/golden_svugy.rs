//! Snapshot the svugy golden as JSON, as static and interactive SVG text and as its static raster.

mod support;

#[path = "golden/svugy.rs"]
mod svugy_example;

use support::pixel_snapshot_test;
use svugy::{InteractiveSvg, StaticSvg};
use svugy_example::SvugyExample;

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
    let saved = model.to_json();
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

pixel_snapshot_test!(svugy_example_static_raster, render_svugy_example());

fn render_svugy_example() -> String {
    let model = SvugyExample::model();
    StaticSvg::render(&model)
}
