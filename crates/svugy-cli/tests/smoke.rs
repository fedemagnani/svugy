//! Prove each svugy-cli dependency covers its intended use.

use std::process::Command;

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("unknown element `db.primry`")]
#[diagnostic(code(svugy::dangling_reference), help("did you mean `db.primary`?"))]
struct DanglingReference {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("referenced here")]
    reference_span: SourceSpan,
}

#[test]
fn clap_parses_the_cli() {
    let svugy_binary = env!("CARGO_BIN_EXE_svugy");

    let version_output = Command::new(svugy_binary)
        .arg("--version")
        .output()
        .expect("binary runs");
    let unknown_flag_output = Command::new(svugy_binary)
        .arg("--no-such-flag")
        .output()
        .expect("binary runs");

    let version_line = String::from_utf8(version_output.stdout).expect("utf-8 stdout");
    let unknown_flag_exit_code = unknown_flag_output.status.code();
    let package_version = env!("CARGO_PKG_VERSION");
    let expected_version_line = format!("svugy {package_version}");
    assert_eq!(version_line.trim(), expected_version_line);
    assert_eq!(unknown_flag_exit_code, Some(2));
}

#[test]
fn miette_fancy_renders_a_source_annotated_report() {
    let source_text = "a -> db.primry".to_owned();
    let source_code = NamedSource::new("model.d2", source_text);
    let diagnostic = DanglingReference {
        source_code,
        reference_span: SourceSpan::from(5..14),
    };
    let plain_theme = GraphicalTheme::unicode_nocolor();
    let handler = GraphicalReportHandler::new_themed(plain_theme);

    let mut rendered = String::new();
    handler
        .render_report(&mut rendered, &diagnostic)
        .expect("renders");

    assert!(rendered.contains("model.d2:1:6"), "{rendered}");
    assert!(rendered.contains("referenced here"));
    assert!(rendered.contains("did you mean `db.primary`?"));
}
