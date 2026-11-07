//! Identify an element by its full path, with segments escaped so any segment content round-trips.

use std::fmt::{self, Write};
use std::iter::Peekable;
use std::str::{CharIndices, FromStr};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Separate a path's segments.
const SEPARATOR: char = '.';
/// Open and close a quoted segment.
const QUOTE: char = '"';
/// Escape the next character of a quoted segment.
const ESCAPE: char = '\\';
/// List the characters a quoted segment escapes and a bare segment cannot hold.
const ESCAPED_CHARACTERS: [char; 2] = [QUOTE, ESCAPE];

/// Identify an element by its path from the top level down; the empty path is the root level, never an element.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementPath {
    segments: Vec<String>,
}

impl ElementPath {
    /// Return the root path, the empty path of the top-level view.
    pub fn root() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Build a path from its segments, outermost first.
    pub fn from_segments<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let segments = segments.into_iter().map(Into::into).collect();
        Self { segments }
    }

    /// Return the path of the child `segment` one level below this path.
    pub fn child(&self, segment: impl Into<String>) -> Self {
        let mut segments = self.segments.clone();
        let segment = segment.into();
        segments.push(segment);
        Self { segments }
    }

    /// Return the level this path is drawn in: its parent, or the root for a top-level element and for the root itself.
    pub fn enclosing_level(&self) -> Self {
        let parent = self.parent();
        parent.unwrap_or_else(Self::root)
    }

    /// Return the path one level up, or `None` for the root.
    pub fn parent(&self) -> Option<Self> {
        let (_last_segment, parent_segment_slice) = self.segments.split_last()?;
        let parent_segments = parent_segment_slice.iter().cloned();
        let parent = Self::from_segments(parent_segments);
        Some(parent)
    }

    /// Tell whether `self` is a proper prefix of `other`.
    pub fn is_ancestor_of(&self, other: &Self) -> bool {
        let is_shorter = self.segments.len() < other.segments.len();
        let is_prefix = other.segments.starts_with(&self.segments);
        is_shorter && is_prefix
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
        proper_prefix_lengths.map(|depth| self.ancestor_or_self_at_depth(depth))
    }

    /// Return the prefix of `depth` segments: the ancestor at that depth, or the path itself at its own depth.
    pub fn ancestor_or_self_at_depth(&self, depth: usize) -> Self {
        let prefix_segments = self.segments[..depth].iter().cloned();
        Self::from_segments(prefix_segments)
    }

    /// Return this path truncated to at most `depth` segments: its ancestor at that depth, or itself when it is no deeper.
    pub fn truncated_to_depth(&self, depth: usize) -> Self {
        let own_depth = self.depth();
        let prefix_length = own_depth.min(depth);
        self.ancestor_or_self_at_depth(prefix_length)
    }

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
        formatter.write_char(QUOTE)?;
        for character in segment.chars() {
            let is_escaped = ESCAPED_CHARACTERS.contains(&character);
            if is_escaped {
                formatter.write_char(ESCAPE)?;
            }
            formatter.write_char(character)?;
        }
        formatter.write_char(QUOTE)
    }

    /// Tell whether this is the root path.
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Return the path's depth, its segment count: 0 for the root.
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Return the last segment, the element's own id, or `None` for the root.
    pub fn name(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    /// Return the path's segments, outermost first.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }
}

impl fmt::Display for ElementPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            let is_after_first = index > 0;
            if is_after_first {
                formatter.write_char(SEPARATOR)?;
            }
            Self::write_segment(formatter, segment)?;
        }
        Ok(())
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

/// Explain why a string fails to parse as an element path.
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
    end_offset: usize,
}

impl<'a> PathParser<'a> {
    /// Create a new parser at the start of `text`.
    fn new(text: &'a str) -> Self {
        let characters = text.char_indices().peekable();
        let end_offset = text.len();
        Self {
            characters,
            end_offset,
        }
    }

    /// Parse the whole text as separator-joined segments.
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

    /// Parse one segment, quoted when it opens with a quote, else bare.
    fn parse_segment(&mut self) -> Result<String, PathParseError> {
        let Some(&(opening_offset, QUOTE)) = self.characters.peek() else {
            return self.parse_bare_segment();
        };
        self.characters.next();
        self.parse_quoted_segment(opening_offset)
    }

    /// Parse a quoted segment's content up to its closing quote, unescaping as it goes.
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

    /// Parse the character an escape stands for, only a quote or an escape.
    fn parse_escaped_character(&mut self, escape_offset: usize) -> Result<char, PathParseError> {
        let Some((_offset, character)) = self.characters.next() else {
            return Err(PathParseError::UnterminatedQuote {
                offset: escape_offset,
            });
        };
        let is_valid_escape = ESCAPED_CHARACTERS.contains(&character);
        if is_valid_escape {
            return Ok(character);
        }
        Err(PathParseError::InvalidEscape {
            character,
            offset: escape_offset,
        })
    }

    /// Parse a non-empty bare segment up to the next separator.
    fn parse_bare_segment(&mut self) -> Result<String, PathParseError> {
        let start_offset = self.peek_offset();
        let mut segment = String::new();
        while let Some(&(offset, character)) = self.characters.peek() {
            let is_separator = character == SEPARATOR;
            if is_separator {
                break;
            }
            let is_reserved = ESCAPED_CHARACTERS.contains(&character);
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

    /// Return the byte offset of the next character, or the text's end.
    fn peek_offset(&mut self) -> usize {
        let next_offset = self.characters.peek().map(|&(offset, _character)| offset);
        next_offset.unwrap_or(self.end_offset)
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
