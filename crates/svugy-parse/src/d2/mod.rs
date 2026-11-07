//! Import a d2 v0.9.0 file into the model through svugy's v1 d2 subset and its `vars: { svugy: { … } }` carrier, reporting every syntax error and ignored construct.

mod ast;
mod carrier;
mod diagnostic;
mod lex;
mod lower;
mod parse;
mod subset;

/// Pin the d2 release (d2lang/d2, formerly terrastruct/d2; latest stable on 2026-11-06) whose grammar, keywords and compiler this importer and its golden files follow.
pub const GRAMMAR_VERSION: &str = "v0.9.0";

pub use ast::{
    D2Document, EdgeDeclaration, EdgeLink, EdgeOperator, Field, FieldAssignment, KeyPath,
    KeySegment, Scalar, ShapeDeclaration, Span, Statement, StyleProperty, SvugyProperty,
    SvugyValue,
};
pub use diagnostic::{Construct, D2Language, SyntaxError, UnsupportedConstruct};

use crate::diagnostic::{Import, ImportParts};
use diagnostic::Diagnostics;
use lex::Token;
use parse::SyntaxStatement;
use subset::SubsetFilter;

/// Collect what importing a d2 file yields: the model's admission, every syntax error, every construct the model does not hold.
pub type D2Import = Import<D2Language>;

/// Split a d2 import into its model, withheld while any diagnostic is an error, its syntax errors, its ignored constructs and the model's report.
pub type D2ImportParts = ImportParts<D2Language>;

/// Collect what parsing a d2 file yields: its partial document, every syntax error and every ignored construct.
#[derive(Debug, Clone, PartialEq)]
pub struct D2Parse {
    pub document: D2Document,
    pub errors: Vec<SyntaxError>,
    pub warnings: Vec<UnsupportedConstruct>,
}

impl D2Parse {
    /// Lex, parse and filter `source` to the v1 subset; broken statements are dropped and reported, never fatal.
    pub fn parse(source: &str) -> Self {
        let mut diagnostics = Diagnostics::default();
        let tokens = Token::lex_all(source, &mut diagnostics);
        let syntax_statements = SyntaxStatement::parse_all(&tokens, source, &mut diagnostics);
        let document = SubsetFilter::filter_document(syntax_statements, &mut diagnostics);
        let (errors, warnings) = diagnostics.into_sorted_parts();
        Self {
            document,
            errors,
            warnings,
        }
    }
}
