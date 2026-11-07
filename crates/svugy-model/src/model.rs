//! Hold the canonical description of a system: elements keyed by path and edges keyed by endpoints plus ordinal.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Bound;

use miette::SourceSpan;
use serde::de;
use serde::{Deserialize, Deserializer, Serialize};
use strum::{EnumString, IntoStaticStr, VariantNames};

use crate::legend::Legend;
use crate::path::ElementPath;
use crate::unique_key_map::UniqueKeyMapVisitor;

/// Largest magnitude a numeric style value, a position coordinate or a height may take, so no valid model can explode the output.
pub const MAX_MAGNITUDE: f64 = 1_000_000.0;

/// Model a system as the diagram's title, subtitle, description and legend, elements keyed by path and edges keyed by id; every model handed out of this crate has its edge endpoints and ancestors declared.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub(crate) title: Option<String>,
    pub(crate) subtitle: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) legend: Option<Legend>,
    pub(crate) elements: BTreeMap<ElementPath, Element>,
    pub(crate) edges: BTreeMap<EdgeId, Edge>,
}

impl Model {
    /// Insert `edge` from `from` to `to` with the next free ordinal for that pair, so parallel edges stay distinct.
    pub(crate) fn insert_edge_with_next_free_ordinal(
        &mut self,
        from: ElementPath,
        to: ElementPath,
        edge: Edge,
    ) -> EdgeId {
        let first_parallel_id = EdgeId::new(from, to, 0);
        let last_parallel_id = first_parallel_id.parallel(u32::MAX);
        let parallel_ids = &first_parallel_id..=&last_parallel_id;
        let mut parallel_edges = self.edges.range::<EdgeId, _>(parallel_ids);
        let last_parallel_edge = parallel_edges.next_back();
        let ordinal = last_parallel_edge.map_or(0, |(id, _edge)| id.ordinal() + 1);
        let id = first_parallel_id.parallel(ordinal);
        let inserted_id = id.clone();
        self.edges.insert(inserted_id, edge);
        id
    }

    /// Yield the edges Layout may see: every edge except those joining an element to its ancestor or descendant.
    pub fn layoutable_edges(&self) -> impl Iterator<Item = (&EdgeId, &Edge)> {
        self.edges
            .iter()
            .filter(|(id, _edge)| !id.joins_ancestor_and_descendant())
    }

    /// Yield the layoutable edges drawn as arcs: every one Layout may see but the badges.
    pub fn arc_edges(&self) -> impl Iterator<Item = (&EdgeId, &Edge)> {
        self.layoutable_edges()
            .filter(|(_id, edge)| !edge.is_badge())
    }

    /// Yield the edges drawn as badges, in edge order.
    pub fn badge_edges(&self) -> impl Iterator<Item = &EdgeId> {
        let badge_edges = self.edges.iter().filter(|(_id, edge)| edge.is_badge());
        badge_edges.map(|(id, _edge)| id)
    }

    /// Yield the edges drawn as badges on `source`, in edge order.
    pub fn badge_edges_from<'a>(
        &'a self,
        source: &'a ElementPath,
    ) -> impl Iterator<Item = &'a EdgeId> {
        let source_path = source.clone();
        let root = ElementPath::root();
        let first_edge_from_source = EdgeId::new(source_path, root, 0);
        let from_source_onwards = self.edges.range(first_edge_from_source..);
        let edges_from_source =
            from_source_onwards.take_while(move |(id, _edge)| id.from() == source);
        let badge_edges_from_source = edges_from_source.filter(|(_id, edge)| edge.is_badge());
        badge_edges_from_source.map(|(id, _edge)| id)
    }

    /// Tell whether `id` is drawn as a badge on its source's top face instead of an arc, as its edge is marked.
    pub fn is_badge_edge(&self, id: &EdgeId) -> bool {
        let edge = self.edge(id);
        edge.is_some_and(Edge::is_badge)
    }

    /// Yield the elements one level below `parent`; the root yields the top-level elements.
    pub fn children<'a>(
        &'a self,
        parent: &'a ElementPath,
    ) -> impl Iterator<Item = (&'a ElementPath, &'a Element)> {
        let child_depth = parent.depth() + 1;
        self.descendants(parent)
            .filter(move |(path, _element)| path.depth() == child_depth)
    }

    /// Yield every element strictly below `ancestor`, in path order.
    pub fn descendants<'a>(
        &'a self,
        ancestor: &'a ElementPath,
    ) -> impl Iterator<Item = (&'a ElementPath, &'a Element)> {
        let after_ancestor = (Bound::Excluded(ancestor), Bound::Unbounded);
        self.elements
            .range::<ElementPath, _>(after_ancestor)
            .take_while(|(path, _element)| ancestor.is_ancestor_of(path))
    }

    /// Tell whether the element at `path` has children, which is what makes it a container.
    pub fn is_container(&self, path: &ElementPath) -> bool {
        let mut descendants = self.descendants(path);
        descendants.next().is_some()
    }

    /// Return the diagram's title, drawn at the top-left of the top-level view.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Return the diagram's subtitle, drawn under its title.
    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    /// Return the diagram's description, Markdown shown in the side panel while nothing is focused.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return the diagram's legend, drawn at the top-right of the top-level view.
    pub fn legend(&self) -> Option<&Legend> {
        self.legend.as_ref()
    }

    /// Return the element at `path`, if one is declared there.
    pub fn element(&self, path: &ElementPath) -> Option<&Element> {
        self.elements.get(path)
    }

    /// Return every element keyed by its path, in path order.
    pub fn elements(&self) -> &BTreeMap<ElementPath, Element> {
        &self.elements
    }

    /// Return the edge identified by `id`, if there is one.
    pub fn edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Return every edge keyed by its id, in edge order.
    pub fn edges(&self) -> &BTreeMap<EdgeId, Edge> {
        &self.edges
    }
}

/// Reject an element's `badge` field, which moved onto edges, naming where it went instead of serde's bare unknown field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MovedElementBadge;

/// Describe an element: a component or container, everything except its path and its children.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
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
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
    #[serde(default, rename = "badge", skip_serializing)]
    moved_badge: MovedElementBadge,
}

impl Element {
    /// Create a new element drawn with `label`, every optional attribute unset.
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            label,
            caption: None,
            description: None,
            shape: None,
            height: None,
            style: BTreeMap::new(),
            child_placement: None,
            position: None,
            link: None,
            link_span: None,
            metadata: BTreeMap::new(),
            span: None,
            moved_badge: MovedElementBadge,
        }
    }

    /// Replace the label `new` was given, as an importer does once a source names the element twice.
    pub fn relabeled(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the secondary line drawn under the label.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        let caption = caption.into();
        self.caption = Some(caption);
        self
    }

    /// Set the Markdown describing the element in the side panel.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(description);
        self
    }

    /// Set the shape the element is drawn as, overriding the renderer's pick.
    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Stand a leaf's top face `height` plan units above its floor instead of its primitive's default; validation bounds it.
    pub fn with_height(mut self, height: f64) -> Self {
        self.height = Some(height);
        self
    }

    /// Record a raw style entry; validation decides later whether the key is known.
    pub fn with_style(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        self.style.insert(key, value);
        self
    }

    /// Set how this element, once a container, places its children.
    pub fn with_child_placement(mut self, child_placement: PlacementHint) -> Self {
        self.child_placement = Some(child_placement);
        self
    }

    /// Pin this element at an explicit plan-space position inside its parent, overriding automatic placement.
    pub fn with_position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }

    /// Set the drill-down target (an element path) or URL this element points to, unresolved.
    pub fn with_link(mut self, link: impl Into<String>) -> Self {
        let link = link.into();
        self.link = Some(link);
        self
    }

    /// Record where in an imported source this element's link value was written.
    pub fn with_link_span(mut self, link_span: ByteRange) -> Self {
        self.link_span = Some(link_span);
        self
    }

    /// Record a free-form metadata entry, carried into the rendered element's annotations.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        self.metadata.insert(key, value);
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

    /// Return the secondary line drawn under the label, if any.
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    /// Return the Markdown describing the element in the side panel, if any.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return the authored shape, or `None` to let the renderer pick (box for components, slab for containers).
    pub fn shape(&self) -> Option<Shape> {
        self.shape
    }

    /// Return the authored height, or `None` to stand at the primitive's default height.
    pub fn height(&self) -> Option<f64> {
        self.height
    }

    /// Return the element's raw style entries, keyed by style name.
    pub fn style(&self) -> &BTreeMap<String, String> {
        &self.style
    }

    /// Return how this element, once a container, places its children, if authored.
    pub fn child_placement(&self) -> Option<PlacementHint> {
        self.child_placement
    }

    /// Return the pinned plan-space position inside the parent, if authored.
    pub fn position(&self) -> Option<Position> {
        self.position
    }

    /// Return the unresolved drill-down target or URL the element links to, if any.
    pub fn link(&self) -> Option<&str> {
        self.link.as_deref()
    }

    /// Return where in an imported source the link value was written, as a byte range.
    pub fn link_span(&self) -> Option<ByteRange> {
        self.link_span
    }

    /// Return the element's metadata entries, keyed by name.
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
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

impl<'de> Deserialize<'de> for MovedElementBadge {
    fn deserialize<D: Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        let moved_badge_message = "`badge` moved from elements to edges: set `\"badge\": true` on each edge into this element";
        let moved_badge_error = de::Error::custom(moved_badge_message);
        Err(moved_badge_error)
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

/// Locate an imported element, edge or link value as a byte range in its source, convertible to a miette span.
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

    /// Return the level the edge is laid out in: the endpoints' lowest common ancestor, or a self-loop's parent.
    pub fn home_level(&self) -> ElementPath {
        let is_self_loop = self.from == self.to;
        if is_self_loop {
            return self.from.enclosing_level();
        }
        self.from.lowest_common_ancestor(&self.to)
    }

    /// Return the id of the parallel edge numbered `ordinal` between this id's endpoints.
    pub(crate) fn parallel(&self, ordinal: u32) -> Self {
        let from = self.from.clone();
        let to = self.to.clone();
        Self { from, to, ordinal }
    }

    /// Tell whether one endpoint contains the other, which Layout cannot draw.
    pub fn joins_ancestor_and_descendant(&self) -> bool {
        let is_from_ancestor = self.from.is_ancestor_of(&self.to);
        let is_to_ancestor = self.to.is_ancestor_of(&self.from);
        is_from_ancestor || is_to_ancestor
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

    /// Set the Markdown describing the dataflow in the side panel.
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

    /// Return the Markdown describing the dataflow in the side panel, if any.
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
