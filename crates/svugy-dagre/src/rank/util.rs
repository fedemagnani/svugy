use std::collections::HashSet;

use crate::graph::{Edge, Graph};

/// Rank each node as low as its out-edges' minlen allows below its successors, visiting from the sources.
pub(super) fn longest_path(graph: &mut Graph) {
    fn dfs(graph: &mut Graph, v: &str, visited: &mut HashSet<String>) -> i32 {
        let is_first_visit = visited.insert(v.to_string());
        if !is_first_visit {
            return graph.node(v).rank.unwrap_or(0);
        }
        let mut min_rank = i32::MAX;
        for e in graph.out_edges(v) {
            let w_rank = dfs(graph, &e.w, visited);
            let minlen = graph.edge(&e).map_or(1, |label| label.minlen.unwrap_or(1));
            let candidate = w_rank - minlen;
            if candidate < min_rank {
                min_rank = candidate;
            }
        }
        let is_sink = min_rank == i32::MAX;
        let rank = if is_sink { 0 } else { min_rank };
        graph.node_mut(v).rank = Some(rank);
        rank
    }

    let mut visited = HashSet::new();
    for v in graph.sources() {
        dfs(graph, &v, &mut visited);
    }
}

/// slack = rank(w) − rank(v) − minlen
pub(super) fn slack(graph: &Graph, edge: &Edge) -> i32 {
    let rank_w = graph.node(&edge.w).rank.unwrap_or(0);
    let rank_v = graph.node(&edge.v).rank.unwrap_or(0);
    let minlen = graph
        .edge(edge)
        .map_or(1, |label| label.minlen.unwrap_or(1));
    rank_w - rank_v - minlen
}
