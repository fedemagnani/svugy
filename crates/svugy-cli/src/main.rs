//! Run the svugy command line.

mod example;
mod failure;
mod flavor;
mod input;
mod output;
mod png;
mod report;
mod skill;
mod tree;
mod view;
mod workflow_choice;

use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use svugy_model::model::ShownWorkflow;
use svugy_parse::json::JsonPrinter;

use crate::example::ExampleCommand;
use crate::failure::Failure;
use crate::flavor::SvgFlavor;
use crate::input::{LoadedInput, ModelInput};
use crate::output::{ModelFile, OutputTarget};
use crate::png::PngExport;
use crate::report::InputReport;
use crate::skill::SkillCommand;
use crate::tree::SourceTree;
use crate::view::{LIVE_VIEW_HELP, ViewCommand};
use crate::workflow_choice::{ShownWorkflows, WorkflowChoice};

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
     validation; or validate finds a stale code reference; or import -o FILE.md
     meets a model Markdown cannot say
  2  usage error: unknown flag, missing argument, INPUT not .md, .d2 or .json,
     render --workflow naming no workflow of the input, render --all-workflows
     with an -o naming no file stem, or render --depth past the workflow's
     deepest layer
  3  I/O failure: a file cannot be read, written or opened, or view cannot
     listen on its port";

/// Explain the code reference check, the validate report's formats and its JSON report's fields, after validate's help.
const VALIDATE_REPORT_HELP: &str = "\
Code references:
  Each component's code=FILE#SYMBOL is checked against the files on disk, FILE
  relative to the git checkout holding INPUT (else INPUT's directory). FILE
  must exist, and it must spell each `::`- or `.`-separated segment of SYMBOL
  as a whole identifier. A miss is a svugy::stale_code_reference error. Only
  validate runs this check; render, view and import never read the tree.

Formats:
  human  miette's graphical report on stderr, with source snippets for .md and
         d2 input
  json   one JSON object on stdout, printed even when the input is clean

The json report is miette's JSON report of the whole input:
  message   \"<INPUT>: <N> error(s), <M> warning(s)\"
  code      \"svugy::input_report\"
  severity  \"error\" if any diagnostic is one, else \"warning\", or \"advice\" when clean
  related   one object per diagnostic: Markdown errors, d2 syntax errors then
            d2 subset warnings, or the malformed-JSON error; then model
            problems; then stale code references
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
    /// Render a model as an SVG, interactive unless --profile static draws one --workflow alone, hand-drawn with --theme sketch, or one --workflow as a PNG with --png.
    Render(RenderCommand),
    /// Serve the interactive SVG in the default browser, reloading it whenever INPUT changes.
    #[command(after_long_help = LIVE_VIEW_HELP)]
    View(ViewCommand),
    /// Validate a model, check its code references against the source tree, and print every diagnostic, errors and warnings alike.
    #[command(after_long_help = VALIDATE_REPORT_HELP)]
    Validate(ValidateCommand),
    /// Import a diagram as Markdown or model JSON, chosen by the output's extension.
    Import(ImportCommand),
    /// Print the svugy skill for agents, or install it with `svugy skill install`.
    Skill(SkillCommand),
    /// Print an example diagram, a web app written in Markdown, or write it with -o.
    Example(ExampleCommand),
}

/// Render the input as an SVG in the chosen profile and theme, or as PNG, into a file or onto stdout, and open the file if asked.
#[derive(Args)]
struct RenderCommand {
    /// Diagram (.md), d2 file or JSON model to render.
    #[arg(value_parser = ModelInput::parse)]
    input: ModelInput,
    #[command(flatten)]
    flavor: SvgFlavor,
    #[command(flatten)]
    workflow_choice: WorkflowChoice,
    #[command(flatten)]
    png_export: PngExport,
    #[command(flatten)]
    output: OutputTarget,
    /// Open the written file in the default browser; needs -o.
    #[arg(long, requires = "output", conflicts_with = "all_workflows")]
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
    /// Run `render`: load the input and write the chosen workflow, or each workflow, as the flavor's SVG or as PNG when `--png` asks.
    fn run(self) -> Result<(), Failure> {
        let model = self.input.load_model_printing_diagnostics()?;
        let shown_workflows = self.workflow_choice.shown_workflows(&model)?;
        match shown_workflows {
            ShownWorkflows::One(shown_workflow) => self.write_opening_it_if_asked(shown_workflow),
            ShownWorkflows::Each(each_workflow) => self.write_per_workflow(each_workflow),
        }
    }

    /// Write `shown_workflow` to the output and open it when `--open` asks.
    fn write_opening_it_if_asked(&self, shown_workflow: ShownWorkflow<'_>) -> Result<(), Failure> {
        self.write_workflow(shown_workflow, &self.output)?;
        let is_open_requested = self.open;
        if is_open_requested {
            self.output.open_in_default_browser()?;
        }
        Ok(())
    }

    /// Write each workflow to `STEM.ID.svg`, or `STEM.ID.png` with `--png`, beside the output file.
    fn write_per_workflow(&self, each_workflow: Vec<ShownWorkflow<'_>>) -> Result<(), Failure> {
        let output_path = self
            .output
            .path()
            .expect("clap requires -o with --all-workflows");
        let is_png_requested = self.png_export.is_requested();
        let extension = if is_png_requested { "png" } else { "svg" };
        for shown_workflow in each_workflow {
            let workflow = shown_workflow.workflow();
            let workflow_id = workflow.id();
            let workflow_tag = workflow_id.as_str();
            let workflow_path = OutputTarget::tagged_path(output_path, workflow_tag, extension)?;
            let workflow_output = OutputTarget::file(workflow_path);
            self.write_workflow(shown_workflow, &workflow_output)?;
        }
        Ok(())
    }

    /// Write `shown_workflow` to `output` as PNG when `--png` asks, else as the flavor's SVG.
    fn write_workflow(
        &self,
        shown_workflow: ShownWorkflow<'_>,
        output: &OutputTarget,
    ) -> Result<(), Failure> {
        let is_png_requested = self.png_export.is_requested();
        if is_png_requested {
            let theme = self.flavor.theme();
            return self.png_export.write(shown_workflow, theme, output);
        }
        let svg = self.flavor.render(shown_workflow);
        output.write(svg)
    }
}

impl ValidateCommand {
    /// Run `validate`: check an admitted model's code references against the source tree, print the input's report in the format, failing when it holds an error.
    fn run(self) -> Result<(), Failure> {
        let LoadedInput { model, report } = self.input.load()?;
        let input_path = self.input.path();
        let source_tree = SourceTree::holding(input_path);
        let stale_code_references = match &model {
            Some(admitted_model) => source_tree.find_stale_code_references(admitted_model),
            None => Vec::new(),
        };
        let report = report.with_stale_code_references(stale_code_references);
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
