//! Lay out every hierarchy level in plan space: place levels bottom-up, anchor their routes, then side stubs where those routes leave containers.

mod arrangements;
mod bundle;
mod edge_levels;
mod place;
mod route;
mod stub;
mod trace;

use std::collections::BTreeMap;
use std::iter;

use kurbo::{Line, Point, Rect, Size, Vec2};
use svugy_model::disclose::DisclosedGraph;
use svugy_model::graph::WorkflowGraph;
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use crate::metrics::TextMeasure;
pub(crate) use arrangements::Arrangements;
pub(crate) use arrangements::{GroupId, RankMove, SlotSide};
use edge_levels::{HomedEdges, LeavingEdges};
pub(crate) use place::PlacementCache;
use place::{
    CompoundRegions, MemoizedPlacement, MovedWithinRanks, RankedPlacement, RearrangedFromSeed,
    SiblingGraph,
};
pub(crate) use route::{Branch, Course, Route};
pub(crate) use stub::Stub;

/// Holds one value per hierarchy level keyed by level path, so the root comes first and parents precede their children.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Levels<L> {
    by_level: BTreeMap<ElementPath, L>,
}

/// Holds every level's placement, before any route or stub is laid out.
type PlacedLevels = Levels<PlacedLevel>;
/// Holds the routes of the edges homed in every level, keyed by edge.
type LevelRoutes = Levels<BTreeMap<EdgeId, Route>>;
/// Holds the stubs of the edges leaving every container, keyed by their external endpoint.
type LevelStubs = Levels<BTreeMap<ElementPath, Stub>>;
/// Holds every level's finished plan.
pub(crate) type LevelPlans = Levels<LevelPlan>;

impl<L> Levels<L> {
    /// Hold one value per level, keyed `by_level`.
    pub(crate) fn new(by_level: BTreeMap<ElementPath, L>) -> Self {
        Self { by_level }
    }
}

impl LevelPlans {
    /// Lay out every level, ranking each ranked level and the ranked containers absorbed in it with one compound dagre layout, the containers free to arrange faced toward their edges and ordered by their neighbours.
    #[cfg(test)]
    pub(crate) fn lay_out(graph: &WorkflowGraph, measure: &impl TextMeasure) -> Self {
        let (plans, _arrangements) = Self::lay_out_seeded(graph, measure);
        plans
    }

    /// Lay out every level of `graph` following exactly `arrangements`, nothing picked on the way: ranked levels as compound layouts, reused from `cache` when it holds them for the same inputs, with their children then moved within their ranks.
    pub(crate) fn lay_out_with(
        graph: &WorkflowGraph,
        arrangements: &Arrangements,
        cache: &PlacementCache,
        measure: &impl TextMeasure,
    ) -> Self {
        let compound_regions = CompoundRegions::of(graph);
        let memoized_placement = MemoizedPlacement::new(&compound_regions, cache);
        let moved_within_ranks = MovedWithinRanks::new(&memoized_placement, arrangements);
        Self::lay_out_ranking_by(graph, &moved_within_ranks, arrangements, measure)
    }

    /// Lay out every layer following exactly `arrangements`, the deepest one afresh and every shallower one rearranged from it, so each layer keeps the deepest arrangement.
    pub(crate) fn lay_out_layers_with(
        layers: &[DisclosedGraph],
        arrangements: &Arrangements,
        measure: &impl TextMeasure,
    ) -> Vec<Self> {
        let Some((deepest_layer, shallower_layers)) = layers.split_last() else {
            return Vec::new();
        };
        let deepest_graph = deepest_layer.graph();
        let placement_cache = PlacementCache::new();
        let deepest_plans =
            Self::lay_out_with(deepest_graph, arrangements, &placement_cache, measure);
        let rearranged_from_seed = RearrangedFromSeed::new(&deepest_plans);
        let mut layer_plans: Vec<Self> = shallower_layers
            .iter()
            .map(|layer| {
                let layer_graph = layer.graph();
                Self::lay_out_ranking_by(layer_graph, &rearranged_from_seed, arrangements, measure)
            })
            .collect();
        layer_plans.push(deepest_plans);
        layer_plans
    }

    /// Lay out every level as authored, again with the containers free to arrange faced toward their edges, and once more with their children ordered by where their neighbours sit, skipping a pass that changes nothing; return the final plans and the arrangements, the seed an arrangement search starts from.
    pub(crate) fn lay_out_seeded(
        graph: &WorkflowGraph,
        measure: &impl TextMeasure,
    ) -> (Self, Arrangements) {
        let compound_regions = CompoundRegions::of(graph);
        let authored = Arrangements::none();
        let authored_plans = Self::lay_out_ranking_by(graph, &compound_regions, &authored, measure);
        let facing = Arrangements::facing_from(graph, &authored_plans);
        let faces_none = facing.faces_none();
        let faced_plans = if faces_none {
            authored_plans
        } else {
            Self::lay_out_ranking_by(graph, &compound_regions, &facing, measure)
        };
        let arrangements = facing.with_packing_orders_from(graph, &faced_plans);
        let keeps_source_orders = arrangements.keeps_source_orders();
        if keeps_source_orders {
            return (faced_plans, arrangements);
        }
        let arranged_plans =
            Self::lay_out_ranking_by(graph, &compound_regions, &arrangements, measure);
        (arranged_plans, arrangements)
    }

    /// Lay out every level: place them bottom-up with `ranked_placement` for the ranked ones and `arrangements` picking the unhinted packings, anchor every level's routes, bundle the routes entering containers together as `arrangements` decides, side every container's stubs where those routes leave it, then plan each level with all three.
    fn lay_out_ranking_by(
        graph: &WorkflowGraph,
        ranked_placement: &impl RankedPlacement,
        arrangements: &Arrangements,
        measure: &impl TextMeasure,
    ) -> Self {
        let homed_edges = HomedEdges::of(graph);
        let leaving_edges = LeavingEdges::of(graph);
        let placed_levels = PlacedLevels::place_bottom_up(
            graph,
            ranked_placement,
            arrangements,
            &homed_edges,
            &leaving_edges,
            measure,
        );
        let mut level_routes = placed_levels.anchor_homed_routes(&homed_edges);
        placed_levels.bundle_routes_entering_containers(
            &homed_edges,
            arrangements,
            &mut level_routes,
        );
        let mut level_stubs =
            placed_levels.side_stubs_at_home_route_crossings(&leaving_edges, &level_routes);
        let plans_by_level = placed_levels
            .by_level
            .into_iter()
            .map(|(level, placed)| {
                let routes = level_routes.by_level.remove(&level).unwrap_or_default();
                let stubs = level_stubs.by_level.remove(&level).unwrap_or_default();
                let plan = LevelPlan::new(placed, routes, stubs);
                (level, plan)
            })
            .collect();
        Self::new(plans_by_level)
    }
}

impl PlacedLevels {
    /// Place every level after the levels nested in it, so each container's extent is known before its parent places it: ranked levels by `ranked_placement`, the rest packed or layered by the hint and order `arrangements` gives them.
    fn place_bottom_up(
        graph: &WorkflowGraph,
        ranked_placement: &impl RankedPlacement,
        arrangements: &Arrangements,
        homed_edges: &HomedEdges,
        leaving_edges: &LeavingEdges,
        measure: &impl TextMeasure,
    ) -> Self {
        let root = ElementPath::root();
        let containers = graph
            .elements()
            .keys()
            .filter(|path| graph.is_container(path));
        let root_level = iter::once(&root);
        let levels_innermost_first = containers.rev().chain(root_level);
        let no_levels_yet = BTreeMap::new();
        let mut placed_levels = Self::new(no_levels_yet);
        for level in levels_innermost_first {
            let is_ranked = CompoundRegions::is_ranked(graph, level);
            if is_ranked {
                let ranked_levels = ranked_placement.place_ranked_level(
                    graph,
                    level,
                    leaving_edges,
                    &placed_levels,
                    measure,
                );
                placed_levels.by_level.extend(ranked_levels);
                continue;
            }
            let joined_pairs = homed_edges.lifted_pairs_in(level);
            let siblings = SiblingGraph::of_measured_children(
                graph,
                level,
                joined_pairs,
                &placed_levels,
                measure,
            );
            let is_left_by_edges = leaving_edges.is_left_by_edges(level);
            let arranged_siblings = arrangements.arranged_siblings(level, siblings);
            let hint = arrangements.hint_of(graph, level);
            let placed_level = PlacedLevel::place_children(
                graph,
                level,
                arranged_siblings,
                hint,
                is_left_by_edges,
                measure,
            );
            let level_key = level.clone();
            placed_levels.by_level.insert(level_key, placed_level);
        }
        placed_levels
    }
}

impl<L: LevelPlacement> Levels<L> {
    /// Return `element`'s footprint in its parent level's placement, or `None` when that level does not place it.
    pub(crate) fn footprint_of(&self, element: &ElementPath) -> Option<Rect> {
        let parent = element.parent()?;
        let parent_level = self.by_level.get(&parent)?;
        let parent_placed = parent_level.placed();
        parent_placed.footprints.get(element).copied()
    }

    /// Return `element`'s footprint in the placement of `level`, any of its ancestor levels, offsetting through the containers between.
    fn footprint_in_level(&self, element: &ElementPath, level: &ElementPath) -> Rect {
        let footprint_in_parent = self
            .footprint_of(element)
            .expect("every element below the root is placed in its parent level");
        let parent = element.enclosing_level();
        let is_parent_the_level = parent == *level;
        if is_parent_the_level {
            return footprint_in_parent;
        }
        let parent_footprint = self.footprint_in_level(&parent, level);
        let parent_origin = parent_footprint.origin();
        let parent_offset = parent_origin.to_vec2();
        footprint_in_parent + parent_offset
    }
}

impl<L> Levels<L> {
    /// Return the value held for `level`.
    pub(crate) fn level(&self, level: &ElementPath) -> Option<&L> {
        self.by_level.get(level)
    }

    /// Return every level's value, keyed by level path.
    #[cfg(test)]
    pub(crate) fn levels(&self) -> &BTreeMap<ElementPath, L> {
        &self.by_level
    }
}

/// Exposes the placement fixed for one level, so a bare placement and a finished plan share footprint lookups.
pub(crate) trait LevelPlacement {
    /// Return the placement fixed for this level.
    fn placed(&self) -> &PlacedLevel;
}

/// Places one level's children on the ground plane, in plan units, with its origin at the level's top-left corner.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlacedLevel {
    extent: Size,
    footprints: BTreeMap<ElementPath, Rect>,
}

impl PlacedLevel {
    /// Place the child `footprints` in a level of `extent`, stub margins included.
    pub(crate) fn new(extent: Size, footprints: BTreeMap<ElementPath, Rect>) -> Self {
        Self { extent, footprints }
    }

    /// Return the footprint of every child element, keyed by the child's full path.
    pub(crate) fn footprints(&self) -> &BTreeMap<ElementPath, Rect> {
        &self.footprints
    }

    /// Return the level's own boundary, where stub slots sit.
    pub(crate) fn boundary(&self) -> Rect {
        Rect::from_origin_size(Point::ORIGIN, self.extent)
    }
}

impl LevelPlacement for PlacedLevel {
    fn placed(&self) -> &PlacedLevel {
        self
    }
}

/// Plans one hierarchy level: its placement, the routes homed in it and the stubs of the edges leaving it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LevelPlan {
    placed: PlacedLevel,
    routes: BTreeMap<EdgeId, Route>,
    stubs: BTreeMap<ElementPath, Stub>,
}

impl LevelPlan {
    /// Plan a level from its `placed` children, homed `routes` and leaving `stubs`.
    pub(crate) fn new(
        placed: PlacedLevel,
        routes: BTreeMap<EdgeId, Route>,
        stubs: BTreeMap<ElementPath, Stub>,
    ) -> Self {
        Self {
            placed,
            routes,
            stubs,
        }
    }

    /// Return the route of every edge homed in this level, keyed by edge.
    pub(crate) fn routes(&self) -> &BTreeMap<EdgeId, Route> {
        &self.routes
    }

    /// Return the stubs of the edges leaving this level, keyed by their external endpoint.
    pub(crate) fn stubs(&self) -> &BTreeMap<ElementPath, Stub> {
        &self.stubs
    }
}

impl LevelPlacement for LevelPlan {
    fn placed(&self) -> &PlacedLevel {
        &self.placed
    }
}

/// Names the end of an edge a port belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EdgeEnd {
    Source,
    Target,
}

impl EdgeEnd {
    /// Return the endpoint of `edge` at this end.
    fn endpoint_of(self, edge: &EdgeId) -> &ElementPath {
        match self {
            Self::Source => edge.from(),
            Self::Target => edge.to(),
        }
    }

    /// Return the other end of an edge.
    fn opposite(self) -> Self {
        match self {
            Self::Source => Self::Target,
            Self::Target => Self::Source,
        }
    }

    /// Return the port of `route` at this end.
    fn port_of(self, route: &Route) -> Port {
        match self {
            Self::Source => route.source,
            Self::Target => route.target,
        }
    }
}

/// Marks the port where a route attaches to a side of a footprint or of the level boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Port {
    pub(crate) side: Side,
    pub(crate) point: Point,
}

impl Port {
    /// Mark the port at `point` on `side`.
    pub(crate) fn new(side: Side, point: Point) -> Self {
        Self { side, point }
    }
}

/// Names a side of a plan-space rectangle: north is min y, east max x, south max y, west min x.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Side {
    North,
    East,
    South,
    West,
}

impl Side {
    /// Return this side of `rect` as a line running toward increasing x or y.
    pub(crate) fn line_along(self, rect: Rect) -> Line {
        match self {
            Self::North => Line::new((rect.x0, rect.y0), (rect.x1, rect.y0)),
            Self::East => Line::new((rect.x1, rect.y0), (rect.x1, rect.y1)),
            Self::South => Line::new((rect.x0, rect.y1), (rect.x1, rect.y1)),
            Self::West => Line::new((rect.x0, rect.y0), (rect.x0, rect.y1)),
        }
    }

    /// Return the unit vector pointing from this side into its rectangle.
    pub(crate) fn inward_normal(self) -> Vec2 {
        match self {
            Self::North => Vec2::new(0.0, 1.0),
            Self::East => Vec2::new(-1.0, 0.0),
            Self::South => Vec2::new(0.0, -1.0),
            Self::West => Vec2::new(1.0, 0.0),
        }
    }

    /// Return the side across the rectangle from this one.
    pub(crate) fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

#[cfg(test)]
mod fixtures {
    use super::place::STUB_MARGIN;
    use super::*;

    impl LevelPlans {
        /// Plan the root level alone, as `root_plan` lays it out.
        pub(crate) fn root_only(root_plan: LevelPlan) -> Self {
            let root = ElementPath::root();
            let plans_by_level = BTreeMap::from([(root, root_plan)]);
            Self::new(plans_by_level)
        }

        /// Return these plans with every stub removed, everything else as laid out.
        pub(crate) fn without_stubs(self) -> Self {
            let stubless_plans_by_level = self
                .by_level
                .into_iter()
                .map(|(level, plan)| {
                    let LevelPlan {
                        placed,
                        routes,
                        stubs: _dropped_stubs,
                    } = plan;
                    let stubless_plan = LevelPlan::new(placed, routes, BTreeMap::new());
                    (level, stubless_plan)
                })
                .collect();
            Self::new(stubless_plans_by_level)
        }
    }

    impl LevelPlan {
        /// Plan the root level with boxes `a` and `b` and one route `a -> b`.
        pub(crate) fn two_boxes_and_one_route() -> Self {
            let a = ElementPath::from_segments(["a"]);
            let b = ElementPath::from_segments(["b"]);
            let edge = EdgeId::new(a.clone(), b.clone(), 0);
            let a_footprint = Rect::new(20.0, 30.0, 80.0, 70.0);
            let b_footprint = Rect::new(140.0, 50.0, 200.0, 90.0);
            let source = Port::new(Side::East, Point::new(80.0, 50.0));
            let target = Port::new(Side::West, Point::new(140.0, 70.0));
            let route = Route::new(source, target);
            let extent = Size::new(220.0, 110.0);
            let footprints = BTreeMap::from([(a, a_footprint), (b, b_footprint)]);
            let placed = PlacedLevel::new(extent, footprints);
            let routes = BTreeMap::from([(edge, route)]);
            Self::new(placed, routes, BTreeMap::new())
        }

        /// Return the band along `side` that bottom-up sizing keeps free of footprints for stub markers, as deep as the stub margin.
        pub(crate) fn stub_band(&self, side: Side) -> Rect {
            let Rect { x0, y0, x1, y1 } = self.placed.boundary();
            match side {
                Side::North => Rect::new(x0, y0, x1, y0 + STUB_MARGIN),
                Side::East => Rect::new(x1 - STUB_MARGIN, y0, x1, y1),
                Side::South => Rect::new(x0, y1 - STUB_MARGIN, x1, y1),
                Side::West => Rect::new(x0, y0, x0 + STUB_MARGIN, y1),
            }
        }
    }

    impl Port {
        /// Tell whether the port sits on its named side of `rect`.
        pub(crate) fn lies_on_side_of(&self, rect: Rect) -> bool {
            let Point { x, y } = self.point;
            let is_within_x_span = (rect.x0..=rect.x1).contains(&x);
            let is_within_y_span = (rect.y0..=rect.y1).contains(&y);
            match self.side {
                Side::North => y == rect.y0 && is_within_x_span,
                Side::East => x == rect.x1 && is_within_y_span,
                Side::South => y == rect.y1 && is_within_x_span,
                Side::West => x == rect.x0 && is_within_y_span,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::one_step::OneStepModelBuilder;
    use kurbo::Shape;
    use svugy_model::model::{Edge, Element};

    use std::collections::BTreeSet;

    use super::*;
    use crate::metrics::{BundledFont, InterFaces};
    use crate::svugy_example::SvugyExample;

    fn lay_out(graph: &WorkflowGraph) -> LevelPlans {
        let font = BundledFont::<InterFaces>::new();
        LevelPlans::lay_out(graph, &font)
    }

    fn path(segments: &[&str]) -> ElementPath {
        ElementPath::from_segments(segments.iter().copied())
    }

    fn plan_of<'a>(plans: &'a LevelPlans, level: &[&str]) -> &'a LevelPlan {
        let level_path = path(level);
        plans
            .level(&level_path)
            .expect("every container has a plan")
    }

    /// Build `a { b { c } }` beside `d` and `e`, with `c` calling `d` twice and `e` calling `c`.
    fn three_levels_graph() -> WorkflowGraph {
        let mut builder = OneStepModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("a"));
        let b = builder.add_child(&a, "b", Element::new("b"));
        let c = builder.add_child(&b, "c", Element::new("c"));
        let d = builder.add_top_level("d", Element::new("d"));
        let e = builder.add_top_level("e", Element::new("e"));
        builder.add_edge(&c, &d, Edge::new().with_label("calls"));
        builder.add_edge(&c, &d, Edge::new());
        builder.add_edge(&e, &c, Edge::new());
        builder.build_graph()
    }

    #[test]
    fn repeated_layouts_give_identical_plans() {
        let graph = SvugyExample::graph();

        let first_plans = lay_out(&graph);
        let repeated_plans: Vec<LevelPlans> = (0..5).map(|_repeat_index| lay_out(&graph)).collect();

        for repeated_plan in &repeated_plans {
            assert_eq!(*repeated_plan, first_plans);
        }
    }

    #[test]
    fn every_level_three_deep_is_planned_and_sized_as_its_footprint_in_its_parent() {
        let graph = three_levels_graph();

        let plans = lay_out(&graph);

        let planned_levels: Vec<&ElementPath> = plans.levels().keys().collect();
        let root = ElementPath::root();
        let a = path(&["a"]);
        let a_b = path(&["a", "b"]);
        assert_eq!(planned_levels, [&root, &a, &a_b]);
        for (level, plan) in plans.levels() {
            let Some(parent) = level.parent() else {
                continue;
            };
            let footprint_in_parent = plans.levels()[&parent].placed.footprints[level];
            assert_eq!(footprint_in_parent.size(), plan.placed.extent);
            for footprint in plan.placed.footprints.values() {
                assert!(plan.placed.boundary().contains_rect(*footprint));
            }
        }
    }

    #[test]
    fn a_lifted_edge_ranks_the_endpoints_ancestors_like_a_sibling_edge() {
        let mut builder = OneStepModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("a"));
        let x = builder.add_child(&a, "x", Element::new("x"));
        let b = builder.add_top_level("b", Element::new("b"));
        let y = builder.add_child(&b, "y", Element::new("y"));
        builder.add_edge(&x, &y, Edge::new());
        let graph = builder.build_graph();

        let plans = lay_out(&graph);

        let root_plan = plan_of(&plans, &[]);
        let a_footprint = root_plan.placed.footprints[&path(&["a"])];
        let b_footprint = root_plan.placed.footprints[&path(&["b"])];
        assert!(b_footprint.x0 > a_footprint.x1);
    }

    #[test]
    fn a_cross_container_route_joins_its_true_endpoints_in_its_home_level() {
        let graph = three_levels_graph();
        let c = path(&["a", "b", "c"]);
        let d = path(&["d"]);
        let root = ElementPath::root();

        let plans = lay_out(&graph);

        let root_plan = plan_of(&plans, &[]);
        let labelled_edge = EdgeId::new(c.clone(), d.clone(), 0);
        let route = &root_plan.routes[&labelled_edge];
        let c_footprint = plans.footprint_in_level(&c, &root);
        let d_footprint = root_plan.placed.footprints[&d];
        assert!(route.source.lies_on_side_of(c_footprint));
        assert!(route.target.lies_on_side_of(d_footprint));
        assert!(root_plan.stubs.is_empty());
    }

    #[test]
    fn edges_toward_one_external_endpoint_merge_into_one_stub_in_every_container_they_leave() {
        let graph = three_levels_graph();
        let c = path(&["a", "b", "c"]);
        let d = path(&["d"]);
        let e = path(&["e"]);
        let to_d_edges = BTreeSet::from([
            EdgeId::new(c.clone(), d.clone(), 0),
            EdgeId::new(c.clone(), d.clone(), 1),
        ]);
        let from_e_edges = BTreeSet::from([EdgeId::new(e.clone(), c.clone(), 0)]);

        let plans = lay_out(&graph);

        for container in [&["a"][..], &["a", "b"][..]] {
            let plan = plan_of(&plans, container);
            let stubbed_endpoints: Vec<&ElementPath> = plan.stubs.keys().collect();
            assert_eq!(stubbed_endpoints, [&d, &e]);
            assert_eq!(plan.stubs[&d].edges, to_d_edges);
            assert_eq!(plan.stubs[&e].edges, from_e_edges);
            for stub in plan.stubs.values() {
                assert!(stub.slot.lies_on_side_of(plan.placed.boundary()));
            }
        }
    }

    #[test]
    fn a_level_routes_only_the_edges_homed_in_it_never_its_stubbed_edges() {
        let graph = three_levels_graph();

        let plans = lay_out(&graph);

        for (level, plan) in plans.levels() {
            for edge_id in plan.routes.keys() {
                assert_eq!(edge_id.home_level(), *level, "{edge_id} in {level}");
            }
            for edge_id in plan.stubs.values().flat_map(|stub| &stub.edges) {
                let is_stubbed_edge_routed = plan.routes.contains_key(edge_id);
                assert!(!is_stubbed_edge_routed, "{edge_id} in {level}");
            }
        }
    }

    #[test]
    fn each_stub_slot_sits_on_the_side_its_home_route_leaves_through_at_its_crossing() {
        let mut builder = OneStepModelBuilder::new();
        let west = builder.add_top_level("west", Element::new("west"));
        let hub = builder.add_top_level("hub", Element::new("hub"));
        let inner = builder.add_child(&hub, "inner", Element::new("inner"));
        let east = builder.add_top_level("east", Element::new("east"));
        builder.add_edge(&west, &inner, Edge::new());
        builder.add_edge(&inner, &east, Edge::new());
        let graph = builder.build_graph();

        let plans = lay_out(&graph);

        let hub_plan = plan_of(&plans, &["hub"]);
        let west_slot = hub_plan.stubs[west.path()].slot;
        let east_slot = hub_plan.stubs[east.path()].slot;
        assert_eq!(west_slot.side, Side::West);
        assert_eq!(east_slot.side, Side::East);
        assert_every_single_edge_slot_sits_on_its_home_route_crossing(&graph, &plans);
    }

    #[test]
    fn every_stub_slot_of_the_example_sits_where_most_of_its_home_routes_leave_the_container() {
        let graph = SvugyExample::graph();

        let plans = lay_out(&graph);

        assert_every_single_edge_slot_sits_on_its_home_route_crossing(&graph, &plans);
        for (level, plan) in plans.levels() {
            for (external_endpoint, stub) in &plan.stubs {
                let is_badge_stub = stub
                    .edges
                    .iter()
                    .any(|edge_id| graph.is_badge_edge(edge_id));
                if is_badge_stub {
                    continue;
                }
                let crossing_sides: Vec<Side> = stub
                    .edges
                    .iter()
                    .map(|edge_id| {
                        let (crossing_side, _crossing_point) =
                            home_route_crossing(&plans, level, edge_id);
                        crossing_side
                    })
                    .collect();
                let count_of_side = |side: Side| {
                    crossing_sides
                        .iter()
                        .filter(|&&other| other == side)
                        .count()
                };
                let slot_side_count = count_of_side(stub.slot.side);
                let busiest_side_count = crossing_sides
                    .iter()
                    .map(|&side| count_of_side(side))
                    .max()
                    .unwrap_or_default();
                assert_eq!(
                    slot_side_count, busiest_side_count,
                    "{external_endpoint} in {level}"
                );
                assert!(stub.slot.lies_on_side_of(plan.placed.boundary()));
            }
        }
    }

    /// Return the side and point where `edge_id`'s home route, mapped into `container`, crosses its boundary, by intersecting lines.
    fn home_route_crossing(
        plans: &LevelPlans,
        container: &ElementPath,
        edge_id: &EdgeId,
    ) -> (Side, Point) {
        let home = edge_id.home_level();
        let home_route = &plans.levels()[&home].routes[edge_id];
        let container_footprint = plans.footprint_in_level(container, &home);
        let home_to_container = container_footprint.origin().to_vec2();
        let source_point = home_route.source.point - home_to_container;
        let target_point = home_route.target.point - home_to_container;
        let route_line = Line::new(source_point, target_point);
        let route_bounds = route_line.bounding_box().inflate(1e-6, 1e-6);
        let boundary = plans.levels()[container].placed.boundary();
        let closed_boundary = boundary.inflate(1e-6, 1e-6);
        let mut crossings: Vec<(Side, Point)> = [Side::North, Side::East, Side::South, Side::West]
            .into_iter()
            .filter_map(|side| {
                let side_line = side.line_along(boundary);
                let crossing = route_line.crossing_point(side_line)?;
                let is_on_side = closed_boundary.contains(crossing);
                let is_on_route = route_bounds.contains(crossing);
                let is_crossed = is_on_side && is_on_route;
                is_crossed.then_some((side, crossing))
            })
            .collect();
        crossings.dedup_by(|(_side, point), (_other_side, other_point)| {
            let gap = *point - *other_point;
            gap.hypot2().sqrt() < 1e-6
        });
        let [crossing] = crossings[..] else {
            panic!("{edge_id} crosses {container} once: {crossings:?}");
        };
        crossing
    }

    /// Assert every routed single-edge stub sits where its home route crosses the boundary; a badge stub has no route to cross.
    fn assert_every_single_edge_slot_sits_on_its_home_route_crossing(
        graph: &WorkflowGraph,
        plans: &LevelPlans,
    ) {
        for (level, plan) in plans.levels() {
            let single_edge_stubs = plan.stubs.values().filter(|stub| stub.edges.len() == 1);
            for stub in single_edge_stubs {
                let edge_id = stub.edges.first().expect("a stub carries an edge");
                let is_badge_stub = graph.is_badge_edge(edge_id);
                if is_badge_stub {
                    continue;
                }
                let (crossing_side, crossing_point) = home_route_crossing(plans, level, edge_id);
                let slot_offset = stub.slot.point - crossing_point;
                let slot_distance = slot_offset.hypot2().sqrt();
                assert_eq!(stub.slot.side, crossing_side, "{edge_id} in {level}");
                assert!(
                    slot_distance < 1e-6,
                    "{edge_id} in {level}: {slot_distance}"
                );
            }
        }
    }

    #[test]
    fn no_footprint_of_a_stubbed_level_of_the_example_enters_its_stub_bands() {
        let graph = SvugyExample::graph();

        let plans = lay_out(&graph);

        let stubbed_plans = plans
            .levels()
            .values()
            .filter(|plan| !plan.stubs.is_empty());
        let mut stubbed_plan_count = 0;
        for plan in stubbed_plans {
            stubbed_plan_count += 1;
            for side in [Side::North, Side::East, Side::South, Side::West] {
                let band = plan.stub_band(side);
                for (path, footprint) in &plan.placed.footprints {
                    let overlap = band.intersect(*footprint);
                    let is_footprint_in_band = overlap.area() > 0.0;
                    assert!(!is_footprint_in_band, "{path} enters the {side:?} band");
                }
            }
        }
        assert!(stubbed_plan_count > 0);
    }

    #[test]
    fn every_route_of_the_example_attaches_to_its_true_endpoints() {
        let graph = SvugyExample::graph();

        let plans = lay_out(&graph);

        for (level, plan) in plans.levels() {
            for (edge_id, route) in &plan.routes {
                let source_footprint = plans.footprint_in_level(edge_id.from(), level);
                let target_footprint = plans.footprint_in_level(edge_id.to(), level);
                assert!(route.source.lies_on_side_of(source_footprint), "{edge_id}");
                assert!(route.target.lies_on_side_of(target_footprint), "{edge_id}");
            }
        }
    }

    #[test]
    fn an_empty_graph_lays_out_an_empty_root_level() {
        let graph = WorkflowGraph::default();

        let plans = lay_out(&graph);

        let root_plan = plan_of(&plans, &[]);
        assert_eq!(plans.levels().len(), 1);
        assert!(root_plan.placed.footprints.is_empty());
        assert!(root_plan.routes.is_empty());
    }

    #[test]
    fn fixture_route_starts_and_ends_on_its_boxes_ports() {
        let plan = LevelPlan::two_boxes_and_one_route();
        for (edge, route) in &plan.routes {
            let source_footprint = plan.placed.footprints[edge.from()];
            let target_footprint = plan.placed.footprints[edge.to()];
            assert!(route.source.lies_on_side_of(source_footprint));
            assert!(route.target.lies_on_side_of(target_footprint));
        }
    }
}
