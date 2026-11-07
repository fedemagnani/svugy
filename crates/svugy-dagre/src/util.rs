use crate::graph::{EdgeLabel, Graph};

/// Merge parallel edges into one, summing their weights and keeping the largest minlen.
pub(crate) fn simplify(graph: &Graph) -> Graph {
    let mut simplified = Graph::directed();
    for v in graph.nodes() {
        let label = graph.node(&v).clone();
        simplified.set_node(&v, label);
    }
    for e in graph.edges() {
        let label = graph.edge(&e).expect("listed edges exist");
        let label_weight = label.weight.unwrap_or(0.0);
        let label_minlen = label.minlen.unwrap_or(1);
        let (weight, minlen) = match simplified.edge_vw(&e.v, &e.w) {
            Some(existing) => {
                let existing_weight = existing.weight.unwrap_or(0.0);
                let existing_minlen = existing.minlen.unwrap_or(1);
                (
                    existing_weight + label_weight,
                    existing_minlen.max(label_minlen),
                )
            }
            None => (label_weight, label_minlen),
        };
        let simple_label = EdgeLabel {
            weight: Some(weight),
            minlen: Some(minlen),
            ..EdgeLabel::default()
        };
        simplified.set_edge(&e.v, &e.w, simple_label, None);
    }
    simplified
}

/// Lay node ids out as `[rank][order]`.
pub(crate) fn build_layer_matrix(graph: &Graph) -> Vec<Vec<String>> {
    let Ok(layer_count) = usize::try_from(max_rank(graph) + 1) else {
        return Vec::new();
    };
    let mut layering: Vec<Vec<String>> = vec![Vec::new(); layer_count];
    for v in graph.nodes() {
        let node = graph.node(&v);
        let Some(rank) = node.rank else {
            continue;
        };
        let order = node.order.unwrap_or(0) as usize;
        let layer = &mut layering[rank as usize];
        if order >= layer.len() {
            layer.resize(order + 1, String::new());
        }
        layer[order] = v;
    }
    layering
}

/// Shift all ranks so the minimum rank is 0.
pub(crate) fn normalize_ranks(graph: &mut Graph) {
    let nodes = graph.nodes();
    let min_rank = nodes
        .iter()
        .filter_map(|v| graph.node(v).rank)
        .min()
        .unwrap_or(0);
    for v in nodes {
        if let Some(rank) = graph.node_mut(&v).rank.as_mut() {
            *rank -= min_rank;
        }
    }
}

pub(crate) fn max_rank(graph: &Graph) -> i32 {
    graph
        .nodes()
        .iter()
        .filter_map(|v| graph.node(v).rank)
        .max()
        .unwrap_or(i32::MIN)
}
