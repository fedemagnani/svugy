//! Load the input file into a model, choosing Markdown, d2 or JSON by its extension, and report what loading it raised.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use miette::{IntoDiagnostic, NamedSource, WrapErr};
use svugy_model::model::Model;
use svugy_parse::d2::D2Language;
use svugy_parse::diagnostic::{ImportParts, SourceLanguage};
use svugy_parse::json::JsonLanguage;
use svugy_parse::markdown::MarkdownLanguage;

use crate::failure::Failure;
use crate::report::{BoxedDiagnostic, InputReport};

/// Type the loader of one input format: its source in, its model and report out.
type LoadSource = fn(NamedSource<String>) -> LoadedInput;

/// Pair an input path with the loader its extension selects.
#[derive(Clone)]
pub struct ModelInput {
    path: PathBuf,
    load_source: LoadSource,
}

/// Hold an input's report and its model, admitted only when the report holds no error.
pub struct LoadedInput {
    pub model: Option<Model>,
    pub report: InputReport,
}

/// Load a source language's files, the languages differing only in whether the model's spans point into the file.
trait SourceLoader: SourceLanguage<Warning: Send + Sync + 'static> + Send + Sync + 'static {
    /// Attach `source` to `report` so its snippets show, unless the model's spans point elsewhere.
    fn attach_snippet_source(report: InputReport, source: NamedSource<String>) -> InputReport;

    /// Load a file of the language, reporting its errors, its warnings and its model problems.
    fn load(source: NamedSource<String>) -> LoadedInput {
        let input_name = source.name().to_owned();
        let source_text = source.inner();
        let import = Self::import(source_text);
        let import_parts = import.into_admitted_parts();
        let ImportParts {
            model,
            errors,
            warnings,
            report: model_report,
        } = import_parts;
        let language_errors = errors
            .into_iter()
            .map(|error| Box::new(error) as BoxedDiagnostic);
        let language_warnings = warnings
            .into_iter()
            .map(|warning| Box::new(warning) as BoxedDiagnostic);
        let importer_diagnostics = language_errors.chain(language_warnings).collect();
        let unsourced_report =
            InputReport::new(input_name, importer_diagnostics).with_model_report(model_report);
        let report = Self::attach_snippet_source(unsourced_report, source);
        LoadedInput { model, report }
    }
}

impl ModelInput {
    /// Parse `argument` into an input whose extension selects the loader, the one place the input format is decided.
    pub fn parse(argument: &str) -> Result<Self, String> {
        let path = PathBuf::from(argument);
        let extension = path.extension().and_then(OsStr::to_str);
        let load_source: LoadSource = match extension {
            Some("md") => MarkdownLanguage::load,
            Some("d2") => D2Language::load,
            Some("json") => JsonLanguage::load,
            _unsupported_extension => {
                let unsupported_message =
                    "expected a `.md` diagram, a `.d2` file or a `.json` model".to_owned();
                return Err(unsupported_message);
            }
        };
        Ok(Self { path, load_source })
    }

    /// Load the model a subcommand consumes, printing the input's diagnostics on stderr and failing on any error.
    pub fn load_model_printing_diagnostics(&self) -> Result<Model, Failure> {
        let LoadedInput { model, report } = self.load()?;
        report.eprint_diagnostics();
        model.ok_or_else(|| Failure::InvalidInput)
    }

    /// Read the input file and load it with the loader its extension selected.
    pub fn load(&self) -> Result<LoadedInput, Failure> {
        let input_name = self.path.display().to_string();
        let source_text = fs::read_to_string(&self.path)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot read input {input_name}"))
            .map_err(Failure::Io)?;
        let source = NamedSource::new(input_name, source_text);
        let loaded = (self.load_source)(source);
        Ok(loaded)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SourceLoader for MarkdownLanguage {
    fn attach_snippet_source(report: InputReport, source: NamedSource<String>) -> InputReport {
        report.with_source_code(source)
    }
}

impl SourceLoader for D2Language {
    fn attach_snippet_source(report: InputReport, source: NamedSource<String>) -> InputReport {
        report.with_source_code(source)
    }
}

impl SourceLoader for JsonLanguage {
    /// Show no snippet: a JSON model's spans, if any, point into the text it was imported from.
    fn attach_snippet_source(
        report: InputReport,
        _json_source: NamedSource<String>,
    ) -> InputReport {
        report
    }
}
