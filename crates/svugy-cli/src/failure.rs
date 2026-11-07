//! Name why a subcommand stopped, which decides the exit code agents branch on.

use std::process::ExitCode;

use miette::Report;

/// Name the failure that stops a subcommand, for a reason that maps to one documented exit code.
#[derive(Debug)]
pub enum Failure {
    InvalidInput,
    Io(Report),
}

impl Failure {
    /// Print on stderr what the failure has not shown yet; an invalid input already printed its diagnostics.
    pub fn eprint(&self) {
        let Self::Io(report) = self else {
            return;
        };
        eprintln!("{report:?}");
    }

    /// Map the failure to its exit code; clap's usage errors exit 2 on their own.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidInput => ExitCode::from(1),
            Self::Io(_report) => ExitCode::from(3),
        }
    }
}
