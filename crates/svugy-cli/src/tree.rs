//! Check each element's code reference against the source tree holding the input, the one check that reads files beyond the input.

use std::fs;
use std::path::{Path, PathBuf};

use miette::{Diagnostic, SourceSpan};
use svugy_model::code::CodeReference;
use svugy_model::model::{Element, Model};
use svugy_model::path::ElementPath;
use thiserror::Error;

/// Name the entry marking a git checkout's root: a directory, or a file in a worktree.
const GIT_ENTRY: &str = ".git";

/// Root the repo-relative files of code references: the git checkout holding the input, else the input's directory.
pub struct SourceTree {
    root: PathBuf,
}

/// Report a code reference the tree no longer backs: its file is gone, or no longer spells its symbol.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` points at code `{code}`, but {reason}")]
#[diagnostic(code(svugy::stale_code_reference))]
pub struct StaleCodeReference {
    element: ElementPath,
    code: String,
    reason: StaleReason,
    #[label("pointed here")]
    span: Option<SourceSpan>,
}

/// Explain why the tree no longer backs a code reference.
#[derive(Debug, Error)]
enum StaleReason {
    #[error("`{file}` is not a readable file under `{root}`")]
    MissingFile { file: String, root: String },
    #[error("`{file}` does not spell every segment of `{symbol}` as a whole identifier")]
    UnspelledSymbol { file: String, symbol: String },
}

impl SourceTree {
    /// Find the tree holding `input_path`: the closest directory above it with a `.git` entry, else the input's own directory.
    pub fn holding(input_path: &Path) -> Self {
        let absolute_input =
            std::path::absolute(input_path).unwrap_or_else(|_error| input_path.to_path_buf());
        let input_directory = absolute_input.parent().unwrap_or(&absolute_input);
        let mut directories = input_directory.ancestors();
        let checkout_root = directories.find(|directory| directory.join(GIT_ENTRY).exists());
        let root = checkout_root.unwrap_or(input_directory).to_path_buf();
        Self { root }
    }

    /// Report each element's code reference whose file is missing or does not spell its symbol, in path order.
    pub fn find_stale_code_references(&self, model: &Model) -> Vec<StaleCodeReference> {
        let elements = model.elements().iter();
        let stale_code_references =
            elements.filter_map(|(path, element)| self.check_code_is_backed(path, element));
        stale_code_references.collect()
    }

    /// Check that the element's code reference, if any, names a file under the root that spells its symbol.
    fn check_code_is_backed(
        &self,
        path: &ElementPath,
        element: &Element,
    ) -> Option<StaleCodeReference> {
        let raw_code = element.code()?;
        let code_reference = CodeReference::parse(raw_code).ok()?;
        let reason = self.find_stale_reason(code_reference)?;
        let code_span = element.code_span().map(SourceSpan::from);
        let span = code_span.or_else(|| element.source_span());
        let pointing_path = path.clone();
        let stale_raw_code = raw_code.to_owned();
        let stale_code_reference = StaleCodeReference {
            element: pointing_path,
            code: stale_raw_code,
            reason,
            span,
        };
        Some(stale_code_reference)
    }

    /// Tell why the tree does not back `code_reference`, or `None` when its file exists and spells its symbol.
    fn find_stale_reason(&self, code_reference: CodeReference<'_>) -> Option<StaleReason> {
        let file = code_reference.file.to_owned();
        let file_path = self.root.join(code_reference.file);
        let Ok(file_bytes) = fs::read(&file_path) else {
            let root = self.root.display().to_string();
            return Some(StaleReason::MissingFile { file, root });
        };
        let file_text = String::from_utf8_lossy(&file_bytes);
        let is_symbol_spelled = code_reference.is_symbol_spelled_in(&file_text);
        if is_symbol_spelled {
            return None;
        }
        let unspelled_symbol = code_reference.symbol.unwrap_or_default();
        let symbol = unspelled_symbol.to_owned();
        Some(StaleReason::UnspelledSymbol { file, symbol })
    }
}
