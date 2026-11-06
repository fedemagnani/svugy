//! Prove `svugy render` writes the interactive or static SVG from JSON and d2 input, and exits with the documented codes.

mod support;

use std::fs;
use std::process::Output;

use support::{fixture, run_svugy, scratch_file, stderr_text};

const SCRIPT_OPENING: &str = "<script>";

fn stdout_text(run: &Output) -> String {
    let stdout = run.stdout.clone();
    String::from_utf8(stdout).expect("utf-8 stdout")
}

#[test]
fn render_writes_the_interactive_svg_unless_the_static_profile_is_asked_for() {
    let model_argument = fixture("svugy.json");

    let default_run = run_svugy(&["render", &model_argument]);
    let interactive_run = run_svugy(&["render", &model_argument, "--profile", "interactive"]);
    let static_run = run_svugy(&["render", &model_argument, "--profile", "static"]);

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
fn an_unknown_profile_is_a_usage_error_naming_the_accepted_ones() {
    let model_argument = fixture("svugy.json");

    let run = run_svugy(&["render", &model_argument, "--profile", "print"]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(2));
    assert!(report.contains("interactive"), "{report}");
    assert!(report.contains("static"), "{report}");
}

#[test]
fn render_writes_the_same_svg_to_stdout_and_to_the_output_file() {
    let model_argument = fixture("svugy.json");
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
    let model_argument = fixture("svugy.json");
    let svg_argument = scratch_file("no_such_directory/svugy.svg");

    let run = run_svugy(&["render", &model_argument, "-o", &svg_argument]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(3));
    assert!(report.contains("cannot write"), "{report}");
}
