//! Build a model through element, workflow and step handles, so ancestors exist and each edge lands in its workflow by construction.

use crate::model::{ByteRange, Edge, EdgeId, Element, ViewAngle};
use crate::path::ElementPath;
use crate::validate::{ModelAdmission, UncheckedModel};
use crate::workflow::{ElementOverride, ListedItem, Step, StepIndex, StepItem, Workflow};

/// Grow a model through element, workflow and step handles, then admit it through every check.
#[derive(Debug, Default)]
pub struct ModelBuilder {
    unchecked: UncheckedModel,
}

impl ModelBuilder {
    /// Create a new builder holding an empty model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the diagram from `view` first.
    pub fn with_view(mut self, view: ViewAngle) -> Self {
        self.unchecked = self.unchecked.with_view(view);
        self
    }

    /// Add `element` as a top-level element named `id`, returning its handle.
    pub fn add_top_level(&mut self, id: &str, element: Element) -> ElementRef {
        let path = ElementPath::root();
        let root = ElementRef { path };
        self.add_child(&root, id, element)
    }

    /// Add `element` as a child named `id` of `parent`, keeping the first declaration and recording any later one as a duplicate.
    pub fn add_child(&mut self, parent: &ElementRef, id: &str, element: Element) -> ElementRef {
        let path = parent.path.child(id);
        let elements = &mut self.unchecked.model.elements;
        let is_duplicate = elements.contains_key(&path);
        if is_duplicate {
            let duplicate_span = element.source_span();
            let duplicate_declaration = (path.clone(), duplicate_span);
            self.unchecked
                .duplicate_declarations
                .push(duplicate_declaration);
            return ElementRef { path };
        }
        elements.insert(path.clone(), element);
        ElementRef { path }
    }

    /// Add `workflow` after the workflows already added, returning its handle.
    pub fn add_workflow(&mut self, workflow: Workflow) -> WorkflowRef {
        let workflows = &mut self.unchecked.model.workflows;
        let position = workflows.len();
        workflows.push(workflow);
        WorkflowRef { position }
    }

    /// Override how `workflow` draws `element`, keeping the first override and recording any later one as a duplicate.
    pub fn override_element(
        &mut self,
        workflow: &WorkflowRef,
        element: &ElementRef,
        element_override: ElementOverride,
    ) {
        let path = element.path.clone();
        let overriding_workflow = self.workflow_mut(workflow);
        let is_duplicate = overriding_workflow.overrides.contains_key(&path);
        if is_duplicate {
            let workflow_id = overriding_workflow.id().clone();
            let duplicate_span = element_override.source_span();
            let duplicate_override = (workflow_id, path, duplicate_span);
            self.unchecked.duplicate_overrides.push(duplicate_override);
            return;
        }
        overriding_workflow.overrides.insert(path, element_override);
    }

    /// Add `step` after `workflow`'s steps already added, returning its handle.
    pub fn add_step(&mut self, workflow: &WorkflowRef, step: Step) -> StepRef {
        let stepping_workflow = self.workflow_mut(workflow);
        let position = stepping_workflow.steps.len();
        stepping_workflow.steps.push(step);
        let workflow = workflow.clone();
        let step = StepIndex::new(position);
        StepRef { workflow, step }
    }

    /// Declare `edge` in the step's workflow with the next free ordinal for its endpoint pair, list it in the step, and return its handle.
    pub fn add_edge(
        &mut self,
        step: &StepRef,
        from: &ElementRef,
        to: &ElementRef,
        edge: Edge,
    ) -> EdgeRef {
        let from_path = from.path.clone();
        let to_path = to.path.clone();
        let declaring_workflow = self.workflow_mut(&step.workflow);
        let id = EdgeId::next_free_in(&declaring_workflow.edges, from_path, to_path);
        declaring_workflow.edges.insert(id.clone(), edge);
        let listed_edge = StepItem::Edge(id.clone());
        self.list_item_written_at(step, listed_edge, None);
        let workflow = step.workflow.clone();
        EdgeRef { workflow, id }
    }

    /// List again in `step` the edge `id`, which any step of its workflow declares; validation reports one that none does.
    pub fn list_edge(&mut self, step: &StepRef, id: EdgeId) {
        self.list_edge_written_at(step, id, None);
    }

    /// List again in `step` the edge `id`, written at `span` of an imported source when there is one.
    pub fn list_edge_written_at(&mut self, step: &StepRef, id: EdgeId, span: Option<ByteRange>) {
        let listed_edge = StepItem::Edge(id);
        self.list_item_written_at(step, listed_edge, span);
    }

    /// List `element` in `step`, shown without an edge.
    pub fn list_element(&mut self, step: &StepRef, element: &ElementRef) {
        self.list_element_written_at(step, element, None);
    }

    /// List `element` in `step`, shown without an edge, written at `span` of an imported source when there is one.
    pub fn list_element_written_at(
        &mut self,
        step: &StepRef,
        element: &ElementRef,
        span: Option<ByteRange>,
    ) {
        let path = element.path.clone();
        let listed_element = StepItem::Element(path);
        self.list_item_written_at(step, listed_element, span);
    }

    /// List in `step` every element added so far, in path order, each shown without an edge.
    pub fn list_every_element(&mut self, step: &StepRef) {
        let element_paths = self.unchecked.model.elements.keys();
        let listed_elements: Vec<StepItem> =
            element_paths.cloned().map(StepItem::Element).collect();
        for listed_element in listed_elements {
            self.list_item_written_at(step, listed_element, None);
        }
    }

    /// Describe in Markdown the element, workflow, step or workflow edge `owner` names.
    pub fn describe(&mut self, owner: &DescribedRef, description: impl Into<String>) {
        let description = description.into();
        let description_slot = match owner {
            DescribedRef::Element(element) => &mut self.element_mut(element).description,
            DescribedRef::Workflow(workflow) => &mut self.workflow_mut(workflow).description,
            DescribedRef::Step(step) => &mut self.step_mut(step).description,
            DescribedRef::Edge(edge) => &mut self.edge_mut(edge).description,
        };
        *description_slot = Some(description);
    }

    /// Admit the built model unless a check finds an error, with the report of every problem found, warnings included.
    pub fn build(self) -> ModelAdmission {
        self.unchecked.admit()
    }

    /// Return the element `element` names, which this builder added.
    fn element_mut(&mut self, element: &ElementRef) -> &mut Element {
        let elements = &mut self.unchecked.model.elements;
        elements
            .get_mut(&element.path)
            .expect("an element handle names an element its own builder added")
    }

    /// Return the edge `edge` names, which `add_edge` declared.
    fn edge_mut(&mut self, edge: &EdgeRef) -> &mut Edge {
        let declaring_workflow = self.workflow_mut(&edge.workflow);
        declaring_workflow
            .edges
            .get_mut(&edge.id)
            .expect("an edge handle names an edge its workflow's `add_edge` declared")
    }

    /// Return the workflow `workflow` names, which this builder added.
    fn workflow_mut(&mut self, workflow: &WorkflowRef) -> &mut Workflow {
        let workflows = &mut self.unchecked.model.workflows;
        workflows
            .get_mut(workflow.position)
            .expect("a workflow handle names a workflow its own builder added")
    }

    /// Append `item`, written at `span`, to the items of `step`.
    fn list_item_written_at(&mut self, step: &StepRef, item: StepItem, span: Option<ByteRange>) {
        let listed_item = ListedItem { item, span };
        let listing_step = self.step_mut(step);
        listing_step.items.push(listed_item);
    }

    /// Return the step `step` names, which this builder added.
    fn step_mut(&mut self, step: &StepRef) -> &mut Step {
        let stepping_workflow = self.workflow_mut(&step.workflow);
        let position = step.step.position();
        stepping_workflow
            .steps
            .get_mut(position)
            .expect("a step handle names a step its own builder added")
    }

    /// Return the workflow `workflow` names as built so far, unchecked.
    pub fn workflow(&self, workflow: &WorkflowRef) -> &Workflow {
        let workflows = &self.unchecked.model.workflows;
        workflows
            .get(workflow.position)
            .expect("a workflow handle names a workflow its own builder added")
    }
}

/// Handle to an element already added to a builder, the only way to name a parent, an edge endpoint or an overridden element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementRef {
    path: ElementPath,
}

/// Handle to a workflow already added to a builder, the only way to override an element or add a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRef {
    position: usize,
}

/// Handle to a step already added to a builder, the only way to declare or list an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRef {
    workflow: WorkflowRef,
    step: StepIndex,
}

/// Handle to an edge already declared through a builder, its id paired with the workflow that numbers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeRef {
    workflow: WorkflowRef,
    id: EdgeId,
}

/// Name what a description describes by the handles a builder returned: an element, a workflow, a step or an edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescribedRef {
    Element(ElementRef),
    Workflow(WorkflowRef),
    Step(StepRef),
    Edge(EdgeRef),
}

impl ElementRef {
    /// Return the path of the element this handle names.
    pub fn path(&self) -> &ElementPath {
        &self.path
    }
}

impl EdgeRef {
    /// Return the id of the edge this handle names, ordinal counted within its workflow.
    pub fn id(&self) -> &EdgeId {
        &self.id
    }
}
