use std::collections::HashSet;

use crate::graph::{Edge, Graph};

/// Reverse each edge a depth-first search finds closing a cycle, naming the reversal so it can sit beside an existing edge.
pub(crate) fn reverse_cycle_closing_edges(graph: &mut Graph) {
    let feedback_arc_set = depth_first_feedback_arc_set(graph);
    for e in feedback_arc_set {
        let label = graph.edge(&e).cloned().unwrap_or_default();
        graph.remove_edge(&e);
        let reversed_name = graph.unique_id("rev");
        graph.set_edge(&e.w, &e.v, label, Some(&reversed_name));
    }
}

fn depth_first_feedback_arc_set(graph: &Graph) -> Vec<Edge> {
    fn dfs(
        graph: &Graph,
        v: &str,
        stack: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        feedback_arc_set: &mut Vec<Edge>,
    ) {
        let is_first_visit = visited.insert(v.to_string());
        if !is_first_visit {
            return;
        }
        stack.insert(v.to_string());
        for e in graph.out_edges(v) {
            let is_back_edge = stack.contains(&e.w);
            if is_back_edge {
                feedback_arc_set.push(e);
                continue;
            }
            let w = e.w.clone();
            dfs(graph, &w, stack, visited, feedback_arc_set);
        }
        stack.remove(v);
    }

    let mut feedback_arc_set = Vec::new();
    let mut stack = HashSet::new();
    let mut visited = HashSet::new();
    for v in graph.nodes() {
        dfs(graph, &v, &mut stack, &mut visited, &mut feedback_arc_set);
    }
    feedback_arc_set
}
