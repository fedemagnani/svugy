//! Print or install the svugy skill, which teaches an agent to write, check and open a diagram.

use std::env;
use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use miette::{IntoDiagnostic, WrapErr, miette};

use crate::failure::Failure;
use crate::output::OutputTarget;

/// Embed the skill's entry file, the one an agent reads first.
const SKILL_MARKDOWN: &str = include_str!("../skill/SKILL.md");
/// Embed the syntax the entry file's example leaves out.
const SYNTAX_MARKDOWN: &str = include_str!("../skill/references/syntax.md");
/// Embed the validate report's fields, exit codes and fixes its messages don't give.
const TROUBLESHOOT_MARKDOWN: &str = include_str!("../skill/references/troubleshoot.md");
/// Embed the model JSON shape, read when a diagram arrives or must leave as JSON.
const JSON_MARKDOWN: &str = include_str!("../skill/references/json.md");
/// Embed the installation steps, read when `svugy` is missing from `PATH`.
const INSTALL_MARKDOWN: &str = include_str!("../skill/references/install.md");
/// List the skill's files by path inside the skill directory, each with its embedded contents; the entry file reads a reference only when its case applies.
const SKILL_FILES: [(&str, &str); 5] = [
    ("SKILL.md", SKILL_MARKDOWN),
    ("references/syntax.md", SYNTAX_MARKDOWN),
    ("references/troubleshoot.md", TROUBLESHOOT_MARKDOWN),
    ("references/json.md", JSON_MARKDOWN),
    ("references/install.md", INSTALL_MARKDOWN),
];
/// Name the directory, under a project or the home directory, where Claude looks for the svugy skill.
const SKILL_DIRECTORY: &str = ".claude/skills/svugy";
/// Name the directory, inside the skill directory, holding the files the entry file links.
const REFERENCES_DIRECTORY: &str = "references";

/// Print the skill's entry file, or install the skill when `install` is given.
#[derive(Args)]
pub struct SkillCommand {
    #[command(subcommand)]
    action: Option<SkillAction>,
}

/// Name what to do with the skill instead of printing it.
#[derive(Subcommand)]
enum SkillAction {
    /// Write the skill to .claude/skills/svugy/ under the current directory, or under the home directory with --user.
    Install(InstallSkill),
}

/// Install the skill's files where Claude finds them.
#[derive(Args)]
struct InstallSkill {
    /// Install for every project of the user, under ~/.claude/skills/svugy/.
    #[arg(long)]
    user: bool,
}

impl SkillCommand {
    /// Run `skill`: print SKILL.md on stdout, or run the action.
    pub fn run(self) -> Result<(), Failure> {
        let Some(SkillAction::Install(install_skill)) = self.action else {
            return OutputTarget::write_stdout(SKILL_MARKDOWN);
        };
        install_skill.run()
    }
}

impl InstallSkill {
    /// Write every skill file into the skill directory, creating it and its references directory, then print where they went.
    fn run(self) -> Result<(), Failure> {
        let installation_root = self.installation_root()?;
        let skill_directory = installation_root.join(SKILL_DIRECTORY);
        let references_directory = skill_directory.join(REFERENCES_DIRECTORY);
        let references_directory_path = references_directory.display();
        fs::create_dir_all(&references_directory)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot create {references_directory_path}"))
            .map_err(Failure::Io)?;
        let mut written_paths = String::new();
        for (file_name, contents) in SKILL_FILES {
            let skill_file = skill_directory.join(file_name);
            OutputTarget::write_file(&skill_file, contents)?;
            let skill_file_path = skill_file.display();
            let written_line = format!("wrote {skill_file_path}\n");
            written_paths.push_str(&written_line);
        }
        OutputTarget::write_stdout(&written_paths)
    }

    /// Return the directory the skill directory goes under: the home directory with `--user`, else the current one.
    fn installation_root(&self) -> Result<PathBuf, Failure> {
        let is_user_installation = self.user;
        if is_user_installation {
            return env::home_dir()
                .ok_or_else(|| miette!("cannot find the home directory"))
                .map_err(Failure::Io);
        }
        env::current_dir()
            .into_diagnostic()
            .wrap_err_with(|| "cannot read the current directory")
            .map_err(Failure::Io)
    }
}
