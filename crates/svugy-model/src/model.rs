//! Hold the canonical description of a system: elements keyed by path and edges keyed by endpoints plus ordinal.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Bound;

use serde::{Deserialize, Serialize};

use crate::legend::Legend;
use crate::path::ElementPath;

/// Model a system that passed every error-level check: every edge endpoint and every ancestor of an element exists.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Model {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legend: Option<Legend>,
    elements: BTreeMap<ElementPath, Element>,
    #[serde(serialize_with = "crate::json::serialize_edges")]
    edges: BTreeMap<EdgeId, Edge>,
}

impl Model {
    /// Assemble a model without checking references; it leaves the crate only through `Model::admit`.
    pub(crate) fn new_unchecked(
        elements: BTreeMap<ElementPath, Element>,
        edges: BTreeMap<EdgeId, Edge>,
    ) -> Self {
        Self {
            title: None,
            subtitle: None,
            legend: None,
            elements,
            edges,
        }
    }

    /// Set the diagram's title, drawn at the top-left of the top-level view; the model leaves the crate only through `Model::admit`.
    pub(crate) fn with_title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.title = Some(title);
        self
    }

    /// Set the diagram's subtitle, drawn under its title; the model leaves the crate only through `Model::admit`.
    pub(crate) fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        let subtitle = subtitle.into();
        self.subtitle = Some(subtitle);
        self
    }

    /// Set the diagram's legend, drawn at the top-right of the top-level view; the model leaves the crate only through `Model::admit`.
    pub(crate) fn with_legend(mut self, legend: Legend) -> Self {
        self.legend = Some(legend);
        self
    }

    /// Insert an element without checking references; the model leaves the crate only through `Model::admit`.
    pub(crate) fn insert_element(&mut self, path: ElementPath, element: Element) {
        self.elements.insert(path, element);
    }

    /// Insert an edge without checking references; the model leaves the crate only through `Model::admit`.
    pub(crate) fn insert_edge(&mut self, id: EdgeId, edge: Edge) {
        self.edges.insert(id, edge);
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
            .filter(|(id, _edge)| !self.is_badge_edge(id))
    }

    /// Yield the edges drawn as badges, in edge order.
    pub fn badge_edges(&self) -> impl Iterator<Item = &EdgeId> {
        let edge_ids = self.edges.keys();
        edge_ids.filter(|id| self.is_badge_edge(id))
    }

    /// Yield the edges drawn as badges on `source`, in edge order.
    pub fn badge_edges_from<'a>(
        &'a self,
        source: &'a ElementPath,
    ) -> impl Iterator<Item = &'a EdgeId> {
        let first_edge_from_source = EdgeId::new(source.clone(), ElementPath::root(), 0);
        let from_source_onwards = self.edges.range(first_edge_from_source..);
        let edges_from_source =
            from_source_onwards.take_while(move |(id, _edge)| id.from() == source);
        let edge_ids = edges_from_source.map(|(id, _edge)| id);
        edge_ids.filter(|id| self.is_badge_edge(id))
    }

    /// Tell whether `id` is drawn as a badge on its source's top face instead of an arc: its target is a leaf marked badge and its source a leaf.
    pub fn is_badge_edge(&self, id: &EdgeId) -> bool {
        let is_badged_target = self.is_badged_leaf(id.to());
        let is_source_leaf = !self.is_container(id.from());
        is_badged_target && is_source_leaf
    }

    /// Tell whether `path` is a leaf marked badge, the only kind of element edges are badged into.
    pub(crate) fn is_badged_leaf(&self, path: &ElementPath) -> bool {
        let element = self.element(path);
        let is_badge_target = element.is_some_and(Element::is_badge_target);
        let is_leaf = !self.is_container(path);
        is_badge_target && is_leaf
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

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    pub fn legend(&self) -> Option<&Legend> {
        self.legend.as_ref()
    }

    pub fn element(&self, path: &ElementPath) -> Option<&Element> {
        self.elements.get(path)
    }

    pub fn elements(&self) -> &BTreeMap<ElementPath, Element> {
        &self.elements
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    pub fn edges(&self) -> &BTreeMap<EdgeId, Edge> {
        &self.edges
    }
}

/// Describe a component or container, everything except its path and its children.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<Shape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    badge: bool,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "crate::json::deserialize_unique_keys"
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
        deserialize_with = "crate::json::deserialize_unique_keys"
    )]
    metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
}

impl Element {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            label,
            caption: None,
            shape: None,
            height: None,
            badge: false,
            style: BTreeMap::new(),
            child_placement: None,
            position: None,
            link: None,
            link_span: None,
            metadata: BTreeMap::new(),
            span: None,
        }
    }

    /// Replace the label `new` was given, as an importer does once a source names the element twice.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the secondary line drawn under the label.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        let caption = caption.into();
        self.caption = Some(caption);
        self
    }

    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Stand a leaf's top face `height` plan units above its floor instead of its primitive's default; validation bounds it.
    pub fn with_height(mut self, height: f64) -> Self {
        self.height = Some(height);
        self
    }

    /// Mark this leaf a badge target: every edge into it is drawn as a mini of it on its source's top face instead of an arc.
    pub fn with_badge(mut self) -> Self {
        self.badge = true;
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

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    /// Return the authored shape, or `None` to let the renderer pick (box for components, slab for containers).
    pub fn shape(&self) -> Option<Shape> {
        self.shape
    }

    /// Return the authored height, or `None` to stand at the primitive's default height.
    pub fn height(&self) -> Option<f64> {
        self.height
    }

    /// Tell whether the element is marked a badge target; `Model::is_badge_edge` decides which edges that badges.
    pub fn is_badge_target(&self) -> bool {
        self.badge
    }

    pub fn style(&self) -> &BTreeMap<String, String> {
        &self.style
    }

    pub fn child_placement(&self) -> Option<PlacementHint> {
        self.child_placement
    }

    pub fn position(&self) -> Option<Position> {
        self.position
    }

    pub fn link(&self) -> Option<&str> {
        self.link.as_deref()
    }

    pub fn link_span(&self) -> Option<ByteRange> {
        self.link_span
    }

    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    pub fn span(&self) -> Option<ByteRange> {
        self.span
    }
}

/// Shape an element as the v1 primitive it is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Box,
    Slab,
    Cylinder,
    Disc,
}

/// Hint the strategy a container places its children with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum PlacementHint {
    Layered,
    Grid { columns: u32 },
    Row,
    Column,
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
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

impl From<ByteRange> for miette::SourceSpan {
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
    pub fn new(from: ElementPath, to: ElementPath, ordinal: u32) -> Self {
        Self { from, to, ordinal }
    }

    /// Return the level the edge is laid out in: the endpoints' lowest common ancestor, or a self-loop's parent.
    pub fn home_level(&self) -> ElementPath {
        let is_self_loop = self.from == self.to;
        if is_self_loop {
            let parent = self.from.parent();
            return parent.unwrap_or_else(ElementPath::root);
        }
        self.from.lowest_common_ancestor(&self.to)
    }

    /// Tell whether one endpoint contains the other, which Layout cannot draw.
    pub fn joins_ancestor_and_descendant(&self) -> bool {
        let is_from_ancestor = self.from.is_ancestor_of(&self.to);
        let is_to_ancestor = self.to.is_ancestor_of(&self.from);
        is_from_ancestor || is_to_ancestor
    }

    pub fn from(&self) -> &ElementPath {
        &self.from
    }

    pub fn to(&self) -> &ElementPath {
        &self.to
    }

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

/// Describe a connection, everything except its identity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edge {
    label: Option<String>,
    style: BTreeMap<String, String>,
    span: Option<ByteRange>,
}

impl Edge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        self.label = Some(label);
        self
    }

    /// Record a raw style entry; validation decides later whether the key is known.
    pub fn with_style(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        self.style.insert(key, value);
        self
    }

    /// Record where in an imported source this edge was declared.
    pub fn with_span(mut self, span: ByteRange) -> Self {
        self.span = Some(span);
        self
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn style(&self) -> &BTreeMap<String, String> {
        &self.style
    }

    pub fn span(&self) -> Option<ByteRange> {
        self.span
    }
}
