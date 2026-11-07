//! Pin the Markdown fixtures: each one lowers into the model its snapshot shows, and errors.md reports exactly the errors it expects.

use std::fs;

use svugy_parse::json::JsonPrinter;
use svugy_parse::markdown::MarkdownParse;

/// Name the fixtures that lower into a model, each snapshotted under its name.
const MODEL_FIXTURES: [&str; 5] = ["nesting", "edges", "properties", "styles", "legend"];

/// Read a Markdown diagram from the crate's directory.
fn read_source(relative_path: &str) -> String {
    let manifest_directory = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest_directory}/{relative_path}");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"))
}

/// Lower `source` into its model's JSON, failing on any diagnostic: a Markdown fixture is a clean diagram.
fn lower_to_json(name: &str, source: &str) -> String {
    let parsed = MarkdownParse::parse(source);
    let import = parsed.lower();
    let diagnostics: Vec<String> = import
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect();
    assert!(diagnostics.is_empty(), "{name} reports {diagnostics:#?}");
    let model = import
        .admitted_model()
        .unwrap_or_else(|| panic!("{name} admits no model"));
    JsonPrinter::print(model)
}

#[test]
fn every_fixture_lowers_into_the_model_its_snapshot_shows() {
    for name in MODEL_FIXTURES {
        let fixture_path = format!("tests/fixtures/markdown/{name}.md");
        let source = read_source(&fixture_path);
        let model_json = lower_to_json(name, &source);
        insta::assert_snapshot!(name, model_json);
    }
    let golden_source = read_source("../../examples/svugy.md");
    let golden_json = lower_to_json("examples/svugy.md", &golden_source);
    insta::assert_snapshot!("golden", golden_json);
}

#[test]
fn errors_md_reports_each_expected_error_on_the_line_after_its_comment_and_nothing_else() {
    let source = read_source("tests/fixtures/markdown/errors.md");
    let mut expected_errors: Vec<(usize, String)> = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let trimmed_line = line.trim();
        let html_expect = trimmed_line
            .strip_prefix("<!-- expect: ")
            .and_then(|rest| rest.strip_suffix(" -->"));
        let yaml_expect = trimmed_line.strip_prefix("# expect: ");
        let Some(message) = html_expect.or(yaml_expect) else {
            continue;
        };
        let reported_line_number = line_index + 2;
        expected_errors.push((reported_line_number, message.to_owned()));
    }

    let parsed = MarkdownParse::parse(&source);
    let import = parsed.lower();
    let mut reported_errors: Vec<(usize, String)> = Vec::new();
    for diagnostic in import.diagnostics() {
        let message = diagnostic.to_string();
        let mut labels = diagnostic.labels().into_iter().flatten();
        let first_label = labels.next();
        let label_offset = first_label.map_or(usize::MAX, |label| label.offset());
        let source_before_label = source.get(..label_offset).unwrap_or(&source);
        let line_number = source_before_label.matches('\n').count() + 1;
        reported_errors.push((line_number, message));
    }

    assert_eq!(
        reported_errors, expected_errors,
        "errors.md must report each `expect` message on the line after it, and nothing else"
    );
}
