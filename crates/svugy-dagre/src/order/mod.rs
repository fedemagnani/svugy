mod barycenter;
mod cross_count;
mod init_order;
mod sort;

use self::barycenter::barycenter;
use self::cross_count::cross_count;
use self::init_order::init_order;
use self::sort::sort;
use crate::graph::Graph;
use crate::util::{build_layer_matrix, max_rank};

/// Which neighbours of a rank a sweep orders it by.
#[derive(Clone, Copy)]
enum SweepNeighbours {
    Predecessors,
    Successors,
}

/// Order the nodes in each rank to reduce crossings: sweep by barycenters, alternating down and up, until four sweeps in a row find no fewer crossings.
pub(crate) fn order(graph: &mut Graph) {
    let last_rank = max_rank(graph);
    let down_ranks: Vec<i32> = (1..=last_rank).collect();
    let up_ranks: Vec<i32> = (0..last_rank).rev().collect();

    let initial_layering = init_order(graph);
    assign_order(graph, &initial_layering);

    let mut best_cross_count = f64::INFINITY;
    let mut best_layering = initial_layering;
    let mut sweeps_since_best = 0usize;
    let mut sweep_index = 0usize;
    while sweeps_since_best < 4 {
        let is_down_sweep = sweep_index % 2 == 1;
        let (ranks, neighbours) = if is_down_sweep {
            (&down_ranks, SweepNeighbours::Predecessors)
        } else {
            (&up_ranks, SweepNeighbours::Successors)
        };
        let bias_right = sweep_index % 4 >= 2;
        sweep_ranks(graph, ranks, neighbours, bias_right);

        let layering = build_layer_matrix(graph);
        let layering_cross_count = cross_count(graph, &layering);
        let is_strictly_fewer_crossings = layering_cross_count < best_cross_count;
        if is_strictly_fewer_crossings {
            sweeps_since_best = 0;
            best_layering = layering;
            best_cross_count = layering_cross_count;
        }
        sweep_index += 1;
        sweeps_since_best += 1;
    }
    assign_order(graph, &best_layering);
}

/// Reorder each rank in turn by the barycenters of its neighbours in the rank swept before.
fn sweep_ranks(graph: &mut Graph, ranks: &[i32], neighbours: SweepNeighbours, bias_right: bool) {
    let nodes_by_rank = build_layer_matrix_in_node_order(graph);
    for &rank in ranks {
        let movable = &nodes_by_rank[rank as usize];
        let barycenters = barycenter(graph, movable, neighbours);
        let sorted = sort(barycenters, bias_right);
        for (order, v) in sorted.iter().enumerate() {
            graph.node_mut(v).order = Some(order as i32);
        }
    }
}

/// Group node ids by rank, each rank in graph insertion order.
fn build_layer_matrix_in_node_order(graph: &Graph) -> Vec<Vec<String>> {
    let layer_count = (max_rank(graph) + 1) as usize;
    let mut nodes_by_rank = vec![Vec::new(); layer_count];
    for v in graph.nodes() {
        let rank = graph.node(&v).rank.expect("ranking ranks every node");
        nodes_by_rank[rank as usize].push(v);
    }
    nodes_by_rank
}

fn assign_order(graph: &mut Graph, layering: &[Vec<String>]) {
    for layer in layering {
        for (order, v) in layer.iter().enumerate() {
            graph.node_mut(v).order = Some(order as i32);
        }
    }
}
