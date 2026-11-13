//! Annotate element, edge and stub groups, and each workflow's group, with the markup the runtime contract puts on them.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use svugy_model::disclose::DisclosedGraph;
use svugy_model::graph::WorkflowGraph;
use svugy_model::link::Link;
use svugy_model::model::{Edge, EdgeId, Element, ProjectedWorkflow};
use svugy_model::path::ElementPath;
use svugy_model::workflow::{Step, StepId, StepIndex, StepItem, Workflow, WorkflowId};

use super::RuntimeContract;
use crate::render::writer::url::SafeUrl;
use crate::render::writer::{Attributes, SvgWriter};

/// Annotates element, edge and stub groups with the contract, reading labels, captions, code references, metadata, links and descriptions from a projected workflow's graph, step ids from its workflow, and closed containers, merged edges and step memberships from the layer's disclosure.
pub(crate) struct ContractAnnotator<'m> {
    workflow: &'m Workflow,
    graph: &'m WorkflowGraph,
    disclosure: Option<&'m DisclosedGraph>,
    carries_descriptions: bool,
}

/// Describes a workflow as the runtime's description panel reads it: its heading, its overview and its steps.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "workflow")]
pub(crate) struct WorkflowDetails<'m> {
    id: &'m WorkflowId,
    title: &'m str,
    #[serde(skip_serializing_if = "str::is_empty")]
    subtitle: &'m str,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    steps: Vec<StepDetails<'m>>,
}

/// Describes one step as the runtime's steps card reads it, with the shallowest layer drawing every item it lists.
#[derive(Serialize)]
struct StepDetails<'m> {
    id: &'m StepId,
    title: &'m str,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    layer: usize,
    items: Vec<StepItemDetails<'m>>,
}

/// Names one item a step lists, as the runtime matches it against group attributes.
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum StepItemDetails<'m> {
    Edge(String),
    Element(&'m ElementPath),
}

/// Wraps one group's content in the contract's markup: an anchor for a URL link, the group's attributes, its `<title>`, then its details if it has any.
pub(crate) struct ContractGroup {
    attributes: Attributes,
    title: String,
    details: Option<String>,
    url: Option<SafeUrl>,
}

/// Describes an element as the runtime's component card reads it, empty keys left out.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "element")]
struct ElementDetails<'a> {
    path: &'a ElementPath,
    #[serde(skip_serializing_if = "str::is_empty")]
    label: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    caption: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    code: &'a str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    metadata: &'a BTreeMap<String, String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
}

/// Describes an edge as the runtime's component card reads it, empty keys left out.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "edge")]
struct EdgeDetails<'a> {
    edge: String,
    from: &'a ElementPath,
    to: &'a ElementPath,
    #[serde(skip_serializing_if = "str::is_empty")]
    label: &'a str,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
}

/// Describes a lifted connection as the runtime's component card reads it: the shown endpoints it is drawn between and every source edge it stands for.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "lifted")]
struct LiftedDetails<'a> {
    from: &'a ElementPath,
    to: &'a ElementPath,
    #[serde(skip_serializing_if = "str::is_empty")]
    label: &'a str,
    edges: Vec<EdgeDetails<'a>>,
}

/// Describes one connection a layer draws: a source edge as written, or a lifted connection standing for several.
#[derive(Serialize)]
#[serde(untagged)]
enum ConnectionDetails<'a> {
    Written(EdgeDetails<'a>),
    Lifted(LiftedDetails<'a>),
}

impl<'m> ContractAnnotator<'m> {
    /// Build an annotator reading the graph of `projection`, drawn whole by its workflow.
    pub(crate) fn new(projection: &'m ProjectedWorkflow<'m>) -> Self {
        let workflow = projection.workflow();
        let graph = projection.graph();
        Self {
            workflow,
            graph,
            disclosure: None,
            carries_descriptions: false,
        }
    }

    /// Read closed containers, merged edges and step memberships from `disclosure`, the layer of the workflow's graph this annotator's groups are drawn in.
    pub(crate) fn with_disclosure(mut self, disclosure: &'m DisclosedGraph) -> Self {
        self.disclosure = Some(disclosure);
        self
    }

    /// Carry the authored descriptions in the details, for the runtime's description panel.
    pub(crate) fn with_descriptions(mut self) -> Self {
        self.carries_descriptions = true;
        self
    }

    /// Annotate the group of element `path`'s solid, titled by its label, marked when its layer draws it closed, with the steps it takes part in and where its link leads: a level to enter, or a URL that passed the policy.
    pub(crate) fn element_group(&self, path: &ElementPath) -> ContractGroup {
        let path_value = path.to_string();
        let element_attributes = Attributes::new().with_text(RuntimeContract::ELEMENT, &path_value);
        let is_closed = self.is_closed(path);
        let closing_attributes = if is_closed {
            element_attributes.with_text(RuntimeContract::CLOSED, "")
        } else {
            element_attributes
        };
        let drawn_graph = self.drawn_graph();
        let element_steps = drawn_graph.steps_of_element(path);
        let attributes = self.add_step_ids(closing_attributes, element_steps);
        let Some(element) = self.graph.element(path) else {
            let bare_details = ElementDetails::bare(path);
            let bare_group = ContractGroup::new(attributes, path_value);
            return bare_group.with_details(&bare_details);
        };
        let authored_description = element.description();
        let description = self.carried_description(authored_description);
        let details = ElementDetails::of(path, element, description);
        let title = element.label().to_owned();
        let properties = element.properties();
        let Some(raw_link) = properties.link() else {
            let unlinked_group = ContractGroup::new(attributes, title);
            return unlinked_group.with_details(&details);
        };
        let shown_elements = self.graph.elements();
        let link = Link::resolve(raw_link, shown_elements);
        match link {
            Link::Element(target) => {
                let entered_level = self.level_entered_at(&target);
                let entered_level_value = entered_level.to_string();
                let goto_attributes =
                    attributes.with_text(RuntimeContract::GOTO, &entered_level_value);
                let goto_group = ContractGroup::new(goto_attributes, title);
                goto_group.with_details(&details)
            }
            Link::Url(raw_url) => {
                let undetailed_group = ContractGroup::new(attributes, title);
                let group = undetailed_group.with_details(&details);
                let Some(url) = SafeUrl::parse(raw_url) else {
                    return group;
                };
                group.with_url(url)
            }
        }
    }

    /// Annotate `edge_id`'s connection with the endpoints its layer draws it between and the steps it takes part in, a written edge titled by its label, else by its identity, a lifted one by its label, else by its endpoints' labels.
    pub(crate) fn edge_group(&self, edge_id: &EdgeId) -> ContractGroup {
        let edge_value = edge_id.to_string();
        let from = edge_id.from();
        let to = edge_id.to();
        let from_value = from.to_string();
        let to_value = to.to_string();
        let endpoint_attributes = Attributes::new()
            .with_text(RuntimeContract::EDGE, &edge_value)
            .with_text(RuntimeContract::FROM, &from_value)
            .with_text(RuntimeContract::TO, &to_value);
        let drawn_graph = self.drawn_graph();
        let edge_steps = drawn_graph.steps_of_edge(edge_id);
        let attributes = self.add_step_ids(endpoint_attributes, edge_steps);
        let connection_details = self.connection_details(edge_id);
        match connection_details {
            ConnectionDetails::Written(details) => {
                let is_unlabelled = details.label.is_empty();
                let title = if is_unlabelled {
                    edge_value
                } else {
                    details.label.to_owned()
                };
                let written_group = ContractGroup::new(attributes, title);
                written_group.with_details(&details)
            }
            ConnectionDetails::Lifted(details) => {
                let is_unlabelled = details.label.is_empty();
                let title = if is_unlabelled {
                    self.endpoint_labels_joined(edge_id)
                } else {
                    details.label.to_owned()
                };
                let lifted_group = ContractGroup::new(attributes, title);
                lifted_group.with_details(&details)
            }
        }
    }

    /// Annotate the stub toward `external_endpoint` with the level a click enters and the steps of every edge in `edges`, with no details since a stub only jumps.
    pub(crate) fn stub_group(
        &self,
        external_endpoint: &ElementPath,
        edges: &BTreeSet<EdgeId>,
    ) -> ContractGroup {
        let endpoint_value = external_endpoint.to_string();
        let entered_level = self.level_entered_at(external_endpoint);
        let entered_level_value = entered_level.to_string();
        let target_attributes = Attributes::new()
            .with_text(RuntimeContract::STUB, &endpoint_value)
            .with_text(RuntimeContract::GOTO, &entered_level_value);
        let drawn_graph = self.drawn_graph();
        let carried_steps: BTreeSet<StepIndex> = edges
            .iter()
            .flat_map(|edge| drawn_graph.steps_of_edge(edge))
            .copied()
            .collect();
        let attributes = self.add_step_ids(target_attributes, &carried_steps);
        let endpoint_label = self.label_or_path_of(external_endpoint);
        let title = format!("to {endpoint_label}");
        ContractGroup::new(attributes, title)
    }

    /// Return the graph this annotator's layer draws: its disclosure's, else the whole workflow's.
    fn drawn_graph(&self) -> &'m WorkflowGraph {
        self.disclosure.map_or(self.graph, DisclosedGraph::graph)
    }

    /// Add to `attributes` the ids of `steps`, space-separated in step order.
    fn add_step_ids(&self, attributes: Attributes, steps: &BTreeSet<StepIndex>) -> Attributes {
        let has_steps = !steps.is_empty();
        assert!(
            has_steps,
            "every drawn group takes part in at least one step of its workflow"
        );
        let step_ids: Vec<&str> = steps
            .iter()
            .map(|index| {
                let step = self.workflow.step(*index).expect(
                    "every step index of a workflow's graph names one of the workflow's steps",
                );
                step.id().as_str()
            })
            .collect();
        let steps_value = step_ids.join(" ");
        attributes.with_text(RuntimeContract::STEPS, &steps_value)
    }

    /// Tell whether this annotator's layer draws the container at `path` closed.
    fn is_closed(&self, path: &ElementPath) -> bool {
        self.disclosure
            .is_some_and(|disclosure| disclosure.closed_containers().contains(path))
    }

    /// Name the level a click on `target` enters: `target` itself when it is a container, else the level displaying it.
    fn level_entered_at(&self, target: &ElementPath) -> ElementPath {
        let is_container = self.graph.is_container(target);
        if is_container {
            return target.clone();
        }
        target.enclosing_level()
    }

    /// Describe the connection `edge_id` names in this annotator's layer: lifted when its disclosure merged it, written otherwise.
    fn connection_details<'e>(&self, edge_id: &'e EdgeId) -> ConnectionDetails<'e>
    where
        'm: 'e,
    {
        let lifted_origins = self
            .disclosure
            .and_then(|disclosure| disclosure.lifted_origins().get(edge_id));
        let Some(lifted_origins) = lifted_origins else {
            let written_details = self.edge_details(edge_id);
            return ConnectionDetails::Written(written_details);
        };
        let drawn_graph = self.drawn_graph();
        let merged_edge = drawn_graph.edge(edge_id);
        let kept_label = merged_edge.and_then(Edge::label);
        let label = kept_label.unwrap_or_default();
        let origin_details = lifted_origins
            .iter()
            .map(|origin| self.edge_details(origin))
            .collect();
        let from = edge_id.from();
        let to = edge_id.to();
        let lifted_details = LiftedDetails {
            from,
            to,
            label,
            edges: origin_details,
        };
        ConnectionDetails::Lifted(lifted_details)
    }

    /// Describe edge `edge_id` as the component card reads it, its label and description empty when absent.
    fn edge_details<'e>(&self, edge_id: &'e EdgeId) -> EdgeDetails<'e>
    where
        'm: 'e,
    {
        let edge = self.graph.edge(edge_id);
        let authored_label = edge.and_then(Edge::label);
        let label = authored_label.unwrap_or_default();
        let authored_description = edge.and_then(Edge::description);
        let description = self.carried_description(authored_description);
        let edge_value = edge_id.to_string();
        let from = edge_id.from();
        let to = edge_id.to();
        EdgeDetails {
            edge: edge_value,
            from,
            to,
            label,
            description,
        }
    }

    /// Return `authored` with its edge targets numbered in the workflow when this annotator carries descriptions, else the empty description the details leave out.
    fn carried_description(&self, authored: Option<&str>) -> String {
        let carried = authored.filter(|_description| self.carries_descriptions);
        let Some(carried) = carried else {
            return String::new();
        };
        self.workflow.numbering_edge_targets_of(carried)
    }

    /// Join the labels of `edge_id`'s endpoints with an arrow, each falling back to its path.
    fn endpoint_labels_joined(&self, edge_id: &EdgeId) -> String {
        let from = edge_id.from();
        let to = edge_id.to();
        let from_label = self.label_or_path_of(from);
        let to_label = self.label_or_path_of(to);
        format!("{from_label} \u{2192} {to_label}")
    }

    /// Return the label of the element at `path`, else `path` itself.
    fn label_or_path_of(&self, path: &ElementPath) -> String {
        let element = self.graph.element(path);
        let label = element.map(Element::label);
        label.map_or_else(|| path.to_string(), str::to_owned)
    }
}

impl ContractGroup {
    /// Group `attributes` and `title`, with no details and linked to no URL.
    fn new(attributes: Attributes, title: String) -> Self {
        Self {
            attributes,
            title,
            details: None,
            url: None,
        }
    }

    /// Carry the JSON of `details` after the group's title.
    fn with_details(mut self, details: &impl Serialize) -> Self {
        let details_json =
            serde_json::to_string(details).expect("details are plain strings and maps");
        self.details = Some(details_json);
        self
    }

    /// Link the group to `url` through an anchor around it.
    fn with_url(mut self, url: SafeUrl) -> Self {
        self.url = Some(url);
        self
    }

    /// Write the group around `write_content`, inside an anchor when it links to a URL.
    pub(crate) fn write_around(
        &self,
        writer: &mut SvgWriter,
        write_content: impl FnOnce(&mut SvgWriter),
    ) {
        let Some(url) = &self.url else {
            self.write_titled_group(writer, write_content);
            return;
        };
        let anchor_attributes = Attributes::new().with_url("href", url);
        writer.open_element("a", &anchor_attributes);
        self.write_titled_group(writer, write_content);
        writer.close_element();
    }

    /// Write the group with its `<title>`, then its details `<desc>` if it has any, as its first children, then `write_content`.
    fn write_titled_group(
        &self,
        writer: &mut SvgWriter,
        write_content: impl FnOnce(&mut SvgWriter),
    ) {
        writer.open_element("g", &self.attributes);
        let no_attributes = Attributes::new();
        writer.text_element("title", &no_attributes, &self.title);
        if let Some(details) = &self.details {
            write_details_carrier(writer, details);
        }
        write_content(writer);
        writer.close_element();
    }
}

impl<'m> WorkflowDetails<'m> {
    /// Describe `workflow` by its heading, its overview and each step, every description's edge targets numbered in it.
    pub(crate) fn of(workflow: &'m Workflow) -> Self {
        let id = workflow.id();
        let title = workflow.title();
        let authored_subtitle = workflow.subtitle();
        let subtitle = authored_subtitle.unwrap_or_default();
        let authored_description = workflow.description();
        let description = authored_description
            .map(|description| workflow.numbering_edge_targets_of(description))
            .unwrap_or_default();
        let workflow_steps = workflow.steps().iter();
        let steps = workflow_steps
            .map(|step| StepDetails::of(workflow, step))
            .collect();
        Self {
            id,
            title,
            subtitle,
            description,
            steps,
        }
    }

    /// Write the workflow's details `<desc>`.
    pub(crate) fn write(&self, writer: &mut SvgWriter) {
        let details = serde_json::to_string(self).expect("details are plain strings");
        write_details_carrier(writer, &details);
    }
}

impl<'m> StepDetails<'m> {
    /// Describe `step` of `workflow` by its heading, its prose, the layer drawing its items and the items themselves.
    fn of(workflow: &'m Workflow, step: &'m Step) -> Self {
        let id = step.id();
        let title = step.title();
        let authored_description = step.description();
        let description = authored_description
            .map(|description| workflow.numbering_edge_targets_of(description))
            .unwrap_or_default();
        let deepest_listed_depth = step.deepest_listed_depth();
        let layer = DisclosedGraph::shallowest_layer_showing(deepest_listed_depth);
        let step_items = step.items();
        let items = step_items.map(StepItemDetails::of).collect();
        Self {
            id,
            title,
            description,
            layer,
            items,
        }
    }
}

impl<'m> StepItemDetails<'m> {
    /// Name `item` as the edge id or element path the groups drawing it carry.
    fn of(item: &'m StepItem) -> Self {
        match item {
            StepItem::Edge(id) => {
                let edge_value = id.to_string();
                Self::Edge(edge_value)
            }
            StepItem::Element(path) => Self::Element(path),
        }
    }
}

/// Write `details` JSON as the `<desc>` the runtime reads them from.
fn write_details_carrier(writer: &mut SvgWriter, details: &str) {
    let details_attributes = Attributes::new().with_text(RuntimeContract::DETAILS, "");
    writer.text_element("desc", &details_attributes, details);
}

impl<'a> ElementDetails<'a> {
    /// Describe `element`, held at `path`, by its label, caption, code, metadata and the `description` carried for it.
    fn of(path: &'a ElementPath, element: &'a Element, description: String) -> Self {
        let label = element.label();
        let properties = element.properties();
        let authored_caption = properties.caption();
        let caption = authored_caption.unwrap_or_default();
        let authored_code = properties.code();
        let code = authored_code.unwrap_or_default();
        let metadata = properties.metadata();
        Self {
            path,
            label,
            caption,
            code,
            metadata,
            description,
        }
    }

    /// Describe an element the graph does not hold by its path alone.
    fn bare(path: &'a ElementPath) -> Self {
        static NO_METADATA: BTreeMap<String, String> = BTreeMap::new();
        Self {
            path,
            label: "",
            caption: "",
            code: "",
            metadata: &NO_METADATA,
            description: String::new(),
        }
    }
}
