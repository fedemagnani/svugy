//! Report what a source language rejects or ignores, and gate the model on it, the same way for every language the crate reads.

use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

use miette::{Diagnostic, LabeledSpan, Severity, SourceSpan};
use svugy_model::model::Model;
use svugy_model::problem::Suggestion;
use svugy_model::validate::{ModelAdmission, ModelReport};

/// Name a source language by the code of its errors and the type of its warnings, and import its text.
pub trait SourceLanguage: fmt::Debug + Clone + PartialEq + Eq + Sized {
    /// Spell the stable diagnostic code of the language's errors.
    const ERROR_CODE: &'static str;
    /// Type the warnings the language raises beside its errors.
    type Warning: Diagnostic + Clone + PartialEq;

    /// Import `source_text` into its model admission, keeping every error and warning on the way.
    fn import(source_text: &str) -> Import<Self>;
}

/// Report a source error: text of the language `L` that cannot be read, left out of the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceError<L> {
    pub message: String,
    pub span: SourceSpan,
    language: PhantomData<L>,
}

/// Stand for the warnings of a language that raises none of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoWarning {}

/// Record the errors and warnings a language's layers raise, handed from one layer to the next.
#[derive(Debug)]
pub(crate) struct Diagnostics<L: SourceLanguage> {
    pub(crate) errors: Vec<SourceError<L>>,
    pub(crate) warnings: Vec<L::Warning>,
}

/// Collect the import of a source file: the model's admission, every error and every warning of the language.
#[derive(Debug)]
pub struct Import<L: SourceLanguage> {
    pub admission: ModelAdmission,
    pub errors: Vec<SourceError<L>>,
    pub warnings: Vec<L::Warning>,
}

/// Split an import into its model, withheld while any diagnostic is an error, its errors, its warnings and the model's report.
#[derive(Debug)]
pub struct ImportParts<L: SourceLanguage> {
    pub model: Option<Model>,
    pub errors: Vec<SourceError<L>>,
    pub warnings: Vec<L::Warning>,
    pub report: ModelReport,
}

/// Locate a labelled diagnostic by where its first label starts in the source.
pub trait LabelledDiagnostic {
    /// Return where the diagnostic's first label starts, or `None` when it has no label.
    fn first_label_offset(&self) -> Option<usize>;
}

impl<L> SourceError<L> {
    /// Create an error of `message` labelled at the byte range `span`.
    pub(crate) fn new(message: String, span: Range<usize>) -> Self {
        let span = SourceSpan::from(span);
        Self {
            message,
            span,
            language: PhantomData,
        }
    }
}

impl<L> fmt::Display for SourceError<L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl<L: fmt::Debug> Error for SourceError<L> {}

impl<L: SourceLanguage> Diagnostic for SourceError<L> {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        let code = Box::new(L::ERROR_CODE);
        Some(code)
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let label_text = Some("here".to_owned());
        let label = LabeledSpan::new_with_span(label_text, self.span);
        let labels = Box::new(std::iter::once(label));
        Some(labels)
    }
}

impl fmt::Display for NoWarning {
    fn fmt(&self, _formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

impl Error for NoWarning {}

impl Diagnostic for NoWarning {}

impl<L: SourceLanguage> Default for Diagnostics<L> {
    fn default() -> Self {
        Self::from_parts(Vec::new(), Vec::new())
    }
}

impl<L: SourceLanguage> Diagnostics<L> {
    /// Create diagnostics carrying on the `errors` and `warnings` an earlier layer recorded.
    pub(crate) fn from_parts(errors: Vec<SourceError<L>>, warnings: Vec<L::Warning>) -> Self {
        Self { errors, warnings }
    }

    /// Record an error of `message` at `span`.
    pub(crate) fn record_error(
        &mut self,
        message: impl Into<String>,
        span: impl Into<Range<usize>>,
    ) {
        let message = message.into();
        let byte_range = span.into();
        let error = SourceError::new(message, byte_range);
        self.errors.push(error);
    }

    /// Record an unknown name after `prefix` at `span`, suggesting the closest of `candidates`.
    pub(crate) fn record_unknown<T: fmt::Display>(
        &mut self,
        prefix: &str,
        unknown: &str,
        candidates: impl IntoIterator<Item = T>,
        span: impl Into<Range<usize>>,
    ) {
        let suggestion = Suggestion::closest_to(unknown, candidates);
        let message = match suggestion {
            Some(suggestion) => format!("{prefix} `{unknown}`; {suggestion}"),
            None => format!("{prefix} `{unknown}`"),
        };
        self.record_error(message, span);
    }

    /// Split the diagnostics into their errors and their warnings, each sorted by where it starts in the source, ties kept in recording order.
    pub(crate) fn into_sorted_parts(self) -> (Vec<SourceError<L>>, Vec<L::Warning>) {
        let Self {
            mut errors,
            mut warnings,
        } = self;
        errors.sort_by_key(|error| error.span.offset());
        warnings.sort_by_key(|warning| {
            let first_label_offset = warning.first_label_offset();
            first_label_offset.unwrap_or(usize::MAX)
        });
        (errors, warnings)
    }
}

impl<L: SourceLanguage> Import<L> {
    /// List every diagnostic in source order: errors, warnings and model problems alike, unlabelled ones last.
    pub fn diagnostics(&self) -> Vec<&dyn Diagnostic> {
        let errors = self.errors.iter().map(|error| error as &dyn Diagnostic);
        let warnings = self
            .warnings
            .iter()
            .map(|warning| warning as &dyn Diagnostic);
        let problems = self
            .admission
            .report
            .problems()
            .iter()
            .map(|problem| problem as &dyn Diagnostic);
        let mut diagnostics: Vec<&dyn Diagnostic> =
            errors.chain(warnings).chain(problems).collect();
        diagnostics.sort_by_key(|diagnostic| {
            let first_label_offset = diagnostic.first_label_offset();
            first_label_offset.unwrap_or(usize::MAX)
        });
        diagnostics
    }

    /// Return the admitted model only when no diagnostic is an error, so a diagram the author has yet to fix is never drawn.
    pub fn admitted_model(&self) -> Option<&Model> {
        let has_errors = self.has_errors();
        if has_errors {
            return None;
        }
        self.admission.model.as_ref()
    }

    /// Split the import into its parts, its model withheld as `admitted_model` withholds it.
    pub fn into_admitted_parts(self) -> ImportParts<L> {
        let has_errors = self.has_errors();
        let Self {
            admission,
            errors,
            warnings,
        } = self;
        let ModelAdmission { model, report } = admission;
        let admitted_model = if has_errors { None } else { model };
        ImportParts {
            model: admitted_model,
            errors,
            warnings,
            report,
        }
    }

    /// Tell whether any diagnostic is an error: an error of the language or of the model's checks.
    fn has_errors(&self) -> bool {
        let has_language_errors = !self.errors.is_empty();
        let has_model_errors = self.admission.report.has_errors();
        has_language_errors || has_model_errors
    }
}

impl<D: Diagnostic + ?Sized> LabelledDiagnostic for D {
    fn first_label_offset(&self) -> Option<usize> {
        let mut labels = self.labels().into_iter().flatten();
        let first_label = labels.next();
        first_label.map(|label| label.offset())
    }
}
