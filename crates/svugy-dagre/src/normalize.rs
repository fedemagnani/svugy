use crate::graph::{Edge, EdgeLabel, Graph, NodeLabel};

const DUMMY_PREFIX: &str = "_d";

/// Split every edge spanning more than one rank into a chain of zero-size dummy nodes, one per crossed rank.
pub(crate) fn split_long_edges(graph: &mut Graph) {
    for edge in graph.edges() {
        split_long_edge(graph, &edge);
    }
}

fn split_long_edge(graph: &mut Graph, e: &Edge) {
    let v_rank = graph.node(&e.v).rank.unwrap_or(0);
    let w_rank = graph.node(&e.w).rank.unwrap_or(0);
    let is_short_edge = w_rank == v_rank + 1;
    if is_short_edge {
        return;
    }
    let weight = graph.edge(e).and_then(|label| label.weight);
    graph.remove_edge(e);

    let mut chain_end = e.v.clone();
    for dummy_rank in v_rank + 1..w_rank {
        let dummy = add_dummy_node(graph, dummy_rank);
        let segment = EdgeLabel {
            weight,
            ..EdgeLabel::default()
        };
        graph.set_edge(&chain_end, &dummy, segment, e.name.as_deref());
        chain_end = dummy;
    }
    let last_segment = EdgeLabel {
        weight,
        ..EdgeLabel::default()
    };
    graph.set_edge(&chain_end, &e.w, last_segment, e.name.as_deref());
}

fn add_dummy_node(graph: &mut Graph, rank: i32) -> String {
    let mut v = DUMMY_PREFIX.to_string();
    while graph.has_node(&v) {
        v = graph.unique_id(DUMMY_PREFIX);
    }
    let dummy = NodeLabel {
        rank: Some(rank),
        is_dummy: true,
        ..NodeLabel::default()
    };
    graph.set_node(&v, dummy);
    v
}
