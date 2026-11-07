//! Represent the v1 subset of a d2 file as a tree of shapes, edges and fields, each carrying its source span.

use std::fmt;
use std::ops::Range;

use svugy_model::model::ByteRange;

/// Hold the top-level statements of a d2 file that fall inside the v1 subset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct D2Document {
    pub statements: Vec<Statement>,
}

/// Hold one statement: a shape, an edge chain, or a field on a shape.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Shape(ShapeDeclaration),
    Edge(EdgeDeclaration),
    Field(FieldAssignment),
}

/// Declare the shape at `path`, optionally labelled, with the statements of its nested map.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeDeclaration {
    pub path: KeyPath,
    pub label: Option<Scalar>,
    pub body: Vec<Statement>,
    pub span: Span,
}

/// Declare the edges of a chain `source -> target -> …`, sharing one optional label and the fields of its map.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDeclaration {
    pub source: KeyPath,
    pub links: Vec<EdgeLink>,
    pub label: Option<Scalar>,
    pub fields: Vec<Field>,
    pub span: Span,
}

/// Link the previous endpoint of an edge chain to `target` through `operator`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLink {
    pub operator: EdgeOperator,
    pub operator_span: Span,
    pub target: KeyPath,
}

/// Name the direction of an edge operator: `->`, `<-`, `<->` or `--` (any number of dashes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeOperator {
    Forward,
    Backward,
    Bidirectional,
    Undirected,
}

/// Assign a field to the shape at `target`; an empty target means the enclosing shape (or the root).
#[derive(Debug, Clone, PartialEq)]
pub struct FieldAssignment {
    pub target: KeyPath,
    pub field: Field,
    pub span: Span,
}

/// Hold the value of one reserved field of the v1 subset; `style.x: v` and `style: { x: v }` both yield `Style`, `vars: { svugy: { … } }` yields `Svugy`.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Shape(Scalar),
    Label(Scalar),
    Direction(Scalar),
    Link(Scalar),
    Style(Vec<StyleProperty>),
    Svugy(Vec<SvugyProperty>),
}

/// Set one `style` property, e.g. `fill: "#f4a261"`.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleProperty {
    pub name: KeySegment,
    pub value: Scalar,
    pub span: Span,
}

/// Carry one attribute d2 cannot say through the d2 variable `svugy`, addressed by the key segments below it, e.g. `["position", "x"]` for `svugy.position.x: 150`.
#[derive(Debug, Clone, PartialEq)]
pub struct SvugyProperty {
    pub attribute_path: Vec<String>,
    pub value: SvugyValue,
    pub span: Span,
}

/// Carry the value of one `svugy` attribute: a plain value, or a `[ … ]` list of plain values.
#[derive(Debug, Clone, PartialEq)]
pub enum SvugyValue {
    Scalar(Scalar),
    List { items: Vec<Scalar>, span: Span },
}

/// Address a shape by the dotted path of its key, e.g. `a.b."c.d"`, after climbing one scope per leading `_`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KeyPath {
    pub parent_references: usize,
    pub segments: Vec<KeySegment>,
    pub span: Span,
}

/// Name one segment of a key path, unquoted and unescaped, so a quoted `"a.b"` stays one segment.
#[derive(Debug, Clone, PartialEq)]
pub struct KeySegment {
    pub name: String,
    pub span: Span,
}

/// Carry a scalar value's decoded text; the model types it against the field it lands in.
#[derive(Debug, Clone, PartialEq)]
pub struct Scalar {
    pub text: String,
    pub span: Span,
}

/// Locate a byte range in the d2 source, convertible to both a model span and a miette span.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Name a d2 reserved keyword outside `style` (d2ast/keywords.go at v0.9.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keyword {
    Shape,
    Label,
    Direction,
    Link,
    Style,
    Vars,
    Class,
    Classes,
    Layers,
    Scenarios,
    Steps,
    Icon,
    Constraint,
    Tooltip,
    Near,
    Width,
    Height,
    Top,
    Left,
    GridRows,
    GridColumns,
    GridGap,
    VerticalGap,
    HorizontalGap,
    SourceArrowhead,
    TargetArrowhead,
}

impl Field {
    /// Name the keyword this field was written under.
    pub(crate) fn keyword(&self) -> Keyword {
        match self {
            Self::Shape(_shape) => Keyword::Shape,
            Self::Label(_label) => Keyword::Label,
            Self::Direction(_direction) => Keyword::Direction,
            Self::Link(_link) => Keyword::Link,
            Self::Style(_properties) => Keyword::Style,
            Self::Svugy(_properties) => Keyword::Vars,
        }
    }
}

impl SvugyProperty {
    /// Name the d2 variable whose entries carry what d2 cannot say, e.g. `vars: { svugy: { caption: … } }`.
    pub const VARIABLE: &str = "svugy";
}

impl Keyword {
    /// Spell every keyword as d2 does, the one table both directions read.
    const SPELLINGS: [(&'static str, Keyword); 26] = [
        ("shape", Self::Shape),
        ("label", Self::Label),
        ("direction", Self::Direction),
        ("link", Self::Link),
        ("style", Self::Style),
        ("vars", Self::Vars),
        ("class", Self::Class),
        ("classes", Self::Classes),
        ("layers", Self::Layers),
        ("scenarios", Self::Scenarios),
        ("steps", Self::Steps),
        ("icon", Self::Icon),
        ("constraint", Self::Constraint),
        ("tooltip", Self::Tooltip),
        ("near", Self::Near),
        ("width", Self::Width),
        ("height", Self::Height),
        ("top", Self::Top),
        ("left", Self::Left),
        ("grid-rows", Self::GridRows),
        ("grid-columns", Self::GridColumns),
        ("grid-gap", Self::GridGap),
        ("vertical-gap", Self::VerticalGap),
        ("horizontal-gap", Self::HorizontalGap),
        ("source-arrowhead", Self::SourceArrowhead),
        ("target-arrowhead", Self::TargetArrowhead),
    ];

    /// Recognise `name` as a keyword case-insensitively, as d2 does.
    pub(crate) fn recognise(name: &str) -> Option<Self> {
        let lowercase_name = name.to_ascii_lowercase();
        let mut spellings = Self::SPELLINGS.iter();
        let spelling = spellings.find(|(spelled_name, _keyword)| *spelled_name == lowercase_name);
        spelling.map(|(_spelled_name, keyword)| *keyword)
    }

    /// Spell the keyword's name as d2 does.
    pub(crate) fn name(self) -> &'static str {
        let mut spellings = Self::SPELLINGS.iter();
        let spelling = spellings.find(|(_spelled_name, keyword)| *keyword == self);
        let (spelled_name, _keyword) = spelling.expect("every keyword has a spelling");
        spelled_name
    }
}

impl EdgeOperator {
    /// Read the operator an arrow spells from its arrowheads, whatever its dash count.
    pub(crate) fn from_arrow(arrow: &str) -> Self {
        let has_source_arrowhead = arrow.starts_with('<');
        let has_target_arrowhead = arrow.ends_with('>');
        match (has_source_arrowhead, has_target_arrowhead) {
            (true, true) => Self::Bidirectional,
            (true, false) => Self::Backward,
            (false, true) => Self::Forward,
            (false, false) => Self::Undirected,
        }
    }

    /// Spell the operator's shortest d2 arrow.
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Forward => "->",
            Self::Backward => "<-",
            Self::Bidirectional => "<->",
            Self::Undirected => "--",
        }
    }
}

impl Span {
    /// Create a new span from byte `start` up to byte `end`.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Return the byte range the span covers.
    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}..{}", self.start, self.end)
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Self::new(range.start, range.end)
    }
}

impl From<Span> for ByteRange {
    fn from(span: Span) -> Self {
        ByteRange::new(span.start, span.end)
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(span: Span) -> Self {
        span.range().into()
    }
}

impl chumsky::span::Span for Span {
    type Context = ();
    type Offset = usize;

    fn new(_context: (), range: Range<usize>) -> Self {
        range.into()
    }

    fn context(&self) {}

    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }
}

impl Statement {
    /// Return the span of the whole statement.
    pub fn span(&self) -> Span {
        match self {
            Self::Shape(shape) => shape.span,
            Self::Edge(edge) => edge.span,
            Self::Field(assignment) => assignment.span,
        }
    }
}
