//! Load the input file into a model, choosing JSON or d2 by its extension, and report what loading it raised.

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

use miette::{IntoDiagnostic, NamedSource, WrapErr};
use svugy_import::d2::D2Parse;
use svugy_model::model::Model;
use svugy_model::validate::ModelAdmission;

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

/// Read a JSON model.
struct Json;

/// Import a d2 file.
struct D2;

impl ModelInput {
    /// Parse `argument` into an input whose extension selects the loader, the one place the input format is decided.
    pub fn parse(argument: &str) -> Result<Self, String> {
        let path = PathBuf::from(argument);
        let extension = path.extension().and_then(OsStr::to_str);
        let load_source: LoadSource = match extension {
            Some("json") => Json::load,
            Some("d2") => D2::load,
            _unsupported_extension => {
                let unsupported_message = "expected a `.json` model or a `.d2` file".to_owned();
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
}

impl Json {
    /// Load a JSON model without snippets: its spans, if any, point into the d2 text it was imported from.
    fn load(source: NamedSource<String>) -> LoadedInput {
        let input_name = source.name().to_owned();
        let source_text = source.inner();
        let loaded_json = Model::load_json(source_text);
        let admission = match loaded_json {
            Ok(admission) => admission,
            Err(malformed) => {
                let malformed_diagnostic: BoxedDiagnostic = Box::new(malformed);
                let importer_diagnostics = vec![malformed_diagnostic];
                let report = InputReport::new(input_name, importer_diagnostics);
                return LoadedInput {
                    model: None,
                    report,
                };
            }
        };
        let ModelAdmission {
            model,
            report: model_report,
        } = admission;
        let no_importer_diagnostics = Vec::new();
        let report =
            InputReport::new(input_name, no_importer_diagnostics).with_model_report(model_report);
        LoadedInput { model, report }
    }
}

impl D2 {
    /// Import a d2 file, reporting its syntax errors, its subset warnings and its model problems against the d2 text.
    fn load(source: NamedSource<String>) -> LoadedInput {
        let input_name = source.name().to_owned();
        let source_text = source.inner();
        let parsed = D2Parse::parse(source_text);
        let import = parsed.lower();
        let (model, errors, warnings, model_report) = import.into_parts();
        let syntax_errors = errors
            .into_iter()
            .map(|error| Box::new(error) as BoxedDiagnostic);
        let unsupported_constructs = warnings
            .into_iter()
            .map(|warning| Box::new(warning) as BoxedDiagnostic);
        let importer_diagnostics = syntax_errors.chain(unsupported_constructs).collect();
        let report = InputReport::new(input_name, importer_diagnostics)
            .with_source_code(source)
            .with_model_report(model_report);
        LoadedInput { model, report }
    }
}
