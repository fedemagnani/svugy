//! Prove examples/svugy/svugy.d2 imports the elements examples/svugy/svugy.json saves, its edges drawn by one workflow.

use serde_json::Value;
use svugy_parse::d2::{D2Parse, GRAMMAR_VERSION};
use svugy_parse::json::JsonPrinter;

const SVUGY_D2: &str = include_str!("../../../examples/svugy/svugy.d2");
const SVUGY_JSON: &str = include_str!("../../../examples/svugy/svugy.json");

#[test]
fn svugy_d2_imports_the_golden_elements_and_one_workflow_drawing_every_edge() {
    let parsed = D2Parse::parse(SVUGY_D2);
    let import = parsed.lower();

    let diagnostic_count = import.diagnostics().len();
    let model = import
        .admission
        .model
        .expect("the twin lowers into a sound model");
    let imported_json = JsonPrinter::print(&model);
    let imported_model = without_spans(&imported_json);
    let golden_model = without_spans(SVUGY_JSON);
    assert_eq!(diagnostic_count, 0);
    assert_eq!(imported_model["elements"], golden_model["elements"]);
    let imported_workflows = &imported_model["workflows"];
    let imported_workflows_json =
        serde_json::to_string_pretty(imported_workflows).expect("a JSON value prints");
    insta::assert_snapshot!(imported_workflows_json);
}

#[test]
fn svugy_d2_names_the_pinned_d2_version_first() {
    let mut lines = SVUGY_D2.lines();
    let header = lines.next().unwrap_or_default();

    let pinned_prefix = format!("# d2 {GRAMMAR_VERSION} ");
    assert!(header.starts_with(&pinned_prefix), "{header}");
}

/// Parse model JSON without any `span` or `link_span`, the import provenance a builder never records.
fn without_spans(model_json: &str) -> Value {
    let mut model: Value = serde_json::from_str(model_json).expect("a model saves as JSON");
    remove_spans(&mut model);
    model
}

fn remove_spans(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            fields.remove("span");
            fields.remove("link_span");
            fields.values_mut().for_each(remove_spans);
        }
        Value::Array(items) => items.iter_mut().for_each(remove_spans),
        _scalar => {}
    }
}
