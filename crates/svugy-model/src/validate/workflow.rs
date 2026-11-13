//! Check one workflow: its steps, its edges, its overrides, the components it shows and the descriptions written inside it.

use std::collections::BTreeSet;

use miette::SourceSpan;

use crate::description::DescriptionOwner;
use crate::graph::WorkflowGraph;
use crate::model::{Element, MAX_MAGNITUDE, Model, PlacementHint, Shape};
use crate::path::ElementPath;
use crate::problem::{
    BadgeEdgeOnContainer, ChildPlacementWithoutChildren, DanglingEdgeEndpoint, DuplicateStep,
    DuplicateStepItem, ElementSite, EmptyGridHint, IgnoredContainerHeight, IgnoredContainerShape,
    LineageEdge, MalformedStepId, OverrideOfUnshownElement, Problem, StepWithoutItems, StyleOwner,
    Suggestion, UnboundedPosition, UncountablePackedTracks, UnknownStepItem, UnlistedEdge,
    UnshownLinkTarget, WorkflowWithoutSteps,
};
use crate::workflow::{ElementOverride, Step, StepItem, Workflow};

use super::target::{TargetScope, WorkflowScope};

/// Check one workflow of a model against the graph it draws.
pub(super) struct WorkflowCheck<'m> {
    model: &'m Model,
    workflow: &'m Workflow,
    graph: &'m WorkflowGraph,
}

impl<'m> WorkflowCheck<'m> {
    /// Create a new check of `workflow`, which draws `graph`.
    pub(super) fn new(model: &'m Model, workflow: &'m Workflow, graph: &'m WorkflowGraph) -> Self {
        Self {
            model,
            workflow,
            graph,
        }
    }

    /// Report every problem of the workflow: steps first, then edges, overrides, shown components and descriptions.
    pub(super) fn problems(&self) -> Vec<Problem> {
        let step_problems = self.find_step_problems();
        let edge_problems = self.find_edge_problems();
        let override_problems = self.find_override_problems();
        let shown_element_problems = self.find_shown_element_problems();
        let description_problems = self.find_description_problems();
        step_problems
            .into_iter()
            .chain(edge_problems)
            .chain(override_problems)
            .chain(shown_element_problems)
            .chain(description_problems)
            .collect()
    }

    /// Report a workflow without steps, then per step a malformed or repeated id, no items, a repeated item, an item naming nothing.
    fn find_step_problems(&self) -> Vec<Problem> {
        let workflow_id = self.workflow.id();
        let steps = self.workflow.steps();
        let has_no_step = steps.is_empty();
        if has_no_step {
            let span = self.workflow.source_span();
            let workflow_without_steps = WorkflowWithoutSteps {
                workflow: workflow_id.clone(),
                span,
            };
            return vec![Problem::WorkflowWithoutSteps(workflow_without_steps)];
        }
        let mut seen_step_ids = BTreeSet::new();
        let mut problems = Vec::new();
        for step in steps {
            let step_id = step.id();
            let span = step.source_span();
            let is_malformed = !step_id.is_well_formed();
            if is_malformed {
                let malformed_step_id = MalformedStepId {
                    workflow: workflow_id.clone(),
                    step: step_id.clone(),
                    span,
                };
                problems.push(Problem::MalformedStepId(malformed_step_id));
            }
            let is_repeated_id = !seen_step_ids.insert(step_id);
            if is_repeated_id {
                let duplicate_step = DuplicateStep {
                    workflow: workflow_id.clone(),
                    step: step_id.clone(),
                    span,
                };
                problems.push(Problem::DuplicateStep(duplicate_step));
            }
            let item_problems = self.find_item_problems(step);
            problems.extend(item_problems);
        }
        problems
    }

    /// Report a step listing nothing, then each item it lists twice or that names nothing, labelled where the item was listed, else at the step.
    fn find_item_problems(&self, step: &Step) -> Vec<Problem> {
        let workflow_id = self.workflow.id();
        let step_id = step.id();
        let span = step.source_span();
        let listed_items = step.listed_items();
        let has_no_item = listed_items.is_empty();
        if has_no_item {
            let step_without_items = StepWithoutItems {
                workflow: workflow_id.clone(),
                step: step_id.clone(),
                span,
            };
            return vec![Problem::StepWithoutItems(step_without_items)];
        }
        let mut seen_items = BTreeSet::new();
        let mut problems = Vec::new();
        for listed_item in listed_items {
            let item = listed_item.item();
            let item_span = listed_item.source_span().or(span);
            let is_repeated_item = !seen_items.insert(item);
            if is_repeated_item {
                let duplicate_item = DuplicateStepItem {
                    workflow: workflow_id.clone(),
                    step: step_id.clone(),
                    item: item.clone(),
                    span: item_span,
                };
                problems.push(Problem::DuplicateStepItem(duplicate_item));
                continue;
            }
            let is_known_item = self.is_known_item(item);
            if is_known_item {
                continue;
            }
            let suggestion = self.suggest_known_item(item);
            let unknown_item = UnknownStepItem {
                workflow: workflow_id.clone(),
                step: step_id.clone(),
                item: item.clone(),
                suggestion,
                span: item_span,
            };
            problems.push(Problem::UnknownStepItem(unknown_item));
        }
        problems
    }

    /// Tell whether `item` names an edge of this workflow or an element of the model.
    fn is_known_item(&self, item: &StepItem) -> bool {
        match item {
            StepItem::Edge(id) => self.workflow.edge(id).is_some(),
            StepItem::Element(path) => self.model.element(path).is_some(),
        }
    }

    /// Suggest the edge of this workflow or the element of the model closest to the unknown `item`, matching its kind.
    fn suggest_known_item(&self, item: &StepItem) -> Option<Suggestion<String>> {
        match item {
            StepItem::Edge(id) => {
                let unknown_text = id.to_string();
                let edge_ids = self.workflow.edges().keys();
                let known_edges = edge_ids.map(ToString::to_string);
                Suggestion::closest_to(&unknown_text, known_edges)
            }
            StepItem::Element(path) => {
                let unknown_text = path.to_string();
                let declared_paths = self.model.elements().keys();
                let known_paths = declared_paths.map(ToString::to_string);
                Suggestion::closest_to(&unknown_text, known_paths)
            }
        }
    }

    /// Report per edge: no step listing it, an endpoint naming nothing, a lineage edge, its style entries, a badge on a container.
    fn find_edge_problems(&self) -> Vec<Problem> {
        let workflow_id = self.workflow.id();
        let mut problems = Vec::new();
        for (id, edge) in self.workflow.edges() {
            let span = edge.source_span();
            let is_unlisted = self.graph.steps_of_edge(id).is_empty();
            if is_unlisted {
                let unlisted_edge = UnlistedEdge {
                    workflow: workflow_id.clone(),
                    edge: id.clone(),
                    span,
                };
                problems.push(Problem::UnlistedEdge(unlisted_edge));
            }
            for endpoint in BTreeSet::from([id.from(), id.to()]) {
                let is_element = self.model.element(endpoint).is_some();
                if is_element {
                    continue;
                }
                let endpoint_text = endpoint.to_string();
                let known_paths = self.model.elements().keys().cloned();
                let suggestion = Suggestion::closest_to(&endpoint_text, known_paths);
                let dangling_endpoint = DanglingEdgeEndpoint {
                    workflow: workflow_id.clone(),
                    edge: id.clone(),
                    endpoint: endpoint.clone(),
                    suggestion,
                    span,
                };
                problems.push(Problem::DanglingEdgeEndpoint(dangling_endpoint));
            }
            let is_lineage_edge = id.joins_element_to_itself_or_its_lineage();
            if is_lineage_edge {
                let lineage_edge = LineageEdge {
                    workflow: workflow_id.clone(),
                    edge: id.clone(),
                    span,
                };
                problems.push(Problem::LineageEdge(lineage_edge));
            }
            let owner = StyleOwner::Edge {
                workflow: workflow_id.clone(),
                edge: id.clone(),
            };
            let style_entries = edge.style().iter();
            let style_problems =
                style_entries.filter_map(|(key, value)| owner.check_style_entry(key, value, span));
            problems.extend(style_problems);
            let is_from_container = self.graph.is_container(id.from());
            let is_to_container = self.graph.is_container(id.to());
            let is_badge_on_container = edge.is_badge() && (is_from_container || is_to_container);
            if is_badge_on_container {
                let badge_edge_on_container = BadgeEdgeOnContainer {
                    workflow: workflow_id.clone(),
                    edge: id.clone(),
                    span,
                };
                problems.push(Problem::BadgeEdgeOnContainer(badge_edge_on_container));
            }
        }
        problems
    }

    /// Report per override: an element the workflow doesn't show, each property value that does not fit, each placement no layout can honour.
    fn find_override_problems(&self) -> Vec<Problem> {
        let workflow_id = self.workflow.id();
        let mut problems = Vec::new();
        for (path, element_override) in self.workflow.overrides() {
            let span = element_override.source_span();
            let is_unshown = self.graph.element(path).is_none();
            if is_unshown {
                let path_text = path.to_string();
                let shown_paths = self.graph.elements().keys().cloned();
                let suggestion = Suggestion::closest_to(&path_text, shown_paths);
                let unshown_override = OverrideOfUnshownElement {
                    workflow: workflow_id.clone(),
                    element: path.clone(),
                    suggestion,
                    span,
                };
                problems.push(Problem::OverrideOfUnshownElement(unshown_override));
                continue;
            }
            let site = ElementSite::Override {
                workflow: workflow_id.clone(),
                element: path.clone(),
            };
            let elements = self.model.elements();
            let overridden_properties = element_override.properties();
            let property_problems = site.check_properties(overridden_properties, span, elements);
            let placement_problems = self.find_placement_problems(path, element_override);
            problems.extend(property_problems);
            problems.extend(placement_problems);
        }
        problems
    }

    /// Report each placement of the override no layout can honour: an empty or uncountable grid, a hint with no shown children, an unbounded position.
    fn find_placement_problems(
        &self,
        path: &ElementPath,
        element_override: &ElementOverride,
    ) -> Vec<Problem> {
        let workflow_id = self.workflow.id();
        let span = element_override.source_span();
        let mut problems = Vec::new();
        if let Some(position) = element_override.position() {
            let is_x_bounded = position.x.abs() <= MAX_MAGNITUDE;
            let is_y_bounded = position.y.abs() <= MAX_MAGNITUDE;
            let is_unbounded = !is_x_bounded || !is_y_bounded;
            if is_unbounded {
                let unbounded_position = UnboundedPosition {
                    workflow: workflow_id.clone(),
                    element: path.clone(),
                    position,
                    span,
                };
                problems.push(Problem::UnboundedPosition(unbounded_position));
            }
        }
        let Some(child_placement) = element_override.child_placement() else {
            return problems;
        };
        let is_empty_grid = child_placement == PlacementHint::Grid { columns: 0 };
        if is_empty_grid {
            let empty_grid = EmptyGridHint {
                workflow: workflow_id.clone(),
                element: path.clone(),
                span,
            };
            problems.push(Problem::EmptyGridHint(empty_grid));
        }
        let is_leaf = !self.graph.is_container(path);
        if is_leaf {
            let placement_without_children = ChildPlacementWithoutChildren {
                workflow: workflow_id.clone(),
                element: path.clone(),
                child_placement,
                span,
            };
            problems.push(Problem::ChildPlacementWithoutChildren(
                placement_without_children,
            ));
        }
        let child_count = self.graph.children(path).count();
        let Some((columns, rows)) = child_placement.packed_tracks(child_count) else {
            return problems;
        };
        let max_tracks = PlacementHint::MAX_PACKED_TRACKS as usize;
        let is_uncountable = columns > max_tracks || rows > max_tracks;
        if is_uncountable {
            let uncountable_tracks = UncountablePackedTracks {
                workflow: workflow_id.clone(),
                element: path.clone(),
                child_count,
                columns,
                rows,
                span,
            };
            problems.push(Problem::UncountablePackedTracks(uncountable_tracks));
        }
        problems
    }

    /// Report per shown component: a container's ignored shape or height, a link to a component this workflow doesn't show.
    fn find_shown_element_problems(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        for (path, element) in self.graph.elements() {
            let is_container = self.graph.is_container(path);
            if is_container {
                let ignored_body = self.find_ignored_container_body(path, element);
                problems.extend(ignored_body);
            }
            let unshown_link_target = self.check_link_target_is_shown(path);
            problems.extend(unshown_link_target);
        }
        problems
    }

    /// Report the container's shape other than a slab and its height, which this workflow ignores, each labelled where it was written.
    fn find_ignored_container_body(&self, path: &ElementPath, element: &Element) -> Vec<Problem> {
        let workflow_id = self.workflow.id();
        let element_override = self.workflow.override_of(path);
        let element_properties = element.properties();
        let mut problems = Vec::new();
        let ignored_shape = element_properties
            .shape()
            .filter(|shape| *shape != Shape::Slab);
        if let Some(shape) = ignored_shape {
            let shape_override =
                element_override.filter(|written| written.properties().shape().is_some());
            let span =
                shape_override.map_or_else(|| element.source_span(), ElementOverride::source_span);
            let ignored_container_shape = IgnoredContainerShape {
                workflow: workflow_id.clone(),
                element: path.clone(),
                shape,
                span,
            };
            problems.push(Problem::IgnoredContainerShape(ignored_container_shape));
        }
        if let Some(height) = element_properties.height() {
            let height_override =
                element_override.filter(|written| written.properties().height().is_some());
            let span =
                height_override.map_or_else(|| element.source_span(), ElementOverride::source_span);
            let ignored_container_height = IgnoredContainerHeight {
                workflow: workflow_id.clone(),
                element: path.clone(),
                height,
                span,
            };
            problems.push(Problem::IgnoredContainerHeight(ignored_container_height));
        }
        problems
    }

    /// Report the link the projection dropped from the shown component because it names a component this workflow doesn't show, labelled where the link was written.
    fn check_link_target_is_shown(&self, path: &ElementPath) -> Option<Problem> {
        let target = self.graph.unshown_link_target(path)?;
        let element = &self.model.elements[path];
        let element_override = self.workflow.override_of(path);
        let link_override =
            element_override.filter(|written| written.properties().link().is_some());
        let link_properties =
            link_override.map_or_else(|| element.properties(), ElementOverride::properties);
        let link_span = link_properties.link_span().map(SourceSpan::from);
        let link_site_span =
            link_override.map_or_else(|| element.source_span(), ElementOverride::source_span);
        let span = link_span.or(link_site_span);
        let unshown_link_target = UnshownLinkTarget {
            workflow: self.workflow.id().clone(),
            element: path.clone(),
            target: target.clone(),
            span,
        };
        Some(Problem::UnshownLinkTarget(unshown_link_target))
    }

    /// Report each link of the workflow's prose, its steps' prose and its edges' descriptions whose target doesn't resolve in this workflow.
    fn find_description_problems(&self) -> Vec<Problem> {
        let scope = WorkflowScope::new(self.model, self.workflow, self.graph);
        let workflow_id = self.workflow.id();
        let mut problems = Vec::new();
        if let Some(description) = self.workflow.description() {
            let owner = DescriptionOwner::Workflow(workflow_id.clone());
            let span = self.workflow.source_span();
            let description_problems = scope.check_description(&owner, description, span);
            problems.extend(description_problems);
        }
        for step in self.workflow.steps() {
            let Some(description) = step.description() else {
                continue;
            };
            let owner = DescriptionOwner::Step(workflow_id.clone(), step.id().clone());
            let span = step.source_span();
            let description_problems = scope.check_description(&owner, description, span);
            problems.extend(description_problems);
        }
        for (id, edge) in self.workflow.edges() {
            let Some(description) = edge.description() else {
                continue;
            };
            let owner = DescriptionOwner::Edge(workflow_id.clone(), id.clone());
            let span = edge.source_span();
            let description_problems = scope.check_description(&owner, description, span);
            problems.extend(description_problems);
        }
        problems
    }
}
