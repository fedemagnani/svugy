//! Run the svugy command line.

mod failure;
mod input;
mod output;
mod report;

use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use svugy::{InteractiveSvg, StaticSvg};
use svugy_model::model::Model;

use crate::failure::Failure;
use crate::input::{LoadedInput, ModelInput};
use crate::output::OutputTarget;
use crate::report::InputReport;

/// Explain the input formats, where diagnostics go and the exit codes, after the top-level help.
const EXIT_CODES_HELP: &str = "\
INPUT is a JSON model (.json) or a d2 file (.d2), told apart by its extension.
Diagnostics go to stderr, except for validate --format json, which prints them
on stdout. Warnings never stop a subcommand; errors always do.

Exit codes, identical for every subcommand:
  0  success, warnings included
  1  the input has errors: d2 syntax, malformed JSON or model validation
  2  usage error: unknown flag, missing argument, INPUT neither .json nor .d2
  3  I/O failure: the input cannot be read or the output cannot be written";

/// Explain the validate report's formats and its JSON report's fields, after validate's help.
const VALIDATE_REPORT_HELP: &str = "\
Formats:
  human  miette's graphical report on stderr, with source snippets for d2 input
  json   one JSON object on stdout, printed even when the input is clean

The json report is miette's JSON report of the whole input:
  message   \"<INPUT>: <N> error(s), <M> warning(s)\"
  code      \"svugy::input_report\"
  severity  \"error\" if any diagnostic is one, else \"warning\", or \"advice\" when clean
  related   one object per diagnostic: d2 syntax errors, then d2 subset warnings,
            then model problems
Each related object:
  message   what is wrong
  code      stable diagnostic code, e.g. \"svugy::dangling_reference\", \"svugy::d2::syntax\"
  severity  \"error\" or \"warning\"
  help      the did-you-mean suggestion; the key is absent when there is none
  filename  INPUT for a labelled diagnostic of a d2 input; \"\" or absent otherwise
  labels    [{\"label\": text, \"span\": {\"offset\": byte offset, \"length\": bytes}}]
            spans of a JSON model point into the d2 text it was imported from
  causes    underlying error messages, e.g. the JSON parser's
  related   always []";

/// Parse the svugy command line and dispatch its subcommand.
#[derive(Parser)]
#[command(name = "svugy", version, about, after_long_help = EXIT_CODES_HELP)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Name the subcommand to run.
#[derive(Subcommand)]
enum Command {
    /// Render a model as an SVG, interactive unless --profile static is given.
    Render(RenderCommand),
    /// Validate a model and print every diagnostic, errors and warnings alike.
    #[command(after_long_help = VALIDATE_REPORT_HELP)]
    Validate(ValidateCommand),
    /// Import a d2 file as model JSON.
    Import(ImportCommand),
}

/// Render the input as an SVG in the chosen profile, into a file or onto stdout.
#[derive(Args)]
struct RenderCommand {
    /// JSON model or d2 file to render.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    /// Which SVG to write.
    #[arg(long, value_enum, default_value_t = RenderProfile::Interactive)]
    profile: RenderProfile,
    #[command(flatten)]
    output: OutputTarget,
}

/// Validate the input and print its diagnostics in the chosen format.
#[derive(Args)]
struct ValidateCommand {
    /// JSON model or d2 file to validate.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    /// How to print the diagnostics.
    #[arg(long, value_enum, default_value_t = ReportFormat::Human)]
    format: ReportFormat,
}

/// Import the input as pretty-printed model JSON, into a file or onto stdout.
#[derive(Args)]
struct ImportCommand {
    /// d2 file to import; a JSON model comes out in its canonical form.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    #[command(flatten)]
    output: OutputTarget,
}

/// Name the SVG profile `render` writes.
#[derive(Clone, Copy, ValueEnum)]
enum RenderProfile {
    /// Explorable in a browser: themed, animated, with the embedded runtime.
    Interactive,
    /// Self-contained for <img> embeds and design tools: no CSS, script, fonts or animation.
    Static,
}

/// Name how `validate` prints an input's report.
#[derive(Clone, Copy, ValueEnum)]
enum ReportFormat {
    Human,
    Json,
}

/// Run the parsed command line and exit with its failure's code, or success.
fn main() -> ExitCode {
    let cli = Cli::parse();
    let outcome = cli.command.run();
    let Err(failure) = outcome else {
        return ExitCode::SUCCESS;
    };
    failure.eprint();
    failure.exit_code()
}

impl Command {
    /// Run the subcommand.
    fn run(self) -> Result<(), Failure> {
        match self {
            Self::Render(render_command) => render_command.run(),
            Self::Validate(validate_command) => validate_command.run(),
            Self::Import(import_command) => import_command.run(),
        }
    }
}

impl RenderCommand {
    /// Run `render`: load the input, render it in the profile and write the SVG.
    fn run(self) -> Result<(), Failure> {
        let model = self.input.load_model_printing_diagnostics()?;
        let svg = self.profile.render(&model);
        self.output.write(&svg)
    }
}

impl RenderProfile {
    /// Render `model` as the SVG of this profile.
    fn render(self, model: &Model) -> String {
        match self {
            Self::Interactive => InteractiveSvg::render(model),
            Self::Static => StaticSvg::render(model),
        }
    }
}

impl ValidateCommand {
    /// Run `validate`: print the input's report in the format, failing when it holds an error.
    fn run(self) -> Result<(), Failure> {
        let LoadedInput {
            model: _admitted_model,
            report,
        } = self.input.load()?;
        self.format.print(&report)?;
        let has_errors = report.has_errors();
        if has_errors {
            return Err(Failure::InvalidInput);
        }
        Ok(())
    }
}

impl ReportFormat {
    /// Print `report` in this format: miette's graphical report on stderr, or its JSON report on stdout.
    fn print(self, report: &InputReport) -> Result<(), Failure> {
        match self {
            Self::Human => {
                report.eprint_diagnostics();
                Ok(())
            }
            Self::Json => {
                let json_report = report.to_json();
                OutputTarget::write_stdout(&json_report)
            }
        }
    }
}

impl ImportCommand {
    /// Run `import`: load the input and write its model as JSON.
    fn run(self) -> Result<(), Failure> {
        let model = self.input.load_model_printing_diagnostics()?;
        let model_json = model.to_json();
        self.output.write(&model_json)
    }
}
