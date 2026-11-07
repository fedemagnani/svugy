//! Guard the printer against the parser: a printed diagram parses back into the model it was printed from.

use serde_json::Value;
use svugy_import::d2::D2Parse;
use svugy_import::svugy::SvugyParse;
use svugy_model::model::Model;

/// List every diagram that parses, by name: each fixture except errors.md, then the golden example.
const DIAGRAMS: [(&str, &str); 6] = [
    ("edges.md", include_str!("fixtures/svugy/edges.md")),
    ("legend.md", include_str!("fixtures/svugy/legend.md")),
    ("nesting.md", include_str!("fixtures/svugy/nesting.md")),
    (
        "properties.md",
        include_str!("fixtures/svugy/properties.md"),
    ),
    ("styles.md", include_str!("fixtures/svugy/styles.md")),
    (
        "examples/svugy.md",
        include_str!("../../../examples/svugy.md"),
    ),
];
const SVUGY_D2: &str = include_str!("../../../examples/svugy.d2");

#[test]
fn a_printed_diagram_parses_back_into_the_model_it_was_printed_from() {
    for (name, source) in DIAGRAMS {
        let parsed_model = parse_model(name, source);
        let printed = parsed_model
            .to_svugy_markdown()
            .unwrap_or_else(|unprintable| {
                panic!("{name} parses into a printable model: {unprintable}")
            });
        let reprinted_name = format!("{name} printed");
        let reparsed_model = parse_model(&reprinted_name, &printed);

        let parsed_json = parsed_model.to_json();
        let reparsed_json = reparsed_model.to_json();
        let parsed_model_json = without_spans(&parsed_json);
        let reparsed_model_json = without_spans(&reparsed_json);
        assert_eq!(
            reparsed_model_json, parsed_model_json,
            "{name}: parsing a printed model must give back that model, spans aside; printed:\n{printed}"
        );
    }
}

#[test]
fn the_d2_golden_model_prints_into_a_snapshot() {
    let parsed = D2Parse::parse(SVUGY_D2);
    let import = parsed.lower();
    let golden_model = import
        .admission
        .model
        .expect("examples/svugy.d2 lowers into a sound model");

    let printed = golden_model
        .to_svugy_markdown()
        .unwrap_or_else(|unprintable| panic!("the d2 golden model is printable: {unprintable}"));
    insta::assert_snapshot!(printed);
}

/// Parse `source` with the svugy parser and lower it, panicking with `name` unless it lowers into a sound model.
fn parse_model(name: &str, source: &str) -> Model {
    let parsed = SvugyParse::parse(source);
    let import = parsed.lower();
    let admission = import.admission;
    admission
        .model
        .unwrap_or_else(|| panic!("{name} lowers into a sound model:\n{source}"))
}

/// Re-print model JSON without the `span` and `link_span` of each element and edge, the provenance a reprint moves.
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
