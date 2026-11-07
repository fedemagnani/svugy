//! Import a d2 v0.9.0 file into the model through svugy's v1 d2 subset and its `vars: { svugy: { … } }` carrier, reporting every syntax error and ignored construct.

mod ast;
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
pub use diagnostic::{Construct, LabelledDiagnostic, SyntaxError, UnsupportedConstruct};
pub use lower::D2Import;

use diagnostic::Diagnostics;
use lex::Token;
use parse::SyntaxStatement;
use subset::SubsetFilter;

/// Collect what parsing a d2 file yields: its partial document, every syntax error and every ignored construct.
#[derive(Debug, Clone, PartialEq)]
pub struct D2Parse {
    document: D2Document,
    errors: Vec<SyntaxError>,
    warnings: Vec<UnsupportedConstruct>,
}

impl D2Parse {
    /// Lex, parse and filter `source` to the v1 subset; broken statements are dropped and reported, never fatal.
    pub fn parse(source: &str) -> Self {
        let mut diagnostics = Diagnostics::default();
        let tokens = Token::lex_all(source, &mut diagnostics);
        let syntax_statements = SyntaxStatement::parse_all(&tokens, source, &mut diagnostics);
        let document = SubsetFilter::filter_document(syntax_statements, &mut diagnostics);
        diagnostics.sort_by_source_offset();
        let (errors, warnings) = diagnostics.into_parts();
        Self {
            document,
            errors,
            warnings,
        }
    }

    /// Return the partial document, every broken statement dropped.
    pub fn document(&self) -> &D2Document {
        &self.document
    }

    /// Return every syntax error, in source order.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    /// Return every warning: each ignored construct, in source order.
    pub fn warnings(&self) -> &[UnsupportedConstruct] {
        &self.warnings
    }
}
