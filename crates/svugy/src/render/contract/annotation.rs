//! Annotate the groups of elements, edges and stubs with the markup the runtime contract puts on them.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use svugy_model::link::Link;
use svugy_model::model::{Edge, EdgeId, Element, Model};
use svugy_model::path::ElementPath;

use super::RuntimeContract;
use crate::render::writer::url::SafeUrl;
use crate::render::writer::{Attributes, SvgWriter};

/// Wraps one group's content in the contract's markup: an anchor for a URL link, the group's attributes, its `<title>`, then its details.
pub(crate) struct ContractGroup {
    attributes: Attributes,
    title: String,
    details: String,
    url: Option<SafeUrl>,
}

/// Annotates element, edge and stub groups with the contract, reading labels, captions, metadata and links from the model.
pub(crate) struct ContractAnnotator<'m> {
    model: &'m Model,
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

/// Describes a stub as the runtime's detail panel reads it: its external endpoint and every edge it carries.
#[derive(Serialize)]
#[serde(tag = "kind", rename = "stub")]
struct StubDetails<'a> {
    target: &'a ElementPath,
    edges: Vec<EdgeDetails<'a>>,
}

impl ContractGroup {
    fn new(attributes: Attributes, title: String, details: &impl Serialize) -> Self {
        let details = serde_json::to_string(details).expect("details are plain strings and maps");
        Self {
            attributes,
            title,
            details,
            url: None,
        }
    }

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
        writer.open_element("title", &no_attributes);
        writer.text(&self.title);
        writer.close_element();
        let details_attributes = Attributes::new().with_text(RuntimeContract::DETAILS, "");
        writer.open_element("desc", &details_attributes);
        writer.text(&self.details);
        writer.close_element();
        write_content(writer);
        writer.close_element();
    }
}

impl<'m> ContractAnnotator<'m> {
    pub(crate) fn new(model: &'m Model) -> Self {
        Self { model }
    }

    /// Annotate `path`'s solid, titled by its label, with where its link leads: a level to enter, or a URL that passed the policy.
    pub(crate) fn element(&self, path: &ElementPath) -> ContractGroup {
        let path_value = path.to_string();
        let attributes = Attributes::new().with_text(RuntimeContract::ELEMENT, &path_value);
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
                let Ok(url) = SafeUrl::parse(raw_url) else {
                    return group;
                };
                group.with_url(url)
            }
        }
    }

    /// Annotate `edge_id`'s connection, titled by its label, else by its identity.
    pub(crate) fn edge(&self, edge_id: &EdgeId) -> ContractGroup {
        let details = self.edge_details(edge_id);
        let attributes = Attributes::new().with_text(RuntimeContract::EDGE, &details.edge);
        let is_unlabelled = details.label.is_empty();
        let title = if is_unlabelled {
            details.edge.clone()
        } else {
            details.label.to_owned()
        };
        ContractGroup::new(attributes, title, &details)
    }

    /// Annotate the stub toward `external_endpoint` with the level a click enters and the details of every edge in `edges`.
    pub(crate) fn stub(
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
        let external_element = self.model.element(external_endpoint);
        let endpoint_label = external_element.map_or(endpoint_value.as_str(), Element::label);
        let title = format!("to {endpoint_label}");
        let edge_details = edges.iter().map(|edge| self.edge_details(edge)).collect();
        let details = StubDetails {
            target: external_endpoint,
            edges: edge_details,
        };
        ContractGroup::new(attributes, title, &details)
    }

    /// Name the level a click on `target` enters: `target` itself when it is a container, else the level displaying it.
    fn level_entered_at(&self, target: &ElementPath) -> ElementPath {
        let is_container = self.model.is_container(target);
        if is_container {
            return target.clone();
        }
        let parent = target.parent();
        parent.unwrap_or_else(ElementPath::root)
    }

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
}

impl<'a> ElementDetails<'a> {
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
