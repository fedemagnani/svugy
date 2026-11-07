//! Import a diagram written in the svugy language, a plain Markdown file, into the model, reporting every error in one pass.

mod ast;
mod diagnostic;
mod frontmatter;
mod lex;
mod lower;
mod parse;
mod property;
mod resolve;
mod skeleton;
mod style;

pub use diagnostic::LanguageError;
pub use lower::{SvugyImport, SvugyImportParts};

use ast::Document;
use diagnostic::Diagnostics;
use skeleton::Skeleton;

/// Collect the parse of a diagram file: its document, every item line parsed, and every error found on the way.
#[derive(Debug)]
pub struct SvugyParse {
    document: Document,
    pub errors: Vec<LanguageError>,
}

impl SvugyParse {
    /// Read the skeleton of `source`, then parse its frontmatter and item lines; broken items are dropped and reported, never fatal.
    pub fn parse(source: &str) -> Self {
        let mut diagnostics = Diagnostics::default();
        let skeleton = Skeleton::read(source, &mut diagnostics);
        let document = Document::parse(skeleton, &mut diagnostics);
        let errors = diagnostics.into_sorted_errors();
        Self { document, errors }
    }
}
