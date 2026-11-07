use super::util::slack;
use crate::graph::{Edge, EdgeLabel, Graph};

/// Grow an undirected spanning tree of tight edges from the first node, shifting the tree's ranks to tighten the least-slack edge whenever it stalls.
pub(super) fn feasible_tree(graph: &mut Graph) -> Graph {
    let mut tree = Graph::undirected();
    let start = graph
        .nodes()
        .into_iter()
        .next()
        .expect("network simplex ranks at least one node");
    let node_count = graph.node_count();
    tree.set_node(&start, Default::default());

    loop {
        let tree_size = tight_tree(&mut tree, graph);
        let is_spanning = tree_size >= node_count;
        if is_spanning {
            break;
        }
        let Some(e) = find_min_slack_edge(&tree, graph) else {
            break;
        };
        let edge_slack = slack(graph, &e);
        let is_tail_in_tree = tree.has_node(&e.v);
        let delta = if is_tail_in_tree {
            edge_slack
        } else {
            -edge_slack
        };
        shift_ranks(&tree, graph, delta);
    }
    tree
}

/// Extend the tree along tight edges as far as they reach and return its node count.
fn tight_tree(tree: &mut Graph, graph: &Graph) -> usize {
    for start in tree.nodes() {
        tight_tree_dfs(tree, graph, &start);
    }
    tree.node_count()
}

fn tight_tree_dfs(tree: &mut Graph, graph: &Graph, v: &str) {
    for e in graph.node_edges(v) {
        let w = if v == e.v { &e.w } else { &e.v };
        let is_tight_edge_to_new_node = !tree.has_node(w) && slack(graph, &e) == 0;
        if is_tight_edge_to_new_node {
            tree.set_node(w, Default::default());
            tree.set_edge(v, w, EdgeLabel::default(), None);
            tight_tree_dfs(tree, graph, w);
        }
    }
}

/// Find the first least-slack edge with exactly one endpoint in the tree.
fn find_min_slack_edge(tree: &Graph, graph: &Graph) -> Option<Edge> {
    let mut min_slack = i32::MAX;
    let mut result = None;
    for e in graph.edges() {
        let is_tail_in_tree = tree.has_node(&e.v);
        let is_head_in_tree = tree.has_node(&e.w);
        let is_incident_on_tree = is_tail_in_tree != is_head_in_tree;
        if !is_incident_on_tree {
            continue;
        }
        let edge_slack = slack(graph, &e);
        if edge_slack < min_slack {
            min_slack = edge_slack;
            result = Some(e);
        }
    }
    result
}

fn shift_ranks(tree: &Graph, graph: &mut Graph, delta: i32) {
    for v in tree.nodes() {
        if let Some(rank) = graph.node_mut(&v).rank.as_mut() {
            *rank += delta;
        }
    }
}
