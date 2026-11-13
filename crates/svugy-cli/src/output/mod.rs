//! Write a subcommand's product to a file or to stdout, choosing the text form of an imported model by the file's extension.

pub mod browser;

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use miette::{IntoDiagnostic, WrapErr};
use svugy_model::model::Model;
use svugy_parse::json::JsonPrinter;
use svugy_parse::markdown::MarkdownPrinter;

use crate::failure::Failure;
use browser::DefaultBrowser;

/// Type the printer of one model text form: the model in, its text out, or why the form cannot say it.
type PrintModel = fn(&Model) -> Result<String, Failure>;

/// Target the file named by `-o`, or stdout when there is none.
#[derive(Args)]
pub struct OutputTarget {
    /// File to write to; stdout when omitted.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

/// Pair the file `import` writes with the printer of the model text form its extension selects.
#[derive(Clone)]
pub struct ModelFile {
    path: PathBuf,
    print_model: PrintModel,
}

impl OutputTarget {
    /// Write `contents` to the output file, or to stdout when there is none.
    pub fn write(&self, contents: impl AsRef<[u8]>) -> Result<(), Failure> {
        let Some(output) = &self.output else {
            return Self::write_stdout(contents);
        };
        Self::write_file(output, contents)
    }

    /// Open the output file in the default browser; stdout has nothing to open, and clap rejects `--open` without `-o`.
    pub fn open_in_default_browser(&self) -> Result<(), Failure> {
        let Some(output) = &self.output else {
            return Ok(());
        };
        let output_path = output.as_os_str();
        DefaultBrowser::open(output_path)
    }

    /// Write `contents` to stdout, a closed pipe being an I/O failure rather than a panic.
    pub fn write_stdout(contents: impl AsRef<[u8]>) -> Result<(), Failure> {
        let mut stdout = io::stdout().lock();
        let content_bytes = contents.as_ref();
        stdout
            .write_all(content_bytes)
            .into_diagnostic()
            .wrap_err_with(|| "cannot write to stdout")
            .map_err(Failure::Io)
    }

    /// Write `contents` to the file at `path`, an unwritable path being an I/O failure.
    pub fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Failure> {
        let output_path = path.display();
        fs::write(path, contents)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot write {output_path}"))
            .map_err(Failure::Io)
    }

    /// Return the output file, or `None` for stdout.
    pub fn path(&self) -> Option<&Path> {
        self.output.as_deref()
    }
}

impl ModelFile {
    /// Parse `argument` into a model file whose extension selects the text form: Markdown for `.md`, JSON for `.json`.
    pub fn parse(argument: &str) -> Result<Self, String> {
        let path = PathBuf::from(argument);
        let extension = path.extension().and_then(OsStr::to_str);
        let print_model: PrintModel = match extension {
            Some("md") => Self::print_markdown,
            Some("json") => Self::print_json,
            _unsupported_extension => {
                let unsupported_message =
                    "expected a `.md` diagram or a `.json` model to write".to_owned();
                return Err(unsupported_message);
            }
        };
        Ok(Self { path, print_model })
    }

    /// Write `model` to the file in its text form, writing nothing when the form cannot say all of it.
    pub fn write(&self, model: &Model) -> Result<(), Failure> {
        let model_text = (self.print_model)(model)?;
        OutputTarget::write_file(&self.path, &model_text)
    }

    /// Print `model` as a Markdown diagram, failing with every part of it Markdown cannot say.
    fn print_markdown(model: &Model) -> Result<String, Failure> {
        MarkdownPrinter::print(model)
            .into_diagnostic()
            .map_err(Failure::UnprintableModel)
    }

    /// Print `model` as pretty-printed JSON, which says every model.
    fn print_json(model: &Model) -> Result<String, Failure> {
        let model_json = JsonPrinter::print(model);
        Ok(model_json)
    }
}
