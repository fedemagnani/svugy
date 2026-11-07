//! Prove the d2 front end parses the v1 subset, recovers from broken lines and warns on unsupported constructs.

use miette::{Diagnostic, Severity, SourceSpan};
use svugy_import::d2::{Construct, D2Parse, Field, KeyPath, Span, Statement, SyntaxError};

const NESTING: &str = include_str!("fixtures/d2/nesting.d2");
const EDGES: &str = include_str!("fixtures/d2/edges.d2");
const STYLES: &str = include_str!("fixtures/d2/styles.d2");

#[test]
fn nesting_and_quoted_keys_parse_into_a_snapshot() {
    let parsed = D2Parse::parse(NESTING);

    insta::assert_debug_snapshot!(parsed);
}

#[test]
fn edge_chains_and_labels_parse_into_a_snapshot() {
    let parsed = D2Parse::parse(EDGES);

    insta::assert_debug_snapshot!(parsed);
}

#[test]
fn styles_dotted_and_nested_parse_into_a_snapshot() {
    let parsed = D2Parse::parse(STYLES);

    insta::assert_debug_snapshot!(parsed);
}

#[test]
fn every_broken_line_is_reported_and_valid_lines_are_kept() {
    let source =
        "a -> b\nc ->\nd: {\n  e -> -> f\n  g\n}\nh: \"open\ni -> j\n}\nk: { l\nm.n: kept\n";

    let parsed = D2Parse::parse(source);

    let error_texts: Vec<&str> = parsed
        .errors()
        .iter()
        .map(|error| text_at(source, error.span))
        .collect();
    let top_level_paths: Vec<String> = parsed
        .document()
        .statements
        .iter()
        .map(statement_paths)
        .collect();
    let Statement::Shape(container_d) = &parsed.document().statements[1] else {
        panic!("`d` is kept as a shape");
    };
    let container_d_children: Vec<String> = container_d.body.iter().map(statement_paths).collect();
    assert_eq!(error_texts, ["\n", "->", "\"open", "}", ""]);
    assert_eq!(top_level_paths, ["a -> b", "d", "h", "i -> j", "m.n"]);
    assert_eq!(container_d_children, ["g"]);
}

#[test]
fn errors_and_warnings_are_distinguishable_diagnostics() {
    let parsed = D2Parse::parse("a ->\nb.icon: x.svg\n");

    let error_severity = parsed.errors()[0].severity();
    let warning_severity = parsed.warnings()[0].severity();
    let error_code = parsed.errors()[0].code().map(|code| code.to_string());
    let warning_code = parsed.warnings()[0].code().map(|code| code.to_string());
    assert_eq!(error_severity, Some(Severity::Error));
    assert_eq!(warning_severity, Some(Severity::Warning));
    assert_eq!(error_code.as_deref(), Some("svugy::d2::syntax"));
    assert_eq!(warning_code.as_deref(), Some("svugy::d2::unsupported"));
}

#[test]
fn unsupported_constructs_warn_and_the_rest_still_parses() {
    let source = r#"vars: {
  d2-config: { layout-engine: elk }
}
classes: { big: { style.font-size: 30 } }
a.class: big
layers: { detail: { x } }
...@base
z: @other.d2
*.style.fill: red
(a -> b)[0].style.stroke: blue
b.icon: https://icons.example/b.svg
b.near: top-center
t: { shape: sql_table }
md: |md
  # Title, it's markdown
|
code: ||go
  a | b
||
lbl: ${name}
gone: null
a.label.near: top-left
kept -> edge: ok
"#;

    let parsed = D2Parse::parse(source);

    let warnings: Vec<(Construct, &str)> = parsed
        .warnings()
        .iter()
        .map(|warning| {
            let text = text_at(source, warning.span);
            (warning.construct.clone(), text)
        })
        .collect();
    let kept_paths: Vec<String> = parsed
        .document()
        .statements
        .iter()
        .map(statement_paths)
        .collect();
    assert_eq!(parsed.errors(), Vec::<SyntaxError>::new());
    assert_eq!(
        warnings,
        [
            (Construct::Variables, "d2-config: { layout-engine: elk }"),
            (
                Construct::Classes,
                "classes: { big: { style.font-size: 30 } }"
            ),
            (Construct::Classes, "a.class: big"),
            (Construct::Board("layers"), "layers: { detail: { x } }"),
            (Construct::Import, "...@base"),
            (Construct::Import, "@other.d2"),
            (Construct::Glob, "*.style.fill"),
            (Construct::EdgeReference, "(a -> b)[0].style.stroke: blue"),
            (
                Construct::ReservedField("icon"),
                "b.icon: https://icons.example/b.svg"
            ),
            (Construct::ReservedField("near"), "b.near: top-center"),
            (Construct::BlockString, "|md\n  # Title, it's markdown\n|"),
            (Construct::BlockString, "||go\n  a | b\n||"),
            (Construct::Variables, "${name}"),
            (Construct::Null, "gone: null"),
            (
                Construct::FieldForm("label.near".to_owned()),
                "a.label.near: top-left"
            ),
        ]
    );
    assert_eq!(kept_paths, ["z", "t", "md", "code", "lbl", "kept -> edge"]);
}

#[test]
fn a_map_beside_a_style_value_warns_and_the_value_is_kept() {
    let source = "a.style.fill: red { opacity: 0.5 }\nb.style: { stroke: blue { width: 2 } }\n";

    let parsed = D2Parse::parse(source);

    let warnings: Vec<(Construct, &str)> = parsed
        .warnings()
        .iter()
        .map(|warning| {
            let text = text_at(source, warning.span);
            (warning.construct.clone(), text)
        })
        .collect();
    let kept_values: Vec<&str> = parsed
        .document()
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Field(assignment) => Some(&assignment.field),
            _other_statement => None,
        })
        .filter_map(|field| match field {
            Field::Style(properties) => Some(properties[0].value.text.as_str()),
            _other_field => None,
        })
        .collect();
    assert_eq!(
        warnings,
        [
            (
                Construct::FieldForm("style".to_owned()),
                "a.style.fill: red { opacity: 0.5 }"
            ),
            (
                Construct::FieldForm("style".to_owned()),
                "stroke: blue { width: 2 }"
            ),
        ]
    );
    assert_eq!(kept_values, ["red", "blue"]);
}

#[test]
fn unknown_style_keys_warn_and_are_dropped_in_both_style_forms() {
    let source = "a.style: { fill: red; glow: 2 }\nb.style.Fill: blue\n";

    let parsed = D2Parse::parse(source);

    let warnings: Vec<(Construct, &str)> = parsed
        .warnings()
        .iter()
        .map(|warning| {
            let text = text_at(source, warning.span);
            (warning.construct.clone(), text)
        })
        .collect();
    let kept_paths: Vec<String> = parsed
        .document()
        .statements
        .iter()
        .map(statement_paths)
        .collect();
    let Statement::Field(assignment) = &parsed.document().statements[0] else {
        panic!("`a.style` is a field");
    };
    let Field::Style(properties) = &assignment.field else {
        panic!("`a.style` is a style field");
    };
    let kept_style_keys: Vec<&str> = properties
        .iter()
        .map(|property| property.name.name.as_str())
        .collect();
    assert_eq!(
        warnings,
        [
            (Construct::UnknownStyleKey("glow".to_owned()), "glow: 2"),
            (
                Construct::UnknownStyleKey("Fill".to_owned()),
                "b.style.Fill: blue"
            ),
        ]
    );
    assert_eq!(kept_paths, ["a"]);
    assert_eq!(kept_style_keys, ["fill"]);
}

#[test]
fn spans_point_at_the_source_bytes_of_each_node() {
    let source = "outer: {\n  \"a.b\" -> c: calls {style.stroke: red}\n}\n";

    let parsed = D2Parse::parse(source);

    let Statement::Shape(outer) = &parsed.document().statements[0] else {
        panic!("`outer` is a shape");
    };
    let Statement::Edge(edge) = &outer.body[0] else {
        panic!("the nested statement is an edge");
    };
    let Some(label) = &edge.label else {
        panic!("the edge has a label");
    };
    let Field::Style(properties) = &edge.fields[0] else {
        panic!("the edge map holds a style field");
    };
    let style_property = &properties[0];
    assert_eq!(slice(source, outer.span), source.trim_end());
    assert_eq!(slice(source, outer.path.span), "outer");
    assert_eq!(
        slice(source, edge.span),
        "\"a.b\" -> c: calls {style.stroke: red}"
    );
    assert_eq!(slice(source, edge.source.span), "\"a.b\"");
    assert_eq!(edge.source.segments[0].name, "a.b");
    assert_eq!(slice(source, edge.links[0].operator_span), "->");
    assert_eq!(slice(source, edge.links[0].target.span), "c");
    assert_eq!(slice(source, label.span), "calls");
    assert_eq!(slice(source, style_property.span), "style.stroke: red");
    assert_eq!(slice(source, style_property.name.span), "stroke");
    assert_eq!(slice(source, style_property.value.span), "red");
}

#[test]
fn comments_separators_and_line_endings_are_skipped() {
    let source =
        "# heading\r\na; b -> c # trailing\r\n\"\"\"\nblock comment\n\"\"\"\nlong \\\n -> d\n";

    let parsed = D2Parse::parse(source);

    let paths: Vec<String> = parsed
        .document()
        .statements
        .iter()
        .map(statement_paths)
        .collect();
    assert_eq!(parsed.errors(), Vec::<SyntaxError>::new());
    assert_eq!(paths, ["a", "b -> c", "long -> d"]);
}

fn statement_paths(statement: &Statement) -> String {
    match statement {
        Statement::Shape(shape) => dotted(&shape.path),
        Statement::Edge(edge) => {
            let source_path = dotted(&edge.source);
            let target_paths = edge.links.iter().map(|link| dotted(&link.target));
            let endpoints: Vec<String> = std::iter::once(source_path).chain(target_paths).collect();
            endpoints.join(" -> ")
        }
        Statement::Field(assignment) => dotted(&assignment.target),
    }
}

fn dotted(path: &KeyPath) -> String {
    let names: Vec<&str> = path
        .segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect();
    names.join(".")
}

fn text_at(source: &str, span: SourceSpan) -> &str {
    let start = span.offset();
    let end = start + span.len();
    &source[start..end]
}

fn slice(source: &str, span: Span) -> &str {
    &source[span.range()]
}
