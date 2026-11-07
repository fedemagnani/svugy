use std::collections::{HashMap, HashSet};

use super::Separation;
use crate::graph::{EdgeLabel, Graph};
use crate::util::build_layer_matrix;

type Conflicts = HashMap<String, HashSet<String>>;
type PositionMap = HashMap<String, f64>;

/// Which way an alignment runs vertically: from the top rank aligning to predecessors, or from the bottom aligning to successors.
#[derive(Clone, Copy)]
enum Vertical {
    Up,
    Down,
}

/// Which way an alignment runs horizontally: packing leftward, or rightward.
#[derive(Clone, Copy)]
enum Horizontal {
    Left,
    Right,
}

/// The four alignments in dagre-js's insertion order, which settles ties in width.
const ALIGNMENTS_IN_TIE_BREAK_ORDER: [(Vertical, Horizontal); 4] = [
    (Vertical::Up, Horizontal::Left),
    (Vertical::Up, Horizontal::Right),
    (Vertical::Down, Horizontal::Left),
    (Vertical::Down, Horizontal::Right),
];

/// Place every node along its rank by Brandes-Köpf: compact four alignments, align them to the narrowest, and take the median of each node's four positions.
pub(super) fn position_x(graph: &Graph, separation: &Separation) -> PositionMap {
    let layering = build_layer_matrix(graph);
    let conflicts = find_type1_conflicts(graph, &layering);
    let mut xss = ALIGNMENTS_IN_TIE_BREAK_ORDER.map(|(vertical, horizontal)| {
        let mut adjusted_layering = layering.clone();
        if let Vertical::Down = vertical {
            adjusted_layering.reverse();
        }
        if let Horizontal::Right = horizontal {
            for layer in &mut adjusted_layering {
                layer.reverse();
            }
        }
        let (root, align) = vertical_alignment(graph, &adjusted_layering, &conflicts, vertical);
        let xs = horizontal_compaction(graph, &adjusted_layering, &root, &align, separation);
        match horizontal {
            Horizontal::Left => xs,
            Horizontal::Right => xs.into_iter().map(|(v, x)| (v, -x)).collect(),
        }
    });
    let smallest_width = find_smallest_width_alignment(graph, &xss);
    align_coordinates(&mut xss, &smallest_width);
    balance(&xss)
}

/// Mark each edge from a real node that crosses an inner segment, an edge between two dummy nodes, as a type-1 conflict.
fn find_type1_conflicts(graph: &Graph, layering: &[Vec<String>]) -> Conflicts {
    let mut conflicts = Conflicts::new();
    for adjacent_layers in layering.windows(2) {
        let [prev_layer, layer] = adjacent_layers else {
            continue;
        };
        visit_layer(graph, &mut conflicts, prev_layer, layer);
    }
    conflicts
}

fn visit_layer(graph: &Graph, conflicts: &mut Conflicts, prev_layer: &[String], layer: &[String]) {
    let mut k0 = 0usize;
    let mut scan_pos = 0usize;
    let last_node = layer.last();
    for (i, v) in layer.iter().enumerate() {
        let inner_segment_node = find_other_inner_segment_node(graph, v);
        let k1 = inner_segment_node.as_ref().map_or(prev_layer.len(), |w| {
            graph.node(w).order.unwrap_or(0) as usize
        });
        let is_last_node = Some(v) == last_node;
        let is_scan_boundary = inner_segment_node.is_some() || is_last_node;
        if !is_scan_boundary {
            continue;
        }
        for scan_node in &layer[scan_pos..=i] {
            let scan_label = graph.node(scan_node);
            for u in graph.predecessors(scan_node) {
                let u_label = graph.node(&u);
                let u_pos = u_label.order.unwrap_or(0) as usize;
                let is_outside_inner_segments = u_pos < k0 || k1 < u_pos;
                let is_inner_segment = u_label.is_dummy && scan_label.is_dummy;
                if is_outside_inner_segments && !is_inner_segment {
                    add_conflict(conflicts, &u, scan_node);
                }
            }
        }
        scan_pos = i + 1;
        k0 = k1;
    }
}

fn find_other_inner_segment_node(graph: &Graph, v: &str) -> Option<String> {
    let is_real_node = !graph.node(v).is_dummy;
    if is_real_node {
        return None;
    }
    graph
        .predecessors(v)
        .into_iter()
        .find(|u| graph.node(u).is_dummy)
}

fn add_conflict(conflicts: &mut Conflicts, v: &str, w: &str) {
    let (v, w) = if v > w { (w, v) } else { (v, w) };
    conflicts
        .entry(v.to_string())
        .or_default()
        .insert(w.to_string());
}

fn has_conflict(conflicts: &Conflicts, v: &str, w: &str) -> bool {
    let (v, w) = if v > w { (w, v) } else { (v, w) };
    conflicts.get(v).is_some_and(|ws| ws.contains(w))
}

/// Chain each node into a block with its median neighbour in the rank before, unless a conflict or an earlier chain crosses it; return each node's block root and its next node in the block.
fn vertical_alignment(
    graph: &Graph,
    layering: &[Vec<String>],
    conflicts: &Conflicts,
    vertical: Vertical,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut root = HashMap::new();
    let mut align = HashMap::new();
    let mut pos = HashMap::new();
    for layer in layering {
        for (order, v) in layer.iter().enumerate() {
            root.insert(v.clone(), v.clone());
            align.insert(v.clone(), v.clone());
            pos.insert(v.clone(), order);
        }
    }

    for layer in layering {
        let mut prev_idx: i32 = -1;
        for v in layer {
            let mut ws = match vertical {
                Vertical::Up => graph.predecessors(v),
                Vertical::Down => graph.successors(v),
            };
            if ws.is_empty() {
                continue;
            }
            ws.sort_by_key(|w| pos.get(w).cloned().unwrap_or(0));
            let mp = (ws.len() as f64 - 1.0) / 2.0;
            let lo = mp.floor() as usize;
            let hi = mp.ceil() as usize;
            for w in &ws[lo..=hi] {
                let Some(&pos_w) = pos.get(w) else {
                    continue;
                };
                let is_v_unaligned = align[v] == *v;
                let is_right_of_prev = prev_idx < pos_w as i32;
                let is_conflicting = has_conflict(conflicts, v, w);
                if is_v_unaligned && is_right_of_prev && !is_conflicting {
                    let root_w = root[w].clone();
                    align.insert(w.clone(), v.clone());
                    align.insert(v.clone(), root_w.clone());
                    root.insert(v.clone(), root_w);
                    prev_idx = pos_w as i32;
                }
            }
        }
    }
    (root, align)
}

/// Give each block root the smallest coordinate its separation from the blocks before it allows, pull it toward the blocks after it, and copy root coordinates to every block member.
fn horizontal_compaction(
    graph: &Graph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    align: &HashMap<String, String>,
    separation: &Separation,
) -> PositionMap {
    let mut xs = PositionMap::new();
    let block_graph = build_block_graph(graph, layering, root, separation);

    let mut stack = block_graph.nodes();
    let mut visited = HashSet::new();
    while let Some(elem) = stack.pop() {
        let is_first_visit = visited.insert(elem.clone());
        if is_first_visit {
            stack.push(elem.clone());
            stack.extend(block_graph.predecessors(&elem));
            continue;
        }
        let smallest_x = block_graph.in_edges(&elem).iter().fold(0.0f64, |acc, e| {
            let xs_v = xs.get(&e.v).cloned().unwrap_or(0.0);
            let edge_weight = block_graph.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
            acc.max(xs_v + edge_weight)
        });
        xs.insert(elem, smallest_x);
    }

    let mut stack = block_graph.nodes();
    let mut visited = HashSet::new();
    while let Some(elem) = stack.pop() {
        let is_first_visit = visited.insert(elem.clone());
        if is_first_visit {
            stack.push(elem.clone());
            stack.extend(block_graph.successors(&elem));
            continue;
        }
        let greatest_x = block_graph
            .out_edges(&elem)
            .iter()
            .fold(f64::INFINITY, |acc, e| {
                let xs_w = xs.get(&e.w).cloned().unwrap_or(0.0);
                let edge_weight = block_graph.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
                acc.min(xs_w - edge_weight)
            });
        let has_successor = greatest_x != f64::INFINITY;
        if has_successor {
            let current_x = xs.get(&elem).cloned().unwrap_or(0.0);
            xs.insert(elem, current_x.max(greatest_x));
        }
    }

    for v in align.keys() {
        let root_x = xs.get(&root[v]).cloned().unwrap_or(0.0);
        xs.insert(v.clone(), root_x);
    }
    xs
}

/// Link each block root to the root of the block to its right in any layer, weighted by the largest separation they need.
fn build_block_graph(
    graph: &Graph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    separation: &Separation,
) -> Graph {
    let mut block_graph = Graph::directed();
    for layer in layering {
        let mut u: Option<&String> = None;
        for v in layer {
            let v_root = &root[v];
            block_graph.set_node(v_root, Default::default());
            if let Some(u) = u {
                let u_root = &root[u];
                let sep_value = sep(graph, v, u, separation);
                let prev_max = block_graph
                    .edge_vw(u_root, v_root)
                    .and_then(|e| e.weight)
                    .unwrap_or(0.0);
                let block_edge = EdgeLabel {
                    weight: Some(sep_value.max(prev_max)),
                    ..EdgeLabel::default()
                };
                block_graph.set_edge(u_root, v_root, block_edge, None);
            }
            u = Some(v);
        }
    }
    block_graph
}

/// Pick the first alignment with the smallest span between its leftmost and rightmost node borders.
fn find_smallest_width_alignment(graph: &Graph, xss: &[PositionMap; 4]) -> PositionMap {
    let mut best: (f64, Option<&PositionMap>) = (f64::INFINITY, None);
    for xs in xss {
        let mut max_val = f64::NEG_INFINITY;
        let mut min_val = f64::INFINITY;
        for (v, &x) in xs {
            let half_width = graph.node_opt(v).map_or(0.0, |n| n.width / 2.0);
            max_val = max_val.max(x + half_width);
            min_val = min_val.min(x - half_width);
        }
        let span = max_val - min_val;
        if span < best.0 {
            best = (span, Some(xs));
        }
    }
    best.1.cloned().unwrap_or_default()
}

/// Shift each left alignment so its minimum matches `align_to`'s, and each right alignment so its maximum does.
fn align_coordinates(xss: &mut [PositionMap; 4], align_to: &PositionMap) {
    let align_to_min = align_to.values().cloned().fold(f64::INFINITY, f64::min);
    let align_to_max = align_to.values().cloned().fold(f64::NEG_INFINITY, f64::max);
    for (xs, (_vertical, horizontal)) in xss.iter_mut().zip(ALIGNMENTS_IN_TIE_BREAK_ORDER) {
        let delta = match horizontal {
            Horizontal::Left => align_to_min - xs.values().cloned().fold(f64::INFINITY, f64::min),
            Horizontal::Right => {
                align_to_max - xs.values().cloned().fold(f64::NEG_INFINITY, f64::max)
            }
        };
        if delta != 0.0 {
            for x in xs.values_mut() {
                *x += delta;
            }
        }
    }
}

/// Place each node at the mean of the middle two of its four aligned positions.
fn balance(xss: &[PositionMap; 4]) -> PositionMap {
    let [up_left, ..] = xss;
    up_left
        .keys()
        .map(|v| {
            let mut xs: Vec<f64> = xss.iter().filter_map(|xs| xs.get(v)).cloned().collect();
            xs.sort_by(|a, b| a.partial_cmp(b).expect("positions are never NaN"));
            let second = xs.get(1).cloned().unwrap_or(0.0);
            let third = xs.get(2).cloned().unwrap_or(0.0);
            (v.clone(), (second + third) / 2.0)
        })
        .collect()
}

/// sep(v, w) = width(v)/2 + gap(v)/2 + gap(w)/2 + width(w)/2, where gap is the edge separation for dummies and the node separation otherwise.
fn sep(graph: &Graph, v: &str, w: &str, separation: &Separation) -> f64 {
    let v_label = graph.node(v);
    let w_label = graph.node(w);
    let v_gap = if v_label.is_dummy {
        separation.edge
    } else {
        separation.node
    };
    let w_gap = if w_label.is_dummy {
        separation.edge
    } else {
        separation.node
    };
    let mut sum = 0.0;
    sum += v_label.width / 2.0;
    sum += v_gap / 2.0;
    sum += w_gap / 2.0;
    sum += w_label.width / 2.0;
    sum
}
