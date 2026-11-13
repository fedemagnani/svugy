//! Prove `svugy render` writes the interactive or static SVG in either theme, or a workflow's PNG per layer, from JSON and d2 input, on the chosen workflow or each in turn, and exits with the documented codes.

mod support;

use std::fs;
use std::process::Output;

use support::{fixture, run_svugy, scratch_file, stderr_text};

const SVUGY_EXAMPLE_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/svugy/svugy.json"
);

const SCRIPT_OPENING: &str = "<script>";

const HACHURE_ATTRIBUTE: &str = " data-svugy-hachure=";

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

fn stdout_text(run: &Output) -> String {
    let stdout = run.stdout.clone();
    String::from_utf8(stdout).expect("utf-8 stdout")
}

#[test]
fn render_writes_the_interactive_svg_unless_the_static_profile_is_asked_for() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();

    let default_run = run_svugy(&["render", &model_argument]);
    let interactive_run = run_svugy(&["render", &model_argument, "--profile", "interactive"]);
    let static_run = run_svugy(&[
        "render",
        &model_argument,
        "--profile",
        "static",
        "--workflow",
        "main",
    ]);

    let default_svg = stdout_text(&default_run);
    let static_svg = stdout_text(&static_run);
    assert!(default_run.status.success());
    assert!(static_run.status.success());
    assert_eq!(default_run.stdout, interactive_run.stdout);
    assert!(
        default_svg.contains(SCRIPT_OPENING),
        "interactive carries the runtime"
    );
    assert!(
        !static_svg.contains(SCRIPT_OPENING),
        "static carries no script"
    );
    assert!(static_svg.starts_with("<svg "));
}

#[test]
fn render_draws_either_profile_hand_drawn_only_under_the_sketch_theme() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();

    let default_run = run_svugy(&["render", &model_argument]);
    let sketch_interactive_run = run_svugy(&["render", &model_argument, "--theme", "sketch"]);
    let sketch_static_run = run_svugy(&[
        "render",
        &model_argument,
        "--theme",
        "sketch",
        "--profile",
        "static",
        "--workflow",
        "main",
    ]);

    let default_svg = stdout_text(&default_run);
    let sketch_interactive_svg = stdout_text(&sketch_interactive_run);
    let sketch_static_svg = stdout_text(&sketch_static_run);
    assert!(
        !default_svg.contains(HACHURE_ATTRIBUTE),
        "the standard theme is the default and hatches no face"
    );
    assert!(sketch_interactive_svg.contains(HACHURE_ATTRIBUTE));
    assert!(sketch_static_svg.contains(HACHURE_ATTRIBUTE));
}

#[test]
fn render_opens_on_the_named_workflow_writes_each_one_as_stem_id_svg_and_rejects_an_unknown_id_or_stem()
 {
    let diagram_argument = fixture("two_workflows.md");
    let extensionless_output_argument = scratch_file("each_workflow");
    let order_svg_path = scratch_file("each_workflow.order.svg");
    let refund_svg_path = scratch_file("each_workflow.refund.svg");
    let stemless_output_argument = scratch_file(".svg");
    let directory_output_argument = scratch_file("some_directory/");

    let refund_run = run_svugy(&["render", &diagram_argument, "--workflow", "refund"]);
    let static_without_workflow_run =
        run_svugy(&["render", &diagram_argument, "--profile", "static"]);
    let each_static_run = run_svugy(&[
        "render",
        &diagram_argument,
        "--profile",
        "static",
        "--all-workflows",
        "-o",
        &extensionless_output_argument,
    ]);
    let unknown_workflow_run = run_svugy(&["render", &diagram_argument, "--workflow", "audit"]);
    let stemless_output_run = run_svugy(&[
        "render",
        &diagram_argument,
        "--all-workflows",
        "-o",
        &stemless_output_argument,
    ]);
    let directory_output_run = run_svugy(&[
        "render",
        &diagram_argument,
        "--all-workflows",
        "-o",
        &directory_output_argument,
    ]);

    let refund_svg = stdout_text(&refund_run);
    let order_svg = fs::read_to_string(&order_svg_path).expect("render wrote the order workflow");
    let refund_static_svg =
        fs::read_to_string(&refund_svg_path).expect("render wrote the refund workflow");
    let unknown_workflow_report = stderr_text(&unknown_workflow_run);
    let order_edge = r#"data-svugy-edge="(web -&gt; api)[0]""#;
    let refund_edge = r#"data-svugy-edge="(api -&gt; web)[0]""#;
    assert!(refund_svg.contains(r#"<g data-svugy-shown="" data-svugy-workflow="refund">"#));
    assert_eq!(static_without_workflow_run.status.code(), Some(2));
    assert!(each_static_run.status.success());
    assert!(order_svg.contains(order_edge) && !order_svg.contains(refund_edge));
    assert!(refund_static_svg.contains(refund_edge) && !refund_static_svg.contains(order_edge));
    assert_eq!(unknown_workflow_run.status.code(), Some(2));
    assert!(
        unknown_workflow_report.contains("order, refund"),
        "an unknown id lists the input's workflows: {unknown_workflow_report}"
    );
    assert_eq!(stemless_output_run.status.code(), Some(2));
    assert_eq!(directory_output_run.status.code(), Some(2));
}

#[test]
fn an_unknown_profile_is_a_usage_error_naming_the_accepted_ones() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();

    let run = run_svugy(&["render", &model_argument, "--profile", "print"]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(2));
    assert!(report.contains("interactive"), "{report}");
    assert!(report.contains("static"), "{report}");
}

#[test]
fn render_writes_the_same_svg_to_stdout_and_to_the_output_file() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();
    let svg_argument = scratch_file("svugy.svg");

    let stdout_run = run_svugy(&["render", &model_argument]);
    let file_run = run_svugy(&["render", &model_argument, "-o", &svg_argument]);

    let written_svg = fs::read(&svg_argument).expect("render wrote the output file");
    assert!(stdout_run.status.success());
    assert!(file_run.status.success());
    assert!(stdout_run.stdout.starts_with(b"<svg "));
    assert!(file_run.stdout.is_empty());
    assert_eq!(written_svg, stdout_run.stdout);
}

#[test]
fn all_depths_writes_one_png_per_layer_the_deepest_being_the_whole_png() {
    let d2_argument = fixture("nesting.d2");
    let whole_png_argument = scratch_file("nesting.png");
    let layers_png_argument = scratch_file("nesting-layers.png");

    let whole_run = run_svugy(&[
        "render",
        &d2_argument,
        "--png",
        "--workflow",
        "main",
        "-o",
        &whole_png_argument,
    ]);
    let layers_run = run_svugy(&[
        "render",
        &d2_argument,
        "--png",
        "--workflow",
        "main",
        "--all-depths",
        "-o",
        &layers_png_argument,
    ]);
    let top_level_run = run_svugy(&[
        "render",
        &d2_argument,
        "--png",
        "--workflow",
        "main",
        "--depth",
        "0",
    ]);

    assert!(whole_run.status.success());
    assert!(layers_run.status.success());
    let whole_png = fs::read(&whole_png_argument).expect("--png wrote the output file");
    let top_level_png = fs::read(scratch_file("nesting-layers-depth-0.png")).expect("layer 0");
    let deepest_png = fs::read(scratch_file("nesting-layers-depth-2.png")).expect("layer 2");
    let is_past_deepest_layer_written = fs::exists(scratch_file("nesting-layers-depth-3.png"));
    assert!(whole_png.starts_with(PNG_SIGNATURE));
    assert_eq!(
        top_level_run.stdout, top_level_png,
        "--depth 0 on stdout is layer 0"
    );
    assert_eq!(
        deepest_png, whole_png,
        "the deepest layer shows the whole workflow"
    );
    assert!(!is_past_deepest_layer_written.expect("scratch directory is readable"));
}

#[test]
fn a_depth_past_the_deepest_layer_is_a_usage_error_naming_the_layers() {
    let d2_argument = fixture("nesting.d2");

    let run = run_svugy(&[
        "render",
        &d2_argument,
        "--png",
        "--workflow",
        "main",
        "--depth",
        "3",
    ]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(2));
    assert!(run.stdout.is_empty());
    assert!(report.contains("layers 0 to 2"), "{report}");
}

#[test]
fn render_accepts_a_d2_file() {
    let d2_argument = fixture("nesting.d2");

    let run = run_svugy(&["render", &d2_argument]);

    assert_eq!(run.status.code(), Some(0));
    assert!(run.stdout.starts_with(b"<svg "));
}

#[test]
fn render_prints_d2_warnings_on_stderr_and_still_renders() {
    let d2_argument = fixture("unmapped.d2");

    let run = run_svugy(&["render", &d2_argument]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(0));
    assert!(run.stdout.starts_with(b"<svg "));
    assert!(report.contains("svugy::d2::unsupported"), "{report}");
}

#[test]
fn an_invalid_model_exits_1_with_its_diagnostic_on_stderr() {
    let model_argument = fixture("dangling_edge.json");

    let run = run_svugy(&["render", &model_argument]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(1));
    assert!(run.stdout.is_empty());
    assert!(report.contains("svugy::dangling_reference"), "{report}");
}

#[test]
fn a_d2_syntax_error_exits_1_with_a_source_snippet_on_stderr() {
    let d2_argument = fixture("syntax_error.d2");

    let run = run_svugy(&["render", &d2_argument]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(1));
    assert!(run.stdout.is_empty());
    assert!(report.contains("svugy::d2::syntax"), "{report}");
    assert!(report.contains("cache -> -> api"), "{report}");
}

#[test]
fn render_without_a_model_is_a_usage_error() {
    let run = run_svugy(&["render"]);

    assert_eq!(run.status.code(), Some(2));
}

#[test]
fn an_unknown_extension_is_a_usage_error_naming_the_accepted_ones() {
    let run = run_svugy(&["render", "model.yaml"]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(2));
    assert!(report.contains(".json"), "{report}");
    assert!(report.contains(".d2"), "{report}");
}

#[test]
fn an_unreadable_input_exits_3() {
    let missing_argument = scratch_file("missing.json");

    let run = run_svugy(&["render", &missing_argument]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(3));
    assert!(report.contains("cannot read input"), "{report}");
}

#[test]
fn an_unwritable_output_exits_3() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();
    let svg_argument = scratch_file("no_such_directory/svugy.svg");

    let run = run_svugy(&["render", &model_argument, "-o", &svg_argument]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(3));
    assert!(report.contains("cannot write"), "{report}");
}
