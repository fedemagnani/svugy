//! Guard the Markdown printer against the parser: a printed model parses back into itself, whether the Markdown parser or the d2 importer built it, and a part that would not is reported instead.

use serde_json::Value;
use svugy_model::model::Model;
use svugy_parse::d2::D2Parse;
use svugy_parse::diagnostic::SourceLanguage;
use svugy_parse::json::{JsonLanguage, JsonPrinter};
use svugy_parse::markdown::{MarkdownParse, MarkdownPrinter};

/// List every diagram that parses, by name: each fixture except errors.md, then the golden example.
const DIAGRAMS: [(&str, &str); 5] = [
    ("edges.md", include_str!("fixtures/markdown/edges.md")),
    ("nesting.md", include_str!("fixtures/markdown/nesting.md")),
    (
        "properties.md",
        include_str!("fixtures/markdown/properties.md"),
    ),
    ("styles.md", include_str!("fixtures/markdown/styles.md")),
    (
        "examples/svugy/svugy.md",
        include_str!("../../../examples/svugy/svugy.md"),
    ),
];
const SVUGY_D2: &str = include_str!("../../../examples/svugy/svugy.d2");
/// Name the span fields a reprint moves, the provenance of an element, workflow, override, step or edge.
const SPAN_FIELDS: [&str; 3] = ["span", "link_span", "code_span"];

#[test]
fn a_printed_model_parses_back_into_itself() {
    let parsed_models = DIAGRAMS.map(|(name, source)| {
        let parsed_model = parse_model(name, source);
        (name, parsed_model)
    });
    let d2_golden_model = lower_d2_golden_model();
    let imported_models = [("examples/svugy/svugy.d2", d2_golden_model)];
    for (name, model) in parsed_models.into_iter().chain(imported_models) {
        let printed = MarkdownPrinter::print(&model)
            .unwrap_or_else(|unprintable| panic!("{name} builds a printable model: {unprintable}"));
        let reprinted_name = format!("{name} printed");
        let reparsed_model = parse_model(&reprinted_name, &printed);

        let model_json = JsonPrinter::print(&model);
        let reparsed_json = JsonPrinter::print(&reparsed_model);
        let model_json_without_spans = without_spans(&model_json);
        let reparsed_json_without_spans = without_spans(&reparsed_json);
        assert_eq!(
            reparsed_json_without_spans, model_json_without_spans,
            "{name}: parsing a printed model must give back that model, spans aside; printed:\n{printed}"
        );
    }
}

#[test]
fn the_d2_golden_model_prints_into_a_snapshot() {
    let golden_model = lower_d2_golden_model();

    let printed = MarkdownPrinter::print(&golden_model)
        .unwrap_or_else(|unprintable| panic!("the d2 golden model is printable: {unprintable}"));
    insta::assert_snapshot!(printed);
}

#[test]
fn a_model_part_that_would_not_parse_back_as_itself_is_reported_instead_of_printed() {
    let links = r#"{
        "elements": {
            "a": { "label": "a", "link": "guide.html" },
            "b": { "label": "b", "link": "inter" },
            "c": { "label": "c", "link": "\"b\"" },
            "p": { "label": "p" },
            "p.inter": { "label": "p.inter" }
        },
        "workflows": [
            {
                "id": "main",
                "title": "Links",
                "steps": [
                    {
                        "id": "all",
                        "title": "Every element",
                        "items": [{ "element": "a" }, { "element": "b" }, { "element": "c" }, { "element": "p.inter" }]
                    }
                ]
            }
        ]
    }"#;
    let workflow = r##"{
        "elements": { "a": { "label": "a" }, "b": { "label": "b" } },
        "workflows": [
            {
                "id": "main",
                "title": "ends with #",
                "description": "Two cases:\n- one\n- two",
                "edges": [{ "from": "a", "to": "b", "ordinal": 3 }],
                "steps": [
                    {
                        "id": "join",
                        "title": "A joins B",
                        "description": "Over [the only edge](#a->b).",
                        "items": [{ "edge": { "from": "a", "to": "b", "ordinal": 3 } }]
                    }
                ]
            }
        ]
    }"##;
    let cases = [
        (
            links,
            "Markdown cannot say 3 part(s) of this model:
- element `a`: link `guide.html` holds no `:`, `/`, `?` or `#`, so the language would read it as a component reference; write it as `./guide.html`
- element `b`: link `inter` holds no `:`, `/`, `?` or `#`, so the language would read it as a component reference; write it as `./inter`
- element `c`: link `\"b\"` spells element `b` another way, and the language keeps only the path; write it as `b`",
        ),
        (
            workflow,
            "Markdown cannot say 2 part(s) of this model:
- workflow `main`: its prose does not read back verbatim under a heading: outside a code fence it holds no `- ` bullet list, `#` heading or comment, and no blank line opens or closes it
- edge `(a -> b)[3]` of workflow `main`: its ordinal is 3, but the language numbers repeated edges in the order their steps declare them, so it would read back as 0",
        ),
    ];

    for (json, expected_report) in cases {
        let report =
            print_json_model(json).expect_err("the model holds parts the language cannot say");
        assert_eq!(
            report, expected_report,
            "the printer must report every part that would parse back as something else, never print it lossy"
        );
    }
}

/// Import examples/svugy/svugy.d2, panicking unless it lowers into a sound model.
fn lower_d2_golden_model() -> Model {
    let parsed = D2Parse::parse(SVUGY_D2);
    let import = parsed.lower();
    import
        .admission
        .model
        .expect("examples/svugy/svugy.d2 lowers into a sound model")
}

/// Parse `source` as Markdown and lower it, panicking with `name` unless it lowers into a sound model.
fn parse_model(name: &str, source: &str) -> Model {
    let parsed = MarkdownParse::parse(source);
    let import = parsed.lower();
    let admission = import.admission;
    admission
        .model
        .unwrap_or_else(|| panic!("{name} lowers into a sound model:\n{source}"))
}

/// Load a JSON model and print it as Markdown, returning the unprintable report as text.
fn print_json_model(json: &str) -> Result<String, String> {
    let import = JsonLanguage::import(json);
    let model = import
        .admitted_model()
        .expect("well-formed JSON of a model with no error");
    let printed = MarkdownPrinter::print(model);
    printed.map_err(|unprintable| unprintable.to_string())
}

/// Re-print model JSON without any span field, the provenance a reprint moves.
fn without_spans(model_json: &str) -> String {
    let mut model: Value = serde_json::from_str(model_json).expect("a model saves as JSON");
    remove_spans(&mut model);
    serde_json::to_string_pretty(&model).expect("a JSON value prints")
}

/// Remove every span field from `value` and from each value nested in it.
fn remove_spans(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for span_field in SPAN_FIELDS {
                fields.remove(span_field);
            }
            for field in fields.values_mut() {
                remove_spans(field);
            }
        }
        Value::Array(items) => {
            for item in items {
                remove_spans(item);
            }
        }
        _scalar => {}
    }
}
