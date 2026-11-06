//! Write a subcommand's product to a file or to stdout.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, WrapErr};

use crate::failure::Failure;

/// Target the file named by `-o`, or stdout when there is none.
#[derive(Args)]
pub struct OutputTarget {
    /// File to write to; stdout when omitted.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl OutputTarget {
    /// Write `contents` to the output file, or to stdout when there is none.
    pub fn write(&self, contents: &str) -> Result<(), Failure> {
        let Some(output) = &self.output else {
            return Self::write_stdout(contents);
        };
        let output_path = output.display();
        fs::write(output, contents)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot write {output_path}"))
            .map_err(Failure::Io)
    }

    /// Write `contents` to stdout, a closed pipe being an I/O failure rather than a panic.
    pub fn write_stdout(contents: &str) -> Result<(), Failure> {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(contents.as_bytes())
            .into_diagnostic()
            .wrap_err_with(|| "cannot write to stdout")
            .map_err(Failure::Io)
    }
}
