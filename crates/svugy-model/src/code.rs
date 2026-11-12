//! Parse an element's `code` value into a repo-relative file and an optional symbol, and tell whether a file's text spells that symbol.

use thiserror::Error;

/// Separate the file from the symbol in a `code` value.
const SYMBOL_SEPARATOR: char = '#';
/// Separate the segments of a symbol path written `Type::name`.
const PATH_SEPARATOR: &str = "::";
/// Separate the segments of a symbol path written `Type.name`.
const MEMBER_SEPARATOR: char = '.';
/// Separate the directories of a repo-relative file.
const DIRECTORY_SEPARATOR: char = '/';
/// Name the parent directory, which would climb out of the repository.
const PARENT_DIRECTORY: &str = "..";

/// Names a place in the source tree: a file relative to the repository root and, after `#`, a symbol the file spells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeReference<'a> {
    pub file: &'a str,
    pub symbol: Option<&'a str>,
}

/// Explain why a `code` value names no place in the repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum MalformedCodeReason {
    #[error("it names no file before `#`")]
    EmptyFile,
    #[error("its file starts with `/`; write it relative to the repository root")]
    AbsoluteFile,
    #[error("its file climbs out with `..`; write it relative to the repository root")]
    ParentDirectory,
    #[error(
        "its symbol has an empty segment: write `#NAME`, `#Type::NAME` or `#Type.NAME`, or drop the `#` to point at the whole file"
    )]
    EmptySymbolSegment,
}

impl<'a> CodeReference<'a> {
    /// Parse `raw`, written `FILE` or `FILE#SYMBOL`, rejecting a file that is empty, absolute or climbs out with `..`, and a symbol with an empty segment.
    pub fn parse(raw: &'a str) -> Result<Self, MalformedCodeReason> {
        let (file, symbol) = match raw.split_once(SYMBOL_SEPARATOR) {
            Some((file, symbol)) => (file, Some(symbol)),
            None => (raw, None),
        };
        let is_empty_file = file.is_empty();
        if is_empty_file {
            return Err(MalformedCodeReason::EmptyFile);
        }
        let is_absolute_file = file.starts_with(DIRECTORY_SEPARATOR);
        if is_absolute_file {
            return Err(MalformedCodeReason::AbsoluteFile);
        }
        let mut directories = file.split(DIRECTORY_SEPARATOR);
        let is_climbing_out = directories.any(|directory| directory == PARENT_DIRECTORY);
        if is_climbing_out {
            return Err(MalformedCodeReason::ParentDirectory);
        }
        let code_reference = Self { file, symbol };
        let mut symbol_segments = code_reference.symbol_segments();
        let has_empty_segment = symbol_segments.any(str::is_empty);
        if has_empty_segment {
            return Err(MalformedCodeReason::EmptySymbolSegment);
        }
        Ok(code_reference)
    }

    /// Tell whether `file_text` spells every segment of the symbol as a whole identifier, trivially true without a symbol.
    pub fn is_symbol_spelled_in(&self, file_text: &str) -> bool {
        let mut symbol_segments = self.symbol_segments();
        symbol_segments.all(|segment| Self::is_spelled_as_identifier(file_text, segment))
    }

    /// List the segments of the symbol, split at `::` and `.`; none without a symbol.
    fn symbol_segments(&self) -> impl Iterator<Item = &'a str> {
        let path_segments = self
            .symbol
            .into_iter()
            .flat_map(|symbol| symbol.split(PATH_SEPARATOR));
        path_segments.flat_map(|path_segment| path_segment.split(MEMBER_SEPARATOR))
    }

    /// Tell whether `text` holds `segment` with no identifier character glued to either side.
    fn is_spelled_as_identifier(text: &str, segment: &str) -> bool {
        let mut occurrences = text.match_indices(segment);
        occurrences.any(|(start, _occurrence)| {
            let end = start + segment.len();
            let character_before = text[..start].chars().next_back();
            let character_after = text[end..].chars().next();
            let is_open_before = !character_before.is_some_and(Self::is_identifier_character);
            let is_open_after = !character_after.is_some_and(Self::is_identifier_character);
            is_open_before && is_open_after
        })
    }

    /// Tell whether `character` may continue an identifier in the common languages: a letter, a digit or `_`.
    fn is_identifier_character(character: char) -> bool {
        character.is_alphanumeric() || character == '_'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_value_names_a_repo_relative_file_and_an_optional_symbol_path() {
        let cases = [
            ("src/lib.rs", Ok(("src/lib.rs", None))),
            (
                "src/model.rs#Model::admit",
                Ok(("src/model.rs", Some("Model::admit"))),
            ),
            (
                "app/view.py#View.render",
                Ok(("app/view.py", Some("View.render"))),
            ),
            ("#Model", Err(MalformedCodeReason::EmptyFile)),
            ("/etc/passwd", Err(MalformedCodeReason::AbsoluteFile)),
            (
                "src/../../secret.rs",
                Err(MalformedCodeReason::ParentDirectory),
            ),
            ("src/lib.rs#", Err(MalformedCodeReason::EmptySymbolSegment)),
            (
                "src/lib.rs#Model::",
                Err(MalformedCodeReason::EmptySymbolSegment),
            ),
        ];
        for (raw, expected) in cases {
            let parsed = CodeReference::parse(raw);
            let file_and_symbol = parsed.map(|reference| (reference.file, reference.symbol));
            assert_eq!(file_and_symbol, expected, "{raw:?}");
        }
    }

    #[test]
    fn a_symbol_is_spelled_only_when_each_segment_stands_as_a_whole_identifier() {
        let file_text = "impl Model {\n    pub fn admit(self) {}\n    fn admitted_parts() {}\n}";
        let cases = [
            ("src/model.rs", true),
            ("src/model.rs#Model", true),
            ("src/model.rs#Model::admit", true),
            ("src/model.rs#Model.admitted_parts", true),
            ("src/model.rs#Mode", false),
            ("src/model.rs#Model::adm", false),
            ("src/model.rs#Model::reject", false),
        ];
        for (raw, expected_spelled) in cases {
            let code_reference = CodeReference::parse(raw).expect("well-formed code value");
            let is_spelled = code_reference.is_symbol_spelled_in(file_text);
            assert_eq!(is_spelled, expected_spelled, "{raw:?}");
        }
    }
}
