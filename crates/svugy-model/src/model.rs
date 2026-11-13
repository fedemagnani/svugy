//! Hold the canonical description of a system: components keyed by path, and the workflows that show them.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::RangeInclusive;

use miette::SourceSpan;
use serde::de;
use serde::{Deserialize, Deserializer, Serialize};
use strum::{EnumString, IntoStaticStr, VariantNames};

use crate::graph::WorkflowGraph;
use crate::path::ElementPath;
use crate::properties::{ComponentProperties, SpannedText};
use crate::unique_key_map::UniqueKeyMapVisitor;
use crate::workflow::{Workflow, WorkflowId};

/// Largest magnitude a numeric style value, a position coordinate or a height may take, so no valid model can explode the output.
pub const MAX_MAGNITUDE: f64 = 1_000_000.0;

/// Model a system as its view angle, its elements keyed by path and its workflows in written order; every model handed out of this crate has its ancestors declared and every workflow's edges and steps naming its own elements.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub(crate) view: Option<ViewAngle>,
    pub(crate) elements: BTreeMap<ElementPath, Element>,
    pub(crate) workflows: Vec<Workflow>,
}

impl Model {
    /// Return the angle the diagram is first seen from, if the author picked one.
    pub fn view(&self) -> Option<ViewAngle> {
        self.view
    }

    /// Return the element at `path`, if one is declared there.
    pub fn element(&self, path: &ElementPath) -> Option<&Element> {
        self.elements.get(path)
    }

    /// Return every element keyed by its path, in path order.
    pub fn elements(&self) -> &BTreeMap<ElementPath, Element> {
        &self.elements
    }

    /// Return the workflow with `id`, the first written when several share it.
    pub fn workflow(&self, id: &WorkflowId) -> Option<&Workflow> {
        let mut workflows = self.workflows.iter();
        workflows.find(|workflow| workflow.id() == id)
    }

    /// Return every workflow in written order, the first being the one a reader starts on.
    pub fn workflows(&self) -> &[Workflow] {
        &self.workflows
    }

    /// Yield every description the model holds: each element's, then each workflow's prose, its steps' prose and its edges' descriptions.
    pub fn descriptions(&self) -> impl Iterator<Item = &str> {
        let element_descriptions = self.elements.values().filter_map(Element::description);
        let workflow_descriptions = self.workflows.iter().flat_map(Workflow::descriptions);
        element_descriptions.chain(workflow_descriptions)
    }

    /// Show the workflow with `id`, the first written when several share it, if the model has one.
    pub fn showing(&self, id: &WorkflowId) -> Option<ShownWorkflow<'_>> {
        let workflow = self.workflow(id)?;
        let shown_workflow = ShownWorkflow {
            model: self,
            workflow,
        };
        Some(shown_workflow)
    }

    /// Show the first workflow, the one a reader starts on.
    pub fn showing_first_workflow(&self) -> ShownWorkflow<'_> {
        let workflow = self
            .workflows
            .first()
            .expect("every model handed out of this crate has a workflow");
        ShownWorkflow {
            model: self,
            workflow,
        }
    }

    /// Show each workflow in turn, in written order.
    pub fn showing_each_workflow(&self) -> impl Iterator<Item = ShownWorkflow<'_>> {
        self.workflows.iter().map(|workflow| ShownWorkflow {
            model: self,
            workflow,
        })
    }
}

/// Pair a model with the workflow a reader is shown, so the workflow is always one of that model's own.
#[derive(Clone, Copy, Debug)]
pub struct ShownWorkflow<'m> {
    model: &'m Model,
    workflow: &'m Workflow,
}

impl<'m> ShownWorkflow<'m> {
    /// Project the shown workflow into the graph it draws, paired with it.
    pub fn projected(self) -> ProjectedWorkflow<'m> {
        let graph = WorkflowGraph::of(self.model, self.workflow);
        ProjectedWorkflow {
            workflow: self.workflow,
            graph,
        }
    }

    /// Return the model the workflow belongs to.
    pub fn model(self) -> &'m Model {
        self.model
    }

    /// Return the shown workflow.
    pub fn workflow(self) -> &'m Workflow {
        self.workflow
    }
}

/// Pair a workflow with the graph projected from it, so every step index of the graph names a step of that workflow.
#[derive(Clone, Debug)]
pub struct ProjectedWorkflow<'m> {
    workflow: &'m Workflow,
    graph: WorkflowGraph,
}

impl<'m> ProjectedWorkflow<'m> {
    /// Hand out the graph alone, for a caller that never reads the steps of its workflow.
    pub fn into_graph(self) -> WorkflowGraph {
        self.graph
    }

    /// Return the projected workflow.
    pub fn workflow(&self) -> &'m Workflow {
        self.workflow
    }

    /// Return the graph the workflow draws.
    pub fn graph(&self) -> &WorkflowGraph {
        &self.graph
    }
}

/// Describe an element: a component or container, everything except its path and its children.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "ElementDocument", into = "ElementDocument")]
pub struct Element {
    label: String,
    pub(crate) description: Option<String>,
    pub(crate) properties: ComponentProperties,
    span: Option<ByteRange>,
}

/// Spell an element as model JSON: every field flat, each field v3 elements had and v4 moved rejected with its fix.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ElementDocument {
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
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
    #[serde(
        default,
        rename = "badge",
        skip_serializing,
        deserialize_with = "ElementDocument::reject_badge"
    )]
    _moved_badge: (),
    #[serde(
        default,
        rename = "position",
        skip_serializing,
        deserialize_with = "ElementDocument::reject_position"
    )]
    _moved_position: (),
    #[serde(
        default,
        rename = "child_placement",
        skip_serializing,
        deserialize_with = "ElementDocument::reject_child_placement"
    )]
    _moved_child_placement: (),
}

impl Element {
    /// Create a new element drawn with `label`, every optional attribute unset.
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            label,
            description: None,
            properties: ComponentProperties::new(),
            span: None,
        }
    }

    /// Replace the label `new` was given, as an importer does once a source names the element twice.
    pub fn relabeled(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the Markdown describing the element in the description panel.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(description);
        self
    }

    /// Replace every property of how the element is drawn and where it points.
    pub fn with_properties(mut self, properties: ComponentProperties) -> Self {
        self.properties = properties;
        self
    }

    /// Set the secondary line drawn under the label.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.properties = self.properties.with_caption(caption);
        self
    }

    /// Set the shape the element is drawn as, overriding the renderer's pick.
    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.properties = self.properties.with_shape(shape);
        self
    }

    /// Stand a leaf's top face `height` plan units above its floor instead of its primitive's default; validation bounds it.
    pub fn with_height(mut self, height: f64) -> Self {
        self.properties = self.properties.with_height(height);
        self
    }

    /// Record a raw style entry; validation decides later whether the key is known.
    pub fn with_style(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties = self.properties.with_style(key, value);
        self
    }

    /// Set the unresolved link, written at `span` of an imported source when there is one.
    pub fn with_link_written_at(
        mut self,
        link: impl Into<String>,
        span: Option<ByteRange>,
    ) -> Self {
        self.properties = self.properties.with_link_written_at(link, span);
        self
    }

    /// Record where in an imported source this element was declared.
    pub fn with_span(mut self, span: ByteRange) -> Self {
        self.span = Some(span);
        self
    }

    /// Return the label drawn on the element.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Return the Markdown describing the element in the description panel, if any.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return how the element is drawn and where it points.
    pub fn properties(&self) -> &ComponentProperties {
        &self.properties
    }

    /// Return where in an imported source this element was declared, as a byte range.
    pub fn span(&self) -> Option<ByteRange> {
        self.span
    }

    /// Return where in an imported source this element was declared, as a miette span.
    pub fn source_span(&self) -> Option<SourceSpan> {
        self.span.map(SourceSpan::from)
    }
}

impl From<ElementDocument> for Element {
    fn from(document: ElementDocument) -> Self {
        let ElementDocument {
            label,
            caption,
            description,
            shape,
            height,
            style,
            link,
            link_span,
            code,
            code_span,
            metadata,
            span,
            _moved_badge,
            _moved_position,
            _moved_child_placement,
        } = document;
        let link = SpannedText::written(link, link_span);
        let code = SpannedText::written(code, code_span);
        let properties = ComponentProperties {
            caption,
            shape,
            height,
            style,
            link,
            code,
            metadata,
        };
        Self {
            label,
            description,
            properties,
            span,
        }
    }
}

impl From<Element> for ElementDocument {
    fn from(element: Element) -> Self {
        let ComponentProperties {
            caption,
            shape,
            height,
            style,
            link,
            code,
            metadata,
        } = element.properties;
        let (link, link_span) = SpannedText::unpaired(link);
        let (code, code_span) = SpannedText::unpaired(code);
        Self {
            label: element.label,
            caption,
            description: element.description,
            shape,
            height,
            style,
            link,
            link_span,
            code,
            code_span,
            metadata,
            span: element.span,
            _moved_badge: (),
            _moved_position: (),
            _moved_child_placement: (),
        }
    }
}

impl ElementDocument {
    /// Reject an element's `badge`, which moved onto edges.
    fn reject_badge<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        let moved_badge_message = "`badge` moved from elements to edges: set `\"badge\": true` on each edge into this element";
        let moved_badge_error = de::Error::custom(moved_badge_message);
        Err(moved_badge_error)
    }

    /// Reject an element's `position`, which moved into each workflow's overrides.
    fn reject_position<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        let moved_position_message =
            "`position` moved from elements to workflows: set it in each workflow's `overrides`";
        let moved_position_error = de::Error::custom(moved_position_message);
        Err(moved_position_error)
    }

    /// Reject an element's `child_placement`, which moved into each workflow's overrides.
    fn reject_child_placement<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        let moved_child_placement_message = "`child_placement` moved from elements to workflows: set it in each workflow's `overrides`";
        let moved_child_placement_error = de::Error::custom(moved_child_placement_message);
        Err(moved_child_placement_error)
    }
}

/// Shape an element as the v1 primitive it is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(EnumString, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum Shape {
    Box,
    Slab,
    Cylinder,
    Disc,
}

/// Name the angle a diagram is seen from.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
#[derive(EnumString, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum ViewAngle {
    /// From above a ground corner, both ground axes receding.
    #[default]
    Isometric,
    /// From straight above, flat.
    Top,
}

/// Name the symbol an animated edge carries along its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(EnumString, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum Traveler {
    Dot,
    Envelope,
    Parcel,
    Document,
    Key,
}

/// Hint the strategy a container places its children with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
#[derive(EnumString, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum PlacementHint {
    Layered,
    Grid { columns: u32 },
    Row,
    Column,
}

impl PlacementHint {
    /// Most columns or rows a packed level holds along one axis: grid lines are counted in an i16, less one.
    pub const MAX_PACKED_TRACKS: u32 = 32_766;

    /// Return the (columns, rows) this hint packs n children in, rows = ⌈n / columns⌉; `None` for layered or an empty grid.
    pub fn packed_tracks(self, child_count: usize) -> Option<(usize, usize)> {
        match self {
            Self::Layered | Self::Grid { columns: 0 } => None,
            Self::Grid { columns } => {
                let column_count = usize::try_from(columns).unwrap_or(usize::MAX);
                let row_count = child_count.div_ceil(column_count);
                Some((column_count, row_count))
            }
            Self::Row => Some((child_count, 1)),
            Self::Column => Some((1, child_count)),
        }
    }
}

/// Position a footprint's min corner (min x, min y) in the parent level's plan space, from that level's origin, in plan units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Locate where an imported source wrote a component, workflow, step, edge, override or property value, as a byte range convertible to a miette span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    /// Create a new byte range from `start` up to `end`.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

impl From<ByteRange> for SourceSpan {
    fn from(range: ByteRange) -> Self {
        let byte_range = range.start..range.end;
        byte_range.into()
    }
}

/// Identify an edge by both endpoint paths plus an ordinal, so parallel edges between one pair stay distinct.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId {
    from: ElementPath,
    to: ElementPath,
    ordinal: u32,
}

impl EdgeId {
    /// Create a new edge id from its endpoints and its ordinal among their parallel edges.
    pub fn new(from: ElementPath, to: ElementPath, ordinal: u32) -> Self {
        Self { from, to, ordinal }
    }

    /// Return the id `from -> to` takes when added to `edges`: the next free ordinal for that pair, so parallel edges stay distinct.
    pub(crate) fn next_free_in(
        edges: &BTreeMap<EdgeId, Edge>,
        from: ElementPath,
        to: ElementPath,
    ) -> Self {
        let parallel_ids = Self::every_parallel_id(from, to);
        let first_parallel_id = parallel_ids.start();
        let mut parallel_edges = edges.range(parallel_ids.clone());
        let last_parallel_edge = parallel_edges.next_back();
        let ordinal = last_parallel_edge.map_or(0, |(id, _edge)| id.ordinal() + 1);
        first_parallel_id.parallel(ordinal)
    }

    /// Return every id an edge `from -> to` can take, ordinal 0 to the last, as an edge-ordered range.
    pub(crate) fn every_parallel_id(from: ElementPath, to: ElementPath) -> RangeInclusive<Self> {
        let first_parallel_id = Self::new(from, to, 0);
        let last_parallel_id = first_parallel_id.parallel(u32::MAX);
        first_parallel_id..=last_parallel_id
    }

    /// Return the level the edge is laid out in: the endpoints' lowest common ancestor.
    pub fn home_level(&self) -> ElementPath {
        self.from.lowest_common_ancestor(&self.to)
    }

    /// Return the id of the parallel edge numbered `ordinal` between this id's endpoints.
    fn parallel(&self, ordinal: u32) -> Self {
        let from = self.from.clone();
        let to = self.to.clone();
        Self { from, to, ordinal }
    }

    /// Tell whether the edge joins an element to itself, its ancestor or its descendant, which no layer can draw.
    pub fn joins_element_to_itself_or_its_lineage(&self) -> bool {
        self.from.shares_lineage_with(&self.to)
    }

    /// Return the path the edge starts from.
    pub fn from(&self) -> &ElementPath {
        &self.from
    }

    /// Return the path the edge points to.
    pub fn to(&self) -> &ElementPath {
        &self.to
    }

    /// Return the ordinal telling apart the parallel edges of one endpoint pair.
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "({} -> {})[{}]",
            self.from, self.to, self.ordinal
        )
    }
}

/// Describe an edge: a connection, everything except its identity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edge {
    pub(crate) label: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) style: BTreeMap<String, String>,
    pub(crate) traveler: Option<Traveler>,
    pub(crate) badge: bool,
    pub(crate) span: Option<ByteRange>,
}

impl Edge {
    /// Create a new edge, unlabelled and unstyled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the label drawn along the edge.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        self.label = Some(label);
        self
    }

    /// Set the Markdown describing the dataflow in the description panel.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(description);
        self
    }

    /// Record a raw style entry; validation decides later whether the key is known.
    pub fn with_style(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        self.style.insert(key, value);
        self
    }

    /// Set the symbol the edge carries along its path when animated, instead of the default dot.
    pub fn with_traveler(mut self, traveler: Traveler) -> Self {
        self.traveler = Some(traveler);
        self
    }

    /// Draw the edge as a mini of its target on its source's top face instead of an arc.
    pub fn with_badge(mut self) -> Self {
        self.badge = true;
        self
    }

    /// Record where in an imported source this edge was declared.
    pub fn with_span(mut self, span: ByteRange) -> Self {
        self.span = Some(span);
        self
    }

    /// Return the label drawn along the edge, if any.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Return the Markdown describing the dataflow in the description panel, if any.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return the edge's raw style entries, keyed by style name.
    pub fn style(&self) -> &BTreeMap<String, String> {
        &self.style
    }

    /// Return the symbol the edge carries along its path, if one was set instead of the default dot.
    pub fn traveler(&self) -> Option<Traveler> {
        self.traveler
    }

    /// Tell whether the edge is marked to be drawn as a badge.
    pub fn is_badge(&self) -> bool {
        self.badge
    }

    /// Return where in an imported source this edge was declared, as a byte range.
    pub fn span(&self) -> Option<ByteRange> {
        self.span
    }

    /// Return where in an imported source this edge was declared, as a miette span.
    pub fn source_span(&self) -> Option<SourceSpan> {
        self.span.map(SourceSpan::from)
    }
}
