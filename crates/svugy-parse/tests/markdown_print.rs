//! Guard the Markdown printer against the parser: a printed model parses back into itself, whether the Markdown parser or the d2 importer built it, and a part that would not is reported instead.

use serde_json::Value;
use svugy_model::model::Model;
use svugy_parse::d2::D2Parse;
use svugy_parse::diagnostic::SourceLanguage;
use svugy_parse::json::{JsonLanguage, JsonPrinter};
use svugy_parse::markdown::{MarkdownParse, MarkdownPrinter};

/// List every diagram that parses, by name: each fixture except errors.md, then the golden example.
const DIAGRAMS: [(&str, &str); 6] = [
    ("edges.md", include_str!("fixtures/markdown/edges.md")),
    ("legend.md", include_str!("fixtures/markdown/legend.md")),
    ("nesting.md", include_str!("fixtures/markdown/nesting.md")),
    (
        "properties.md",
        include_str!("fixtures/markdown/properties.md"),
    ),
    ("styles.md", include_str!("fixtures/markdown/styles.md")),
    (
        "examples/svugy.md",
        include_str!("../../../examples/svugy.md"),
    ),
];
const SVUGY_D2: &str = include_str!("../../../examples/svugy.d2");

#[test]
fn a_printed_model_parses_back_into_itself() {
    let parsed_models = DIAGRAMS.map(|(name, source)| {
        let parsed_model = parse_model(name, source);
        (name, parsed_model)
    });
    let d2_golden_model = lower_d2_golden_model();
    let imported_models = [("examples/svugy.d2", d2_golden_model)];
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
        }
    }"#;
    let edges = r#"{
        "elements": { "a": { "label": "a" }, "p": { "label": "p" }, "p.c": { "label": "c" } },
        "edges": [
            { "from": "a", "to": "a" },
            { "from": "a", "to": "p", "ordinal": 3 },
            { "from": "p", "to": "p.c" }
        ]
    }"#;
    let legend = r##"{
        "legend": {
            "title": "keys #",
            "entries": ["1. first", "# h", "> q", "---", "<!-- c -->", "", " (line=dashed): x", "(line=dashed): x", "(note) fine"]
        },
        "elements": { "a": { "label": "a" } }
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
            edges,
            "Markdown cannot say 3 part(s) of this model:
- edge `(a -> a)[0]`: it joins an element to itself, which the language rejects
- edge `(a -> p)[3]`: its ordinal is 3, but the language numbers repeated edges in the order written, so it would read back as 0
- edge `(p -> p.c)[0]`: it joins an element to its own ancestor or descendant, which the language rejects",
        ),
        (
            legend,
            "Markdown cannot say 9 part(s) of this model:
- the legend: its title ends with `#`, which Markdown reads as the heading's closing `#`s
- the legend: entry `1. first` opens like a Markdown heading, quote, list, rule, code fence, HTML block or link definition, so it would not read back as text
- the legend: entry `# h` opens like a Markdown heading, quote, list, rule, code fence, HTML block or link definition, so it would not read back as text
- the legend: entry `> q` opens like a Markdown heading, quote, list, rule, code fence, HTML block or link definition, so it would not read back as text
- the legend: entry `---` opens like a Markdown heading, quote, list, rule, code fence, HTML block or link definition, so it would not read back as text
- the legend: its entry \"<!-- c -->\" holds `<!--`, which the language reads as a comment and drops
- the legend: it has an empty entry, which no list item can hold
- the legend: its entry \" (line=dashed): x\" starts or ends with whitespace, which the language trims
- the legend: entry `(line=dashed): x` starts with `(` followed by `key=`, so it would read as a key row",
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

/// Import examples/svugy.d2, panicking unless it lowers into a sound model.
fn lower_d2_golden_model() -> Model {
    let parsed = D2Parse::parse(SVUGY_D2);
    let import = parsed.lower();
    import
        .admission
        .model
        .expect("examples/svugy.d2 lowers into a sound model")
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
