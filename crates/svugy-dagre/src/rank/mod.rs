mod feasible_tree;
mod network_simplex;
mod util;

use crate::graph::{EdgeLabel, Graph, NodeLabel};
use crate::util::simplify;

const RANK_ROOT: &str = "_root";

/// Assign every node the rank network simplex finds, honouring each edge's minlen while shortening weighted edges.
pub(crate) fn rank(graph: &mut Graph) {
    let mut ranked = simplify(graph);
    connect_every_node_from_a_weightless_root(&mut ranked);
    network_simplex::network_simplex(&mut ranked);
    for v in graph.nodes() {
        let rank = ranked.node(&v).rank;
        graph.node_mut(&v).rank = rank;
    }
}

/// Join every node to one extra root by a weight-0 edge, so network simplex sees one connected graph without pulling any node.
fn connect_every_node_from_a_weightless_root(graph: &mut Graph) {
    let nodes = graph.nodes();
    graph.set_node(RANK_ROOT, NodeLabel::default());
    for v in nodes {
        let root_edge = EdgeLabel {
            weight: Some(0.0),
            minlen: Some(1),
            ..EdgeLabel::default()
        };
        graph.set_edge(RANK_ROOT, &v, root_edge, None);
    }
}
