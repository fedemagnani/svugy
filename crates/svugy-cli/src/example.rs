//! Print or write the example diagram shipped with the install, so a new user can render something at once.

use clap::Args;

use crate::failure::Failure;
use crate::output::OutputTarget;

/// Embed the example diagram, a copy of examples/webapp/webapp.md: a generic web app written in Markdown.
const WEBAPP_MARKDOWN: &str = include_str!("../example/webapp.md");

/// Print the example diagram, or write it to the file `-o` names.
#[derive(Args)]
pub struct ExampleCommand {
    #[command(flatten)]
    output: OutputTarget,
}

impl ExampleCommand {
    /// Run `example`: write the embedded diagram to the output.
    pub fn run(self) -> Result<(), Failure> {
        self.output.write(WEBAPP_MARKDOWN)
    }
}
