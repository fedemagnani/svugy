//! Prove examples/svugy.d2 imports into the model the golden builder saves as examples/svugy.json.

use serde_json::Value;
use svugy_import::d2::{D2Document, GRAMMAR_VERSION};

const SVUGY_D2: &str = include_str!("../../../examples/svugy.d2");
const SVUGY_JSON: &str = include_str!("../../../examples/svugy.json");

#[test]
fn svugy_d2_imports_into_the_golden_model() {
    let parsed = D2Document::parse(SVUGY_D2);
    let import = parsed.lower();

    let diagnostic_count = import.diagnostics().len();
    let model = import
        .admission
        .model
        .expect("the twin lowers into a sound model");
    let imported_json = model.to_json();
    let imported_model_json = without_spans(&imported_json);
    let golden_model_json = without_spans(SVUGY_JSON);
    assert_eq!(diagnostic_count, 0);
    assert_eq!(imported_model_json, golden_model_json);
}

#[test]
fn svugy_d2_names_the_pinned_d2_version_first() {
    let mut lines = SVUGY_D2.lines();
    let header = lines.next().unwrap_or_default();

    let pinned_prefix = format!("# d2 {GRAMMAR_VERSION} ");
    assert!(header.starts_with(&pinned_prefix), "{header}");
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
