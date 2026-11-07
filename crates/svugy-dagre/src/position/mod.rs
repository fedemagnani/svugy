mod bk;

use crate::graph::Graph;
use crate::util::build_layer_matrix;

/// Gap between nodes within a rank, between dummy nodes within a rank, and between ranks.
pub(crate) struct Separation {
    pub(crate) node: f64,
    pub(crate) edge: f64,
    pub(crate) rank: f64,
}

/// Centre each rank on its own row and place nodes along the row by Brandes-Köpf.
pub(crate) fn position(graph: &mut Graph, separation: &Separation) {
    position_y(graph, separation.rank);
    let xs = bk::position_x(graph, separation);
    for (v, x) in xs {
        graph.node_mut(&v).x = Some(x);
    }
}

/// Stack ranks top to bottom, each as tall as its tallest node, and centre every node on its rank's midline.
fn position_y(graph: &mut Graph, rank_separation: f64) {
    let layering = build_layer_matrix(graph);
    let mut prev_y = 0.0f64;
    for layer in layering {
        let max_height = layer.iter().fold(0.0f64, |acc, v| {
            let height = graph.node(v).height;
            if acc > height { acc } else { height }
        });
        for v in &layer {
            graph.node_mut(v).y = Some(prev_y + max_height / 2.0);
        }
        prev_y += max_height + rank_separation;
    }
}
