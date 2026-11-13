//! Disclose a workflow's graph layer by layer: each layer shows one more depth, closing the containers at its deepest shown depth and lifting the edges of what they hide onto them.

use std::collections::{BTreeMap, BTreeSet};

use crate::graph::WorkflowGraph;
use crate::model::{Edge, EdgeId};
use crate::path::ElementPath;
use crate::workflow::StepIndex;

/// Disclose a graph as one layer shows it: its elements down to one depth, the containers at that depth closed like leaves, the edges of hidden elements lifted onto them.
#[derive(Clone, Debug, PartialEq)]
pub struct DisclosedGraph {
    graph: WorkflowGraph,
    closed_containers: BTreeSet<ElementPath>,
    lifted_origins: BTreeMap<EdgeId, BTreeSet<EdgeId>>,
}

impl DisclosedGraph {
    /// Disclose `graph` at every layer its containers give, from its top-level elements all closed down to the whole graph.
    pub fn every_layer_of(graph: &WorkflowGraph) -> Vec<Self> {
        let containers = graph
            .elements()
            .keys()
            .filter(|path| graph.is_container(path));
        let container_depths = containers.map(ElementPath::depth);
        let deepest_container_depth = container_depths.max().unwrap_or(0);
        let layers = 0..=deepest_container_depth;
        layers.map(|layer| Self::at_layer(graph, layer)).collect()
    }

    /// Disclose `graph` as layer `layer` shows it: every element down to its deepest shown depth, the containers at that depth closed, each edge reaching below it lifted onto the shown ancestors of its endpoints, merged per shown pair, drawn as an arc and taking part in every step of its sources.
    fn at_layer(graph: &WorkflowGraph, layer: usize) -> Self {
        let deepest_shown_depth = Self::deepest_depth_shown_at(layer);
        let mut disclosed_graph = graph.edgeless_down_to_depth(deepest_shown_depth);
        let closed_containers = disclosed_graph
            .elements()
            .keys()
            .filter(|path| {
                let is_at_deepest_shown_depth = path.depth() == deepest_shown_depth;
                let is_container = graph.is_container(path);
                is_at_deepest_shown_depth && is_container
            })
            .cloned()
            .collect();
        let mut lifted_edges_by_shown_pair: BTreeMap<
            (ElementPath, ElementPath),
            Vec<(&EdgeId, &Edge)>,
        > = BTreeMap::new();
        for (id, edge) in graph.edges() {
            let from = id.from();
            let to = id.to();
            let shown_from = from.truncated_to_depth(deepest_shown_depth);
            let shown_to = to.truncated_to_depth(deepest_shown_depth);
            let is_from_shown = shown_from == *from;
            let is_to_shown = shown_to == *to;
            let is_between_shown_endpoints = is_from_shown && is_to_shown;
            if is_between_shown_endpoints {
                let edge_steps = graph.steps_of_edge(id).clone();
                disclosed_graph.insert_edge(id.clone(), edge.clone(), edge_steps);
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
            let disclosed_edges = disclosed_graph.edges();
            let merged_id = EdgeId::next_free_in(disclosed_edges, shown_from, shown_to);
            let origins: BTreeSet<EdgeId> = lifted_edges
                .into_iter()
                .map(|(id, _edge)| id.clone())
                .collect();
            let origin_steps: BTreeSet<StepIndex> = origins
                .iter()
                .flat_map(|origin| graph.steps_of_edge(origin))
                .copied()
                .collect();
            disclosed_graph.insert_edge(merged_id.clone(), merged_edge, origin_steps);
            lifted_origins.insert(merged_id, origins);
        }
        Self {
            graph: disclosed_graph,
            closed_containers,
            lifted_origins,
        }
    }

    /// Return the deepest depth layer `layer` shows: depth = layer + 1, top-level elements being at depth 1.
    fn deepest_depth_shown_at(layer: usize) -> usize {
        layer + 1
    }

    /// Return the shallowest layer showing every element at `depth` or above: layer = max(0, depth − 1), the inverse of `deepest_depth_shown_at`.
    pub fn shallowest_layer_showing(depth: usize) -> usize {
        depth.saturating_sub(1)
    }

    /// Return the disclosed graph, whose closed containers are leaves and whose lifted edges are merged edges.
    pub fn graph(&self) -> &WorkflowGraph {
        &self.graph
    }

    /// Return the containers this layer draws closed, the leaves of the disclosed graph that hide children.
    pub fn closed_containers(&self) -> &BTreeSet<ElementPath> {
        &self.closed_containers
    }

    /// Return the source edges each merged edge stands for, keyed by the merged edge's id in the disclosed graph.
    pub fn lifted_origins(&self) -> &BTreeMap<EdgeId, BTreeSet<EdgeId>> {
        &self.lifted_origins
    }
}
