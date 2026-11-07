//! Report every diagnostic loading one input raised, as miette's graphical report or its JSON report.

use std::fmt;

use miette::{
    Diagnostic, GraphicalReportHandler, JSONReportHandler, NamedSource, Severity, SourceCode,
    SourceSpan,
};
use serde_json::Value;
use svugy_model::problem::ErrorSeverity;
use svugy_model::validate::ModelReport;

/// Box an importer diagnostic so diagnostics of every importer stage share one list.
pub type BoxedDiagnostic = Box<dyn Diagnostic + Send + Sync>;

/// Collect an input's importer diagnostics and model problems into one report whose severity is its worst diagnostic's.
#[derive(Debug)]
pub struct InputReport {
    input_name: String,
    importer_diagnostics: Vec<BoxedDiagnostic>,
    source_code: Option<NamedSource<String>>,
    model_report: Option<ModelReport>,
}

impl InputReport {
    /// Create a new report of `input_name` holding its importer diagnostics, without source code or model report.
    pub fn new(input_name: String, importer_diagnostics: Vec<BoxedDiagnostic>) -> Self {
        Self {
            input_name,
            importer_diagnostics,
            source_code: None,
            model_report: None,
        }
    }

    /// Set the source code the diagnostics' spans point into, so snippets are shown.
    pub fn with_source_code(mut self, source_code: NamedSource<String>) -> Self {
        self.source_code = Some(source_code);
        self
    }

    /// Set the model report whose problems follow the importer diagnostics.
    pub fn with_model_report(mut self, model_report: ModelReport) -> Self {
        self.model_report = Some(model_report);
        self
    }

    /// Print every diagnostic on stderr as miette's graphical report, with source snippets where spans exist.
    pub fn eprint_diagnostics(&self) {
        let is_clean = self.diagnostics().next().is_none();
        if is_clean {
            return;
        }
        let handler = GraphicalReportHandler::new();
        let mut rendered = String::new();
        handler
            .render_report(&mut rendered, self)
            .expect("every span points into the source it is rendered against");
        eprint!("{rendered}");
    }

    /// Render the report as miette's machine-readable JSON, one `related` entry per diagnostic, each label located by line and column when the source is known.
    pub fn to_json(&self) -> String {
        let handler = JSONReportHandler::new();
        let mut miette_json = String::new();
        handler
            .render_report(&mut miette_json, self)
            .expect("a JSON report renders into a string: unreadable spans are skipped");
        let Some(source_code) = &self.source_code else {
            miette_json.push('\n');
            return miette_json;
        };
        let mut json_report: Value =
            serde_json::from_str(&miette_json).expect("miette renders valid JSON");
        Self::locate_every_label(&mut json_report, source_code);
        let mut located_json = json_report.to_string();
        located_json.push('\n');
        located_json
    }

    /// Add to every label of every related diagnostic its span's 1-based `line` and `column`, as miette's graphical report counts them.
    fn locate_every_label(json_report: &mut Value, source_code: &NamedSource<String>) {
        let related_diagnostics = json_report["related"].as_array_mut().into_iter().flatten();
        let labels = related_diagnostics
            .flat_map(|diagnostic| diagnostic["labels"].as_array_mut())
            .flatten();
        for label in labels {
            let span_offset = label["span"]["offset"].as_u64();
            let span_length = label["span"]["length"].as_u64();
            let (Some(span_offset), Some(span_length)) = (span_offset, span_length) else {
                continue;
            };
            let span = SourceSpan::from((span_offset as usize, span_length as usize));
            let Ok(span_contents) = source_code.read_span(&span, 0, 0) else {
                continue;
            };
            let line = span_contents.line() + 1;
            let column = span_contents.column() + 1;
            label["line"] = Value::from(line);
            label["column"] = Value::from(column);
        }
    }

    /// Tell whether any diagnostic is an error, which stops every subcommand.
    pub fn has_errors(&self) -> bool {
        self.diagnostics().any(|diagnostic| diagnostic.is_error())
    }

    /// List every diagnostic: the importer's first, then the model's problems.
    fn diagnostics(&self) -> impl Iterator<Item = &dyn Diagnostic> {
        let importer_diagnostics = self
            .importer_diagnostics
            .iter()
            .map(|diagnostic| diagnostic.as_ref() as &dyn Diagnostic);
        let model_problems = self
            .model_report
            .iter()
            .flat_map(ModelReport::problems)
            .map(|problem| problem as &dyn Diagnostic);
        importer_diagnostics.chain(model_problems)
    }
}

impl fmt::Display for InputReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let input_name = &self.input_name;
        let error_count = self
            .diagnostics()
            .filter(|diagnostic| diagnostic.is_error())
            .count();
        let diagnostic_count = self.diagnostics().count();
        let warning_count = diagnostic_count - error_count;
        write!(
            formatter,
            "{input_name}: {error_count} error(s), {warning_count} warning(s)"
        )
    }
}

impl std::error::Error for InputReport {}

impl Diagnostic for InputReport {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        let code: Box<dyn fmt::Display + 'a> = Box::new("svugy::input_report");
        Some(code)
    }

    fn severity(&self) -> Option<Severity> {
        let has_errors = self.has_errors();
        if has_errors {
            return Some(Severity::Error);
        }
        let is_clean = self.diagnostics().next().is_none();
        if is_clean {
            return Some(Severity::Advice);
        }
        Some(Severity::Warning)
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        let source_code = self.source_code.as_ref()?;
        Some(source_code)
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        let diagnostics = self.diagnostics();
        let related: Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a> = Box::new(diagnostics);
        Some(related)
    }
}
