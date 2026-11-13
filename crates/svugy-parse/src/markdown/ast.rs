//! Hold a diagram file as written: its frontmatter, its component tree and its workflows, each value spanned in the source.

use std::fmt;

use chumsky::span::{SimpleSpan, Spanned};
use svugy_model::model::ViewAngle;

/// Span a piece of the diagram file as a byte range of its source.
pub(crate) type Span = SimpleSpan<usize>;

/// Hold the document a diagram file writes, every item line and heading parsed but nothing resolved.
#[derive(Debug, Default)]
pub(crate) struct Document {
    pub(crate) view: Option<ViewAngle>,
    pub(crate) styles: Vec<StyleDefinition>,
    pub(crate) components: Vec<ComponentItem>,
    pub(crate) workflows: Vec<WorkflowSection>,
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
    pub(crate) description: Option<Description>,
    pub(crate) children: Vec<ComponentItem>,
    pub(crate) span: Span,
}

/// Declare a workflow by its heading `### ID: TITLE [(subtitle=TEXT)]`, with its prose, its overrides and its steps.
#[derive(Debug)]
pub(crate) struct WorkflowSection {
    pub(crate) id: Spanned<String>,
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) prose: Option<Description>,
    pub(crate) overrides: Vec<OverrideItem>,
    pub(crate) steps: Vec<StepSection>,
}

/// Declare a step by its heading `#### ID: TITLE`, with its prose and its items.
#[derive(Debug)]
pub(crate) struct StepSection {
    pub(crate) id: Spanned<String>,
    pub(crate) title: String,
    pub(crate) prose: Option<Description>,
    pub(crate) items: Vec<StepItem>,
}

/// Override how a workflow draws a component by the item line `REF (PROPERTIES)`.
#[derive(Debug)]
pub(crate) struct OverrideItem {
    pub(crate) reference: Reference,
    pub(crate) entries: Vec<Entry>,
    pub(crate) span: Span,
}

/// List one item of a step: a component, an edge declared again by `[n]`, a new edge, or a chain of new edges.
#[derive(Debug)]
pub(crate) enum StepItem {
    Element(Reference),
    ListedEdge(ListedEdge),
    Edge(EdgeItem),
    Chain(ChainItem),
}

/// List again the edge `FROM -> TO[n]` some step of the workflow declares.
#[derive(Debug)]
pub(crate) struct ListedEdge {
    pub(crate) from: Reference,
    pub(crate) to: Reference,
    pub(crate) ordinal: u32,
    pub(crate) span: Span,
}

/// Declare one edge by the item line `REF -> REF[: LABEL] [(PROPERTIES)]`, with the description under it.
#[derive(Debug)]
pub(crate) struct EdgeItem {
    pub(crate) from: Reference,
    pub(crate) to: Reference,
    pub(crate) label: Option<Spanned<String>>,
    pub(crate) entries: Vec<Entry>,
    pub(crate) description: Option<Description>,
}

/// Chain two or more new plain edges by the item line `REF -> REF -> REF (-> REF)*`, one per hop.
#[derive(Debug)]
pub(crate) struct ChainItem {
    pub(crate) start: Reference,
    pub(crate) first_hop: Reference,
    pub(crate) second_hop: Reference,
    pub(crate) later_hops: Vec<Reference>,
}

/// Hold a description as stored, a description block's content or a heading's prose, with the span it was written at.
#[derive(Debug)]
pub(crate) struct Description {
    pub(crate) text: String,
    pub(crate) span: Span,
}

/// Reference a component by a dotted suffix of its path.
#[derive(Debug, Clone)]
pub(crate) struct Reference {
    pub(crate) segments: Vec<String>,
    pub(crate) span: Span,
}

/// Write one entry of a properties list: a style name, which may be a label's unquoted end when the list follows a `:` and holds no `=`, or `key=value`.
#[derive(Debug, Clone)]
pub(crate) enum Entry {
    Style {
        name: Spanned<String>,
        may_end_label: bool,
    },
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
            Self::Style {
                name,
                may_end_label,
            } => {
                let relabelled_name = Spanned {
                    inner: name.inner,
                    span,
                };
                Self::Style {
                    name: relabelled_name,
                    may_end_label,
                }
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

impl Reference {
    /// Read a reference from its dotted spelling, as a `link` value writes it.
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
