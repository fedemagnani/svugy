//! Join names or phrases in prose, the one way every importer's messages list them.

use std::fmt;

/// Join names or phrases in prose as `` `a`, `b` and `c` ``, closed by a conjunction.
pub(crate) struct ProseList;

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
