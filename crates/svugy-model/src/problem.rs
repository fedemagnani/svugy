//! Describe each problem validation reports, as a diagnostic with its severity, labels and did-you-mean help.

use std::fmt;

use miette::{Diagnostic, Severity, SourceSpan};
use thiserror::Error;

use crate::code::MalformedCodeReason;
use crate::description::DescriptionOwner;
use crate::model::{EdgeId, MAX_MAGNITUDE, PlacementHint, Position, Shape};
use crate::path::ElementPath;
use crate::style::{AnyStyleKey, StyleValueKind};
use crate::workflow::{StepId, StepItem, WorkflowId};

/// Report one problem: something the author must fix (an error) or should look at (a warning).
#[derive(Debug, Error, Diagnostic)]
pub enum Problem {
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingAncestor(MissingAncestor),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateElement(DuplicateElement),
    #[error(transparent)]
    #[diagnostic(transparent)]
    NoWorkflow(NoWorkflow),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateWorkflow(DuplicateWorkflow),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedWorkflowId(MalformedWorkflowId),
    #[error(transparent)]
    #[diagnostic(transparent)]
    WorkflowWithoutSteps(WorkflowWithoutSteps),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateStep(DuplicateStep),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedStepId(MalformedStepId),
    #[error(transparent)]
    #[diagnostic(transparent)]
    StepWithoutItems(StepWithoutItems),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateStepItem(DuplicateStepItem),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnknownStepItem(UnknownStepItem),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnlistedEdge(UnlistedEdge),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DanglingEdgeEndpoint(DanglingEdgeEndpoint),
    #[error(transparent)]
    #[diagnostic(transparent)]
    LineageEdge(LineageEdge),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateOverride(DuplicateOverride),
    #[error(transparent)]
    #[diagnostic(transparent)]
    OverrideOfUnshownElement(OverrideOfUnshownElement),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnshownElement(UnshownElement),
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
    UnshownLinkTarget(UnshownLinkTarget),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedCodeReference(MalformedCodeReference),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnresolvedDescriptionTarget(UnresolvedDescriptionTarget),
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
}

/// Name where a component property was written: on the component itself, or in one workflow's override of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementSite {
    Declaration(ElementPath),
    Override {
        workflow: WorkflowId,
        element: ElementPath,
    },
}

/// Identify the owner of a style entry: a component site or a workflow edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleOwner {
    Element(ElementSite),
    Edge { workflow: WorkflowId, edge: EdgeId },
}

/// Tell why a description link's target doesn't resolve, and in which workflow when the reason is scoped to one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnresolvedTargetReason {
    NamesNothing,
    NamesNothingIn(WorkflowId),
    NamesSeveralEdgesIn(WorkflowId),
    IsNotShownBy(WorkflowId),
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

/// Report a duplicate element: one declared twice at the same path.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` is declared more than once")]
#[diagnostic(code(svugy::duplicate_id))]
pub struct DuplicateElement {
    pub element: ElementPath,
    #[label("declared again here")]
    pub span: Option<SourceSpan>,
}

/// Report a model without workflows, which draws nothing.
#[derive(Debug, Error, Diagnostic)]
#[error("the model has no workflow, so nothing is drawn")]
#[diagnostic(code(svugy::no_workflow))]
pub struct NoWorkflow;

/// Report a workflow id used by an earlier workflow.
#[derive(Debug, Error, Diagnostic)]
#[error("workflow `{workflow}` is declared more than once")]
#[diagnostic(code(svugy::duplicate_id))]
pub struct DuplicateWorkflow {
    pub workflow: WorkflowId,
    #[label("declared again here")]
    pub span: Option<SourceSpan>,
}

/// Report a workflow id that is not spelled like a component id.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "workflow id `{workflow}` is not an id: ASCII letters, digits, `_` and single inner `-`, starting with a letter or `_`"
)]
#[diagnostic(code(svugy::malformed_id))]
pub struct MalformedWorkflowId {
    pub workflow: WorkflowId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a workflow without steps, which shows nothing.
#[derive(Debug, Error, Diagnostic)]
#[error("workflow `{workflow}` has no step, so it shows nothing")]
#[diagnostic(code(svugy::empty_workflow))]
pub struct WorkflowWithoutSteps {
    pub workflow: WorkflowId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a step id used by an earlier step of the same workflow.
#[derive(Debug, Error, Diagnostic)]
#[error("step `{workflow}/{step}` is declared more than once")]
#[diagnostic(code(svugy::duplicate_id))]
pub struct DuplicateStep {
    pub workflow: WorkflowId,
    pub step: StepId,
    #[label("declared again here")]
    pub span: Option<SourceSpan>,
}

/// Report a step id that is not spelled like a component id.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "step id `{step}` of workflow `{workflow}` is not an id: ASCII letters, digits, `_` and single inner `-`, starting with a letter or `_`"
)]
#[diagnostic(code(svugy::malformed_id))]
pub struct MalformedStepId {
    pub workflow: WorkflowId,
    pub step: StepId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a step listing nothing.
#[derive(Debug, Error, Diagnostic)]
#[error("step `{workflow}/{step}` lists no edge or component")]
#[diagnostic(code(svugy::empty_step))]
pub struct StepWithoutItems {
    pub workflow: WorkflowId,
    pub step: StepId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report an item a step lists twice.
#[derive(Debug, Error, Diagnostic)]
#[error("step `{workflow}/{step}` lists {item} more than once")]
#[diagnostic(code(svugy::duplicate_step_item))]
pub struct DuplicateStepItem {
    pub workflow: WorkflowId,
    pub step: StepId,
    pub item: StepItem,
    #[label("in this step")]
    pub span: Option<SourceSpan>,
}

/// Report a step item naming no edge of its workflow or no element of the model.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "step `{workflow}/{step}` lists {item}, which names nothing in {}",
    Self::searched_scope(.item, .workflow)
)]
#[diagnostic(code(svugy::dangling_reference))]
pub struct UnknownStepItem {
    pub workflow: WorkflowId,
    pub step: StepId,
    pub item: StepItem,
    #[help]
    pub suggestion: Option<Suggestion<String>>,
    #[label("in this step")]
    pub span: Option<SourceSpan>,
}

/// Report a workflow edge no step lists, which is never drawn.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` of workflow `{workflow}` is listed by no step, so it is never drawn")]
#[diagnostic(code(svugy::unlisted_edge))]
pub struct UnlistedEdge {
    pub workflow: WorkflowId,
    pub edge: EdgeId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a dangling edge endpoint: a path that is not an element.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` of workflow `{workflow}` points to `{endpoint}`, which is not an element")]
#[diagnostic(code(svugy::dangling_reference))]
pub struct DanglingEdgeEndpoint {
    pub workflow: WorkflowId,
    pub edge: EdgeId,
    pub endpoint: ElementPath,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("referenced here")]
    pub span: Option<SourceSpan>,
}

/// Report an edge joining an element to itself, its ancestor or its descendant, which no layer can draw.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "edge `{edge}` of workflow `{workflow}` joins an element to itself or to its own ancestor, which no layer can draw"
)]
#[diagnostic(code(svugy::lineage_edge))]
pub struct LineageEdge {
    pub workflow: WorkflowId,
    pub edge: EdgeId,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a second override of one element in one workflow.
#[derive(Debug, Error, Diagnostic)]
#[error("workflow `{workflow}` overrides `{element}` more than once")]
#[diagnostic(code(svugy::duplicate_override))]
pub struct DuplicateOverride {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    #[label("overridden again here")]
    pub span: Option<SourceSpan>,
}

/// Report an override of an element the workflow doesn't show, which changes nothing.
#[derive(Debug, Error, Diagnostic)]
#[error("workflow `{workflow}` overrides `{element}`, which it doesn't show")]
#[diagnostic(code(svugy::unshown_override))]
pub struct OverrideOfUnshownElement {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("overridden here")]
    pub span: Option<SourceSpan>,
}

/// Report an element no workflow shows, which is never drawn.
#[derive(Debug, Error, Diagnostic)]
#[error("element `{element}` is shown by no workflow, so it is never drawn")]
#[diagnostic(code(svugy::unshown_element), severity(Warning))]
pub struct UnshownElement {
    pub element: ElementPath,
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
#[error(
    "workflow `{workflow}` asks `{element}` for a grid of 0 columns, which has no cell for any child"
)]
#[diagnostic(code(svugy::impossible_placement))]
pub struct EmptyGridHint {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    #[label("overridden here")]
    pub span: Option<SourceSpan>,
}

/// Report a packing hint laying children in more tracks, columns or rows, than a packed level can count.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "workflow `{workflow}` packs the {child_count} children of `{element}` into {columns} columns and {rows} rows, but a packed level holds at most {max} of either",
    max = PlacementHint::MAX_PACKED_TRACKS
)]
#[diagnostic(code(svugy::impossible_placement))]
pub struct UncountablePackedTracks {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    pub child_count: usize,
    pub columns: usize,
    pub rows: usize,
    #[label("overridden here")]
    pub span: Option<SourceSpan>,
}

/// Report a child placement hint on an element the workflow shows without children, which has nothing to place.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "workflow `{workflow}` gives `{element}` child placement `{child_placement:?}`, but shows no children of it"
)]
#[diagnostic(code(svugy::impossible_placement), severity(Warning))]
pub struct ChildPlacementWithoutChildren {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    pub child_placement: PlacementHint,
    #[label("overridden here")]
    pub span: Option<SourceSpan>,
}

/// Report a pinned position with a coordinate that is not a finite number within `MAX_MAGNITUDE` of its level's origin.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "workflow `{workflow}` pins `{element}` at ({}, {}), which is not within {MAX_MAGNITUDE} plan units of its level's origin",
    .position.x,
    .position.y
)]
#[diagnostic(code(svugy::impossible_placement))]
pub struct UnboundedPosition {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    pub position: Position,
    #[label("overridden here")]
    pub span: Option<SourceSpan>,
}

/// Report an unresolved link: one shaped like an element path that names no element, the likely typo of a drill-down target.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "{site} links to `{link}`, which is not an element; write `./{link}` if it is a relative URL"
)]
#[diagnostic(code(svugy::unresolved_link), severity(Warning))]
pub struct UnresolvedLink {
    pub site: ElementSite,
    pub link: String,
    #[help]
    pub suggestion: Option<Suggestion<ElementPath>>,
    #[label("linked here")]
    pub span: Option<SourceSpan>,
}

/// Report a link to an element a workflow doesn't show, so the linking element links nowhere there.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` links to `{target}`, which workflow `{workflow}` doesn't show, so it links nowhere there"
)]
#[diagnostic(code(svugy::unshown_link_target), severity(Warning))]
pub struct UnshownLinkTarget {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    pub target: ElementPath,
    #[label("linked here")]
    pub span: Option<SourceSpan>,
}

/// Report a `code` value that names no place in the repository, so no tree check could resolve it.
#[derive(Debug, Error, Diagnostic)]
#[error("{site} points at code `{code}`, but {reason}")]
#[diagnostic(code(svugy::malformed_code_reference))]
pub struct MalformedCodeReference {
    pub site: ElementSite,
    pub code: String,
    pub reason: MalformedCodeReason,
    #[label("pointed here")]
    pub span: Option<SourceSpan>,
}

/// Report a description link whose `#` target doesn't resolve, so the runtime shows its text unlinked.
#[derive(Debug, Error, Diagnostic)]
#[error("the description of {owner} links to `#{target}`, which {reason}")]
#[diagnostic(code(svugy::unresolved_description_target), severity(Warning))]
pub struct UnresolvedDescriptionTarget {
    pub owner: DescriptionOwner,
    pub target: String,
    pub reason: UnresolvedTargetReason,
    #[help]
    pub suggestion: Option<Suggestion<String>>,
    #[label("described here")]
    pub span: Option<SourceSpan>,
}

/// Report a container's shape, which a workflow ignores: there the element is a slab, whose rectangular top holds its interior.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is a container in workflow `{workflow}`, so it is drawn as a slab there and its shape `{shape:?}` is ignored"
)]
#[diagnostic(code(svugy::ignored_container_shape), severity(Warning))]
pub struct IgnoredContainerShape {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    pub shape: Shape,
    #[label("shaped here")]
    pub span: Option<SourceSpan>,
}

/// Report a height that is not a positive number within `MAX_MAGNITUDE` plan units, so no solid can stand that tall.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "{site} stands {height} plan units tall, which is not a positive number no greater than {MAX_MAGNITUDE}"
)]
#[diagnostic(code(svugy::impossible_height))]
pub struct ImpossibleHeight {
    pub site: ElementSite,
    pub height: f64,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a container's height, which a workflow ignores: there the element is a slab, whose tag hangs from its low front face.
#[derive(Debug, Error, Diagnostic)]
#[error(
    "element `{element}` is a container in workflow `{workflow}`, so it is drawn as a slab there and its height {height} is ignored"
)]
#[diagnostic(code(svugy::ignored_container_height), severity(Warning))]
pub struct IgnoredContainerHeight {
    pub workflow: WorkflowId,
    pub element: ElementPath,
    pub height: f64,
    #[label("declared here")]
    pub span: Option<SourceSpan>,
}

/// Report a badge edge with a container at either end in its workflow, since a badge is drawn from a leaf's top face as a mini of a leaf.
#[derive(Debug, Error, Diagnostic)]
#[error("edge `{edge}` of workflow `{workflow}` is a badge, which needs a leaf at both ends")]
#[diagnostic(code(svugy::badge_edge_on_container))]
pub struct BadgeEdgeOnContainer {
    pub workflow: WorkflowId,
    pub edge: EdgeId,
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

impl UnknownStepItem {
    /// Name where a step item is looked up: its workflow for an edge, the whole model for an element.
    fn searched_scope(item: &StepItem, workflow: &WorkflowId) -> String {
        match item {
            StepItem::Edge(_id) => format!("workflow `{workflow}`"),
            StepItem::Element(_path) => "the model".to_owned(),
        }
    }
}

impl fmt::Display for ElementSite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(path) => write!(formatter, "element `{path}`"),
            Self::Override { workflow, element } => write!(
                formatter,
                "the override of `{element}` in workflow `{workflow}`"
            ),
        }
    }
}

impl fmt::Display for StyleOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(site) => write!(formatter, "{site}"),
            Self::Edge { workflow, edge } => {
                write!(formatter, "edge `{edge}` of workflow `{workflow}`")
            }
        }
    }
}

impl fmt::Display for UnresolvedTargetReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamesNothing => formatter.write_str(
                "names no element `#PATH`, edge `#FROM->TO[n]` or step `#WORKFLOW/STEP`",
            ),
            Self::NamesNothingIn(workflow) => {
                write!(formatter, "names nothing in workflow `{workflow}`")
            }
            Self::NamesSeveralEdgesIn(workflow) => write!(
                formatter,
                "names several edges in workflow `{workflow}`; add `[n]` to pick one"
            ),
            Self::IsNotShownBy(workflow) => {
                write!(
                    formatter,
                    "names a component workflow `{workflow}` doesn't show"
                )
            }
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
