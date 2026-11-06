//! Render d2 source into the interactive SVG inside a browser, through three plain exports and no bindings generator.

use std::cell::RefCell;

use miette::{Diagnostic, Severity};
use serde::Serialize;
use svugy::InteractiveSvg;
use svugy_import::d2::D2Document;

thread_local! {
    static PLAYGROUND: RefCell<Playground> = RefCell::new(Playground::default());
}

/// Size the input buffer to `length` bytes and return where the caller writes the d2 source.
#[unsafe(no_mangle)]
pub extern "C" fn input_buffer(length: usize) -> *mut u8 {
    PLAYGROUND.with_borrow_mut(|playground| playground.input_buffer(length))
}

/// Render the d2 source in the input buffer and return the byte length of the JSON verdict.
#[unsafe(no_mangle)]
pub extern "C" fn render() -> usize {
    PLAYGROUND.with_borrow_mut(Playground::render_input)
}

/// Return where the JSON verdict of the last render starts.
#[unsafe(no_mangle)]
pub extern "C" fn output_pointer() -> *const u8 {
    PLAYGROUND.with_borrow(|playground| playground.output.as_ptr())
}

/// Holds the d2 source a page writes in and the JSON verdict it reads back, so no pointer ever crosses the boundary inward.
#[derive(Default)]
pub struct Playground {
    input: Vec<u8>,
    output: String,
}

impl Playground {
    /// Render `source` as the JSON verdict a page reads: the interactive SVG when nothing is an error, and every diagnostic.
    pub fn verdict(source: &str) -> String {
        let import = D2Document::parse(source).lower();
        let diagnostics = import.diagnostics();
        let errors = diagnostics
            .iter()
            .filter(|diagnostic| Self::is_error(**diagnostic));
        let is_renderable = errors.count() == 0;
        let located_diagnostics: Vec<LocatedDiagnostic> = diagnostics
            .iter()
            .map(|diagnostic| LocatedDiagnostic::locate(*diagnostic, source))
            .collect();
        let admitted_model = import.admission.model.filter(|_model| is_renderable);
        let svg = admitted_model.as_ref().map(InteractiveSvg::render);
        let verdict = Verdict {
            svg,
            diagnostics: located_diagnostics,
        };
        serde_json::to_string(&verdict).expect("a verdict serialises")
    }

    fn input_buffer(&mut self, length: usize) -> *mut u8 {
        self.input.clear();
        self.input.resize(length, 0);
        self.input.as_mut_ptr()
    }

    fn render_input(&mut self) -> usize {
        let source = String::from_utf8_lossy(&self.input);
        self.output = Self::verdict(&source);
        self.output.len()
    }

    /// Tell whether `diagnostic` blocks a render: an error, or a diagnostic that states no severity.
    fn is_error(diagnostic: &dyn Diagnostic) -> bool {
        let severity = diagnostic.severity().unwrap_or(Severity::Error);
        severity == Severity::Error
    }
}

/// Carries a render's outcome to the page: the SVG, absent while any diagnostic is an error, and every diagnostic.
#[derive(Serialize)]
struct Verdict {
    svg: Option<String>,
    diagnostics: Vec<LocatedDiagnostic>,
}

/// Places one diagnostic in the editor by its first label's 1-based line and column.
#[derive(Serialize)]
struct LocatedDiagnostic {
    is_error: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}

impl LocatedDiagnostic {
    fn locate(diagnostic: &dyn Diagnostic, source: &str) -> Self {
        let is_error = Playground::is_error(diagnostic);
        let message = diagnostic.to_string();
        let help = diagnostic.help().map(|help| help.to_string());
        let mut labels = diagnostic.labels().into_iter().flatten();
        let first_label = labels.next();
        let label_offset = first_label.map(|label| label.offset());
        let line_and_column = label_offset.map(|offset| Self::line_and_column(source, offset));
        let line = line_and_column.map(|(line, _column)| line);
        let column = line_and_column.map(|(_line, column)| column);
        Self {
            is_error,
            message,
            help,
            line,
            column,
        }
    }

    /// Count lines and characters before byte `offset`: line = 1 + newlines before it, column = 1 + characters since the last newline.
    fn line_and_column(source: &str, offset: usize) -> (usize, usize) {
        let clamped_offset = offset.min(source.len());
        let before = source.get(..clamped_offset).unwrap_or(source);
        let line = 1 + before.matches('\n').count();
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        let column = 1 + before[line_start..].chars().count();
        (line, column)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn verdict_of(source: &str) -> Value {
        let verdict = Playground::verdict(source);
        serde_json::from_str(&verdict).expect("the verdict is JSON")
    }

    #[test]
    fn a_sound_diagram_renders_the_interactive_svg_with_no_diagnostics() {
        let verdict = verdict_of("a -> b: hello\n");

        let svg = verdict["svg"].as_str().expect("an SVG");
        let diagnostics = verdict["diagnostics"].as_array().expect("a list");
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("<script>"));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn an_error_withholds_the_svg_and_points_at_its_line_and_column() {
        let verdict = verdict_of("a -> b\nc: { vars: { svugy: { height: tall } } }\n");

        let diagnostics = verdict["diagnostics"].as_array().expect("a list");
        let first = &diagnostics[0];
        assert!(verdict["svg"].is_null());
        assert_eq!(first["is_error"], true);
        assert_eq!(first["line"], 2);
        assert!(first["column"].as_u64().expect("a column") > 1);
    }

    #[test]
    fn a_warning_still_renders() {
        let verdict = verdict_of("a: { shape: sql_table }\n");

        let diagnostics = verdict["diagnostics"].as_array().expect("a list");
        assert!(verdict["svg"].is_string());
        assert_eq!(diagnostics[0]["is_error"], false);
    }

    #[test]
    fn exports_render_what_the_input_buffer_holds() {
        let source = b"a -> b\n";
        let buffer = input_buffer(source.len());
        let input = PLAYGROUND.with_borrow_mut(|playground| playground.input.as_mut_ptr());
        assert_eq!(buffer, input);
        PLAYGROUND.with_borrow_mut(|playground| playground.input.copy_from_slice(source));

        let output_length = render();

        let output = PLAYGROUND.with_borrow(|playground| playground.output.clone());
        assert_eq!(output.len(), output_length);
        let output_start = PLAYGROUND.with_borrow(|playground| playground.output.as_ptr());
        assert_eq!(output_pointer(), output_start);
        assert!(output.contains("\"svg\":\"<svg "));
    }
}
