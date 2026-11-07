//! Describe each problem validation reports, as a diagnostic with its severity, labels and did-you-mean help.

use std::fmt;

use miette::{Diagnostic, Severity, SourceSpan};
use thiserror::Error;

use crate::legend::DashKey;
use crate::model::{EdgeId, MAX_MAGNITUDE, PlacementHint, Position, Shape};
use crate::path::ElementPath;
use crate::style::{AnyStyleKey, StyleValueKind};

/// Report one problem: something the author must fix (an error) or should look at (a warning).
#[derive(Debug, Error, Diagnostic)]
pub enum Problem {
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingAncestor(MissingAncestor),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DanglingEdgeEndpoint(DanglingEdgeEndpoint),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateElement(DuplicateElement),
    #[error(transparent)]
    #[diagnostic(transparent)]
    AncestorDescendantEdge(AncestorDescendantEdge),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnknownStyleKey(UnknownStyleKey),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MismatchedStyleValue(MismatchedStyleValue),
    #[error(transparent)]
    #[diagnostic(transparent)]
    EmptyGridHint(EmptyGridHint),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UncountablePackedTracks(UncountablePackedTracks),
    #[error(transparent)]
    #[diagnostic(transparent)]
    ChildPlacementWithoutChildren(ChildPlacementWithoutChildren),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnboundedPosition(UnboundedPosition),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnresolvedLink(UnresolvedLink),
    #[error(transparent)]
    #[diagnostic(transparent)]
    IgnoredContainerShape(IgnoredContainerShape),
    #[error(transparent)]
    #[diagnostic(transparent)]
    ImpossibleHeight(ImpossibleHeight),
    #[error(transparent)]
    #[diagnostic(transparent)]
    IgnoredContainerHeight(IgnoredContainerHeight),
    #[error(transparent)]
    #[diagnostic(transparent)]
    BadgeEdgeOnContainer(BadgeEdgeOnContainer),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedLegendDash(MalformedLegendDash),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UndrawnLegendKey(UndrawnLegendKey),
}

/// Report an element whose ancestor is missing: its enclosing container was never declared.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` is inside `{ancestor}`, which is not an element")]
#[diagnostic(code(svugy::dangling_reference))]
pub struct MissingAncestor {
    pub element: ElementPath,
    pub ancestor: ElementPath,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Suggest the known name the author most likely meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion<T>(pub T);

impl<T: fmt::Display> Suggestion<T> {
    /// Suggest c* = argmin over c of levenshtein(lower(unknown), lower(c)), kept iff that distance ≤ ⌊len(unknown) / 3⌋.
    pub fn closest_to(unknown: &str, candidates: impl IntoIterator<Item = T>) -> Option<Self> {
        let lowercase_unknown = unknown.to_lowercase();
        let unknown_length = unknown.chars().count();
        let max_distance = unknown_length / 3;
        let distances = candidates.into_iter().map(|candidate| {
            let candidate_text = candidate.to_string();
            let lowercase_candidate = candidate_text.to_lowercase();
            let distance = strsim::levenshtein(&lowercase_unknown, &lowercase_candidate);
            (distance, candidate)
        });
        let (closest_distance, closest) =
            distances.min_by_key(|(distance, _candidate)| *distance)?;
        let is_too_far = closest_distance > max_distance;
        if is_too_far {
            return None;
        }
        let suggestion = Suggestion(closest);
        Some(suggestion)
    }
}

impl<T: fmt::Display> fmt::Display for Suggestion<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "did you mean `{}`?", self.0)
    }
}

/// Report a dangling edge endpoint: a path that is not an element.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` points to `{endpoint}`, which is not an element")]
#[diagnostic(code(svugy::dangling_reference))]
pub struct DanglingEdgeEndpoint {
    pub edge: EdgeId,
    pub endpoint: ElementPath,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("referenced here")]
    pub span: Option<SourceSpan>,
}

/// Report a duplicate element: one declared twice at the same path.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` is declared more than once")]
#[diagnostic(code(svugy::duplicate_id))]
pub struct DuplicateElement {
    pub element: ElementPath,
    #[label("declared again here")]
    pub span: Option<SourceSpan>,
}

/// Report an edge joining an element to its own ancestor or descendant, which layout leaves out.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` joins an element to its own ancestor, so layout leaves it out")]
#[diagnostic(code(svugy::ancestor_descendant_edge), severity(Warning))]
pub struct AncestorDescendantEdge {
    pub edge: EdgeId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report an unknown style key: one outside the v1 set.
#[derive(Debug, Error, Diagnostic)]
#[error("{owner} has style `{key}`, which is not a v1 style key")]
#[diagnostic(code(svugy::unknown_style))]
pub struct UnknownStyleKey {
    pub owner: StyleOwner,
    pub key: String,
    #[help]
    pub suggestion: Option<Suggestion<&'static str>>,
    #[label("styled here")]
    pub span: Option<SourceSpan>,
}

/// Identify the owner of a style entry: an element or an edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleOwner {
    Element(ElementPath),
    Edge(EdgeId),
}

impl fmt::Display for StyleOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(path) => write!(formatter, "element `{path}`"),
            Self::Edge(id) => write!(formatter, "edge `{id}`"),
        }
    }
}

/// Report a mismatched style value: a v1 style key set to a value of the wrong kind.
#[derive(Debug, Error, Diagnostic)]
#[error("{owner} sets style `{key}` to `{value}`, which is not {expected}")]
#[diagnostic(code(svugy::mismatched_style_value))]
pub struct MismatchedStyleValue {
    pub owner: StyleOwner,
    pub key: AnyStyleKey,
    pub value: String,
    pub expected: StyleValueKind,
    #[label("styled here")]
    pub span: Option<SourceSpan>,
}

/// Report an empty grid hint: zero columns, so no cell to place a child in.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` asks for a grid of 0 columns, which has no cell for any child")]
#[diagnostic(code(svugy::impossible_placement))]
pub struct EmptyGridHint {
    pub element: ElementPath,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a packing hint laying children in more tracks, columns or rows, than a packed level can count.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` packs its {child_count} children into {columns} columns and {rows} rows, but a packed level holds at most {max} of either",
    max = PlacementHint::MAX_PACKED_TRACKS
)]
#[diagnostic(code(svugy::impossible_placement))]
pub struct UncountablePackedTracks {
    pub element: ElementPath,
    pub child_count: usize,
    pub columns: usize,
    pub rows: usize,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a child placement hint on an element without children, which has nothing to place.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` has child placement `{child_placement:?}` but no children to place")]
#[diagnostic(code(svugy::impossible_placement), severity(Warning))]
pub struct ChildPlacementWithoutChildren {
    pub element: ElementPath,
    pub child_placement: PlacementHint,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a pinned position with a coordinate that is not a finite number within `MAX_MAGNITUDE` of its level's origin.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is pinned at ({}, {}), which is not within {MAX_MAGNITUDE} plan units of its level's origin",
    .position.x,
    .position.y
)]
#[diagnostic(code(svugy::impossible_placement))]
pub struct UnboundedPosition {
    pub element: ElementPath,
    pub position: Position,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report an unresolved link: one shaped like an element path that names no element, the likely typo of a drill-down target.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` links to `{link}`, which is not an element; write `./{link}` if it is a relative URL"
)]
#[diagnostic(code(svugy::unresolved_link), severity(Warning))]
pub struct UnresolvedLink {
    pub element: ElementPath,
    pub link: String,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("linked here")]
    pub span: Option<SourceSpan>,
}

/// Report a container's authored shape, which v1 ignores: a container is a slab, whose rectangular top holds its interior.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is a container, so it is drawn as a slab and its shape `{shape:?}` is ignored in v1"
)]
#[diagnostic(code(svugy::ignored_container_shape), severity(Warning))]
pub struct IgnoredContainerShape {
    pub element: ElementPath,
    pub shape: Shape,
    #[label("shaped here")]
    pub span: Option<SourceSpan>,
}

/// Report a height that is not a positive number within `MAX_MAGNITUDE` plan units, so no solid can stand that tall.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` stands {height} plan units tall, which is not a positive number no greater than {MAX_MAGNITUDE}"
)]
#[diagnostic(code(svugy::impossible_height))]
pub struct ImpossibleHeight {
    pub element: ElementPath,
    pub height: f64,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a container's authored height, which v1 ignores: a container is a slab, whose tag hangs from its low front face onto the ground.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is a container, so it is drawn as a slab and its height {height} is ignored in v1"
)]
#[diagnostic(code(svugy::ignored_container_height), severity(Warning))]
pub struct IgnoredContainerHeight {
    pub element: ElementPath,
    pub height: f64,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a badge edge with a container at either end, since a badge is drawn from a leaf's top face as a mini of a leaf.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` is a badge, which needs a leaf at both ends")]
#[diagnostic(code(svugy::badge_edge_on_container))]
pub struct BadgeEdgeOnContainer {
    pub edge: EdgeId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a legend dash key that is not a dash length spelled in its shortest form, so no arc's dash can match it.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "the legend describes dash `{dash}`, which is not a dash length in its shortest spelling, e.g. `5` or `1.5`"
)]
#[diagnostic(code(svugy::malformed_legend))]
pub struct MalformedLegendDash {
    pub dash: DashKey,
}

/// Report a key row the legend describes but the diagram never draws, so the description is never shown.
#[derive(Debug, Error, Diagnostic)]
#[error("the legend describes {key}, which the diagram never draws")]
#[diagnostic(code(svugy::undrawn_legend_key), severity(Warning))]
pub struct UndrawnLegendKey {
    pub key: LegendKey,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
}

/// Name a key row of the legend: the arcs of one dash length, or the badges of one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegendKey {
    Dash(DashKey),
    Badge(ElementPath),
}

impl fmt::Display for LegendKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dash(dash) => write!(formatter, "the arcs dashed `{dash}`"),
            Self::Badge(target) => write!(formatter, "the badges of `{target}`"),
        }
    }
}

/// Classify a diagnostic as an error, one that blocks a build or a load, when it says so or states no severity at all.
pub trait ErrorSeverity {
    /// Tell whether the diagnostic is an error.
    fn is_error(&self) -> bool;
}

impl<D: Diagnostic + ?Sized> ErrorSeverity for D {
    fn is_error(&self) -> bool {
        let severity = self.severity();
        let severity_or_error = severity.unwrap_or(Severity::Error);
        severity_or_error == Severity::Error
    }
}
