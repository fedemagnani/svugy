//! Prove `svugy import` turns a d2 file into a diagram and model JSON that `svugy render` draws alike.

mod support;

use std::fs;

use support::{fixture, run_svugy, scratch_file, stderr_text};

#[test]
fn a_d2_file_imported_as_a_diagram_renders_like_it_and_imports_back_to_json() {
    let d2_argument = fixture("checkout.d2");
    let diagram_argument = scratch_file("checkout.md");
    let json_argument = scratch_file("checkout_from_diagram.json");

    let d2_to_diagram_run = run_svugy(&["import", &d2_argument, "-o", &diagram_argument]);
    let diagram_to_json_run = run_svugy(&["import", &diagram_argument, "-o", &json_argument]);
    let d2_render_run = run_svugy(&["render", &d2_argument]);
    let diagram_render_run = run_svugy(&["render", &diagram_argument]);
    let json_render_run = run_svugy(&["render", &json_argument]);

    let diagram = fs::read_to_string(&diagram_argument).expect("import wrote the diagram");
    let model_json = fs::read_to_string(&json_argument).expect("import wrote the model JSON");
    assert_eq!(d2_to_diagram_run.status.code(), Some(0));
    assert_eq!(diagram_to_json_run.status.code(), Some(0));
    assert!(d2_to_diagram_run.stdout.is_empty());
    assert!(diagram.contains("## Components"), "{diagram}");
    assert!(model_json.contains("\"elements\""), "{model_json}");
    assert_eq!(diagram_render_run.status.code(), Some(0));
    assert_eq!(diagram_render_run.stdout, d2_render_run.stdout);
    assert_eq!(json_render_run.stdout, d2_render_run.stdout);
}

#[test]
fn import_prints_d2_warnings_on_stderr_and_the_model_on_stdout() {
    let d2_argument = fixture("unmapped.d2");

    let run = run_svugy(&["import", &d2_argument]);

    let report = stderr_text(&run);
    assert_eq!(run.status.code(), Some(0));
    assert!(run.stdout.starts_with(b"{"));
    assert!(report.contains("svugy::d2::unsupported"), "{report}");
}

#[test]
fn a_d2_syntax_error_exits_1_without_output() {
    let d2_argument = fixture("syntax_error.d2");

    let run = run_svugy(&["import", &d2_argument]);

    assert_eq!(run.status.code(), Some(1));
    assert!(run.stdout.is_empty());
}

#[test]
fn an_unknown_extension_is_a_usage_error() {
    let run = run_svugy(&["import", "diagram.mmd"]);

    assert_eq!(run.status.code(), Some(2));
}

#[test]
fn an_unreadable_input_exits_3() {
    let missing_argument = scratch_file("missing.d2");

    let run = run_svugy(&["import", &missing_argument]);

    assert_eq!(run.status.code(), Some(3));
}
