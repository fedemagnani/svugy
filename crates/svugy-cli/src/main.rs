//! Run the svugy command line.

mod example;
mod failure;
mod flavor;
mod input;
mod output;
mod report;
mod skill;
mod view;

use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use svugy_parse::json::JsonPrinter;

use crate::example::ExampleCommand;
use crate::failure::Failure;
use crate::flavor::SvgFlavor;
use crate::input::{LoadedInput, ModelInput};
use crate::output::{ModelFile, OutputTarget};
use crate::report::InputReport;
use crate::skill::SkillCommand;
use crate::view::{LIVE_VIEW_HELP, ViewCommand};

/// Point a new user at the example, then explain the input formats, where diagnostics go and the exit codes, after the top-level help.
const EXIT_CODES_HELP: &str = "\
New here? svugy example -o webapp.md writes an example diagram, and
svugy render webapp.md -o webapp.svg --open draws it.

INPUT is a Markdown diagram (.md), a d2 file (.d2) or a JSON model (.json), told
apart by its extension. Diagnostics go to stderr, except for validate --format
json, which prints them on stdout. Warnings never stop a subcommand; errors
always do.

Exit codes, identical for every subcommand:
  0  success, warnings included
  1  the input has errors: Markdown syntax, d2 syntax, malformed JSON or model
     validation; or import -o FILE.md meets a model Markdown cannot say
  2  usage error: unknown flag, missing argument, INPUT not .md, .d2 or .json
  3  I/O failure: a file cannot be read, written or opened, or view cannot
     listen on its port";

/// Explain the validate report's formats and its JSON report's fields, after validate's help.
const VALIDATE_REPORT_HELP: &str = "\
Formats:
  human  miette's graphical report on stderr, with source snippets for .md and
         d2 input
  json   one JSON object on stdout, printed even when the input is clean

The json report is miette's JSON report of the whole input:
  message   \"<INPUT>: <N> error(s), <M> warning(s)\"
  code      \"svugy::input_report\"
  severity  \"error\" if any diagnostic is one, else \"warning\", or \"advice\" when clean
  related   one object per diagnostic: Markdown errors, d2 syntax errors then
            d2 subset warnings, or the malformed-JSON error; then model problems
Each related object:
  message   what is wrong; a Markdown error ends with its did-you-mean
  code      stable diagnostic code, e.g. \"svugy::markdown\", \"svugy::d2::syntax\",
            \"svugy::dangling_reference\"
  severity  \"error\" or \"warning\"
  help      the did-you-mean of a d2 diagnostic or model problem; the key is
            absent when there is none
  filename  INPUT for a labelled diagnostic of a .md or d2 input; \"\" or absent
            otherwise
  labels    [{\"label\": text, \"span\": {\"offset\": byte offset, \"length\": bytes},
              \"line\": 1-based line, \"column\": 1-based byte column}]
            line and column are absent for a JSON model, whose spans point
            into the text it was imported from; a malformed-JSON message
            ends with serde's line and column instead
  causes    underlying error messages, when there are any
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
    /// Render a model as an SVG, interactive unless --profile static is given, hand-drawn with --theme sketch.
    Render(RenderCommand),
    /// Serve the interactive SVG in the default browser, reloading it whenever INPUT changes.
    #[command(after_long_help = LIVE_VIEW_HELP)]
    View(ViewCommand),
    /// Validate a model and print every diagnostic, errors and warnings alike.
    #[command(after_long_help = VALIDATE_REPORT_HELP)]
    Validate(ValidateCommand),
    /// Import a diagram as Markdown or model JSON, chosen by the output's extension.
    Import(ImportCommand),
    /// Print the svugy skill for agents, or install it with `svugy skill install`.
    Skill(SkillCommand),
    /// Print an example diagram, a web app written in Markdown, or write it with -o.
    Example(ExampleCommand),
}

/// Render the input as an SVG in the chosen profile and theme, into a file or onto stdout, and open the file if asked.
#[derive(Args)]
struct RenderCommand {
    /// Diagram (.md), d2 file or JSON model to render.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    #[command(flatten)]
    flavor: SvgFlavor,
    #[command(flatten)]
    output: OutputTarget,
    /// Open the written SVG in the default browser; needs -o.
    #[arg(long, requires = "output")]
    open: bool,
}

/// Validate the input and print its diagnostics in the chosen format.
#[derive(Args)]
struct ValidateCommand {
    /// Diagram (.md), d2 file or JSON model to validate.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    /// How to print the diagnostics.
    #[arg(long, value_enum, default_value_t = ReportFormat::Human)]
    format: ReportFormat,
}

/// Import the input into the file's text form, or onto stdout as pretty-printed model JSON.
#[derive(Args)]
struct ImportCommand {
    /// Diagram (.md), d2 file or JSON model to import; it comes out in canonical form.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    /// File to write: a Markdown diagram when it ends in .md, model JSON when it ends in .json; JSON on stdout when omitted.
    #[arg(short, long, value_parser = ModelFile::parse)]
    output: Option<ModelFile>,
}

/// Name how `validate` prints an input's report.
#[derive(Clone, Copy, ValueEnum)]
enum ReportFormat {
    Human,
    Json,
}

/// Serve as the binary's main entry: run the parsed command line and exit with its failure's code, or success.
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
            Self::View(view_command) => view_command.run(),
            Self::Validate(validate_command) => validate_command.run(),
            Self::Import(import_command) => import_command.run(),
            Self::Skill(skill_command) => skill_command.run(),
            Self::Example(example_command) => example_command.run(),
        }
    }
}

impl RenderCommand {
    /// Run `render`: load the input, render it in the flavor, write the SVG and open it when `--open` asks.
    fn run(self) -> Result<(), Failure> {
        let model = self.input.load_model_printing_diagnostics()?;
        let svg = self.flavor.render(&model);
        self.output.write(&svg)?;
        let is_open_requested = self.open;
        if is_open_requested {
            self.output.open_in_default_browser()?;
        }
        Ok(())
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
    /// Run `import`: load the input and write its model in the output file's text form, or as JSON on stdout.
    fn run(self) -> Result<(), Failure> {
        let model = self.input.load_model_printing_diagnostics()?;
        let Some(model_file) = self.output else {
            let model_json = JsonPrinter::print(&model);
            return OutputTarget::write_stdout(&model_json);
        };
        model_file.write(&model)
    }
}
