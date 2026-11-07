use crate::graph::{EdgeLabel, Graph, NodeLabel};
use crate::position::{Separation, position};
use crate::util::normalize_ranks;
use crate::{LeftToRightLayout, Point, Size, acyclic, normalize, order, rank};

/// Gap between two dummy nodes within a rank, dagre's default edge separation.
const EDGE_GAP: f64 = 20.0;

impl LeftToRightLayout {
    /// Centre each node so edges run rightward across ranks; node `i` is sized `node_sizes[i]`, edges are `(from, to)` index pairs, and self-loops are ignored.
    pub fn node_centres(&self, node_sizes: &[Size], edges: &[(usize, usize)]) -> Vec<Point> {
        let is_graph_empty = node_sizes.is_empty();
        if is_graph_empty {
            return Vec::new();
        }
        let mut graph = Self::top_to_bottom_graph_of(node_sizes, edges);
        acyclic::reverse_cycle_closing_edges(&mut graph);
        rank::rank(&mut graph);
        normalize_ranks(&mut graph);
        normalize::split_long_edges(&mut graph);
        order::order(&mut graph);
        let rank_gap_halved_for_dummy_ranks = self.rank_gap / 2.0;
        let separation = Separation {
            node: self.node_gap,
            edge: EDGE_GAP,
            rank: rank_gap_halved_for_dummy_ranks,
        };
        position(&mut graph, &separation);
        Self::translated_left_to_right_centres_of(&graph, node_sizes)
    }

    /// Build the top-to-bottom graph dagre lays out, with each node's width and height swapped and every edge spanning at least two ranks, so a dummy rank separates any two real ones.
    fn top_to_bottom_graph_of(node_sizes: &[Size], edges: &[(usize, usize)]) -> Graph {
        let mut graph = Graph::directed();
        for (index, size) in node_sizes.iter().enumerate() {
            let transposed_node = NodeLabel {
                width: size.height,
                height: size.width,
                ..NodeLabel::default()
            };
            let node_name = index.to_string();
            graph.set_node(&node_name, transposed_node);
        }
        for &(from, to) in edges {
            let is_self_loop = from == to;
            if is_self_loop {
                continue;
            }
            let edge = EdgeLabel {
                minlen: Some(2),
                weight: Some(1.0),
                ..EdgeLabel::default()
            };
            let from_name = from.to_string();
            let to_name = to.to_string();
            graph.set_edge(&from_name, &to_name, edge, None);
        }
        graph
    }

    /// Swap each laid-out centre back to left-to-right axes and translate all of them so the nodes' bounding box starts at the origin.
    fn translated_left_to_right_centres_of(graph: &Graph, node_sizes: &[Size]) -> Vec<Point> {
        let centres: Vec<Point> = (0..node_sizes.len())
            .map(|index| {
                let node_name = index.to_string();
                let node = graph.node(&node_name);
                let x = node.y.expect("position places every node");
                let y = node.x.expect("position places every node");
                Point { x, y }
            })
            .collect();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        for (centre, size) in centres.iter().zip(node_sizes) {
            let half_width = size.width / 2.0;
            let half_height = size.height / 2.0;
            min_x = min_x.min(centre.x - half_width);
            min_y = min_y.min(centre.y - half_height);
        }
        centres
            .into_iter()
            .map(|centre| Point {
                x: centre.x - min_x,
                y: centre.y - min_y,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{LeftToRightLayout, Size};

    #[test]
    fn a_chain_advances_one_rank_gap_rightward_per_edge_on_one_row() {
        let layout = LeftToRightLayout {
            node_gap: 48.0,
            rank_gap: 72.0,
        };
        let wide_box = Size {
            width: 100.0,
            height: 40.0,
        };
        let (client, server, database) = (0, 1, 2);
        let chain_edges = [(server, database), (client, server)];

        let centres = layout.node_centres(&[wide_box; 3], &chain_edges);

        let client_x = centres[client].x;
        let server_x = centres[server].x;
        let database_x = centres[database].x;
        let box_left_to_left = wide_box.width + layout.rank_gap;
        let half_width = wide_box.width / 2.0;
        let half_height = wide_box.height / 2.0;
        assert_eq!(client_x, half_width, "the leftmost box touches x = 0");
        assert_eq!(server_x - client_x, box_left_to_left);
        assert_eq!(database_x - server_x, box_left_to_left);
        let is_one_row_touching_y_zero = centres.iter().all(|centre| centre.y == half_height);
        assert!(is_one_row_touching_y_zero, "{centres:?}");
    }
}
