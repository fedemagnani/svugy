//! Resolve description link targets in the scope their description is written in, and report each one that doesn't resolve.

use std::cell::LazyCell;

use miette::SourceSpan;

use crate::description::{DescriptionOwner, DescriptionTarget, TargetLink};
use crate::graph::WorkflowGraph;
use crate::model::{EdgeId, Model};
use crate::path::ElementPath;
use crate::problem::{Problem, Suggestion, UnresolvedDescriptionTarget, UnresolvedTargetReason};
use crate::workflow::{StepId, Workflow, WorkflowId};

/// Resolve a description's link targets as the place it is written in decides, and report each one that doesn't resolve.
pub(super) trait TargetScope {
    /// Return why `target` doesn't resolve in this scope, once per workflow it fails in; empty when it resolves.
    fn unresolved_reasons(&self, target: &DescriptionTarget) -> Vec<UnresolvedTargetReason>;

    /// Return every target this scope resolves, spelled as after a link's `#`: the candidates a suggestion picks from.
    fn known_targets(&self) -> Vec<DescriptionTarget>;

    /// Report each link of `description`, written for `owner`, whose target doesn't resolve in this scope.
    fn check_description(
        &self,
        owner: &DescriptionOwner,
        description: &str,
        span: Option<SourceSpan>,
    ) -> Vec<Problem> {
        let known_targets = LazyCell::new(|| self.known_targets());
        let mut problems = Vec::new();
        for target_link in TargetLink::find_all(description) {
            let target_text = target_link.target;
            let parsed_target = DescriptionTarget::parse(target_text);
            let reasons = match parsed_target {
                Some(target) => self.unresolved_reasons(&target),
                None => vec![UnresolvedTargetReason::NamesNothing],
            };
            for reason in reasons {
                let suggestion = Suggestion::closest_to(target_text, known_targets.iter());
                let suggested_target = suggestion.map(|Suggestion(known_target)| {
                    let marked_target = format!("#{known_target}");
                    Suggestion(marked_target)
                });
                let unresolved_target = UnresolvedDescriptionTarget {
                    owner: owner.clone(),
                    target: target_text.to_owned(),
                    reason,
                    suggestion: suggested_target,
                    span,
                };
                problems.push(Problem::UnresolvedDescriptionTarget(unresolved_target));
            }
        }
        problems
    }
}

/// Scope a workflow's prose, its steps' prose and its edges' descriptions: components it shows, its own edges, and any step.
pub(super) struct WorkflowScope<'m> {
    model: &'m Model,
    workflow: &'m Workflow,
    graph: &'m WorkflowGraph,
}

/// Scope a component's description: any component, any step, and the edges of each workflow showing the component, resolved in each separately.
pub(super) struct ComponentScope<'m> {
    model: &'m Model,
    showing_workflows: Vec<&'m Workflow>,
}

impl<'m> WorkflowScope<'m> {
    /// Create a new scope for descriptions written inside `workflow`, which draws `graph`.
    pub(super) fn new(model: &'m Model, workflow: &'m Workflow, graph: &'m WorkflowGraph) -> Self {
        Self {
            model,
            workflow,
            graph,
        }
    }
}

impl<'m> ComponentScope<'m> {
    /// Create a new scope for the description of the component at `path`, each workflow of `model` drawing the graph of `graphs` at its index.
    pub(super) fn new(model: &'m Model, graphs: &'m [WorkflowGraph], path: &ElementPath) -> Self {
        let workflows_with_graphs = model.workflows().iter().zip(graphs);
        let showing_workflows = workflows_with_graphs
            .filter(|(_workflow, graph)| graph.element(path).is_some())
            .map(|(workflow, _graph)| workflow)
            .collect();
        Self {
            model,
            showing_workflows,
        }
    }
}

impl TargetScope for WorkflowScope<'_> {
    fn unresolved_reasons(&self, target: &DescriptionTarget) -> Vec<UnresolvedTargetReason> {
        let unresolved_reason = match target {
            DescriptionTarget::Element(path) => self.unshown_element_reason(path),
            DescriptionTarget::Edge(id) => self.workflow.numbered_edge_reason(id),
            DescriptionTarget::UnnumberedEdge { from, to } => {
                self.workflow.unnumbered_edge_reason(from, to)
            }
            DescriptionTarget::Step { workflow, step } => self.model.step_reason(workflow, step),
        };
        unresolved_reason.into_iter().collect()
    }

    fn known_targets(&self) -> Vec<DescriptionTarget> {
        let shown_paths = self.graph.elements().keys();
        let element_targets = shown_paths.cloned().map(DescriptionTarget::Element);
        let edge_ids = self.workflow.edges().keys();
        let edge_targets = edge_ids.cloned().map(DescriptionTarget::Edge);
        let step_targets = self.model.step_targets();
        element_targets
            .chain(edge_targets)
            .chain(step_targets)
            .collect()
    }
}

impl WorkflowScope<'_> {
    /// Tell why the element target `path` names no component this workflow shows.
    fn unshown_element_reason(&self, path: &ElementPath) -> Option<UnresolvedTargetReason> {
        let is_shown = self.graph.element(path).is_some();
        if is_shown {
            return None;
        }
        let is_declared = self.model.element(path).is_some();
        if is_declared {
            let workflow_id = self.workflow.id().clone();
            return Some(UnresolvedTargetReason::IsNotShownBy(workflow_id));
        }
        Some(UnresolvedTargetReason::NamesNothing)
    }
}

impl TargetScope for ComponentScope<'_> {
    fn unresolved_reasons(&self, target: &DescriptionTarget) -> Vec<UnresolvedTargetReason> {
        let showing_workflows = self.showing_workflows.iter();
        match target {
            DescriptionTarget::Element(path) => {
                let is_declared = self.model.element(path).is_some();
                if is_declared {
                    return Vec::new();
                }
                vec![UnresolvedTargetReason::NamesNothing]
            }
            DescriptionTarget::Step { workflow, step } => {
                let step_reason = self.model.step_reason(workflow, step);
                step_reason.into_iter().collect()
            }
            DescriptionTarget::Edge(id) => {
                let has_undeclared_endpoint = self.has_undeclared_endpoint(id.from(), id.to());
                if has_undeclared_endpoint {
                    return vec![UnresolvedTargetReason::NamesNothing];
                }
                showing_workflows
                    .filter_map(|workflow| workflow.numbered_edge_reason(id))
                    .collect()
            }
            DescriptionTarget::UnnumberedEdge { from, to } => {
                let has_undeclared_endpoint = self.has_undeclared_endpoint(from, to);
                if has_undeclared_endpoint {
                    return vec![UnresolvedTargetReason::NamesNothing];
                }
                showing_workflows
                    .filter_map(|workflow| workflow.unnumbered_edge_reason(from, to))
                    .collect()
            }
        }
    }

    fn known_targets(&self) -> Vec<DescriptionTarget> {
        let declared_paths = self.model.elements().keys();
        let element_targets = declared_paths.cloned().map(DescriptionTarget::Element);
        let showing_workflows = self.showing_workflows.iter();
        let edge_ids = showing_workflows.flat_map(|workflow| workflow.edges().keys());
        let edge_targets = edge_ids.cloned().map(DescriptionTarget::Edge);
        let step_targets = self.model.step_targets();
        element_targets
            .chain(edge_targets)
            .chain(step_targets)
            .collect()
    }
}

impl ComponentScope<'_> {
    /// Tell whether the edge target `from -> to` ends where no component is declared, so it names nothing in any workflow.
    fn has_undeclared_endpoint(&self, from: &ElementPath, to: &ElementPath) -> bool {
        let is_from_undeclared = self.model.element(from).is_none();
        let is_to_undeclared = self.model.element(to).is_none();
        is_from_undeclared || is_to_undeclared
    }
}

impl Model {
    /// Tell why the step target `workflow/step` names no step of the model.
    fn step_reason(&self, workflow: &WorkflowId, step: &StepId) -> Option<UnresolvedTargetReason> {
        let named_workflow = self.workflow(workflow);
        let step_index = named_workflow.and_then(|named_workflow| named_workflow.step_index(step));
        let is_step = step_index.is_some();
        if is_step {
            return None;
        }
        Some(UnresolvedTargetReason::NamesNothing)
    }

    /// Yield a target for every step of every workflow, in written order.
    fn step_targets(&self) -> impl Iterator<Item = DescriptionTarget> + '_ {
        let workflows = self.workflows().iter();
        workflows.flat_map(|workflow| {
            let steps = workflow.steps().iter();
            steps.map(|step| {
                let workflow = workflow.id().clone();
                let step = step.id().clone();
                DescriptionTarget::Step { workflow, step }
            })
        })
    }
}

impl Workflow {
    /// Tell why the numbered edge target `id` names no edge of this workflow.
    fn numbered_edge_reason(&self, id: &EdgeId) -> Option<UnresolvedTargetReason> {
        let is_edge = self.edge(id).is_some();
        if is_edge {
            return None;
        }
        let workflow_id = self.id().clone();
        Some(UnresolvedTargetReason::NamesNothingIn(workflow_id))
    }

    /// Tell why the unnumbered edge target `from -> to` names no single edge of this workflow.
    fn unnumbered_edge_reason(
        &self,
        from: &ElementPath,
        to: &ElementPath,
    ) -> Option<UnresolvedTargetReason> {
        let only_edge = self.only_edge_between(from, to);
        let is_single_edge_target = only_edge.is_some();
        if is_single_edge_target {
            return None;
        }
        let workflow_id = self.id().clone();
        let mut joining_edges = self.edges_between(from, to);
        let has_joining_edges = joining_edges.next().is_some();
        if has_joining_edges {
            return Some(UnresolvedTargetReason::NamesSeveralEdgesIn(workflow_id));
        }
        Some(UnresolvedTargetReason::NamesNothingIn(workflow_id))
    }
}
