use super::SweepNeighbours;
use crate::graph::Graph;

/// A node of the rank being sorted, with its position before the sort and its neighbours' weighted mean order.
#[derive(Debug, Clone)]
pub(super) struct BarycenterEntry {
    pub(super) v: String,
    pub(super) i: usize,
    pub(super) barycenter: Option<f64>,
}

/// barycenter(v) = Σ weight(e)·order(neighbour) / Σ weight(e) over the edges to v's swept neighbours; none without such edges.
pub(super) fn barycenter(
    graph: &Graph,
    movable: &[String],
    neighbours: SweepNeighbours,
) -> Vec<BarycenterEntry> {
    movable
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let edges = match neighbours {
                SweepNeighbours::Predecessors => graph.in_edges(v),
                SweepNeighbours::Successors => graph.out_edges(v),
            };
            let has_neighbours = !edges.is_empty();
            let barycenter = if has_neighbours {
                let (weighted_order_sum, weight_sum) =
                    edges.iter().fold((0.0f64, 0.0f64), |(sum, weight), e| {
                        let edge_weight = graph.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
                        let neighbour = match neighbours {
                            SweepNeighbours::Predecessors => &e.v,
                            SweepNeighbours::Successors => &e.w,
                        };
                        let neighbour_order = graph.node(neighbour).order.unwrap_or(0) as f64;
                        (sum + edge_weight * neighbour_order, weight + edge_weight)
                    });
                Some(weighted_order_sum / weight_sum)
            } else {
                None
            };
            BarycenterEntry {
                v: v.clone(),
                i,
                barycenter,
            }
        })
        .collect()
}
