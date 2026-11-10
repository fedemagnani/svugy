//! Disclose a model layer by layer: each layer shows one more depth, closing the containers at its deepest shown depth and lifting the edges of what they hide onto them.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Edge, EdgeId, Element, Model};
use crate::path::ElementPath;

/// Disclose a model as one layer shows it: its elements down to one depth, the containers at that depth closed like leaves, the edges of hidden elements lifted onto them.
#[derive(Clone, Debug, PartialEq)]
pub struct DisclosedModel {
    model: Model,
    closed_containers: BTreeSet<ElementPath>,
    lifted_origins: BTreeMap<EdgeId, BTreeSet<EdgeId>>,
}

impl DisclosedModel {
    /// Disclose `model` at every layer its containers give, from its top-level elements all closed down to the whole model.
    pub fn every_layer_of(model: &Model) -> Vec<Self> {
        let containers = model
            .elements
            .keys()
            .filter(|path| model.is_container(path));
        let container_depths = containers.map(ElementPath::depth);
        let deepest_container_depth = container_depths.max().unwrap_or(0);
        let layers = 0..=deepest_container_depth;
        layers.map(|layer| Self::at_layer(model, layer)).collect()
    }

    /// Disclose `model` as layer `layer` shows it: every element down to depth `layer + 1`, the containers at that depth closed, each edge reaching below it lifted onto the shown ancestors of its endpoints, merged per shown pair and drawn as an arc.
    fn at_layer(model: &Model, layer: usize) -> Self {
        let deepest_shown_depth = layer + 1;
        let shown_elements: BTreeMap<ElementPath, Element> = model
            .elements
            .iter()
            .filter(|(path, _element)| path.depth() <= deepest_shown_depth)
            .map(|(path, element)| {
                let shown_path = path.clone();
                let shown_element = element.clone();
                (shown_path, shown_element)
            })
            .collect();
        let closed_containers = shown_elements
            .keys()
            .filter(|path| {
                let is_at_deepest_shown_depth = path.depth() == deepest_shown_depth;
                let is_container = model.is_container(path);
                is_at_deepest_shown_depth && is_container
            })
            .cloned()
            .collect();
        let title = model.title.clone();
        let subtitle = model.subtitle.clone();
        let view = model.view;
        let description = model.description.clone();
        let legend = model.legend.clone();
        let edges = BTreeMap::new();
        let mut disclosed_model = Model {
            title,
            subtitle,
            view,
            description,
            legend,
            elements: shown_elements,
            edges,
        };
        let mut lifted_edges_by_shown_pair: BTreeMap<
            (ElementPath, ElementPath),
            Vec<(&EdgeId, &Edge)>,
        > = BTreeMap::new();
        for (id, edge) in &model.edges {
            let from = id.from();
            let to = id.to();
            let shown_from = from.truncated_to_depth(deepest_shown_depth);
            let shown_to = to.truncated_to_depth(deepest_shown_depth);
            let is_from_shown = shown_from == *from;
            let is_to_shown = shown_to == *to;
            let is_between_shown_endpoints = is_from_shown && is_to_shown;
            if is_between_shown_endpoints {
                let shown_id = id.clone();
                let shown_edge = edge.clone();
                disclosed_model.edges.insert(shown_id, shown_edge);
                continue;
            }
            let is_inside_one_shown_element = shown_from == shown_to;
            if is_inside_one_shown_element {
                continue;
            }
            let shown_pair = (shown_from, shown_to);
            let lifted_edges = lifted_edges_by_shown_pair.entry(shown_pair).or_default();
            lifted_edges.push((id, edge));
        }
        let mut lifted_origins = BTreeMap::new();
        for ((shown_from, shown_to), lifted_edges) in lifted_edges_by_shown_pair {
            let merged_edge = match lifted_edges.as_slice() {
                [(only_id, only_edge)] => {
                    let mut lifted_edge = (*only_edge).clone();
                    let is_target_lifted = only_id.to() != &shown_to;
                    if is_target_lifted {
                        lifted_edge.badge = false;
                    }
                    lifted_edge
                }
                _several_edges => Edge::new(),
            };
            let merged_id = disclosed_model.insert_edge_with_next_free_ordinal(
                shown_from,
                shown_to,
                merged_edge,
            );
            let origins = lifted_edges
                .into_iter()
                .map(|(id, _edge)| id.clone())
                .collect();
            lifted_origins.insert(merged_id, origins);
        }
        Self {
            model: disclosed_model,
            closed_containers,
            lifted_origins,
        }
    }

    /// Return the disclosed model, whose closed containers are leaves and whose lifted edges are merged edges.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Return the containers this layer draws closed, the leaves of the disclosed model that hide children.
    pub fn closed_containers(&self) -> &BTreeSet<ElementPath> {
        &self.closed_containers
    }

    /// Return the source edges each merged edge stands for, keyed by the merged edge's id in the disclosed model.
    pub fn lifted_origins(&self) -> &BTreeMap<EdgeId, BTreeSet<EdgeId>> {
        &self.lifted_origins
    }
}
