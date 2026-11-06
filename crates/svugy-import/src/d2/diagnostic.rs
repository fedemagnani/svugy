//! Report what the d2 importer rejected (errors) and what it ignored as outside the v1 subset or the model (warnings).

use std::fmt;

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use super::ast::EdgeOperator;

/// Reject source text d2 cannot read or compile; the statement it sits in is dropped.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(svugy::d2::syntax), severity(Error))]
pub struct SyntaxError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

/// Ignore a construct d2 accepts but the v1 subset or the model does not hold.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
#[error("{construct} is outside svugy's d2 subset and is ignored")]
#[diagnostic(code(svugy::d2::unsupported), severity(Warning))]
pub struct UnsupportedConstruct {
    pub construct: Construct,
    #[label("ignored")]
    pub span: SourceSpan,
}

/// Name the d2 construct that fell outside the v1 subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Construct {
    Variables,
    Classes,
    Board(String),
    Import,
    Glob,
    EdgeReference,
    SpecialShape(String),
    BlockString,
    Null,
    Array,
    ArrayMap,
    ReservedField(String),
    FieldForm(String),
    DeclarationInEdgeMap,
    UnmappedShape(String),
    UnmappedDirection(String),
    ArrowheadForm(EdgeOperator),
    FieldOnEdge(String),
    FieldOnRoot(String),
    UnknownStyleKey(String),
}

impl SyntaxError {
    pub(crate) fn new(message: String, span: impl Into<SourceSpan>) -> Self {
        let span = span.into();
        Self { message, span }
    }
}

impl UnsupportedConstruct {
    pub(crate) fn new(construct: Construct, span: impl Into<SourceSpan>) -> Self {
        let span = span.into();
        Self { construct, span }
    }
}

impl fmt::Display for Construct {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Variables => write!(formatter, "a variable (`vars`, `${{…}}`)"),
            Self::Classes => write!(formatter, "a class (`classes`, `class`)"),
            Self::Board(keyword) => write!(formatter, "a board (`{keyword}`)"),
            Self::Import => write!(formatter, "an import (`@…`)"),
            Self::Glob => write!(formatter, "a glob or filter (`*`, `&`, `!&`)"),
            Self::EdgeReference => write!(formatter, "an edge reference (`(a -> b)[0]`)"),
            Self::SpecialShape(shape) => write!(formatter, "the special shape `{shape}`"),
            Self::BlockString => write!(formatter, "a block string (`|md … |`)"),
            Self::Null => write!(formatter, "a `null` deletion"),
            Self::Array => write!(formatter, "an array value (`[…]`)"),
            Self::ArrayMap => write!(formatter, "a map inside an array (`[{{…}}]`)"),
            Self::ReservedField(keyword) => write!(formatter, "the field `{keyword}`"),
            Self::FieldForm(keyword) => write!(formatter, "this form of the field `{keyword}`"),
            Self::DeclarationInEdgeMap => write!(formatter, "a declaration inside an edge map"),
            Self::UnmappedShape(shape) => write!(formatter, "the shape `{shape}`"),
            Self::UnmappedDirection(direction) => write!(formatter, "`direction: {direction}`"),
            Self::ArrowheadForm(operator) => {
                let arrow = operator.arrow();
                write!(formatter, "the arrowheads of `{arrow}`")
            }
            Self::FieldOnEdge(keyword) => write!(formatter, "the field `{keyword}` on an edge"),
            Self::FieldOnRoot(keyword) => write!(formatter, "the field `{keyword}` on the root"),
            Self::UnknownStyleKey(key) => write!(formatter, "the style key `{key}`"),
        }
    }
}
