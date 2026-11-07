use std::collections::HashSet;

use crate::graph::Graph;

/// Order each rank as a depth-first search along successors meets its nodes, starting from nodes in ascending rank.
pub(super) fn init_order(graph: &Graph) -> Vec<Vec<String>> {
    fn dfs(graph: &Graph, v: &str, visited: &mut HashSet<String>, layers: &mut [Vec<String>]) {
        let is_first_visit = visited.insert(v.to_string());
        if !is_first_visit {
            return;
        }
        let rank = graph.node(v).rank.unwrap_or(0) as usize;
        layers[rank].push(v.to_string());
        for w in graph.successors(v) {
            dfs(graph, &w, visited, layers);
        }
    }

    let mut nodes_by_ascending_rank = graph.nodes();
    let max_rank = nodes_by_ascending_rank
        .iter()
        .filter_map(|v| graph.node(v).rank)
        .max()
        .unwrap_or(0);
    let layer_count = (max_rank + 1) as usize;
    let mut layers = vec![Vec::new(); layer_count];
    nodes_by_ascending_rank.sort_by_key(|v| graph.node(v).rank.unwrap_or(0));
    let mut visited = HashSet::new();
    for v in nodes_by_ascending_rank {
        dfs(graph, &v, &mut visited, &mut layers);
    }
    layers
}
