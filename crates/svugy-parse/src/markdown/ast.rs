//! Hold a diagram file as written: its frontmatter, its component tree, its edges and its legend, each value spanned in the source.

use std::fmt;
use std::ops::Range;

use chumsky::span::{SimpleSpan, Spanned};
use svugy_model::model::ViewAngle;

/// Span a piece of the diagram file as a byte range of its source.
pub(crate) type Span = SimpleSpan<usize>;

/// Hold the document a diagram file writes, every item line parsed but nothing resolved.
#[derive(Debug, Default)]
pub(crate) struct Document {
    pub(crate) title: Option<String>,
    pub(crate) subtitle: Option<String>,
    pub(crate) view: Option<ViewAngle>,
    pub(crate) description: Option<DescriptionBlock>,
    pub(crate) styles: Vec<StyleDefinition>,
    pub(crate) components: Vec<ComponentItem>,
    pub(crate) edges: Vec<EdgeItem>,
    pub(crate) legend: Option<LegendSection>,
}

/// Define a named style from the frontmatter as the entries of its properties list.
#[derive(Debug)]
pub(crate) struct StyleDefinition {
    pub(crate) name: Spanned<String>,
    pub(crate) entries: Vec<Entry>,
}

/// Declare a component by its item line `ID[: DISPLAY NAME] [(PROPERTIES)]`, with its description and the items nested under it.
#[derive(Debug)]
pub(crate) struct ComponentItem {
    pub(crate) id: Spanned<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) entries: Vec<Entry>,
    pub(crate) description: Option<DescriptionBlock>,
    pub(crate) children: Vec<ComponentItem>,
    pub(crate) span: Span,
}

/// Declare one edge per hop of the item line `REF -> REF ... [: LABEL] [(PROPERTIES)]`, with the description under it.
#[derive(Debug)]
pub(crate) struct EdgeItem {
    pub(crate) references: Vec<Reference>,
    pub(crate) label: Option<Spanned<String>>,
    pub(crate) properties: Option<Spanned<Vec<Entry>>>,
    pub(crate) description: Option<DescriptionBlock>,
}

/// Hold a description block's content as stored, with where each of its lines starts in the source, `None` where the line is not a suffix of its source line.
#[derive(Debug)]
pub(crate) struct DescriptionBlock {
    pub(crate) text: String,
    pub(crate) line_starts: Vec<Option<usize>>,
    pub(crate) span: Span,
}

/// Reference a component by a dotted suffix of its path.
#[derive(Debug, Clone)]
pub(crate) struct Reference {
    pub(crate) segments: Vec<String>,
    pub(crate) span: Span,
}

/// Hold the `## Legend` section: its title and its items in order.
#[derive(Debug)]
pub(crate) struct LegendSection {
    pub(crate) title: Option<String>,
    pub(crate) items: Vec<LegendItem>,
}

/// Write one legend item: a free line, or a key row describing what the diagram draws.
#[derive(Debug)]
pub(crate) enum LegendItem {
    Line(String),
    KeyRow(LegendKeyRow),
}

/// Describe what a key row's properties `(line=…[, target=REF])` draw.
#[derive(Debug)]
pub(crate) struct LegendKeyRow {
    pub(crate) entries: Vec<Entry>,
    pub(crate) description: String,
    pub(crate) span: Span,
}

/// Write one entry of a properties list: a style name, or `key=value`.
#[derive(Debug, Clone)]
pub(crate) enum Entry {
    Style(Spanned<String>),
    Assignment(Assignment),
}

/// Hold the assignment of a value, still unread, to a property key, still unchecked.
#[derive(Debug, Clone)]
pub(crate) struct Assignment {
    pub(crate) key: Spanned<String>,
    pub(crate) value: Spanned<Value>,
}

/// Spell a value as written: quoted, or bare pieces the commas between them split, which only `position` reads as one value.
#[derive(Debug, Clone)]
pub(crate) enum Value {
    Quoted(String),
    Bare(Vec<String>),
}

impl Entry {
    /// Label the entry, and each spanned part of it, at `span`.
    pub(crate) fn relabelled(self, span: Span) -> Self {
        match self {
            Self::Style(style_name) => {
                let relabelled_name = Spanned {
                    inner: style_name.inner,
                    span,
                };
                Self::Style(relabelled_name)
            }
            Self::Assignment(Assignment { key, value }) => {
                let relabelled_key = Spanned {
                    inner: key.inner,
                    span,
                };
                let relabelled_value = Spanned {
                    inner: value.inner,
                    span,
                };
                let relabelled_assignment = Assignment {
                    key: relabelled_key,
                    value: relabelled_value,
                };
                Self::Assignment(relabelled_assignment)
            }
        }
    }
}

impl DescriptionBlock {
    /// Return where `range`, a range of the text within one of its lines, is written in the source, else the whole block's span.
    pub(crate) fn source_span(&self, range: Range<usize>) -> Span {
        let text_before = &self.text[..range.start];
        let line_index = text_before.matches('\n').count();
        let line_text_start = text_before.rfind('\n').map_or(0, |newline| newline + 1);
        let line_source_start = self.line_starts.get(line_index).copied().flatten();
        let Some(line_source_start) = line_source_start else {
            return self.span;
        };
        let column = range.start - line_text_start;
        let start = line_source_start + column;
        let end = start + range.len();
        Span::from(start..end)
    }
}

impl Reference {
    /// Read a reference from its dotted spelling, as a `link` or `target` value writes it.
    pub(crate) fn from_dotted(dotted: &str, span: Span) -> Self {
        let segments = dotted.split('.').map(str::to_owned).collect();
        Self { segments, span }
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dotted = self.segments.join(".");
        formatter.write_str(&dotted)
    }
}
