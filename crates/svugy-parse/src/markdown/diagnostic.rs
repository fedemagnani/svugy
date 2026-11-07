//! Report what a Markdown diagram file holds that cannot be read, each error labelled at its source, and each description link left unlinked.

use std::ops::Range;

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use super::ast::Span;
use super::{MarkdownImport, MarkdownParse};
use crate::diagnostic::{SourceError, SourceLanguage};

/// Mark Markdown as a source language: its errors are Markdown errors, its warnings the description links it leaves unlinked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownLanguage;

/// Report a Markdown error: a part of the diagram file that cannot be read, left out of the model.
pub type MarkdownError = SourceError<MarkdownLanguage>;

/// Report a description link whose `#` target names no component or edge, or several, so it stays as written and shows as plain text.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(svugy::markdown::unresolved_target), severity(Warning))]
pub struct UnresolvedTarget {
    pub message: String,
    #[label("linked here")]
    pub span: SourceSpan,
}

/// Record the Markdown errors and warnings the layers raise, handed from one layer to the next.
pub(crate) type Diagnostics = crate::diagnostic::Diagnostics<MarkdownLanguage>;

impl SourceLanguage for MarkdownLanguage {
    const ERROR_CODE: &'static str = "svugy::markdown";
    type Warning = UnresolvedTarget;

    fn import(source_text: &str) -> MarkdownImport {
        let parsed = MarkdownParse::parse(source_text);
        parsed.lower()
    }
}

impl Diagnostics {
    /// Record that the description link target at `span` stays unlinked, `message` saying why.
    pub(crate) fn record_unresolved_target(&mut self, message: String, span: Span) {
        let byte_range: Range<usize> = span.into();
        let span = SourceSpan::from(byte_range);
        let warning = UnresolvedTarget { message, span };
        self.warnings.push(warning);
    }
}
