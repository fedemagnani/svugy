//! Report what a Markdown diagram file holds that cannot be read, each error labelled at its source.

use super::{MarkdownImport, MarkdownParse};
use crate::diagnostic::{NoWarning, SourceError, SourceLanguage};

/// Mark Markdown as a source language: its errors are Markdown errors, and it raises no warnings of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownLanguage;

/// Report a Markdown error: a part of the diagram file that cannot be read, left out of the model.
pub type MarkdownError = SourceError<MarkdownLanguage>;

/// Record the Markdown errors the layers raise, handed from one layer to the next.
pub(crate) type Diagnostics = crate::diagnostic::Diagnostics<MarkdownLanguage>;

impl SourceLanguage for MarkdownLanguage {
    const ERROR_CODE: &'static str = "svugy::markdown";
    type Warning = NoWarning;

    fn import(source_text: &str) -> MarkdownImport {
        let parsed = MarkdownParse::parse(source_text);
        parsed.lower()
    }
}
