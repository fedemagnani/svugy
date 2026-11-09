//! Hold every arrangement svugy picks on its own: for each container free to arrange a column instead of a row and an order for its children, and for each complete group of edges whether it bundles and where its slots sit.

use std::collections::{BTreeMap, BTreeSet};

use svugy_model::model::{EdgeId, Element, Model, PlacementHint};
use svugy_model::path::ElementPath;

use super::LevelPlans;
use super::edge_levels::{HomedEdges, LeavingEdges};
use super::place::SiblingGraph;

/// Holds the decisions a layout follows beyond what the author wrote: for each container free to arrange, those with no author's hint and no edge among their children, a column instead of a row and an order for their children; for each complete group of edges, whether it stays unbundled and which side its slots take.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Arrangements {
    columns: BTreeSet<ElementPath>,
    packing_orders: BTreeMap<ElementPath, Vec<ElementPath>>,
    unbundled_groups: BTreeSet<GroupId>,
    slot_sides: BTreeMap<GroupId, SlotSide>,
}

/// Names a complete group of edges, one sharing a trunk when bundled, by its first edge in edge order: groups partition the edges, so no two share it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct GroupId(pub(crate) EdgeId);

/// Names where a bundle's slot sits on a container's boundary: where the line toward the far child leaves it, or on the container's side toward plan +x or +y nearest that line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlotSide {
    #[default]
    TowardFarChild,
    TowardPlanEastOrSouth,
}

impl Arrangements {
    /// Arrange nothing: every container packs as authored, unhinted ones in a row, all in source order, and every complete group bundles with its slots toward the far child.
    pub(crate) fn none() -> Self {
        let columns = BTreeSet::new();
        let packing_orders = BTreeMap::new();
        let unbundled_groups = BTreeSet::new();
        let slot_sides = BTreeMap::new();
        Self {
            columns,
            packing_orders,
            unbundled_groups,
            slot_sides,
        }
    }

    /// Stack `level`'s children in a column instead of a row.
    pub(crate) fn with_column(mut self, level: ElementPath) -> Self {
        self.columns.insert(level);
        self
    }

    /// Spread `level`'s children in a row, as an unhinted level packs them by default.
    pub(crate) fn with_row(mut self, level: &ElementPath) -> Self {
        self.columns.remove(level);
        self
    }

    /// Pack `level`'s children in `packing_order`, a permutation of them.
    pub(crate) fn with_packing_order(
        mut self,
        level: ElementPath,
        packing_order: Vec<ElementPath>,
    ) -> Self {
        self.packing_orders.insert(level, packing_order);
        self
    }

    /// Keep the complete group `group` as separate arcs instead of one trunk.
    pub(crate) fn with_unbundled_group(mut self, group: GroupId) -> Self {
        self.unbundled_groups.insert(group);
        self
    }

    /// Seat the slots of the complete group `group` on `slot_side`.
    pub(crate) fn with_slot_side(mut self, group: GroupId, slot_side: SlotSide) -> Self {
        self.slot_sides.insert(group, slot_side);
        self
    }

    /// Count the pairs of siblings packed in the opposite order to their source, over every container given a packing order.
    pub(crate) fn source_order_distance(&self, model: &Model) -> usize {
        self.packing_orders
            .iter()
            .map(|(level, packing_order)| {
                let source_order = SiblingGraph::source_order_of_children(model, level);
                let source_rank_by_child: BTreeMap<&ElementPath, usize> = source_order
                    .iter()
                    .enumerate()
                    .map(|(source_rank, child)| (child, source_rank))
                    .collect();
                let source_ranks: Vec<usize> = packing_order
                    .iter()
                    .map(|child| source_rank_by_child[child])
                    .collect();
                let inverted_pairs = source_ranks.iter().enumerate().flat_map(|(index, rank)| {
                    let later_ranks = &source_ranks[index + 1..];
                    later_ranks
                        .iter()
                        .filter(move |later_rank| *later_rank < rank)
                });
                inverted_pairs.count()
            })
            .sum()
    }

    /// Return the containers free to arrange: left by edges, packing two or more children with no author's hint and no edge among them.
    pub(crate) fn free_containers(model: &Model) -> BTreeSet<ElementPath> {
        let leaving_edges = LeavingEdges::of(model);
        Self::free_containers_of(model, &leaving_edges)
    }

    /// Return the order `level`'s children pack in: the one picked for it, else source order.
    pub(crate) fn packing_order_of(&self, model: &Model, level: &ElementPath) -> Vec<ElementPath> {
        let Some(packing_order) = self.packing_orders.get(level) else {
            return SiblingGraph::source_order_of_children(model, level);
        };
        packing_order.clone()
    }

    /// Tell whether the complete group `group` stays separate arcs.
    pub(crate) fn is_unbundled(&self, group: &GroupId) -> bool {
        self.unbundled_groups.contains(group)
    }

    /// Return the side the slots of the complete group `group` take.
    pub(crate) fn slot_side_of(&self, group: &GroupId) -> SlotSide {
        let slot_side = self.slot_sides.get(group).copied();
        slot_side.unwrap_or_default()
    }

    /// Face toward their edges, from a first layout `plans` of `model`, the containers free to arrange whose leaving edges run mostly along plan x, stacking their children in a column.
    pub(super) fn facing_from(model: &Model, plans: &LevelPlans) -> Self {
        let leaving_edges = LeavingEdges::of(model);
        let columns = Self::free_containers_of(model, &leaving_edges)
            .into_iter()
            .filter(|container| Self::is_left_mostly_along_x(container, &leaving_edges, plans))
            .collect();
        let packing_orders = BTreeMap::new();
        let unbundled_groups = BTreeSet::new();
        let slot_sides = BTreeMap::new();
        Self {
            columns,
            packing_orders,
            unbundled_groups,
            slot_sides,
        }
    }

    /// Order the children of every container free to arrange by where their neighbours sit in `plans`, a layout with these columns, keeping the orders that differ from source order.
    pub(super) fn with_packing_orders_from(self, model: &Model, plans: &LevelPlans) -> Self {
        let leaving_edges = LeavingEdges::of(model);
        let free_containers = Self::free_containers_of(model, &leaving_edges);
        let packing_orders = free_containers
            .iter()
            .filter_map(|container| {
                let source_order = SiblingGraph::source_order_of_children(model, container);
                let packing_order = self.packing_order_by_neighbours(
                    container,
                    source_order.clone(),
                    &free_containers,
                    &leaving_edges,
                    plans,
                );
                let is_source_order = packing_order == source_order;
                if is_source_order {
                    return None;
                }
                let container_key = container.clone();
                Some((container_key, packing_order))
            })
            .collect();
        Self {
            packing_orders,
            ..self
        }
    }

    /// Return the hint `level` packs its children by: its author's, else a column when faced.
    pub(super) fn hint_of(&self, model: &Model, level: &ElementPath) -> Option<PlacementHint> {
        let level_element = model.element(level);
        let authored_hint = level_element.and_then(Element::child_placement);
        let is_faced_column = self.columns.contains(level);
        let faced_hint = is_faced_column.then_some(PlacementHint::Column);
        authored_hint.or(faced_hint)
    }

    /// Return `siblings`, the children of `level`, packed in the order picked for it, if any.
    pub(super) fn arranged_siblings(
        &self,
        level: &ElementPath,
        siblings: SiblingGraph,
    ) -> SiblingGraph {
        let Some(packing_order) = self.packing_orders.get(level) else {
            return siblings;
        };
        siblings.packed_in(packing_order)
    }

    /// Tell whether no container is faced in a column.
    pub(super) fn faces_none(&self) -> bool {
        self.columns.is_empty()
    }

    /// Tell whether every container keeps its source order.
    pub(super) fn keeps_source_orders(&self) -> bool {
        self.packing_orders.is_empty()
    }

    /// Return the containers left by edges that pack two or more children with no author's hint and no edge among them, so their arrangement is svugy's to pick.
    fn free_containers_of(model: &Model, leaving_edges: &LeavingEdges) -> BTreeSet<ElementPath> {
        let homed_edges = HomedEdges::of(model);
        leaving_edges
            .containers_left_by_edges()
            .filter(|container| {
                let container_element = model.element(container);
                let authored_hint = container_element.and_then(Element::child_placement);
                let is_unhinted = authored_hint.is_none();
                let has_no_edge_among_children = homed_edges.homed_in(container).is_empty();
                let child_count = model.children(container).count();
                let has_children_to_arrange = child_count >= 2;
                is_unhinted && has_no_edge_among_children && has_children_to_arrange
            })
            .cloned()
            .collect()
    }

    /// Tell whether the edges leaving `container` run mostly along plan x in `plans`, from its centre to each external endpoint's centre in the edge's home level.
    // Σ|uₓ| > Σ|u_y| over the unit vectors u from the container's centre toward each external endpoint
    fn is_left_mostly_along_x(
        container: &ElementPath,
        leaving_edges: &LeavingEdges,
        plans: &LevelPlans,
    ) -> bool {
        let mut along_x = 0.0;
        let mut along_y = 0.0;
        for leaving_edge in leaving_edges.leaving(container) {
            let edge = leaving_edge.edge();
            let home = edge.home_level();
            let external_endpoint = leaving_edge.external_endpoint();
            let container_footprint = plans.footprint_in_level(container, &home);
            let external_footprint = plans.footprint_in_level(external_endpoint, &home);
            let container_center = container_footprint.center();
            let external_center = external_footprint.center();
            let offset = external_center - container_center;
            let squared_distance = offset.hypot2();
            let distance = squared_distance.sqrt();
            let is_coincident = distance == 0.0;
            if is_coincident {
                continue;
            }
            along_x += offset.x.abs() / distance;
            along_y += offset.y.abs() / distance;
        }
        along_x > along_y
    }

    /// Sort `source_order`, `container`'s children, along its packing axis by where their neighbours sit in `plans`: only a neighbour outside every container free to arrange counts, its place being final; a child with none keeps its own place.
    // key(child) = mean of its counted neighbours' root-plan coordinates along the packing axis, else its own; ties keep source order
    fn packing_order_by_neighbours(
        &self,
        container: &ElementPath,
        mut source_order: Vec<ElementPath>,
        free_containers: &BTreeSet<ElementPath>,
        leaving_edges: &LeavingEdges,
        plans: &LevelPlans,
    ) -> Vec<ElementPath> {
        let root = ElementPath::root();
        let child_depth = container.depth() + 1;
        let is_column = self.columns.contains(container);
        let coordinate_along_axis = |path: &ElementPath| {
            let footprint = plans.footprint_in_level(path, &root);
            let center = footprint.center();
            if is_column {
                return center.y;
            }
            center.x
        };
        let mut neighbour_coordinates_by_child: BTreeMap<ElementPath, Vec<f64>> = BTreeMap::new();
        for leaving_edge in leaving_edges.leaving(container) {
            let external_endpoint = leaving_edge.external_endpoint();
            let is_inside_free_container = external_endpoint
                .ancestors()
                .any(|ancestor| free_containers.contains(&ancestor));
            if is_inside_free_container {
                continue;
            }
            let edge = leaving_edge.edge();
            let inner_end = leaving_edge.inner_end();
            let inner_endpoint = inner_end.endpoint_of(edge);
            let child = inner_endpoint.ancestor_or_self_at_depth(child_depth);
            let neighbour_coordinate = coordinate_along_axis(external_endpoint);
            let neighbour_coordinates = neighbour_coordinates_by_child.entry(child).or_default();
            neighbour_coordinates.push(neighbour_coordinate);
        }
        let sort_key = |child: &ElementPath| {
            let Some(neighbour_coordinates) = neighbour_coordinates_by_child.get(child) else {
                return coordinate_along_axis(child);
            };
            let coordinate_sum: f64 = neighbour_coordinates.iter().sum();
            let neighbour_count = neighbour_coordinates.len() as f64;
            coordinate_sum / neighbour_count
        };
        source_order.sort_by(|child, other_child| {
            let key = sort_key(child);
            let other_key = sort_key(other_child);
            key.total_cmp(&other_key)
        });
        source_order
    }
}

impl GroupId {
    /// Name the complete group of `edges` by its first edge in edge order.
    pub(super) fn of_group(edges: &[&EdgeId]) -> Self {
        let first_edge = edges
            .iter()
            .min()
            .expect("a complete group holds at least one edge");
        let first_edge_key = (*first_edge).clone();
        Self(first_edge_key)
    }
}
