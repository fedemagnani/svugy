//! Report what the svugy language rejects in a diagram file, each error labelled at its source.

use std::fmt;

use miette::{Diagnostic, SourceSpan};
use svugy_model::problem::Suggestion;
use thiserror::Error;

use super::ast::Span;

/// Report a language error: a part of the diagram file the language cannot read, left out of the model.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(svugy::language), severity(Error))]
pub struct LanguageError {
    pub message: String,
    #[label("here")]
    pub span: SourceSpan,
}

/// Record the diagnostics, every language error the layers raise, handed from one layer to the next.
#[derive(Debug, Default)]
pub(crate) struct Diagnostics {
    errors: Vec<LanguageError>,
}

/// Join names or phrases in prose as `` `a`, `b` and `c` ``, closed by a conjunction.
pub(crate) struct ProseList;

impl Diagnostics {
    /// Create diagnostics carrying on the `errors` an earlier layer recorded.
    pub(crate) fn from_errors(errors: Vec<LanguageError>) -> Self {
        Self { errors }
    }

    /// Record an error of `message` at `span`.
    pub(crate) fn record(&mut self, message: impl Into<String>, span: Span) {
        let message = message.into();
        let byte_range = span.into_range();
        let span = SourceSpan::from(byte_range);
        let error = LanguageError { message, span };
        self.errors.push(error);
    }

    /// Record an unknown name after `prefix`, suggesting the closest of `candidates`.
    pub(crate) fn record_unknown<T: fmt::Display>(
        &mut self,
        prefix: &str,
        unknown: &str,
        candidates: impl IntoIterator<Item = T>,
        span: Span,
    ) {
        let suggestion = Suggestion::closest_to(unknown, candidates);
        let message = match suggestion {
            Some(suggestion) => format!("{prefix} `{unknown}`; {suggestion}"),
            None => format!("{prefix} `{unknown}`"),
        };
        self.record(message, span);
    }

    /// Split the diagnostics into their errors, sorted by where they start in the source, ties kept in recording order.
    pub(crate) fn into_sorted_errors(self) -> Vec<LanguageError> {
        let mut errors = self.errors;
        errors.sort_by_key(|error| error.span.offset());
        errors
    }
}

impl ProseList {
    /// Join `names`, each in backticks, with commas and `conjunction` before the last.
    pub(crate) fn join_quoted<T: fmt::Display>(names: &[T], conjunction: &str) -> String {
        let quoted_names: Vec<String> = names.iter().map(|name| format!("`{name}`")).collect();
        Self::join(&quoted_names, conjunction)
    }

    /// Join `phrases` with commas and `conjunction` before the last.
    pub(crate) fn join(phrases: &[String], conjunction: &str) -> String {
        let Some((last_phrase, leading_phrases)) = phrases.split_last() else {
            return String::new();
        };
        let is_single = leading_phrases.is_empty();
        if is_single {
            return last_phrase.clone();
        }
        let leading_list = leading_phrases.join(", ");
        format!("{leading_list} {conjunction} {last_phrase}")
    }
}
