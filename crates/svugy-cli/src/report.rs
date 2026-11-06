//! Report every diagnostic loading one input raised, as miette's graphical report or its JSON report.

use std::fmt;

use miette::{
    Diagnostic, GraphicalReportHandler, JSONReportHandler, NamedSource, Severity, SourceCode,
};
use svugy_model::validate::ModelReport;

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
    pub fn new(input_name: String, importer_diagnostics: Vec<BoxedDiagnostic>) -> Self {
        Self {
            input_name,
            importer_diagnostics,
            source_code: None,
            model_report: None,
        }
    }

    pub fn with_source_code(mut self, source_code: NamedSource<String>) -> Self {
        self.source_code = Some(source_code);
        self
    }

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

    /// Render the report as miette's machine-readable JSON, one `related` entry per diagnostic.
    pub fn to_json(&self) -> String {
        let handler = JSONReportHandler::new();
        let mut rendered = String::new();
        handler
            .render_report(&mut rendered, self)
            .expect("a JSON report renders into a string: unreadable spans are skipped");
        rendered.push('\n');
        rendered
    }

    /// Tell whether any diagnostic is an error, which stops every subcommand.
    pub fn has_errors(&self) -> bool {
        self.diagnostics().any(Self::is_error)
    }

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

    fn is_error(diagnostic: &dyn Diagnostic) -> bool {
        let severity = diagnostic.severity();
        let severity_or_error = severity.unwrap_or(Severity::Error);
        severity_or_error == Severity::Error
    }
}

impl fmt::Display for InputReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let input_name = &self.input_name;
        let error_count = self
            .diagnostics()
            .filter(|diagnostic| Self::is_error(*diagnostic))
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
