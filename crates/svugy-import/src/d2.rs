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
pub use diagnostic::{Construct, SyntaxError, UnsupportedConstruct};
pub use lower::D2Import;

use lex::Tokens;
use parse::SyntaxStatement;
use subset::SubsetFilter;

/// Collect what parsing a d2 file yields: its partial document, every syntax error and every ignored construct.
#[derive(Debug, Clone, PartialEq)]
pub struct D2Parse {
    pub document: D2Document,
    pub errors: Vec<SyntaxError>,
    pub warnings: Vec<UnsupportedConstruct>,
}

impl D2Document {
    /// Lex, parse and filter `source` to the v1 subset; broken statements are dropped and reported, never fatal.
    pub fn parse(source: &str) -> D2Parse {
        let Tokens {
            tokens,
            errors: lex_errors,
        } = Tokens::lex(source);
        let (syntax_statements, parse_errors) = SyntaxStatement::parse_all(&tokens, source);
        let (document, warnings, subset_errors) = SubsetFilter::filter_document(syntax_statements);
        let mut errors = lex_errors;
        errors.extend(parse_errors);
        errors.extend(subset_errors);
        errors.sort_by_key(|error| error.span.offset());
        D2Parse {
            document,
            errors,
            warnings,
        }
    }
}
