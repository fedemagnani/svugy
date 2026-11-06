//! Lay the hierarchy out recursively: place levels bottom-up, anchor their routes, then side stubs where those routes leave containers.

mod edge_levels;

use std::collections::BTreeMap;
use std::iter;

use kurbo::{Line, Rect};
use svugy_model::model::{EdgeId, Model};
use svugy_model::path::ElementPath;

use super::place::SiblingGraph;
use super::route::{EdgeEnd, RouteEnds};
use super::stub::BoundaryCrossings;
use super::{LevelPlan, LevelPlans, Port, Route};
use crate::metrics::TextMeasure;
use edge_levels::{EdgeLevels, LeavingEdge};

impl LevelPlans {
    /// Lay out every level: place them bottom-up, anchor every level's routes, then side every container's stubs where those routes leave it.
    pub(crate) fn lay_out(model: &Model, measure: &impl TextMeasure) -> Self {
        let edge_levels = EdgeLevels::of(model);
        let mut plans = Self::place_bottom_up(model, &edge_levels, measure);
        plans.anchor_homed_routes(&edge_levels);
        plans.side_stubs_at_home_route_crossings(&edge_levels);
        plans
    }

    /// Place every level after the levels nested in it, so each container's extent is known before its parent places it.
    fn place_bottom_up(
        model: &Model,
        edge_levels: &EdgeLevels,
        measure: &impl TextMeasure,
    ) -> Self {
        let root = ElementPath::root();
        let containers = model
            .elements()
            .keys()
            .filter(|path| model.is_container(path));
        let levels_innermost_first = containers.rev().chain(iter::once(&root));
        let mut plans = Self::new(BTreeMap::new());
        for level in levels_innermost_first {
            let joined_pairs = edge_levels.lifted_pairs_in(level);
            let siblings =
                SiblingGraph::measure_children(model, level, joined_pairs, &plans, measure);
            let is_left_by_edges = edge_levels.is_left_by_edges(level);
            let plan = LevelPlan::place_children(model, level, siblings, is_left_by_edges, measure);
            plans.plans_by_level.insert(level.clone(), plan);
        }
        plans
    }

    /// Anchor the routes of the edges homed in every level between their true endpoints, once every footprint is fixed.
    fn anchor_homed_routes(&mut self, edge_levels: &EdgeLevels) {
        let levels: Vec<ElementPath> = self.plans_by_level.keys().cloned().collect();
        for level in levels {
            let routes = self.homed_routes_of(&level, edge_levels);
            let plan = self.plan_mut(&level);
            plan.routes = routes;
        }
    }

    /// Route the edges homed in `level` between their true endpoints, each end facing the other.
    fn homed_routes_of(
        &self,
        level: &ElementPath,
        edge_levels: &EdgeLevels,
    ) -> BTreeMap<EdgeId, Route> {
        let mut route_ends = RouteEnds::new();
        for edge in edge_levels.homed_in(level) {
            let source_footprint = self.footprint_in_level(edge.from(), level);
            let target_footprint = self.footprint_in_level(edge.to(), level);
            route_ends.face_each_other(edge, source_footprint, target_footprint);
        }
        route_ends.into_routes()
    }

    /// Side the stubs of every container left by edges where the home routes of those edges leave its boundary.
    fn side_stubs_at_home_route_crossings(&mut self, edge_levels: &EdgeLevels) {
        for container in edge_levels.containers_left_by_edges() {
            let leaving_edges = edge_levels.leaving(container);
            let crossings_by_external_endpoint =
                self.boundary_crossings_of(container, leaving_edges);
            let plan = self.plan_mut(container);
            plan.side_stubs(crossings_by_external_endpoint);
        }
    }

    /// Cross the home line of every edge leaving `container` with its boundary, gathering the crossings per external endpoint.
    fn boundary_crossings_of(
        &self,
        container: &ElementPath,
        leaving_edges: &[LeavingEdge],
    ) -> BTreeMap<ElementPath, BoundaryCrossings> {
        let boundary = self.plans_by_level[container].boundary();
        let mut crossings_by_external_endpoint: BTreeMap<ElementPath, BoundaryCrossings> =
            BTreeMap::new();
        for leaving_edge in leaving_edges {
            let edge = leaving_edge.edge();
            let home = edge.home_level();
            let container_footprint = self.footprint_in_level(container, &home);
            let home_to_container = container_footprint.origin().to_vec2();
            let inner_end = leaving_edge.inner_end();
            let home_line = self.home_line_of(edge, inner_end);
            let inner_point = home_line.p0 - home_to_container;
            let outer_point = home_line.p1 - home_to_container;
            let crossing = Port::leaving(boundary, inner_point, outer_point);
            let external_endpoint = leaving_edge.external_endpoint();
            let crossings = crossings_by_external_endpoint
                .entry(external_endpoint.clone())
                .or_default();
            crossings.add(edge.clone(), crossing);
        }
        crossings_by_external_endpoint
    }

    /// Return the straight line `edge` runs along in its home level, from its `inner_end` out: between its route's ports, or for a badge, which has no route, between its endpoints' footprint centres.
    fn home_line_of(&self, edge: &EdgeId, inner_end: EdgeEnd) -> Line {
        let home = edge.home_level();
        let outer_end = inner_end.opposite();
        let home_route = self.plans_by_level[&home].routes.get(edge);
        let Some(home_route) = home_route else {
            let inner_endpoint = inner_end.endpoint_of(edge);
            let outer_endpoint = outer_end.endpoint_of(edge);
            let inner_footprint = self.footprint_in_level(inner_endpoint, &home);
            let outer_footprint = self.footprint_in_level(outer_endpoint, &home);
            let inner_center = inner_footprint.center();
            let outer_center = outer_footprint.center();
            return Line::new(inner_center, outer_center);
        };
        let inner_port = inner_end.port_of(home_route);
        let outer_port = outer_end.port_of(home_route);
        Line::new(inner_port.point, outer_port.point)
    }

    /// Return `element`'s footprint in the plan of `level`, any of its ancestor levels, offsetting through the containers between.
    fn footprint_in_level(&self, element: &ElementPath, level: &ElementPath) -> Rect {
        let parent = element.parent().unwrap_or_else(ElementPath::root);
        let footprint_in_parent = self.plans_by_level[&parent].footprints[element];
        let is_parent_the_level = parent == *level;
        if is_parent_the_level {
            return footprint_in_parent;
        }
        let parent_footprint = self.footprint_in_level(&parent, level);
        let parent_offset = parent_footprint.origin().to_vec2();
        footprint_in_parent + parent_offset
    }

    fn plan_mut(&mut self, level: &ElementPath) -> &mut LevelPlan {
        self.plans_by_level
            .get_mut(level)
            .expect("every level is placed before it is routed or sided")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kurbo::{Line, Point, Shape};
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element};

    use super::*;
    use crate::layout::Side;
    use crate::metrics::BundledFont;

    use crate::svugy_example::SvugyExample;

    fn lay_out(model: &Model) -> LevelPlans {
        let font = BundledFont::new();
        LevelPlans::lay_out(model, &font)
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
    fn three_levels_model() -> Model {
        let mut builder = ModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("a"));
        let b = builder.add_child(&a, "b", Element::new("b"));
        let c = builder.add_child(&b, "c", Element::new("c"));
        let d = builder.add_top_level("d", Element::new("d"));
        let e = builder.add_top_level("e", Element::new("e"));
        builder.add_edge(&c, &d, Edge::new().with_label("calls"));
        builder.add_edge(&c, &d, Edge::new());
        builder.add_edge(&e, &c, Edge::new());
        builder.build().model.expect("fixture model is valid")
    }

    #[test]
    fn repeated_layouts_give_identical_plans() {
        let model = SvugyExample::model();

        let first_plans = lay_out(&model);
        let repeated_plans: Vec<LevelPlans> = (0..5).map(|_repeat_index| lay_out(&model)).collect();

        for repeated_plan in &repeated_plans {
            assert_eq!(*repeated_plan, first_plans);
        }
    }

    #[test]
    fn every_level_three_deep_is_planned_and_sized_as_its_footprint_in_its_parent() {
        let model = three_levels_model();

        let plans = lay_out(&model);

        let planned_levels: Vec<&ElementPath> = plans.levels().keys().collect();
        let root = ElementPath::root();
        let a = path(&["a"]);
        let a_b = path(&["a", "b"]);
        assert_eq!(planned_levels, [&root, &a, &a_b]);
        for (level, plan) in plans.levels() {
            let Some(parent) = level.parent() else {
                continue;
            };
            let footprint_in_parent = plans.levels()[&parent].footprints[level];
            assert_eq!(footprint_in_parent.size(), plan.extent);
            for footprint in plan.footprints.values() {
                assert!(plan.boundary().contains_rect(*footprint));
            }
        }
    }

    #[test]
    fn a_lifted_edge_ranks_the_endpoints_ancestors_like_a_sibling_edge() {
        let mut builder = ModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("a"));
        let x = builder.add_child(&a, "x", Element::new("x"));
        let b = builder.add_top_level("b", Element::new("b"));
        let y = builder.add_child(&b, "y", Element::new("y"));
        builder.add_edge(&x, &y, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");

        let plans = lay_out(&model);

        let root_plan = plan_of(&plans, &[]);
        let a_footprint = root_plan.footprints[&path(&["a"])];
        let b_footprint = root_plan.footprints[&path(&["b"])];
        assert!(b_footprint.x0 > a_footprint.x1);
    }

    #[test]
    fn a_cross_container_route_joins_its_true_endpoints_in_its_home_level() {
        let model = three_levels_model();
        let c = path(&["a", "b", "c"]);
        let d = path(&["d"]);
        let root = ElementPath::root();

        let plans = lay_out(&model);

        let root_plan = plan_of(&plans, &[]);
        let labelled_edge = EdgeId::new(c.clone(), d.clone(), 0);
        let route = &root_plan.routes[&labelled_edge];
        let c_footprint = plans.footprint_in_level(&c, &root);
        let d_footprint = root_plan.footprints[&d];
        assert!(route.source.lies_on_side_of(c_footprint));
        assert!(route.target.lies_on_side_of(d_footprint));
        assert!(root_plan.stubs.is_empty());
    }

    #[test]
    fn edges_toward_one_external_endpoint_merge_into_one_stub_in_every_container_they_leave() {
        let model = three_levels_model();
        let c = path(&["a", "b", "c"]);
        let d = path(&["d"]);
        let e = path(&["e"]);
        let to_d_edges = BTreeSet::from([
            EdgeId::new(c.clone(), d.clone(), 0),
            EdgeId::new(c.clone(), d.clone(), 1),
        ]);
        let from_e_edges = BTreeSet::from([EdgeId::new(e.clone(), c.clone(), 0)]);

        let plans = lay_out(&model);

        for container in [&["a"][..], &["a", "b"][..]] {
            let plan = plan_of(&plans, container);
            let stubbed_endpoints: Vec<&ElementPath> = plan.stubs.keys().collect();
            assert_eq!(stubbed_endpoints, [&d, &e]);
            assert_eq!(plan.stubs[&d].edges, to_d_edges);
            assert_eq!(plan.stubs[&e].edges, from_e_edges);
            for stub in plan.stubs.values() {
                assert!(stub.slot.lies_on_side_of(plan.boundary()));
            }
        }
    }

    #[test]
    fn a_level_routes_only_the_edges_homed_in_it_never_its_stubbed_edges() {
        let model = three_levels_model();

        let plans = lay_out(&model);

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
        let mut builder = ModelBuilder::new();
        let west = builder.add_top_level("west", Element::new("west"));
        let hub = builder.add_top_level("hub", Element::new("hub"));
        let inner = builder.add_child(&hub, "inner", Element::new("inner"));
        let east = builder.add_top_level("east", Element::new("east"));
        builder.add_edge(&west, &inner, Edge::new());
        builder.add_edge(&inner, &east, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");

        let plans = lay_out(&model);

        let hub_plan = plan_of(&plans, &["hub"]);
        let west_slot = hub_plan.stubs[west.path()].slot;
        let east_slot = hub_plan.stubs[east.path()].slot;
        assert_eq!(west_slot.side, Side::West);
        assert_eq!(east_slot.side, Side::East);
        assert_every_single_edge_slot_sits_on_its_home_route_crossing(&model, &plans);
    }

    #[test]
    fn every_stub_slot_of_the_example_sits_where_most_of_its_home_routes_leave_the_container() {
        let model = SvugyExample::model();

        let plans = lay_out(&model);

        assert_every_single_edge_slot_sits_on_its_home_route_crossing(&model, &plans);
        for (level, plan) in plans.levels() {
            for (external_endpoint, stub) in &plan.stubs {
                let is_badge_stub = stub
                    .edges
                    .iter()
                    .any(|edge_id| model.is_badge_edge(edge_id));
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
                assert!(stub.slot.lies_on_side_of(plan.boundary()));
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
        let boundary = plans.levels()[container].boundary();
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
        model: &Model,
        plans: &LevelPlans,
    ) {
        for (level, plan) in plans.levels() {
            let single_edge_stubs = plan.stubs.values().filter(|stub| stub.edges.len() == 1);
            for stub in single_edge_stubs {
                let edge_id = stub.edges.first().expect("a stub carries an edge");
                let is_badge_stub = model.is_badge_edge(edge_id);
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
        let model = SvugyExample::model();

        let plans = lay_out(&model);

        let stubbed_plans = plans
            .levels()
            .values()
            .filter(|plan| !plan.stubs.is_empty());
        let mut stubbed_plan_count = 0;
        for plan in stubbed_plans {
            stubbed_plan_count += 1;
            for side in [Side::North, Side::East, Side::South, Side::West] {
                let band = plan.stub_band(side);
                for (path, footprint) in &plan.footprints {
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
        let model = SvugyExample::model();

        let plans = lay_out(&model);

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
    fn an_empty_model_lays_out_an_empty_root_level() {
        let model = ModelBuilder::new()
            .build()
            .model
            .expect("an empty model is valid");

        let plans = lay_out(&model);

        let root_plan = plan_of(&plans, &[]);
        assert_eq!(plans.levels().len(), 1);
        assert!(root_plan.footprints.is_empty());
        assert!(root_plan.routes.is_empty());
    }
}
