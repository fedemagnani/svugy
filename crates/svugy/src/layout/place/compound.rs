//! Place a ranked level and every ranked container nested in it with one compound dagre layout, so ranks and crossings span containers.

use std::collections::{BTreeMap, BTreeSet};

use dagre_dgl_rs::{EdgeLabel, Graph, GraphLabel, NodeLabel};
use kurbo::{Point, Rect, Size};
use svugy_model::model::{Element, Model, PlacementHint};
use svugy_model::path::ElementPath;

use super::footprints::Footprints;
use super::{RankedPlacement, SiblingGraph};
use crate::layout::edge_levels::LeavingEdges;
use crate::layout::{PlacedLevel, PlacedLevels};
use crate::metrics::{LabelBlock, LabelRole, TextMeasure};

/// Gap between neighbours within one rank, as in the per-level layered layout.
const NODE_GAP: f64 = 48.0;
/// Gap between consecutive ranks, halved because every edge spans two ranks so a dummy rank separates any two real ones.
const RANK_GAP_HALVED: f64 = 36.0;
/// Gap between two dummy or border nodes within a rank, dagre's default.
const EDGE_GAP: f64 = 20.0;

/// Holds the ranked containers that join their parent's compound layout instead of being placed on their own.
pub(in crate::layout) struct CompoundRegions {
    absorbed: BTreeSet<ElementPath>,
}

impl CompoundRegions {
    /// Absorb every ranked container whose parent is ranked, unless an edge ends on it: dagre cannot attach an edge to a subgraph.
    pub(in crate::layout) fn of(model: &Model) -> Self {
        let edge_endpoints: BTreeSet<&ElementPath> = model
            .arc_edges()
            .flat_map(|(edge_id, _edge)| [edge_id.from(), edge_id.to()])
            .collect();
        let absorbed = model
            .elements()
            .keys()
            .filter(|path| {
                let is_container = model.is_container(path);
                let is_edge_endpoint = edge_endpoints.contains(path);
                let parent = path.enclosing_level();
                let is_parent_ranked = Self::is_ranked(model, &parent);
                let is_self_ranked = Self::is_ranked(model, path);
                is_container && !is_edge_endpoint && is_parent_ranked && is_self_ranked
            })
            .cloned()
            .collect();
        Self { absorbed }
    }

    /// Tell whether `level` places its children in dagre ranks: layered by request, or by default when edges join its children, with no child pinned.
    pub(in crate::layout) fn is_ranked(model: &Model, level: &ElementPath) -> bool {
        let level_element = model.element(level);
        let hint = level_element.and_then(Element::child_placement);
        let is_layered_by_request = hint == Some(PlacementHint::Layered);
        let has_edges_among_children = model
            .arc_edges()
            .any(|(edge_id, _edge)| edge_id.home_level() == *level);
        let is_layered_by_default = hint.is_none() && has_edges_among_children;
        let has_pinned_child = model
            .children(level)
            .any(|(_path, element)| element.position().is_some());
        let is_layered = is_layered_by_request || is_layered_by_default;
        is_layered && !has_pinned_child
    }
}

impl RankedPlacement for CompoundRegions {
    /// Place a region root with its absorbed containers in one compound layout; an absorbed level yields nothing, its region root places it.
    fn place_ranked_level(
        &self,
        model: &Model,
        level: &ElementPath,
        leaving_edges: &LeavingEdges,
        interiors: &PlacedLevels,
        measure: &impl TextMeasure,
    ) -> Vec<(ElementPath, PlacedLevel)> {
        let is_absorbed = self.is_absorbed(level);
        if is_absorbed {
            return Vec::new();
        }
        self.place_region(model, level, leaving_edges, interiors, measure)
    }
}

impl CompoundRegions {
    /// Tell whether `level` is placed inside its parent's compound layout.
    fn is_absorbed(&self, level: &ElementPath) -> bool {
        self.absorbed.contains(level)
    }

    /// Tell whether `level` roots a compound layout: ranked, and not absorbed into its parent's.
    pub(in crate::layout) fn is_region_root(&self, model: &Model, level: &ElementPath) -> bool {
        let is_ranked = Self::is_ranked(model, level);
        let is_absorbed = self.is_absorbed(level);
        is_ranked && !is_absorbed
    }

    /// Place `region_root` and its absorbed containers in one compound layout, returning each of their levels.
    fn place_region(
        &self,
        model: &Model,
        region_root: &ElementPath,
        leaving_edges: &LeavingEdges,
        interiors: &PlacedLevels,
        measure: &impl TextMeasure,
    ) -> Vec<(ElementPath, PlacedLevel)> {
        let region_levels = self.region_levels_of(model, region_root);
        let node_sizes = Self::node_sizes_of(model, &region_levels, interiors, measure, self);
        let joined_pairs = Self::joined_pairs_of(model, region_root, &region_levels);
        let centres = Self::compound_centres(&region_levels, &node_sizes, &joined_pairs, self);
        let mut footprints: BTreeMap<ElementPath, Rect> = node_sizes
            .iter()
            .map(|(path, &size)| {
                let centre = centres[path];
                let footprint = Rect::from_center_size(centre, size);
                (path.clone(), footprint)
            })
            .collect();
        let mut placed = Vec::new();
        let absorbed_innermost_first = region_levels
            .iter()
            .rev()
            .filter(|level| *level != region_root);
        for container in absorbed_innermost_first {
            let children_footprints = Self::children_footprints(&footprints, container);
            let is_left_by_edges = leaving_edges.is_left_by_edges(container);
            let inset = PlacedLevel::inset_inside_boundary(is_left_by_edges);
            let children_box = Self::union_of(&children_footprints);
            let container_element = model.element(container);
            let tag_width = container_element.map_or(0.0, |element| {
                let tag_block = LabelBlock::measure(element, LabelRole::ContainerTag, measure);
                tag_block.hanging_width()
            });
            let padded_width = children_box.width() + 2.0 * inset;
            let width = padded_width.max(tag_width);
            let depth = children_box.height() + 2.0 * inset;
            let origin = Point::new(children_box.x0 - inset, children_box.y0 - inset);
            let container_rect = Rect::from_origin_size(origin, Size::new(width, depth));
            let local_footprints = children_footprints
                .into_iter()
                .map(|(path, footprint)| (path, footprint - origin.to_vec2()))
                .collect();
            let level = PlacedLevel::new(container_rect.size(), local_footprints);
            placed.push((container.clone(), level));
            footprints.insert(container.clone(), container_rect);
        }
        let root_footprints = Self::children_footprints(&footprints, region_root);
        let is_root_left_by_edges = leaving_edges.is_left_by_edges(region_root);
        let inset = PlacedLevel::inset_inside_boundary(is_root_left_by_edges);
        let inset_footprints = Footprints::new(root_footprints).shifted_inside_inset(inset);
        let root_element = model.element(region_root);
        let extent = PlacedLevel::extent_fitting_footprints_and_tag(
            &inset_footprints,
            inset,
            root_element,
            measure,
        );
        let root_level = PlacedLevel::new(extent, inset_footprints.into_by_path());
        placed.push((region_root.clone(), root_level));
        placed
    }

    /// Return `region_root` and every container absorbed below it through absorbed containers, parents first.
    fn region_levels_of(&self, model: &Model, region_root: &ElementPath) -> Vec<ElementPath> {
        let mut region_levels = vec![region_root.clone()];
        let mut next_index = 0;
        while next_index < region_levels.len() {
            let level = region_levels[next_index].clone();
            let absorbed_children = model
                .children(&level)
                .map(|(path, _element)| path.clone())
                .filter(|path| self.is_absorbed(path));
            region_levels.extend(absorbed_children);
            next_index += 1;
        }
        region_levels
    }

    /// Size every child of a region level that is not itself absorbed: a placed container by its extent, a leaf by its labels.
    fn node_sizes_of(
        model: &Model,
        region_levels: &[ElementPath],
        interiors: &PlacedLevels,
        measure: &impl TextMeasure,
        regions: &Self,
    ) -> BTreeMap<ElementPath, Size> {
        region_levels
            .iter()
            .flat_map(|level| model.children(level))
            .filter(|(path, _element)| !regions.is_absorbed(path))
            .map(|(path, element)| {
                let interior = interiors.level(path);
                let size = interior.map_or_else(
                    || SiblingGraph::footprint_size_fitting_labels(element, measure),
                    |interior_placed| interior_placed.extent,
                );
                (path.clone(), size)
            })
            .collect()
    }

    /// Join, once for every edge inside `region_root`, the region nodes holding its endpoints, so parallel edges pull their ends into line harder.
    fn joined_pairs_of(
        model: &Model,
        region_root: &ElementPath,
        region_levels: &[ElementPath],
    ) -> Vec<(ElementPath, ElementPath)> {
        let region_level_set: BTreeSet<&ElementPath> = region_levels.iter().collect();
        let region_node_holding = |endpoint: &ElementPath| {
            let mut node = endpoint.clone();
            loop {
                let parent = node.enclosing_level();
                let is_parent_in_region = region_level_set.contains(&parent);
                if is_parent_in_region {
                    return node;
                }
                node = parent;
            }
        };
        model
            .arc_edges()
            .filter(|(edge_id, _edge)| {
                let is_from_inside = region_root.is_ancestor_of(edge_id.from());
                let is_to_inside = region_root.is_ancestor_of(edge_id.to());
                is_from_inside && is_to_inside
            })
            .map(|(edge_id, _edge)| {
                let from_node = region_node_holding(edge_id.from());
                let to_node = region_node_holding(edge_id.to());
                (from_node, to_node)
            })
            .filter(|(from_node, to_node)| from_node != to_node)
            .collect()
    }

    /// Lay the region out as one left-to-right compound graph under each Brandes-Köpf alignment and return the sized nodes' centres from the one that bends the joined pairs least.
    fn compound_centres(
        region_levels: &[ElementPath],
        node_sizes: &BTreeMap<ElementPath, Size>,
        joined_pairs: &[(ElementPath, ElementPath)],
        regions: &Self,
    ) -> BTreeMap<ElementPath, Point> {
        let is_region_empty = node_sizes.is_empty();
        if is_region_empty {
            return BTreeMap::new();
        }
        let mut graph = Graph::with_options(true, true, true);
        let graph_label = GraphLabel {
            rankdir: Some("LR".to_owned()),
            nodesep: Some(NODE_GAP),
            ranksep: Some(RANK_GAP_HALVED),
            edgesep: Some(EDGE_GAP),
            ..GraphLabel::default()
        };
        graph.set_graph(graph_label);
        let region_root = &region_levels[0];
        let absorbed_levels = region_levels.iter().skip(1);
        let named_paths = absorbed_levels.chain(node_sizes.keys());
        // dagre reserves "\x00" and "_"-prefixed names for its own nodes, so ours are indices
        let node_name_by_path: BTreeMap<&ElementPath, String> = named_paths
            .enumerate()
            .map(|(index, path)| (path, format!("n{index}")))
            .collect();
        for level in region_levels.iter().skip(1) {
            let level_name = &node_name_by_path[level];
            graph.set_node(level_name, NodeLabel::default());
        }
        for (path, size) in node_sizes {
            let node = NodeLabel {
                width: size.width,
                height: size.height,
                ..NodeLabel::default()
            };
            let node_name = &node_name_by_path[path];
            graph.set_node(node_name, node);
        }
        for (path, node_name) in &node_name_by_path {
            let parent = path.enclosing_level();
            let is_parent_absorbed = regions.is_absorbed(&parent);
            let is_parent_in_region = parent != *region_root && is_parent_absorbed;
            if !is_parent_in_region {
                continue;
            }
            let parent_name = &node_name_by_path[&parent];
            graph.set_parent(node_name, Some(parent_name));
        }
        for (edge_index, (from, to)) in joined_pairs.iter().enumerate() {
            let edge = EdgeLabel {
                minlen: Some(2),
                weight: Some(1.0),
                ..EdgeLabel::default()
            };
            let from_name = &node_name_by_path[from];
            let to_name = &node_name_by_path[to];
            let edge_name = format!("e{edge_index}");
            graph.set_edge(from_name, to_name, edge, Some(&edge_name));
        }
        let alignments = ["UL", "UR", "DL", "DR"];
        let laid_out_centres = alignments.map(|alignment| {
            let mut aligned_graph = graph.clone();
            let aligned_label = GraphLabel {
                align: Some(alignment.to_owned()),
                ..graph.graph().clone()
            };
            aligned_graph.set_graph(aligned_label);
            dagre_dgl_rs::layout(&mut aligned_graph);
            node_sizes
                .keys()
                .map(|path| {
                    let node_name = &node_name_by_path[path];
                    let node = aligned_graph.node(node_name);
                    let x = node.x.expect("dagre places every node");
                    let y = node.y.expect("dagre places every node");
                    (path.clone(), Point::new(x, y))
                })
                .collect::<BTreeMap<ElementPath, Point>>()
        });
        laid_out_centres
            .into_iter()
            .min_by(|centres, other_centres| {
                let bend = Self::total_bend_of(centres, joined_pairs);
                let other_bend = Self::total_bend_of(other_centres, joined_pairs);
                bend.total_cmp(&other_bend)
            })
            .expect("four alignments are laid out")
    }

    /// Sum how far each joined pair's ends sit apart across the rank axis, Σ |y_from − y_to|, so a parallel edge counts once more.
    fn total_bend_of(
        centres: &BTreeMap<ElementPath, Point>,
        joined_pairs: &[(ElementPath, ElementPath)],
    ) -> f64 {
        joined_pairs
            .iter()
            .map(|(from, to)| {
                let from_y = centres[from].y;
                let to_y = centres[to].y;
                (from_y - to_y).abs()
            })
            .sum()
    }

    /// Return the footprints of `level`'s direct children among `footprints`.
    fn children_footprints(
        footprints: &BTreeMap<ElementPath, Rect>,
        level: &ElementPath,
    ) -> BTreeMap<ElementPath, Rect> {
        footprints
            .iter()
            .filter(|(path, _footprint)| path.enclosing_level() == *level)
            .map(|(path, footprint)| (path.clone(), *footprint))
            .collect()
    }

    /// Return the smallest rectangle enclosing every footprint.
    fn union_of(footprints: &BTreeMap<ElementPath, Rect>) -> Rect {
        footprints
            .values()
            .copied()
            .reduce(|union, footprint| union.union(footprint))
            .expect("an absorbed container has at least one child")
    }
}
