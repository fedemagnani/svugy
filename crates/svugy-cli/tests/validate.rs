//! Prove `svugy validate` reports diagnostics in human and JSON form and exits 1 only on errors.

mod support;

use std::fs;

use serde_json::Value;
use support::{fixture, run_svugy, scratch_file, stderr_text};

const SVUGY_EXAMPLE_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/svugy/svugy.json"
);
const WEBAPP_EXAMPLE_MD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/webapp/webapp.md"
);

fn parse_json_report(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("validate --format json prints JSON")
}

fn related_codes(json_report: &Value) -> Vec<&str> {
    let related = json_report["related"]
        .as_array()
        .expect("the report lists its diagnostics");
    related
        .iter()
        .map(|diagnostic| {
            diagnostic["code"]
                .as_str()
                .expect("every diagnostic has a code")
        })
        .collect()
}

#[test]
fn a_clean_model_exits_0_and_prints_nothing() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();

    let run = run_svugy(&["validate", &model_argument]);

    assert_eq!(run.status.code(), Some(0));
    assert!(run.stdout.is_empty());
    assert!(run.stderr.is_empty());
}

#[test]
fn the_shipped_example_is_examples_webapp_and_validates_with_no_diagnostics() {
    let example_argument = scratch_file("webapp.md");
    let example_run = run_svugy(&["example", "-o", &example_argument]);

    let validate_run = run_svugy(&["validate", &example_argument, "--format", "json"]);

    let shipped_example = fs::read(&example_argument).expect("`svugy example -o` wrote the file");
    let workspace_example = fs::read(WEBAPP_EXAMPLE_MD).expect("examples/webapp/webapp.md exists");
    let json_report = parse_json_report(&validate_run.stdout);
    let codes = related_codes(&json_report);
    assert_eq!(example_run.status.code(), Some(0));
    assert!(
        shipped_example == workspace_example,
        "crates/svugy-cli/example/webapp.md must be a byte-identical copy of examples/webapp/webapp.md"
    );
    assert!(
        codes.is_empty(),
        "the example a new user renders first must stay clean as the language evolves: {codes:?}"
    );
}

#[test]
fn a_clean_model_prints_an_empty_json_report() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();

    let run = run_svugy(&["validate", &model_argument, "--format", "json"]);

    let json_report = parse_json_report(&run.stdout);
    let codes = related_codes(&json_report);
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(json_report["severity"], "advice");
    assert!(codes.is_empty());
}

#[test]
fn a_broken_model_exits_1_with_its_error_code_in_the_json_report() {
    let model_argument = fixture("dangling_edge.json");

    let run = run_svugy(&["validate", &model_argument, "--format", "json"]);

    let json_report = parse_json_report(&run.stdout);
    let codes = related_codes(&json_report);
    let first_diagnostic = &json_report["related"][0];
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(json_report["severity"], "error");
    assert_eq!(codes, ["svugy::dangling_reference"]);
    assert_eq!(first_diagnostic["severity"], "error");
    assert_eq!(first_diagnostic["help"], Value::Null);
}

#[test]
fn only_validate_reads_the_tree_and_fails_on_each_code_reference_it_no_longer_backs() {
    let diagram_argument = fixture("code_references.md");
    let source = fs::read_to_string(&diagram_argument).expect("the fixture reads");
    let svg_argument = scratch_file("code_references.svg");

    let validate_run = run_svugy(&["validate", &diagram_argument, "--format", "json"]);
    let render_run = run_svugy(&["render", &diagram_argument, "-o", &svg_argument]);

    let json_report = parse_json_report(&validate_run.stdout);
    let codes = related_codes(&json_report);
    let missing_file_message = json_report["related"][0]["message"]
        .as_str()
        .unwrap_or_default();
    let unspelled_symbol_message = json_report["related"][1]["message"]
        .as_str()
        .unwrap_or_default();
    let missing_file_label_span = &json_report["related"][0]["labels"][0]["span"];
    let missing_file_value = "crates/svugy-cli/src/removed.rs";
    let missing_file_offset = source
        .find(missing_file_value)
        .expect("the fixture names the removed file");
    assert_eq!(validate_run.status.code(), Some(1));
    assert_eq!(
        codes,
        ["svugy::stale_code_reference", "svugy::stale_code_reference"]
    );
    assert!(
        missing_file_message.contains("`gone`"),
        "{missing_file_message}"
    );
    assert!(
        missing_file_message.contains("is not a readable file"),
        "{missing_file_message}"
    );
    assert!(
        unspelled_symbol_message.contains("`renamed`"),
        "{unspelled_symbol_message}"
    );
    assert!(
        unspelled_symbol_message.contains("does not spell"),
        "{unspelled_symbol_message}"
    );
    assert_eq!(missing_file_label_span["offset"], missing_file_offset);
    assert_eq!(missing_file_label_span["length"], missing_file_value.len());
    assert_eq!(
        render_run.status.code(),
        Some(0),
        "render must never read the tree: {}",
        stderr_text(&render_run)
    );
}

#[test]
fn a_d2_syntax_error_reports_its_byte_offset_line_column_and_source_name() {
    let d2_argument = fixture("syntax_error.d2");

    let run = run_svugy(&["validate", &d2_argument, "--format", "json"]);

    let json_report = parse_json_report(&run.stdout);
    let codes = related_codes(&json_report);
    let first_diagnostic = &json_report["related"][0];
    let first_label = &first_diagnostic["labels"][0];
    let first_label_offset = &first_label["span"]["offset"];
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(codes, ["svugy::d2::syntax", "svugy::d2::syntax"]);
    assert_eq!(first_diagnostic["filename"], d2_argument.as_str());
    assert_eq!(first_label_offset, 37);
    assert_eq!(first_label["line"], 4);
    assert_eq!(first_label["column"], 3);
}

#[test]
fn warnings_only_input_exits_0_with_its_warnings_reported() {
    let model_argument = fixture("unshown_element.json");
    let d2_argument = fixture("broken_link.d2");

    let json_run = run_svugy(&["validate", &model_argument, "--format", "json"]);
    let human_run = run_svugy(&["validate", &d2_argument]);

    let json_report = parse_json_report(&json_run.stdout);
    let codes = related_codes(&json_report);
    let human_report = stderr_text(&human_run);
    assert_eq!(json_run.status.code(), Some(0));
    assert_eq!(json_report["severity"], "warning");
    assert_eq!(codes, ["svugy::unshown_element"]);
    assert_eq!(human_run.status.code(), Some(0));
    assert!(
        human_report.contains("svugy::unresolved_link"),
        "{human_report}"
    );
    assert!(human_report.contains("link: b.c"), "{human_report}");
}

#[test]
fn an_unresolved_d2_link_labels_its_link_value() {
    let d2_argument = fixture("broken_link.d2");
    let source = std::fs::read_to_string(&d2_argument).expect("the fixture reads");

    let run = run_svugy(&["validate", &d2_argument, "--format", "json"]);

    let json_report = parse_json_report(&run.stdout);
    let link_label_span = &json_report["related"][0]["labels"][0]["span"];
    let link_value_offset = source.find("b.c").expect("the fixture links to b.c");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(link_label_span["offset"], link_value_offset);
    assert_eq!(link_label_span["length"], "b.c".len());
}

#[test]
fn malformed_json_exits_1_with_its_code() {
    let model_argument = fixture("malformed.json");

    let run = run_svugy(&["validate", &model_argument, "--format", "json"]);

    let json_report = parse_json_report(&run.stdout);
    let codes = related_codes(&json_report);
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(codes, ["svugy::malformed_json"]);
}

#[test]
fn an_unknown_format_is_a_usage_error() {
    let model_argument = SVUGY_EXAMPLE_JSON.to_owned();

    let run = run_svugy(&["validate", &model_argument, "--format", "yaml"]);

    assert_eq!(run.status.code(), Some(2));
}

#[test]
fn an_unreadable_input_exits_3() {
    let missing_argument = scratch_file("missing.d2");

    let run = run_svugy(&["validate", &missing_argument, "--format", "json"]);

    assert_eq!(run.status.code(), Some(3));
    assert!(run.stdout.is_empty());
}
