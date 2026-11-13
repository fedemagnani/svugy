//! Parse a diagram written in Markdown into the model, reporting every error in one pass, and print a model back as Markdown.

mod ast;
mod diagnostic;
mod frontmatter;
mod lex;
mod lower;
mod parse;
mod print;
mod property;
mod resolve;
mod skeleton;
mod style;
mod target;

pub use diagnostic::{MarkdownError, MarkdownLanguage};
pub use print::{
    MarkdownPrinter, TextField, Unprintable, UnprintableModel, UnprintableOwner, UnprintableReason,
};

use crate::diagnostic::{Import, ImportParts};
use ast::Document;
use diagnostic::Diagnostics;
use skeleton::Skeleton;

/// Collect the import of a Markdown diagram: the model's admission and every Markdown error.
pub type MarkdownImport = Import<MarkdownLanguage>;

/// Split a Markdown import into its model, withheld while any diagnostic is an error, its Markdown errors and the model's report.
pub type MarkdownImportParts = ImportParts<MarkdownLanguage>;

/// Collect the parse of a Markdown diagram: its document, every item line and heading parsed, and every error found on the way.
#[derive(Debug)]
pub struct MarkdownParse {
    document: Document,
    pub errors: Vec<MarkdownError>,
}

impl MarkdownParse {
    /// Read the skeleton of `source`, then parse its frontmatter, headings and item lines; broken parts are reported, never fatal.
    pub fn parse(source: &str) -> Self {
        let mut diagnostics = Diagnostics::default();
        let skeleton = Skeleton::read(source, &mut diagnostics);
        let document = Document::parse(skeleton, &mut diagnostics);
        let (errors, _no_warnings) = diagnostics.into_sorted_parts();
        Self { document, errors }
    }
}
