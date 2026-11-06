//! Identify an element by its full path, with segments escaped so any segment content round-trips.

use std::fmt;
use std::iter::Peekable;
use std::str::{CharIndices, FromStr};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

const SEPARATOR: char = '.';
const QUOTE: char = '"';
const ESCAPE: char = '\\';

/// Identify an element by its path from the top level down; the empty path is the root level, never an element.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementPath {
    segments: Vec<String>,
}

impl ElementPath {
    pub fn root() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn from_segments<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let segments = segments.into_iter().map(Into::into).collect();
        Self { segments }
    }

    pub fn child(&self, segment: impl Into<String>) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment.into());
        Self { segments }
    }

    /// Return the path one level up, or `None` for the root.
    pub fn parent(&self) -> Option<Self> {
        let (_last_segment, parent_segments) = self.segments.split_last()?;
        let parent = Self::from_segments(parent_segments.iter().cloned());
        Some(parent)
    }

    /// Tell whether `self` is a proper prefix of `other`.
    pub fn is_ancestor_of(&self, other: &Self) -> bool {
        let is_shorter = self.segments.len() < other.segments.len();
        is_shorter && other.segments.starts_with(&self.segments)
    }

    /// Return the longest common prefix of both paths.
    pub fn lowest_common_ancestor(&self, other: &Self) -> Self {
        let shared_segments = self
            .segments
            .iter()
            .zip(&other.segments)
            .take_while(|(own, others)| own == others)
            .map(|(own, _others)| own.clone());
        Self::from_segments(shared_segments)
    }

    /// Yield every proper non-root prefix, outermost first: the elements that must exist for this one to exist.
    pub fn ancestors(&self) -> impl Iterator<Item = Self> + '_ {
        let proper_prefix_lengths = 1..self.segments.len();
        proper_prefix_lengths.map(|length| {
            let prefix_segments = self.segments[..length].iter().cloned();
            Self::from_segments(prefix_segments)
        })
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Return the last segment, the element's own id, or `None` for the root.
    pub fn name(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }
}

impl fmt::Display for ElementPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            let is_after_first = index > 0;
            if is_after_first {
                formatter.write_str(".")?;
            }
            Self::write_segment(formatter, segment)?;
        }
        Ok(())
    }
}

impl ElementPath {
    /// Write a segment bare when it parses back unchanged, otherwise quoted.
    fn write_segment(formatter: &mut fmt::Formatter<'_>, segment: &str) -> fmt::Result {
        let is_empty = segment.is_empty();
        let has_reserved_character = segment.contains([SEPARATOR, QUOTE, ESCAPE]);
        let needs_quotes = is_empty || has_reserved_character;
        if needs_quotes {
            return Self::write_quoted_segment(formatter, segment);
        }
        formatter.write_str(segment)
    }

    /// Write a segment between `"` with every `"` and `\` backslash-escaped.
    fn write_quoted_segment(formatter: &mut fmt::Formatter<'_>, segment: &str) -> fmt::Result {
        formatter.write_str("\"")?;
        for character in segment.chars() {
            let is_escaped = character == QUOTE || character == ESCAPE;
            if is_escaped {
                formatter.write_str("\\")?;
            }
            write!(formatter, "{character}")?;
        }
        formatter.write_str("\"")
    }
}

impl FromStr for ElementPath {
    type Err = PathParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let is_empty = text.is_empty();
        if is_empty {
            return Err(PathParseError::Empty);
        }
        let mut parser = PathParser::new(text);
        parser.parse_path()
    }
}

/// Reason a string is not a well-formed element path.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PathParseError {
    #[error("an element path must have at least one segment")]
    Empty,
    #[error("empty segment at byte {offset}; quote it as \"\" to mean an empty id")]
    EmptySegment { offset: usize },
    #[error("unexpected `{character}` at byte {offset}; quote the segment to use it")]
    UnexpectedCharacter { character: char, offset: usize },
    #[error("unterminated quoted segment starting at byte {offset}")]
    UnterminatedQuote { offset: usize },
    #[error("invalid escape `\\{character}` at byte {offset}; only `\\\"` and `\\\\` are allowed")]
    InvalidEscape { character: char, offset: usize },
}

/// Parse the escaped display form of a path back into its segments.
struct PathParser<'a> {
    characters: Peekable<CharIndices<'a>>,
    length: usize,
}

impl<'a> PathParser<'a> {
    fn new(text: &'a str) -> Self {
        let characters = text.char_indices().peekable();
        Self {
            characters,
            length: text.len(),
        }
    }

    fn parse_path(&mut self) -> Result<ElementPath, PathParseError> {
        let mut segments = Vec::new();
        loop {
            let segment = self.parse_segment()?;
            segments.push(segment);
            let Some((offset, character)) = self.characters.next() else {
                return Ok(ElementPath { segments });
            };
            let is_unexpected = character != SEPARATOR;
            if is_unexpected {
                return Err(PathParseError::UnexpectedCharacter { character, offset });
            }
        }
    }

    fn parse_segment(&mut self) -> Result<String, PathParseError> {
        let Some(&(opening_offset, QUOTE)) = self.characters.peek() else {
            return self.parse_bare_segment();
        };
        self.characters.next();
        self.parse_quoted_segment(opening_offset)
    }

    fn parse_quoted_segment(&mut self, opening_offset: usize) -> Result<String, PathParseError> {
        let mut segment = String::new();
        loop {
            let Some((offset, character)) = self.characters.next() else {
                return Err(PathParseError::UnterminatedQuote {
                    offset: opening_offset,
                });
            };
            let is_closing_quote = character == QUOTE;
            if is_closing_quote {
                return Ok(segment);
            }
            let is_escape = character == ESCAPE;
            if is_escape {
                let escaped = self.parse_escaped_character(offset)?;
                segment.push(escaped);
                continue;
            }
            segment.push(character);
        }
    }

    fn parse_escaped_character(&mut self, escape_offset: usize) -> Result<char, PathParseError> {
        let Some((_offset, character)) = self.characters.next() else {
            return Err(PathParseError::UnterminatedQuote {
                offset: escape_offset,
            });
        };
        let is_valid_escape = character == QUOTE || character == ESCAPE;
        if is_valid_escape {
            return Ok(character);
        }
        Err(PathParseError::InvalidEscape {
            character,
            offset: escape_offset,
        })
    }

    fn parse_bare_segment(&mut self) -> Result<String, PathParseError> {
        let start_offset = self.peek_offset();
        let mut segment = String::new();
        while let Some(&(offset, character)) = self.characters.peek() {
            let is_separator = character == SEPARATOR;
            if is_separator {
                break;
            }
            let is_reserved = character == QUOTE || character == ESCAPE;
            if is_reserved {
                return Err(PathParseError::UnexpectedCharacter { character, offset });
            }
            segment.push(character);
            self.characters.next();
        }
        let is_empty = segment.is_empty();
        if is_empty {
            return Err(PathParseError::EmptySegment {
                offset: start_offset,
            });
        }
        Ok(segment)
    }

    fn peek_offset(&mut self) -> usize {
        let next_offset = self.characters.peek().map(|&(offset, _character)| offset);
        next_offset.unwrap_or(self.length)
    }
}

impl Serialize for ElementPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ElementPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(ElementPathVisitor)
    }
}

/// Parse a path from a JSON string, map key included.
struct ElementPathVisitor;

impl Visitor<'_> for ElementPathVisitor {
    type Value = ElementPath;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an element path such as `backend.api` or `backend.\"v1.2\"`")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<ElementPath, E> {
        text.parse().map_err(E::custom)
    }
}
