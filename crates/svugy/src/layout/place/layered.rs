use std::collections::BTreeMap;

use dagre_dgl_rs::{EdgeLabel, Graph, GraphLabel, NodeLabel, layout};
use kurbo::{Point, Rect};
use svugy_model::path::ElementPath;

use super::{LevelLayout, SiblingGraph};

/// Ranks advance along plan x, so a request path reads from the back left toward the front right.
const RANK_DIRECTION: &str = "LR";
/// Gap between neighbours within one rank, twice the example's gap between a container's packed children.
const NODE_GAP: f64 = 48.0;
/// Gap between consecutive ranks, wider than the node gap so the arcs between ranks have room.
const RANK_GAP: f64 = 72.0;

/// Places footprints in dagre ranks along plan x, so edges flow in one direction.
pub(super) struct Layered;

impl LevelLayout for Layered {
    fn place_footprints(&self, siblings: &SiblingGraph) -> BTreeMap<ElementPath, Rect> {
        let is_graph_empty_which_dagre_cannot_order = siblings.sizes().is_empty();
        if is_graph_empty_which_dagre_cannot_order {
            return BTreeMap::new();
        }
        let node_names = Self::dagre_node_names_of(siblings);
        let mut graph = Self::dagre_graph_of(siblings, &node_names);
        layout(&mut graph);
        siblings
            .sizes()
            .iter()
            .map(|(path, &size)| {
                let node_name = &node_names[path];
                let node = graph.node(node_name);
                let center_x = node.x.expect("dagre positions every node");
                let center_y = node.y.expect("dagre positions every node");
                let center = Point::new(center_x, center_y);
                let footprint = Rect::from_center_size(center, size);
                (path.clone(), footprint)
            })
            .collect()
    }
}

impl Layered {
    /// Name each footprint's dagre node by its index in path order, so no element id can collide with dagre's own node names.
    fn dagre_node_names_of(siblings: &SiblingGraph) -> BTreeMap<&ElementPath, String> {
        siblings
            .sizes()
            .keys()
            .enumerate()
            .map(|(path_index, path)| (path, path_index.to_string()))
            .collect()
    }

    /// Build a dagre graph with one named node per footprint and one edge per pair of distinct joined siblings.
    fn dagre_graph_of(
        siblings: &SiblingGraph,
        node_names: &BTreeMap<&ElementPath, String>,
    ) -> Graph {
        let mut graph = Graph::with_options(true, false, false);
        let rank_direction = RANK_DIRECTION.to_owned();
        graph.set_graph(GraphLabel {
            rankdir: Some(rank_direction),
            nodesep: Some(NODE_GAP),
            ranksep: Some(RANK_GAP),
            ..GraphLabel::default()
        });
        for (path, size) in siblings.sizes() {
            let node = NodeLabel {
                width: size.width,
                height: size.height,
                ..NodeLabel::default()
            };
            let node_name = &node_names[path];
            graph.set_node(node_name, node);
        }
        for (from, to) in siblings.joined_pairs() {
            let is_self_loop = from == to;
            if is_self_loop {
                continue;
            }
            let from_name = &node_names[from];
            let to_name = &node_names[to];
            graph.set_edge(from_name, to_name, EdgeLabel::default(), None);
        }
        graph
    }
}
