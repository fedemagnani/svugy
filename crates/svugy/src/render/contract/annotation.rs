//! Annotate the groups of elements, edges and stubs with the markup the runtime contract puts on them.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use svugy_model::disclose::DisclosedModel;
use svugy_model::link::Link;
use svugy_model::model::{Edge, EdgeId, Element, Model};
use svugy_model::path::ElementPath;

use super::RuntimeContract;
use crate::render::writer::url::SafeUrl;
use crate::render::writer::{Attributes, SvgWriter};

/// Annotates element, edge and stub groups with the contract, reading labels, captions, metadata and links from the source model, and closed containers and merged edges from the layer's disclosure.
pub(crate) struct ContractAnnotator<'m> {
    model: &'m Model,
    disclosure: Option<&'m DisclosedModel>,
}

/// Wraps one group's content in the contract's markup: an anchor for a URL link, the group's attributes, its `<title>`, then its details.
pub(crate) struct ContractGroup {
    attributes: Attributes,
    title: String,
    details: String,
    url: Option<SafeUrl>,
}

/// Describes an element as the runtime's detail panel reads it, empty keys left out.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "element")]
struct ElementDetails<'a> {
    path: &'a ElementPath,
    #[serde(skip_serializing_if = "str::is_empty")]
    label: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    caption: &'a str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    metadata: &'a BTreeMap<String, String>,
}

/// Describes an edge as the runtime's detail panel reads it, empty keys left out.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "edge")]
struct EdgeDetails<'a> {
    edge: String,
    from: &'a ElementPath,
    to: &'a ElementPath,
    #[serde(skip_serializing_if = "str::is_empty")]
    label: &'a str,
}

/// Describes a lifted connection as the runtime's detail panel reads it: the shown endpoints it is drawn between and every source edge it stands for.
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

/// Describes a stub as the runtime's detail panel reads it: its external endpoint and every connection it carries.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "stub")]
struct StubDetails<'a> {
    target: &'a ElementPath,
    edges: Vec<ConnectionDetails<'a>>,
}

impl<'m> ContractAnnotator<'m> {
    /// Build an annotator reading `model`, drawn whole.
    pub(crate) fn new(model: &'m Model) -> Self {
        Self {
            model,
            disclosure: None,
        }
    }

    /// Read closed containers and merged edges from `disclosure`, the layer of the source model this annotator's groups are drawn in.
    pub(crate) fn with_disclosure(mut self, disclosure: &'m DisclosedModel) -> Self {
        self.disclosure = Some(disclosure);
        self
    }

    /// Annotate the group of element `path`'s solid, titled by its label, marked when its layer draws it closed, with where its link leads: a level to enter, or a URL that passed the policy.
    pub(crate) fn element_group(&self, path: &ElementPath) -> ContractGroup {
        let path_value = path.to_string();
        let element_attributes = Attributes::new().with_text(RuntimeContract::ELEMENT, &path_value);
        let is_closed = self.is_closed(path);
        let attributes = if is_closed {
            element_attributes.with_text(RuntimeContract::CLOSED, "")
        } else {
            element_attributes
        };
        let Some(element) = self.model.element(path) else {
            let bare_details = ElementDetails::bare(path);
            return ContractGroup::new(attributes, path_value, &bare_details);
        };
        let details = ElementDetails::of(path, element);
        let title = element.label().to_owned();
        let Some(raw_link) = element.link() else {
            return ContractGroup::new(attributes, title, &details);
        };
        let link = Link::resolve(raw_link, self.model);
        match link {
            Link::Element(target) => {
                let entered_level = self.level_entered_at(&target);
                let entered_level_value = entered_level.to_string();
                let goto_attributes =
                    attributes.with_text(RuntimeContract::GOTO, &entered_level_value);
                ContractGroup::new(goto_attributes, title, &details)
            }
            Link::Url(raw_url) => {
                let group = ContractGroup::new(attributes, title, &details);
                let Some(url) = SafeUrl::parse(raw_url) else {
                    return group;
                };
                group.with_url(url)
            }
        }
    }

    /// Annotate `edge_id`'s connection with the endpoints its layer draws it between, a written edge titled by its label, else by its identity, a lifted one by its label, else by its endpoints' labels.
    pub(crate) fn edge_group(&self, edge_id: &EdgeId) -> ContractGroup {
        let edge_value = edge_id.to_string();
        let from = edge_id.from();
        let to = edge_id.to();
        let from_value = from.to_string();
        let to_value = to.to_string();
        let attributes = Attributes::new()
            .with_text(RuntimeContract::EDGE, &edge_value)
            .with_text(RuntimeContract::FROM, &from_value)
            .with_text(RuntimeContract::TO, &to_value);
        let connection_details = self.connection_details(edge_id);
        match connection_details {
            ConnectionDetails::Written(details) => {
                let is_unlabelled = details.label.is_empty();
                let title = if is_unlabelled {
                    edge_value
                } else {
                    details.label.to_owned()
                };
                ContractGroup::new(attributes, title, &details)
            }
            ConnectionDetails::Lifted(details) => {
                let is_unlabelled = details.label.is_empty();
                let title = if is_unlabelled {
                    self.endpoint_labels_joined(edge_id)
                } else {
                    details.label.to_owned()
                };
                ContractGroup::new(attributes, title, &details)
            }
        }
    }

    /// Annotate the stub toward `external_endpoint` with the level a click enters and the details of every edge in `edges`.
    pub(crate) fn stub_group(
        &self,
        external_endpoint: &ElementPath,
        edges: &BTreeSet<EdgeId>,
    ) -> ContractGroup {
        let endpoint_value = external_endpoint.to_string();
        let entered_level = self.level_entered_at(external_endpoint);
        let entered_level_value = entered_level.to_string();
        let attributes = Attributes::new()
            .with_text(RuntimeContract::STUB, &endpoint_value)
            .with_text(RuntimeContract::GOTO, &entered_level_value);
        let endpoint_label = self.label_or_path_of(external_endpoint);
        let title = format!("to {endpoint_label}");
        let connection_details = edges
            .iter()
            .map(|edge| self.connection_details(edge))
            .collect();
        let details = StubDetails {
            target: external_endpoint,
            edges: connection_details,
        };
        ContractGroup::new(attributes, title, &details)
    }

    /// Tell whether this annotator's layer draws the container at `path` closed.
    fn is_closed(&self, path: &ElementPath) -> bool {
        self.disclosure
            .is_some_and(|disclosure| disclosure.closed_containers().contains(path))
    }

    /// Name the level a click on `target` enters: `target` itself when it is a container, else the level displaying it.
    fn level_entered_at(&self, target: &ElementPath) -> ElementPath {
        let is_container = self.model.is_container(target);
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
        let merged_edge = self
            .disclosure
            .and_then(|disclosure| disclosure.model().edge(edge_id));
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

    /// Describe edge `edge_id` as the detail panel reads it, its label empty when unlabelled.
    fn edge_details<'e>(&self, edge_id: &'e EdgeId) -> EdgeDetails<'e>
    where
        'm: 'e,
    {
        let edge = self.model.edge(edge_id);
        let authored_label = edge.and_then(Edge::label);
        let label = authored_label.unwrap_or_default();
        let edge_value = edge_id.to_string();
        let from = edge_id.from();
        let to = edge_id.to();
        EdgeDetails {
            edge: edge_value,
            from,
            to,
            label,
        }
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
        let element = self.model.element(path);
        let label = element.map(Element::label);
        label.map_or_else(|| path.to_string(), str::to_owned)
    }
}

impl ContractGroup {
    /// Group `attributes`, `title` and the JSON of `details`, linked to no URL.
    fn new(attributes: Attributes, title: String, details: &impl Serialize) -> Self {
        let details = serde_json::to_string(details).expect("details are plain strings and maps");
        Self {
            attributes,
            title,
            details,
            url: None,
        }
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

    /// Write the group with its `<title>` and details `<desc>` as its first two children, then `write_content`.
    fn write_titled_group(
        &self,
        writer: &mut SvgWriter,
        write_content: impl FnOnce(&mut SvgWriter),
    ) {
        writer.open_element("g", &self.attributes);
        let no_attributes = Attributes::new();
        writer.text_element("title", &no_attributes, &self.title);
        let details_attributes = Attributes::new().with_text(RuntimeContract::DETAILS, "");
        writer.text_element("desc", &details_attributes, &self.details);
        write_content(writer);
        writer.close_element();
    }
}

impl<'a> ElementDetails<'a> {
    /// Describe `element`, held at `path`, by its label, caption and metadata.
    fn of(path: &'a ElementPath, element: &'a Element) -> Self {
        let label = element.label();
        let authored_caption = element.caption();
        let caption = authored_caption.unwrap_or_default();
        let metadata = element.metadata();
        Self {
            path,
            label,
            caption,
            metadata,
        }
    }

    /// Describe an element the model does not hold by its path alone.
    fn bare(path: &'a ElementPath) -> Self {
        static NO_METADATA: BTreeMap<String, String> = BTreeMap::new();
        Self {
            path,
            label: "",
            caption: "",
            metadata: &NO_METADATA,
        }
    }
}
