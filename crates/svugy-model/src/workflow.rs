//! Hold a model's workflows: each one tells, step by step, how the components it shows interact over its own edges.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;

use miette::SourceSpan;
use serde::{Deserialize, Serialize};

use crate::description::{DescriptionTarget, TargetLink};
use crate::model::{ByteRange, Edge, EdgeId, Element, PlacementHint, Position, Shape};
use crate::path::ElementPath;
use crate::properties::{ComponentProperties, SpannedText};
use crate::unique_key_map::UniqueKeyMapVisitor;

/// Identify a workflow among the model's workflows.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowId(String);

/// Identify a step among its workflow's steps.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepId(String);

/// Locate a step by its place in its workflow's written order, counted from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepIndex(usize);

/// Name what a step lists: an edge of its workflow, or a component shown without an edge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StepItem {
    Edge(EdgeId),
    Element(ElementPath),
}

/// Pair a step item with where an imported source listed it.
#[derive(Clone, Debug, PartialEq)]
pub struct ListedItem {
    pub(crate) item: StepItem,
    pub(crate) span: Option<ByteRange>,
}

/// Describe one step of a workflow: its id, title, prose and the items it lists, in written order.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    id: StepId,
    title: String,
    pub(crate) description: Option<String>,
    pub(crate) items: Vec<ListedItem>,
    span: Option<ByteRange>,
}

/// Describe a workflow: its id, title, subtitle and prose, the overrides it applies to the components it shows, its edges and its steps.
#[derive(Clone, Debug, PartialEq)]
pub struct Workflow {
    id: WorkflowId,
    title: String,
    subtitle: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) overrides: BTreeMap<ElementPath, ElementOverride>,
    pub(crate) edges: BTreeMap<EdgeId, Edge>,
    pub(crate) steps: Vec<Step>,
    span: Option<ByteRange>,
}

/// Override how one workflow draws a component: any component property, plus where it is pinned and how it places its children, which only a workflow sets.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "ElementOverrideDocument", into = "ElementOverrideDocument")]
pub struct ElementOverride {
    pub(crate) properties: ComponentProperties,
    child_placement: Option<PlacementHint>,
    position: Option<Position>,
    span: Option<ByteRange>,
}

/// Spell an override as model JSON: every field flat.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ElementOverrideDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<Shape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    style: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    child_placement: Option<PlacementHint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    position: Option<Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link_span: Option<ByteRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code_span: Option<ByteRange>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
}

impl WorkflowId {
    /// Create a new workflow id spelled `id`; validation checks its shape.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self(id)
    }

    /// Tell whether the id is spelled like a component id, the shape a contract attribute can carry.
    pub fn is_well_formed(&self) -> bool {
        ElementPath::is_component_id(&self.0)
    }

    /// Return the id as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl StepId {
    /// Create a new step id spelled `id`; validation checks its shape.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self(id)
    }

    /// Tell whether the id is spelled like a component id, the shape a contract attribute can carry.
    pub fn is_well_formed(&self) -> bool {
        ElementPath::is_component_id(&self.0)
    }

    /// Return the id as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl StepIndex {
    /// Create a new index naming the step at `position` in written order, from 0.
    pub fn new(position: usize) -> Self {
        Self(position)
    }

    /// Return the step's place in written order, from 0.
    pub fn position(self) -> usize {
        self.0
    }
}

impl ListedItem {
    /// Return the item listed.
    pub fn item(&self) -> &StepItem {
        &self.item
    }

    /// Return where in an imported source the item was listed, as a miette span.
    pub fn source_span(&self) -> Option<SourceSpan> {
        self.span.map(SourceSpan::from)
    }
}

impl Step {
    /// Create a new step named `id` and titled `title`, listing nothing yet.
    pub fn new(id: StepId, title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            id,
            title,
            description: None,
            items: Vec::new(),
            span: None,
        }
    }

    /// Set the Markdown describing what happens in this step.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(description);
        self
    }

    /// List `item` after the items already listed.
    pub fn with_item(mut self, item: StepItem) -> Self {
        let span = None;
        let listed_item = ListedItem { item, span };
        self.items.push(listed_item);
        self
    }

    /// Record where in an imported source this step was declared.
    pub fn with_span(mut self, span: ByteRange) -> Self {
        self.span = Some(span);
        self
    }

    /// Return the greatest depth among the elements and edge endpoints the step lists, 0 when it lists nothing.
    pub fn deepest_listed_depth(&self) -> usize {
        let listed_paths = self.items().flat_map(|item| match item {
            StepItem::Edge(id) => vec![id.from(), id.to()],
            StepItem::Element(path) => vec![path],
        });
        let listed_depths = listed_paths.map(ElementPath::depth);
        listed_depths.max().unwrap_or(0)
    }

    /// Return the step's id.
    pub fn id(&self) -> &StepId {
        &self.id
    }

    /// Return the step's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return the Markdown describing what happens in this step, if any.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Yield the items the step lists, in written order.
    pub fn items(&self) -> impl Iterator<Item = &StepItem> {
        self.items.iter().map(ListedItem::item)
    }

    /// Return the items the step lists with where each was written, in written order.
    pub fn listed_items(&self) -> &[ListedItem] {
        &self.items
    }

    /// Return where in an imported source this step was declared, as a byte range.
    pub fn span(&self) -> Option<ByteRange> {
        self.span
    }

    /// Return where in an imported source this step was declared, as a miette span.
    pub fn source_span(&self) -> Option<SourceSpan> {
        self.span.map(SourceSpan::from)
    }
}

impl Workflow {
    /// Create a new workflow named `id` and titled `title`, with no override, edge or step yet.
    pub fn new(id: WorkflowId, title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            id,
            title,
            subtitle: None,
            description: None,
            overrides: BTreeMap::new(),
            edges: BTreeMap::new(),
            steps: Vec::new(),
            span: None,
        }
    }

    /// Set the line drawn under the workflow's title.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        let subtitle = subtitle.into();
        self.subtitle = Some(subtitle);
        self
    }

    /// Set the Markdown giving the workflow's overview.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(description);
        self
    }

    /// Set every override, keyed by the path of the element each applies to, as a source holding them in one map writes them.
    pub fn with_overrides(mut self, overrides: BTreeMap<ElementPath, ElementOverride>) -> Self {
        self.overrides = overrides;
        self
    }

    /// Set every edge under the id its source wrote, ordinal included, for a source that numbers its edges itself.
    pub fn with_edges(mut self, edges: BTreeMap<EdgeId, Edge>) -> Self {
        self.edges = edges;
        self
    }

    /// Add `step` after the steps already added.
    pub fn with_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Record where in an imported source this workflow was declared.
    pub fn with_span(mut self, span: ByteRange) -> Self {
        self.span = Some(span);
        self
    }

    /// Return `description` with each unnumbered edge target `FROM->TO` numbered `FROM->TO[n]` when exactly one edge of this workflow joins the pair, every other target as written.
    pub fn numbering_edge_targets_of(&self, description: &str) -> String {
        TargetLink::respell_all(description, |target_link| {
            let target = DescriptionTarget::parse(target_link.target)?;
            let DescriptionTarget::UnnumberedEdge { from, to } = target else {
                return None;
            };
            let only_edge = self.only_edge_between(&from, &to)?;
            let numbered_target = DescriptionTarget::Edge(only_edge.clone());
            let numbered_spelling = numbered_target.to_string();
            Some(numbered_spelling)
        })
    }

    /// Return the id of the only edge of this workflow from `from` to `to`, the edge an unnumbered target `FROM->TO` names; `None` when no edge or several join the pair.
    pub fn only_edge_between(&self, from: &ElementPath, to: &ElementPath) -> Option<&EdgeId> {
        let mut joining_edges = self.edges_between(from, to);
        let only_edge = joining_edges.next()?;
        let has_several_edges = joining_edges.next().is_some();
        if has_several_edges {
            return None;
        }
        Some(only_edge)
    }

    /// Yield the ids of this workflow's edges from `from` to `to`, by ordinal.
    pub(crate) fn edges_between<'w>(
        &'w self,
        from: &ElementPath,
        to: &ElementPath,
    ) -> impl Iterator<Item = &'w EdgeId> + use<'w> {
        let from_path = from.clone();
        let to_path = to.clone();
        let parallel_ids = EdgeId::every_parallel_id(from_path, to_path);
        let parallel_edges = self.edges.range(parallel_ids);
        parallel_edges.map(|(id, _edge)| id)
    }

    /// Return the index of the first step named `id`, if any.
    pub fn step_index(&self, id: &StepId) -> Option<StepIndex> {
        let mut steps = self.steps.iter();
        let position = steps.position(|step| step.id() == id)?;
        let index = StepIndex::new(position);
        Some(index)
    }

    /// Return the workflow's id.
    pub fn id(&self) -> &WorkflowId {
        &self.id
    }

    /// Return the workflow's title, drawn top-left while it is shown.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return the line drawn under the workflow's title, if any.
    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    /// Return the Markdown giving the workflow's overview, if any.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Yield every description written inside the workflow: its prose, its steps' prose, then its edges' descriptions.
    pub fn descriptions(&self) -> impl Iterator<Item = &str> {
        let step_descriptions = self.steps.iter().filter_map(Step::description);
        let edge_descriptions = self.edges.values().filter_map(Edge::description);
        self.description()
            .into_iter()
            .chain(step_descriptions)
            .chain(edge_descriptions)
    }

    /// Return the override of the element at `path`, if this workflow has one.
    pub fn override_of(&self, path: &ElementPath) -> Option<&ElementOverride> {
        self.overrides.get(path)
    }

    /// Return every override keyed by the path of the element it applies to.
    pub fn overrides(&self) -> &BTreeMap<ElementPath, ElementOverride> {
        &self.overrides
    }

    /// Return the edge `id` names, if this workflow has it.
    pub fn edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Return every edge keyed by its id, ordinals counted within this workflow.
    pub fn edges(&self) -> &BTreeMap<EdgeId, Edge> {
        &self.edges
    }

    /// Return the steps in written order.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Return the step at `index`, if there is one.
    pub fn step(&self, index: StepIndex) -> Option<&Step> {
        let position = index.position();
        self.steps.get(position)
    }

    /// Return where in an imported source this workflow was declared, as a byte range.
    pub fn span(&self) -> Option<ByteRange> {
        self.span
    }

    /// Return where in an imported source this workflow was declared, as a miette span.
    pub fn source_span(&self) -> Option<SourceSpan> {
        self.span.map(SourceSpan::from)
    }
}

impl ElementOverride {
    /// Create a new override that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace every component property this override sets.
    pub fn with_properties(mut self, properties: ComponentProperties) -> Self {
        self.properties = properties;
        self
    }

    /// Set how the element, once a container, places its children in this workflow.
    pub fn with_child_placement(mut self, child_placement: PlacementHint) -> Self {
        self.child_placement = Some(child_placement);
        self
    }

    /// Pin the element at a plan-space position inside its parent in this workflow.
    pub fn with_position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }

    /// Record where in an imported source this override was declared.
    pub fn with_span(mut self, span: ByteRange) -> Self {
        self.span = Some(span);
        self
    }

    /// Return `element` as this workflow draws it: each property this override sets replaces the element's, style and metadata entry by entry.
    pub(crate) fn applied_to(&self, element: &Element) -> Element {
        let mut overridden = element.clone();
        overridden.properties = element.properties.overridden_by(&self.properties);
        overridden
    }

    /// Return the component properties this override sets.
    pub fn properties(&self) -> &ComponentProperties {
        &self.properties
    }

    /// Return how the element places its children in this workflow, if set.
    pub fn child_placement(&self) -> Option<PlacementHint> {
        self.child_placement
    }

    /// Return where the element is pinned in this workflow, if set.
    pub fn position(&self) -> Option<Position> {
        self.position
    }

    /// Return where in an imported source this override was declared, as a byte range.
    pub fn span(&self) -> Option<ByteRange> {
        self.span
    }

    /// Return where in an imported source this override was declared, as a miette span.
    pub fn source_span(&self) -> Option<SourceSpan> {
        self.span.map(SourceSpan::from)
    }
}

impl From<ElementOverrideDocument> for ElementOverride {
    fn from(document: ElementOverrideDocument) -> Self {
        let link = SpannedText::written(document.link, document.link_span);
        let code = SpannedText::written(document.code, document.code_span);
        let properties = ComponentProperties {
            caption: document.caption,
            shape: document.shape,
            height: document.height,
            style: document.style,
            link,
            code,
            metadata: document.metadata,
        };
        Self {
            properties,
            child_placement: document.child_placement,
            position: document.position,
            span: document.span,
        }
    }
}

impl From<ElementOverride> for ElementOverrideDocument {
    fn from(element_override: ElementOverride) -> Self {
        let ComponentProperties {
            caption,
            shape,
            height,
            style,
            link,
            code,
            metadata,
        } = element_override.properties;
        let (link, link_span) = SpannedText::unpaired(link);
        let (code, code_span) = SpannedText::unpaired(code);
        Self {
            caption,
            shape,
            height,
            style,
            child_placement: element_override.child_placement,
            position: element_override.position,
            link,
            link_span,
            code,
            code_span,
            metadata,
            span: element_override.span,
        }
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for WorkflowId {
    type Err = Infallible;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        let workflow_id = Self::new(id);
        Ok(workflow_id)
    }
}

impl fmt::Display for StepId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for StepItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edge(id) => write!(formatter, "edge `{id}`"),
            Self::Element(path) => write!(formatter, "element `{path}`"),
        }
    }
}
