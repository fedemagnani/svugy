use std::collections::HashSet;

use super::feasible_tree::feasible_tree;
use super::util::{longest_path, slack};
use crate::graph::{Edge, EdgeLabel, Graph, NodeLabel, postorder, preorder};

/// Rank the nodes of a connected, simple graph by network simplex: start from a feasible tight tree, then swap out tree edges with negative cut values.
pub(super) fn network_simplex(graph: &mut Graph) {
    longest_path(graph);
    let mut tree = feasible_tree(graph);
    init_low_lim_values(&mut tree);
    init_cut_values(&mut tree, graph);
    while let Some(leaving) = leave_edge(&tree) {
        let Some(entering) = enter_edge(&tree, graph, &leaving) else {
            break;
        };
        exchange_edges(&mut tree, graph, &leaving, &entering);
    }
}

/// Compute every tree edge's cut value, children before parents.
fn init_cut_values(tree: &mut Graph, graph: &Graph) {
    let tree_nodes = tree.nodes();
    let mut visited_nodes = postorder(tree, tree_nodes);
    let _root = visited_nodes.pop();
    for v in visited_nodes {
        assign_cut_value(tree, graph, &v);
    }
}

fn assign_cut_value(tree: &mut Graph, graph: &Graph, child: &str) {
    let parent = tree
        .node(child)
        .parent_node
        .clone()
        .expect("every non-root tree node has a parent");
    let cut_value = calc_cut_value(tree, graph, child);
    let tree_edge = tree
        .edge_vw_mut(child, &parent)
        .expect("a tree node and its parent share a tree edge");
    tree_edge.cutvalue = Some(cut_value);
}

/// Sum the weights of graph edges crossing the cut made by removing the edge between `child` and its tree parent, signed by direction.
fn calc_cut_value(tree: &Graph, graph: &Graph, child: &str) -> f64 {
    let parent = tree
        .node(child)
        .parent_node
        .clone()
        .expect("every non-root tree node has a parent");
    let edge_from_child = graph.edge_vw(child, &parent);
    let child_is_tail = edge_from_child.is_some();
    let parent_edge = edge_from_child.or_else(|| graph.edge_vw(&parent, child));
    let mut cut_value = parent_edge.map_or(0.0, |e| e.weight.unwrap_or(0.0));

    for edge in graph.node_edges(child) {
        let is_out_edge = edge.v == child;
        let other = if is_out_edge { &edge.w } else { &edge.v };
        let is_parent_edge = *other == parent;
        if is_parent_edge {
            continue;
        }
        let points_to_head = is_out_edge == child_is_tail;
        let other_weight = graph.edge(&edge).map_or(0.0, |e| e.weight.unwrap_or(0.0));
        cut_value += if points_to_head {
            other_weight
        } else {
            -other_weight
        };
        let is_tree_edge = tree.has_edge(child, other);
        if !is_tree_edge {
            continue;
        }
        let tree_edge_cut_value = tree
            .edge_vw(child, other)
            .and_then(|e| e.cutvalue)
            .unwrap_or(0.0);
        cut_value += if points_to_head {
            -tree_edge_cut_value
        } else {
            tree_edge_cut_value
        };
    }
    cut_value
}

/// Number the tree in postorder from its first node, giving each node its subtree's `low..=lim` range and its parent.
fn init_low_lim_values(tree: &mut Graph) {
    let root = tree.nodes().into_iter().next().unwrap_or_default();
    dfs_assign_low_lim(tree, &mut HashSet::new(), 1, &root, None);
}

fn dfs_assign_low_lim(
    tree: &mut Graph,
    visited: &mut HashSet<String>,
    mut next_lim: i32,
    v: &str,
    parent: Option<&str>,
) -> i32 {
    let low = next_lim;
    visited.insert(v.to_string());
    for w in tree.neighbors(v) {
        let is_unvisited = !visited.contains(&w);
        if is_unvisited {
            next_lim = dfs_assign_low_lim(tree, visited, next_lim, &w, Some(v));
        }
    }
    let label = tree.node_mut(v);
    label.low = Some(low);
    label.lim = Some(next_lim);
    label.parent_node = parent.map(str::to_string);
    next_lim + 1
}

fn leave_edge(tree: &Graph) -> Option<Edge> {
    tree.edges().into_iter().find(|e| {
        tree.edge(e)
            .and_then(|label| label.cutvalue)
            .is_some_and(|cut_value| cut_value < 0.0)
    })
}

/// Pick the first least-slack graph edge that reconnects the two halves the leaving edge splits the tree into.
fn enter_edge(tree: &Graph, graph: &Graph, leaving: &Edge) -> Option<Edge> {
    let is_leaving_edge_forward = graph.has_edge(&leaving.v, &leaving.w);
    let (tail, head) = if is_leaving_edge_forward {
        (&leaving.v, &leaving.w)
    } else {
        (&leaving.w, &leaving.v)
    };
    let tail_label = tree.node_opt(tail)?;
    let head_label = tree.node_opt(head)?;
    let tail_lim = tail_label.lim.unwrap_or(0);
    let head_lim = head_label.lim.unwrap_or(0);
    let is_flipped = tail_lim > head_lim;
    let subtree_label = if is_flipped { head_label } else { tail_label };

    let crosses_cut = |e: &Edge| {
        let Some(v_label) = tree.node_opt(&e.v) else {
            return false;
        };
        let Some(w_label) = tree.node_opt(&e.w) else {
            return false;
        };
        let is_v_in_subtree = is_descendant(v_label, subtree_label);
        let is_w_in_subtree = is_descendant(w_label, subtree_label);
        is_flipped == is_v_in_subtree && is_flipped != is_w_in_subtree
    };
    graph
        .edges()
        .into_iter()
        .filter(crosses_cut)
        .reduce(|least_slack, e| {
            let is_less_slack = slack(graph, &e) < slack(graph, &least_slack);
            if is_less_slack { e } else { least_slack }
        })
}

fn exchange_edges(tree: &mut Graph, graph: &mut Graph, leaving: &Edge, entering: &Edge) {
    tree.remove_edge(leaving);
    tree.set_edge(&entering.v, &entering.w, EdgeLabel::default(), None);
    init_low_lim_values(tree);
    init_cut_values(tree, graph);
    update_ranks(tree, graph);
}

/// Re-rank every node from the tree root down, keeping each tree edge tight.
fn update_ranks(tree: &Graph, graph: &mut Graph) {
    let Some(root) = tree
        .nodes()
        .into_iter()
        .find(|v| tree.node(v).parent_node.is_none())
    else {
        return;
    };
    let mut descendants = preorder(tree, vec![root]);
    let _root = descendants.remove(0);
    for v in descendants {
        let parent = tree
            .node(&v)
            .parent_node
            .clone()
            .expect("every non-root tree node has a parent");
        let edge_to_parent = graph.edge_vw(&v, &parent);
        let is_flipped = edge_to_parent.is_none();
        let tree_edge = edge_to_parent.or_else(|| graph.edge_vw(&parent, &v));
        let minlen = tree_edge.and_then(|e| e.minlen).unwrap_or(1);
        let parent_rank = graph.node(&parent).rank.unwrap_or(0);
        let rank_offset = if is_flipped { minlen } else { -minlen };
        graph.node_mut(&v).rank = Some(parent_rank + rank_offset);
    }
}

fn is_descendant(v_label: &NodeLabel, root_label: &NodeLabel) -> bool {
    let root_low = root_label.low.unwrap_or(0);
    let root_lim = root_label.lim.unwrap_or(0);
    let v_lim = v_label.lim.unwrap_or(0);
    root_low <= v_lim && v_lim <= root_lim
}
