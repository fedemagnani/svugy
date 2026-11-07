//! Report what the d2 importer rejected (errors) and what it ignored as outside the v1 subset or the model (warnings).

use std::fmt;

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use super::ast::{EdgeOperator, Span};

/// Report a syntax error: source text d2 cannot read or compile, whose statement is dropped.
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
    Board(&'static str),
    Import,
    Glob,
    EdgeReference,
    SpecialShape(String),
    BlockString,
    Null,
    Array,
    ArrayMap,
    ReservedField(&'static str),
    FieldForm(String),
    DeclarationInEdgeMap,
    UnmappedShape(String),
    UnmappedDirection(String),
    ArrowheadForm(EdgeOperator),
    FieldOnEdge(&'static str),
    FieldOnRoot(&'static str),
    UnknownStyleKey(String),
}

/// Record the diagnostics, every error and warning, the importer's stages raise, handed from one stage to the next.
#[derive(Debug, Default)]
pub(crate) struct Diagnostics {
    errors: Vec<SyntaxError>,
    warnings: Vec<UnsupportedConstruct>,
}

impl Diagnostics {
    /// Create diagnostics carrying on the `errors` and `warnings` an earlier stage recorded.
    pub(crate) fn from_parts(
        errors: Vec<SyntaxError>,
        warnings: Vec<UnsupportedConstruct>,
    ) -> Self {
        Self { errors, warnings }
    }

    /// Record a warning that `construct` at `span` is ignored.
    pub(crate) fn record_warning(&mut self, construct: Construct, span: Span) {
        let span = SourceSpan::from(span);
        let warning = UnsupportedConstruct { construct, span };
        self.warnings.push(warning);
    }

    /// Record a syntax error of `message` at `span`.
    pub(crate) fn record_error(&mut self, message: impl Into<String>, span: Span) {
        let message = message.into();
        let span = SourceSpan::from(span);
        let error = SyntaxError { message, span };
        self.errors.push(error);
    }

    /// Sort the errors and the warnings each by where they start in the source, ties kept in recording order.
    pub(crate) fn sort_by_source_offset(&mut self) {
        self.errors.sort_by_key(|error| error.span.offset());
        self.warnings.sort_by_key(|warning| warning.span.offset());
    }

    /// Split the diagnostics into their errors and their warnings.
    pub(crate) fn into_parts(self) -> (Vec<SyntaxError>, Vec<UnsupportedConstruct>) {
        (self.errors, self.warnings)
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

/// Locate a labelled diagnostic by where its first label starts in the source.
pub trait LabelledDiagnostic {
    /// Return where the diagnostic's first label starts, or `None` when it has no label.
    fn first_label_offset(&self) -> Option<usize>;
}

impl<D: Diagnostic + ?Sized> LabelledDiagnostic for D {
    fn first_label_offset(&self) -> Option<usize> {
        let mut labels = self.labels().into_iter().flatten();
        let first_label = labels.next();
        first_label.map(|label| label.offset())
    }
}
