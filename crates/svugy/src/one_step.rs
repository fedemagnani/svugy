//! Grow the models unit tests lay out and render: one workflow whose one step shows everything added.

use svugy_model::builder::{ElementRef, ModelBuilder, StepRef, WorkflowRef};
use svugy_model::graph::WorkflowGraph;
use svugy_model::model::{Edge, EdgeId, Element, Model};
use svugy_model::workflow::{ElementOverride, Step, StepId, Workflow, WorkflowId};

/// Grows a model of one workflow whose one step lists every edge and every element added, so the workflow draws all of them.
pub(crate) struct OneStepModelBuilder {
    builder: ModelBuilder,
    workflow: WorkflowRef,
    step: StepRef,
}

impl OneStepModelBuilder {
    /// Create a new builder of workflow `main`, titled `main`.
    pub(crate) fn new() -> Self {
        let main_id = WorkflowId::new("main");
        let main_workflow = Workflow::new(main_id, "main");
        Self::of_workflow(main_workflow)
    }

    /// Create a new builder of `workflow`, adding to it the one step `main`, titled `main`.
    pub(crate) fn of_workflow(workflow: Workflow) -> Self {
        let mut builder = ModelBuilder::new();
        let workflow = builder.add_workflow(workflow);
        let main_step_id = StepId::new("main");
        let main_step = Step::new(main_step_id, "main");
        let step = builder.add_step(&workflow, main_step);
        Self {
            builder,
            workflow,
            step,
        }
    }

    /// Add `element` as a top-level element named `id`, returning its handle.
    pub(crate) fn add_top_level(&mut self, id: &str, element: Element) -> ElementRef {
        self.builder.add_top_level(id, element)
    }

    /// Add `element` as a child named `id` of `parent`, returning its handle.
    pub(crate) fn add_child(
        &mut self,
        parent: &ElementRef,
        id: &str,
        element: Element,
    ) -> ElementRef {
        self.builder.add_child(parent, id, element)
    }

    /// Declare `edge` from `from` to `to` in the one step, returning its id in the one workflow.
    pub(crate) fn add_edge(&mut self, from: &ElementRef, to: &ElementRef, edge: Edge) -> EdgeId {
        let edge_ref = self.builder.add_edge(&self.step, from, to, edge);
        edge_ref.id().clone()
    }

    /// Override how the workflow draws `element`, as a workflow pins it or places its children.
    pub(crate) fn override_element(
        &mut self,
        element: &ElementRef,
        element_override: ElementOverride,
    ) {
        self.builder
            .override_element(&self.workflow, element, element_override);
    }

    /// List every element added in the one step, then admit the model, panicking on any error.
    pub(crate) fn build(mut self) -> Model {
        self.builder.list_every_element(&self.step);
        let admission = self.builder.build();
        admission.model.expect("a one-step fixture model is valid")
    }

    /// Build the model, then project its one workflow into the graph layout and geometry read.
    pub(crate) fn build_graph(self) -> WorkflowGraph {
        let model = self.build();
        let main_workflow = model.showing_first_workflow();
        let projection = main_workflow.projected();
        projection.into_graph()
    }
}
