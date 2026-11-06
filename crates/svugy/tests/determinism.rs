//! Prove the same model renders to the same bytes, whatever the hash order of the process.

use std::collections::BTreeSet;

use svugy::StaticSvg;
use svugy_model::builder::{ElementRef, ModelBuilder};
use svugy_model::model::{Edge, Element, Model};

const REPEATED_RENDER_COUNT: usize = 200;
const RANDOM_GRAPH_COUNT: usize = 300;
const RANDOM_GRAPH_SEED: u64 = 0x0005_eed0_f3a6;
const MAX_RANDOM_NODE_COUNT: u64 = 6;
const MAX_RANDOM_EDGE_COUNT: u64 = 8;

/// Draws a repeatable stream of small integers from a 64-bit linear congruential generator.
struct LinearCongruentialGenerator {
    state: u64,
}

impl LinearCongruentialGenerator {
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
    const INCREMENT: u64 = 1_442_695_040_888_963_407;

    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Advance `state ← state·a + c (mod 2⁶⁴)` and return `(state >> 33) mod bound`.
    fn next_below(&mut self, bound: u64) -> u64 {
        let multiplied_state = self.state.wrapping_mul(Self::MULTIPLIER);
        self.state = multiplied_state.wrapping_add(Self::INCREMENT);
        let high_bits = self.state >> 33;
        high_bits % bound
    }
}

/// Build a model of top-level elements with the given ids, joined by the given index pairs.
fn top_level_model_of(element_ids: &[&str], edge_index_pairs: &[(usize, usize)]) -> Model {
    let mut builder = ModelBuilder::new();
    let element_refs: Vec<ElementRef> = element_ids
        .iter()
        .map(|&element_id| {
            let element = Element::new(element_id);
            builder.add_top_level(element_id, element)
        })
        .collect();
    for &(from_index, to_index) in edge_index_pairs {
        let from = &element_refs[from_index];
        let to = &element_refs[to_index];
        let edge = Edge::new();
        builder.add_edge(from, to, edge);
    }
    builder.build().model.expect("the model is valid")
}

/// Draw a top-level model of a few elements joined by a few edges between distinct elements.
fn random_small_model(generator: &mut LinearCongruentialGenerator) -> Model {
    let extra_node_count = generator.next_below(MAX_RANDOM_NODE_COUNT - 1);
    let node_count = 2 + extra_node_count as usize;
    let element_ids: Vec<String> = (0..node_count)
        .map(|node_index| format!("n{node_index}"))
        .collect();
    let element_id_refs: Vec<&str> = element_ids.iter().map(String::as_str).collect();
    let edge_count = generator.next_below(MAX_RANDOM_EDGE_COUNT + 1);
    let node_count_bound = node_count as u64;
    let edge_index_pairs: Vec<(usize, usize)> = (0..edge_count)
        .filter_map(|_edge_index| {
            let from_index = generator.next_below(node_count_bound) as usize;
            let to_index = generator.next_below(node_count_bound) as usize;
            let is_self_loop = from_index == to_index;
            if is_self_loop {
                return None;
            }
            Some((from_index, to_index))
        })
        .collect();
    top_level_model_of(&element_id_refs, &edge_index_pairs)
}

#[test]
fn a_graph_whose_alignments_tie_in_width_renders_identically_every_time() {
    let edge_index_pairs = [(0, 1), (0, 2), (1, 0), (1, 2)];
    let model = top_level_model_of(&["n0", "n1", "n2", "n3"], &edge_index_pairs);

    let distinct_renders: BTreeSet<String> = (0..REPEATED_RENDER_COUNT)
        .map(|_render_index| StaticSvg::render(&model))
        .collect();

    assert_eq!(distinct_renders.len(), 1);
}

#[test]
fn random_small_graphs_render_identically_twice() {
    let mut generator = LinearCongruentialGenerator::new(RANDOM_GRAPH_SEED);

    for graph_index in 0..RANDOM_GRAPH_COUNT {
        let model = random_small_model(&mut generator);
        let first_render = StaticSvg::render(&model);
        let second_render = StaticSvg::render(&model);
        assert!(
            first_render == second_render,
            "random graph {graph_index} rendered differently"
        );
    }
}

#[test]
fn a_lone_element_named_like_dagres_root_sentinel_renders() {
    let model = top_level_model_of(&["\u{0}"], &[]);

    let render = StaticSvg::render(&model);

    assert!(render.starts_with("<svg"));
}

#[test]
fn an_element_named_like_dagres_root_sentinel_renders_with_edges() {
    let edge_index_pairs = [(0, 1), (1, 2), (2, 0)];
    let model = top_level_model_of(&["\u{0}", "a", "b"], &edge_index_pairs);

    let render = StaticSvg::render(&model);

    assert!(render.starts_with("<svg"));
}
