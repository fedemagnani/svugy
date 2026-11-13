//! Report what the d2 importer rejected (errors) and what it ignored as outside the v1 subset or the model (warnings).

use std::fmt;

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use super::ast::{EdgeOperator, Span};
use super::{D2Import, D2Parse};
use crate::diagnostic::{SourceError, SourceLanguage};

/// Mark d2 as a source language: its errors are syntax errors, its warnings the constructs outside the v1 subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D2Language;

/// Report a syntax error: source text d2 cannot read or compile, whose statement is dropped.
pub type SyntaxError = SourceError<D2Language>;

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
    Board(&'static str),
    Import,
    Glob,
    EdgeReference,
    SpecialShape(String),
    BlockString,
    Null,
    Array,
    ReservedField(&'static str),
    FieldForm(String),
    DeclarationInEdgeMap,
    UnmappedShape(String),
    UnmappedDirection(String),
    ArrowheadForm(EdgeOperator),
    FieldOnEdge(&'static str),
    FieldOnRoot(&'static str),
    UnknownStyleKey(String),
    ContainerBadge,
    LineageEdge,
}

/// Record the diagnostics of d2's stages, handed from one stage to the next.
pub(crate) type Diagnostics = crate::diagnostic::Diagnostics<D2Language>;

impl SourceLanguage for D2Language {
    const ERROR_CODE: &'static str = "svugy::d2::syntax";
    type Warning = UnsupportedConstruct;

    fn import(source_text: &str) -> D2Import {
        let parsed = D2Parse::parse(source_text);
        parsed.lower()
    }
}

impl Diagnostics {
    /// Record a warning that `construct` at `span` is ignored.
    pub(crate) fn record_warning(&mut self, construct: Construct, span: Span) {
        let span = SourceSpan::from(span);
        let warning = UnsupportedConstruct { construct, span };
        self.warnings.push(warning);
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
            Self::ContainerBadge => write!(formatter, "`svugy.badge` on a container"),
            Self::LineageEdge => write!(
                formatter,
                "an edge between a component and its own lineage, which svugy can't draw,"
            ),
        }
    }
}
