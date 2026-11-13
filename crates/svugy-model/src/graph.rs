//! Project one workflow of a model into the graph it draws: the elements it shows with its overrides applied, where it places them, its edges, and the steps each takes part in.

use std::collections::{BTreeMap, BTreeSet};
use std::iter;
use std::ops::Bound;

use crate::link::Link;
use crate::model::{Edge, EdgeId, Element, Model, PlacementHint, Position};
use crate::path::ElementPath;
use crate::workflow::{ElementOverride, StepIndex, StepItem, Workflow};

/// Hold the empty step set returned for an element or edge the graph doesn't draw.
const NO_STEPS: &BTreeSet<StepIndex> = &BTreeSet::new();

/// Hold the graph one workflow draws: the elements its steps list plus their ancestors, overridden for it, the links it drops, its listed edges, and each one's steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkflowGraph {
    elements: BTreeMap<ElementPath, Element>,
    child_placements: BTreeMap<ElementPath, PlacementHint>,
    positions: BTreeMap<ElementPath, Position>,
    unshown_link_targets: BTreeMap<ElementPath, ElementPath>,
    edges: BTreeMap<EdgeId, Edge>,
    element_steps: BTreeMap<ElementPath, BTreeSet<StepIndex>>,
    edge_steps: BTreeMap<EdgeId, BTreeSet<StepIndex>>,
}

impl WorkflowGraph {
    /// Project `workflow` of `model`: an element takes part in each step listing it, a descendant of it, or an edge ending at either; whatever names nothing in the model is left out, and so is a link to a component the workflow doesn't show.
    pub(crate) fn of(model: &Model, workflow: &Workflow) -> Self {
        let (mut element_steps, edge_steps) = Self::step_memberships(workflow);
        element_steps.retain(|path, _steps| model.element(path).is_some());
        let mut elements = BTreeMap::new();
        let mut child_placements = BTreeMap::new();
        let mut positions = BTreeMap::new();
        let mut unshown_link_targets = BTreeMap::new();
        for path in element_steps.keys() {
            let element = &model.elements[path];
            let element_override = workflow.override_of(path);
            let mut shown_element = element_override.map_or_else(
                || element.clone(),
                |element_override| element_override.applied_to(element),
            );
            let unshown_link_target =
                Self::unshown_link_target_of(&shown_element, model, &element_steps);
            if let Some(unshown_link_target) = unshown_link_target {
                shown_element.properties.link = None;
                unshown_link_targets.insert(path.clone(), unshown_link_target);
            }
            elements.insert(path.clone(), shown_element);
            let child_placement = element_override.and_then(ElementOverride::child_placement);
            if let Some(child_placement) = child_placement {
                child_placements.insert(path.clone(), child_placement);
            }
            let position = element_override.and_then(ElementOverride::position);
            if let Some(position) = position {
                positions.insert(path.clone(), position);
            }
        }
        let edges = edge_steps
            .keys()
            .map(|id| {
                let edge = workflow.edges[id].clone();
                (id.clone(), edge)
            })
            .collect();
        Self {
            elements,
            child_placements,
            positions,
            unshown_link_targets,
            edges,
            element_steps,
            edge_steps,
        }
    }

    /// Map each path and workflow edge the steps name to the steps naming it: an edge by being listed, a path by being listed, an endpoint, or an ancestor of either; a listed edge the workflow lacks names nothing.
    fn step_memberships(
        workflow: &Workflow,
    ) -> (
        BTreeMap<ElementPath, BTreeSet<StepIndex>>,
        BTreeMap<EdgeId, BTreeSet<StepIndex>>,
    ) {
        let mut element_steps: BTreeMap<ElementPath, BTreeSet<StepIndex>> = BTreeMap::new();
        let mut edge_steps: BTreeMap<EdgeId, BTreeSet<StepIndex>> = BTreeMap::new();
        for (position, step) in workflow.steps().iter().enumerate() {
            let step_index = StepIndex::new(position);
            for item in step.items() {
                let listed_paths = match item {
                    StepItem::Element(path) => vec![path],
                    StepItem::Edge(id) if workflow.edge(id).is_none() => continue,
                    StepItem::Edge(id) => {
                        let listing_steps = edge_steps.entry(id.clone()).or_default();
                        listing_steps.insert(step_index);
                        vec![id.from(), id.to()]
                    }
                };
                for listed_path in listed_paths {
                    let ancestors = listed_path.ancestors();
                    let lineage = ancestors.chain(iter::once(listed_path.clone()));
                    for shown_path in lineage {
                        let taking_part_steps = element_steps.entry(shown_path).or_default();
                        taking_part_steps.insert(step_index);
                    }
                }
            }
        }
        (element_steps, edge_steps)
    }

    /// Return the component of `model` that `element` links to when the workflow doesn't show it.
    fn unshown_link_target_of(
        element: &Element,
        model: &Model,
        shown_steps: &BTreeMap<ElementPath, BTreeSet<StepIndex>>,
    ) -> Option<ElementPath> {
        let properties = element.properties();
        let raw_link = properties.link()?;
        let declared_elements = model.elements();
        let link = Link::resolve(raw_link, declared_elements);
        let Link::Element(target) = link else {
            return None;
        };
        let is_target_shown = shown_steps.contains_key(&target);
        if is_target_shown {
            return None;
        }
        Some(target)
    }

    /// Return the elements down to `depth` with their placements, positions, dropped links and steps, and no edge yet.
    pub(crate) fn edgeless_down_to_depth(&self, depth: usize) -> Self {
        let elements = Self::entries_down_to_depth(&self.elements, depth);
        let child_placements = Self::entries_down_to_depth(&self.child_placements, depth);
        let positions = Self::entries_down_to_depth(&self.positions, depth);
        let unshown_link_targets = Self::entries_down_to_depth(&self.unshown_link_targets, depth);
        let element_steps = Self::entries_down_to_depth(&self.element_steps, depth);
        Self {
            elements,
            child_placements,
            positions,
            unshown_link_targets,
            edges: BTreeMap::new(),
            element_steps,
            edge_steps: BTreeMap::new(),
        }
    }

    /// Copy the entries of `by_path` whose path lies at `depth` or above.
    fn entries_down_to_depth<V: Clone>(
        by_path: &BTreeMap<ElementPath, V>,
        depth: usize,
    ) -> BTreeMap<ElementPath, V> {
        let kept_entries = by_path
            .iter()
            .filter(|(path, _value)| path.depth() <= depth);
        kept_entries
            .map(|(path, value)| (path.clone(), value.clone()))
            .collect()
    }

    /// Draw `edge` as `id`, taking part in `steps`.
    pub(crate) fn insert_edge(&mut self, id: EdgeId, edge: Edge, steps: BTreeSet<StepIndex>) {
        self.edges.insert(id.clone(), edge);
        self.edge_steps.insert(id, steps);
    }

    /// Yield the edges drawn as arcs: every edge but the badges.
    pub fn arc_edges(&self) -> impl Iterator<Item = (&EdgeId, &Edge)> {
        self.edges.iter().filter(|(_id, edge)| !edge.is_badge())
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

    /// Yield the shown elements one level below `parent`; the root yields the top-level elements.
    pub fn children<'a>(
        &'a self,
        parent: &'a ElementPath,
    ) -> impl Iterator<Item = (&'a ElementPath, &'a Element)> {
        let child_depth = parent.depth() + 1;
        self.descendants(parent)
            .filter(move |(path, _element)| path.depth() == child_depth)
    }

    /// Yield every shown element strictly below `ancestor`, in path order.
    pub fn descendants<'a>(
        &'a self,
        ancestor: &'a ElementPath,
    ) -> impl Iterator<Item = (&'a ElementPath, &'a Element)> {
        let after_ancestor = (Bound::Excluded(ancestor), Bound::Unbounded);
        self.elements
            .range::<ElementPath, _>(after_ancestor)
            .take_while(|(path, _element)| ancestor.is_ancestor_of(path))
    }

    /// Tell whether the element at `path` has shown children, which is what makes it a container in this workflow.
    pub fn is_container(&self, path: &ElementPath) -> bool {
        let mut descendants = self.descendants(path);
        descendants.next().is_some()
    }

    /// Return how the element at `path` places its children in this workflow, if the workflow says.
    pub fn child_placement(&self, path: &ElementPath) -> Option<PlacementHint> {
        self.child_placements.get(path).copied()
    }

    /// Return where the element at `path` is pinned in this workflow, if the workflow says.
    pub fn position(&self, path: &ElementPath) -> Option<Position> {
        self.positions.get(path).copied()
    }

    /// Return the component the link of the element at `path` names, when this workflow drops that link because it doesn't show the component.
    pub(crate) fn unshown_link_target(&self, path: &ElementPath) -> Option<&ElementPath> {
        self.unshown_link_targets.get(path)
    }

    /// Return the steps the element at `path` takes part in, in step order.
    pub fn steps_of_element(&self, path: &ElementPath) -> &BTreeSet<StepIndex> {
        self.element_steps.get(path).unwrap_or(NO_STEPS)
    }

    /// Return the steps listing the edge `id`, in step order.
    pub fn steps_of_edge(&self, id: &EdgeId) -> &BTreeSet<StepIndex> {
        self.edge_steps.get(id).unwrap_or(NO_STEPS)
    }

    /// Return the shown element at `path`, as this workflow draws it.
    pub fn element(&self, path: &ElementPath) -> Option<&Element> {
        self.elements.get(path)
    }

    /// Return every shown element keyed by its path, in path order.
    pub fn elements(&self) -> &BTreeMap<ElementPath, Element> {
        &self.elements
    }

    /// Return the edge identified by `id`, if this workflow draws it.
    pub fn edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Return every drawn edge keyed by its id, in edge order.
    pub fn edges(&self) -> &BTreeMap<EdgeId, Edge> {
        &self.edges
    }
}
