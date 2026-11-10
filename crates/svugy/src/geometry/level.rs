//! Project every laid-out level into nested scene groups.

use std::collections::{BTreeMap, BTreeSet};
use std::iter;
use std::marker::PhantomData;

use glam::DVec3;
use kurbo::{Affine, Point, Rect, Vec2};
use svugy_model::model::{Edge, EdgeId, Model};
use svugy_model::path::ElementPath;

use super::badge::{BadgeIcon, BadgeSeater};
use super::camera::Camera;
use super::connection::{ConnectionShape, FlatTrace, LiftedArc};
use super::depth::DepthOrder;
use super::obstruction::Obstructions;
use super::primitive::ResolvedPrimitive;
use super::{Connection, Group, PlatedLabel, Solid};
use crate::layout::{Branch, Course, LevelPlacement, LevelPlan, LevelPlans, Route};
use crate::metrics::TextMeasure;

/// Projects levels top-down as camera `C` sees them, each container's interior onto its solid's top, a sheet under each closed container.
pub(super) struct LevelProjector<'a, M, C> {
    model: &'a Model,
    closed_containers: &'a BTreeSet<ElementPath>,
    plans: &'a LevelPlans,
    measure: &'a M,
    camera: PhantomData<C>,
}

/// Hands one bundled edge's branch to the projector of the container it runs on, traced on that container's floor in its frame.
pub(super) struct FloorBranch {
    edge_id: EdgeId,
    trace: FlatTrace,
    ends_in_arrowhead: bool,
    carries_label: bool,
}

impl<'a, M: TextMeasure, C: Camera> LevelProjector<'a, M, C> {
    /// Set up a projector of the levels `plans` lays out for `model`, closing `closed_containers`, its labels measured by `measure`.
    pub(super) fn new(
        model: &'a Model,
        closed_containers: &'a BTreeSet<ElementPath>,
        plans: &'a LevelPlans,
        measure: &'a M,
    ) -> Self {
        Self {
            model,
            closed_containers,
            plans,
            measure,
            camera: PhantomData,
        }
    }

    /// Project `level` as `plan` lays it out, mapped into its parent by `frame`: solids back to front with their interiors, then traces of the edges homed here and of the `floor_branches` handed to it, then arcs of the edges homed here, trace plates placed first, every plate clear of the solids' labels and of the plates before it.
    pub(super) fn project_level(
        &self,
        level: &ElementPath,
        plan: &LevelPlan,
        frame: Affine,
        floor_branches: Vec<FloorBranch>,
    ) -> Group {
        let solids = self.project_solids_back_to_front(plan);
        let mut obstructions = Obstructions::around_text_of(&solids);
        let mut traces = self.trace_connections_homed_at(plan, &mut obstructions);
        let branch_traces = self.trace_floor_branches(floor_branches, &mut obstructions);
        traces.extend(branch_traces);
        let connections = self.arc_connections_homed_at(level, plan, &mut obstructions);
        let empty_group = Group::new(frame);
        Group {
            traces,
            solids,
            connections,
            ..empty_group
        }
    }

    /// Project each footprint's element back to front, stacking a sheet under it when closed, seating the badges of the edges it is the source of on its top, stacking the interior of every container the plans lay out there.
    fn project_solids_back_to_front(&self, plan: &LevelPlan) -> Vec<Solid> {
        let placed = plan.placed();
        let footprints = placed.footprints();
        let routes = plan.routes();
        let back_to_front = DepthOrder::<C>::back_to_front(footprints);
        back_to_front
            .into_iter()
            .filter_map(|path| {
                let element = self.model.element(path)?;
                let primitive = ResolvedPrimitive::<C>::of_path(self.model, path)?;
                let footprint = footprints[path];
                let drawn_solid = primitive
                    .draw(path, element, footprint, self.measure)
                    .with_plan_height(primitive.height);
                let sheeted_solid =
                    self.stack_sheet_if_closed(path, footprint, &primitive, drawn_solid);
                let badge_icons = self.badge_icons_from(path);
                let badge_seater =
                    BadgeSeater::on_top_of(&sheeted_solid, footprint, primitive.height);
                let badges = badge_seater.seat_badges(badge_icons);
                let badged_solid = sheeted_solid.with_badges(badges);
                let solid = self.stack_planned_interior(
                    path,
                    footprint,
                    primitive.height,
                    routes,
                    badged_solid,
                );
                Some(solid)
            })
            .collect()
    }

    /// Stack a sheet outlined from `primitive` over `footprint` under the solid of `path`, if `path` is a closed container.
    fn stack_sheet_if_closed(
        &self,
        path: &ElementPath,
        footprint: Rect,
        primitive: &ResolvedPrimitive<C>,
        drawn_solid: Solid,
    ) -> Solid {
        let hides_no_children = !self.closed_containers.contains(path);
        if hides_no_children {
            return drawn_solid;
        }
        let sheet = primitive.sheet_under(footprint);
        drawn_solid.with_sheet(sheet)
    }

    /// Shape the icon of every edge drawn as a badge on `source`, each its target shrunk, in edge order.
    fn badge_icons_from(&self, source: &ElementPath) -> Vec<(EdgeId, BadgeIcon<C>)> {
        let badge_edges = self.model.badge_edges_from(source);
        badge_edges
            .filter_map(|edge_id| {
                let target = edge_id.to();
                let icon = BadgeIcon::of_target_in(self.model, self.plans, target)?;
                let badge_edge_id = edge_id.clone();
                Some((badge_edge_id, icon))
            })
            .collect()
    }

    /// Stack the planned interior of `path`, if any, on its solid, handing it the branches of `enclosing_level_routes` that run on its floor: the interior's origin is the footprint's top-left corner raised to the primitive's top, `primitive_height` up.
    fn stack_planned_interior(
        &self,
        path: &ElementPath,
        footprint: Rect,
        primitive_height: f64,
        enclosing_level_routes: &BTreeMap<EdgeId, Route>,
        drawn_solid: Solid,
    ) -> Solid {
        let Some(interior_plan) = self.plans.level(path) else {
            return drawn_solid;
        };
        let floor_branches =
            FloorBranch::every_one_on_floor_of::<C>(path, footprint, enclosing_level_routes);
        let raised_origin = DVec3::new(footprint.x0, footprint.y0, primitive_height);
        let projected_origin = C::project(raised_origin);
        let interior_offset = projected_origin.to_vec2();
        let interior_frame = Affine::translate(interior_offset);
        let interior = self.project_level(path, interior_plan, interior_frame, floor_branches);
        drawn_solid.with_interior(interior)
    }

    /// Trace the flat route of every edge homed at this level on its floor, ports at floor height, plating each label where its trace is least obstructed and recording it among `obstructions`, in route order.
    fn trace_connections_homed_at(
        &self,
        plan: &LevelPlan,
        obstructions: &mut Obstructions,
    ) -> Vec<Connection> {
        let level_floor_height = 0.0;
        let routes = plan.routes();
        routes
            .iter()
            .filter_map(|(edge_id, route)| {
                let Course::Flat { bends } = &route.course else {
                    return None;
                };
                let source_port = iter::once(route.source.point);
                let bend_points = bends.iter().copied();
                let target_port = iter::once(route.target.point);
                let plan_corners = source_port.chain(bend_points).chain(target_port);
                let trace = FlatTrace::on_floor_through::<C>(level_floor_height, plan_corners);
                let connection = self.connection_along(edge_id, &trace, obstructions);
                Some(connection)
            })
            .collect()
    }

    /// Trace every branch handed to this level on its floor, headed only where it reaches its edge's target, plating each label it carries where its trace is least obstructed and recording it among `obstructions`.
    fn trace_floor_branches(
        &self,
        floor_branches: Vec<FloorBranch>,
        obstructions: &mut Obstructions,
    ) -> Vec<Connection> {
        floor_branches
            .into_iter()
            .map(|floor_branch| {
                let FloorBranch {
                    edge_id,
                    trace,
                    ends_in_arrowhead,
                    carries_label,
                } = floor_branch;
                let stroke = Self::stroke_along(&edge_id, &trace, ends_in_arrowhead);
                let is_label_elsewhere = !carries_label;
                if is_label_elsewhere {
                    return stroke;
                }
                self.with_label_plated_along(stroke, &trace, obstructions)
            })
            .collect()
    }

    /// Arc the lifted route of every edge homed at `level`, a bundled one as its trunk, plating each arc's label where it is least obstructed and recording it among `obstructions`, in route order.
    fn arc_connections_homed_at(
        &self,
        level: &ElementPath,
        plan: &LevelPlan,
        obstructions: &mut Obstructions,
    ) -> Vec<Connection> {
        let routes = plan.routes();
        routes
            .iter()
            .filter_map(|(edge_id, route)| {
                let (source_branch, target_branch) = match &route.course {
                    Course::Lifted => (None, None),
                    Course::Bundled {
                        source_branch,
                        target_branch,
                    } => (source_branch.as_ref(), target_branch.as_ref()),
                    Course::Flat { bends: _bends } => return None,
                };
                let connection = self.arc_connection_homed_at(
                    level,
                    edge_id,
                    route,
                    source_branch,
                    target_branch,
                    obstructions,
                );
                Some(connection)
            })
            .collect()
    }

    /// Arc `edge_id`'s `route`, homed at `level`, between its trunk ends. Unbranched, its dot runs port to port and its label is plated where the arc is least obstructed and recorded among `obstructions`; branched, its dot runs along `source_branch`, the trunk and `target_branch`, and the arrowhead and the label are the branches'.
    fn arc_connection_homed_at(
        &self,
        level: &ElementPath,
        edge_id: &EdgeId,
        route: &Route,
        source_branch: Option<&Branch>,
        target_branch: Option<&Branch>,
        obstructions: &mut Obstructions,
    ) -> Connection {
        let source = edge_id.from();
        let target = edge_id.to();
        let source_port = route.source.point;
        let target_port = route.target.point;
        let source_end = self.trunk_end_above(level, source, source_port, source_branch);
        let target_end = self.trunk_end_above(level, target, target_port, target_branch);
        let trunk = LiftedArc::between::<C>(source_end, target_end);
        let is_unbranched = matches!((source_branch, target_branch), (None, None));
        if is_unbranched {
            return self.connection_along(edge_id, &trunk, obstructions);
        }
        let source_branch_trace = source_branch.map(|branch| {
            let plan_corners = branch.corners_from_port_to_slot(source_port);
            self.branch_trace_above(level, branch, plan_corners)
        });
        let target_branch_trace = target_branch.map(|branch| {
            let plan_corners = branch.corners_from_slot_to_port(target_port);
            self.branch_trace_above(level, branch, plan_corners)
        });
        let dot_path =
            trunk.port_to_port_path_via(source_branch_trace.as_ref(), target_branch_trace.as_ref());
        let is_trunk_reaching_target = target_branch.is_none();
        let trunk_stroke = Self::stroke_along(edge_id, &trunk, is_trunk_reaching_target);
        trunk_stroke.with_dot_path(dot_path)
    }

    /// Return where a trunk lands, above `level`'s floor, at the end on `endpoint`: the slot of `branch`, if any, at its container's floor height, else `port` raised by the solids under it.
    fn trunk_end_above(
        &self,
        level: &ElementPath,
        endpoint: &ElementPath,
        port: Point,
        branch: Option<&Branch>,
    ) -> DVec3 {
        let Some(branch) = branch else {
            let endpoint_elevation = self.elevation_above(level, endpoint);
            return DVec3::new(port.x, port.y, endpoint_elevation);
        };
        let slot = branch.slot.point;
        let container_floor_height = self.elevation_above(level, &branch.container);
        DVec3::new(slot.x, slot.y, container_floor_height)
    }

    /// Trace `branch` through `plan_corners` on its container's floor, as high above `level`'s floor as the container's top.
    fn branch_trace_above(
        &self,
        level: &ElementPath,
        branch: &Branch,
        plan_corners: Vec<Point>,
    ) -> FlatTrace {
        let container_floor_height = self.elevation_above(level, &branch.container);
        FlatTrace::on_floor_through::<C>(container_floor_height, plan_corners)
    }

    /// Connect `edge_id` along `shape`, stroked under its target arrowhead, its dot travelling port to port, its label, if any, plated where `shape` is least obstructed and recorded among `obstructions`.
    fn connection_along(
        &self,
        edge_id: &EdgeId,
        shape: &impl ConnectionShape,
        obstructions: &mut Obstructions,
    ) -> Connection {
        let stroked_path = shape.path_under_target_arrowhead();
        let dot_path = shape.port_to_port_path();
        let plan_course = shape.plan_course();
        let connection_edge_id = edge_id.clone();
        let connection = Connection::new(connection_edge_id, stroked_path)
            .with_dot_path(dot_path)
            .with_plan_course(plan_course);
        self.with_label_plated_along(connection, shape, obstructions)
    }

    /// Stroke `edge_id` along `shape`: under the arrowhead when it reaches the edge's target, else whole and headless.
    fn stroke_along(
        edge_id: &EdgeId,
        shape: &impl ConnectionShape,
        is_reaching_target: bool,
    ) -> Connection {
        let connection_edge_id = edge_id.clone();
        let plan_course = shape.plan_course();
        if is_reaching_target {
            let stroked_path = shape.path_under_target_arrowhead();
            return Connection::new(connection_edge_id, stroked_path).with_plan_course(plan_course);
        }
        let whole_path = shape.port_to_port_path();
        Connection::new(connection_edge_id, whole_path)
            .with_plan_course(plan_course)
            .without_arrowhead()
    }

    /// Plate the label of `connection`'s edge, if any, where `shape` is least obstructed, recording it among `obstructions`.
    fn with_label_plated_along(
        &self,
        connection: Connection,
        shape: &impl ConnectionShape,
        obstructions: &mut Obstructions,
    ) -> Connection {
        let edge = self.model.edge(&connection.edge);
        let edge_label = edge.and_then(Edge::label);
        let Some(label_text) = edge_label else {
            return connection;
        };
        let plated_label = PlatedLabel::place_least_obstructed_along::<C>(
            label_text,
            shape,
            obstructions,
            self.measure,
        );
        connection.with_label(plated_label)
    }

    /// Return how high `endpoint`'s top stands above the floor of `level`: the heights of the primitives from `level`'s child down to `endpoint`, summed.
    fn elevation_above(&self, level: &ElementPath, endpoint: &ElementPath) -> f64 {
        let endpoint_path = endpoint.clone();
        let endpoint_itself = iter::once(endpoint_path);
        let ancestors = endpoint.ancestors();
        let endpoint_and_ancestors = endpoint_itself.chain(ancestors);
        endpoint_and_ancestors
            .filter(|path| level.is_ancestor_of(path))
            .filter_map(|path| {
                let primitive = ResolvedPrimitive::<C>::of_path(self.model, &path)?;
                Some(primitive.height)
            })
            .sum()
    }
}

impl FloorBranch {
    /// Trace in `container`'s frame, as camera `C` sees it, the branch of every bundled route among `routes` that runs on its floor, `footprint` being where it stands in their plan: a target branch ends in the arrowhead and carries the label, a source branch carries it only when its route has no target branch.
    fn every_one_on_floor_of<C: Camera>(
        container: &ElementPath,
        footprint: Rect,
        routes: &BTreeMap<EdgeId, Route>,
    ) -> Vec<Self> {
        let container_offset = footprint.origin().to_vec2();
        let mut floor_branches = Vec::new();
        for (edge_id, route) in routes {
            let Course::Bundled {
                source_branch,
                target_branch,
            } = &route.course
            else {
                continue;
            };
            let source_branch_here = source_branch
                .as_ref()
                .filter(|branch| branch.container == *container);
            if let Some(branch) = source_branch_here {
                let source_port = route.source.point;
                let plan_corners = branch.corners_from_port_to_slot(source_port);
                let trace = Self::trace_on_container_floor::<C>(plan_corners, container_offset);
                let is_target_unbranched = target_branch.is_none();
                let floor_branch = Self {
                    edge_id: edge_id.clone(),
                    trace,
                    ends_in_arrowhead: false,
                    carries_label: is_target_unbranched,
                };
                floor_branches.push(floor_branch);
            }
            let target_branch_here = target_branch
                .as_ref()
                .filter(|branch| branch.container == *container);
            if let Some(branch) = target_branch_here {
                let target_port = route.target.point;
                let plan_corners = branch.corners_from_slot_to_port(target_port);
                let trace = Self::trace_on_container_floor::<C>(plan_corners, container_offset);
                let floor_branch = Self {
                    edge_id: edge_id.clone(),
                    trace,
                    ends_in_arrowhead: true,
                    carries_label: true,
                };
                floor_branches.push(floor_branch);
            }
        }
        floor_branches
    }

    /// Trace the polyline through `plan_corners`, in the enclosing level's plan, on the floor of the container whose footprint corner lies `container_offset` from that plan's origin.
    fn trace_on_container_floor<C: Camera>(
        plan_corners: Vec<Point>,
        container_offset: Vec2,
    ) -> FlatTrace {
        let container_floor_height = 0.0;
        let container_corners = plan_corners
            .into_iter()
            .map(|plan_corner| plan_corner - container_offset);
        FlatTrace::on_floor_through::<C>(container_floor_height, container_corners)
    }
}

impl Branch {
    /// Return the branch's plan corners in flow order at a source end: from `source_port` back through the bends to the slot.
    fn corners_from_port_to_slot(&self, source_port: Point) -> Vec<Point> {
        let port_corner = iter::once(source_port);
        let bends_from_port = self.bends.iter().rev().copied();
        let slot_corner = iter::once(self.slot.point);
        port_corner
            .chain(bends_from_port)
            .chain(slot_corner)
            .collect()
    }

    /// Return the branch's plan corners in flow order at a target end: from the slot through the bends to `target_port`.
    fn corners_from_slot_to_port(&self, target_port: Point) -> Vec<Point> {
        let slot_corner = iter::once(self.slot.point);
        let bends_from_slot = self.bends.iter().copied();
        let port_corner = iter::once(target_port);
        slot_corner
            .chain(bends_from_slot)
            .chain(port_corner)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use kurbo::{BezPath, ParamCurve, PathEl, Point, Shape as _, Size};
    use svugy_model::builder::ModelBuilder;
    use svugy_model::disclose::DisclosedModel;
    use svugy_model::model::{Element, Shape};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::geometry::{ArrowheadMarker, Face, Label, Scene};
    use crate::geometry::{Camera, Isometric};
    use crate::layout::{PlacedLevel, Port, Side, Stub};

    const SLAB_TOP: f64 = 10.5;
    const BLOCK_TOP: f64 = 22.5;

    fn path(dotted: &str) -> ElementPath {
        let segments = dotted.split('.');
        ElementPath::from_segments(segments)
    }

    /// Nest leaf `a.b.c` in container `a.b` in container `a`, beside leaf `a.d`, with `r -> a.b.c` homed at the root and `a.d -> a.b.c` homed in `a`.
    fn three_nested_levels_model() -> Model {
        let mut builder = ModelBuilder::new();
        let r = builder.add_top_level("r", Element::new("relay"));
        let a = builder.add_top_level("a", Element::new("outer"));
        let b = builder.add_child(&a, "b", Element::new("inner"));
        let c = builder.add_child(&b, "c", Element::new("core"));
        let d = builder.add_child(&a, "d", Element::new("dealer"));
        let into_core = Edge::new().with_label("feeds");
        builder.add_edge(&r, &c, into_core);
        builder.add_edge(&d, &c, Edge::new());
        builder.build().model.expect("fixture model is valid")
    }

    impl LevelPlans {
        /// Plan the root, `a` and `a.b` of the nested model: `a` sits at (100, 20), `a.b` at (30, 30) in `a`, `a.b.c` at (20, 20) in `a.b`.
        fn three_nested_levels() -> Self {
            let relay_into_core = EdgeId::new(path("r"), path("a.b.c"), 0);
            let dealer_into_core = EdgeId::new(path("a.d"), path("a.b.c"), 0);
            let root_footprints = BTreeMap::from([
                (path("r"), Rect::new(0.0, 0.0, 60.0, 40.0)),
                (path("a"), Rect::new(100.0, 20.0, 340.0, 260.0)),
            ]);
            let relay_port = Port::new(Side::East, Point::new(60.0, 20.0));
            let core_west_port_in_root = Port::new(Side::West, Point::new(150.0, 90.0));
            let relay_route = Route::new(relay_port, core_west_port_in_root);
            let root_routes = BTreeMap::from([(relay_into_core.clone(), relay_route)]);
            let root_extent = Size::new(400.0, 300.0);
            let root_placed = PlacedLevel::new(root_extent, root_footprints);
            let root_plan = LevelPlan::new(root_placed, root_routes, BTreeMap::new());
            let outer_footprints = BTreeMap::from([
                (path("a.b"), Rect::new(30.0, 30.0, 150.0, 150.0)),
                (path("a.d"), Rect::new(180.0, 40.0, 220.0, 80.0)),
            ]);
            let dealer_port = Port::new(Side::West, Point::new(180.0, 60.0));
            let core_east_port_in_outer = Port::new(Side::East, Point::new(110.0, 70.0));
            let dealer_route = Route::new(dealer_port, core_east_port_in_outer);
            let outer_routes = BTreeMap::from([(dealer_into_core.clone(), dealer_route)]);
            let outer_relay_slot = Port::new(Side::West, Point::new(0.0, 70.0));
            let outer_relay_edges = BTreeSet::from([relay_into_core.clone()]);
            let outer_relay_stub = Stub::new(outer_relay_slot, outer_relay_edges);
            let outer_stubs = BTreeMap::from([(path("r"), outer_relay_stub)]);
            let outer_extent = Size::new(240.0, 240.0);
            let outer_placed = PlacedLevel::new(outer_extent, outer_footprints);
            let outer_plan = LevelPlan::new(outer_placed, outer_routes, outer_stubs);
            let inner_footprints =
                BTreeMap::from([(path("a.b.c"), Rect::new(20.0, 20.0, 80.0, 60.0))]);
            let inner_relay_slot = Port::new(Side::West, Point::new(0.0, 40.0));
            let inner_dealer_slot = Port::new(Side::East, Point::new(120.0, 40.0));
            let inner_relay_edges = BTreeSet::from([relay_into_core]);
            let inner_relay_stub = Stub::new(inner_relay_slot, inner_relay_edges);
            let inner_dealer_edges = BTreeSet::from([dealer_into_core]);
            let inner_dealer_stub = Stub::new(inner_dealer_slot, inner_dealer_edges);
            let inner_stubs = BTreeMap::from([
                (path("r"), inner_relay_stub),
                (path("a.d"), inner_dealer_stub),
            ]);
            let inner_extent = Size::new(120.0, 120.0);
            let inner_placed = PlacedLevel::new(inner_extent, inner_footprints);
            let inner_plan = LevelPlan::new(inner_placed, BTreeMap::new(), inner_stubs);
            let plans_by_level = BTreeMap::from([
                (ElementPath::root(), root_plan),
                (path("a"), outer_plan),
                (path("a.b"), inner_plan),
            ]);
            Self::new(plans_by_level)
        }
    }

    fn nested_scene() -> Scene {
        let model = three_nested_levels_model();
        let plans = LevelPlans::three_nested_levels();
        Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure)
    }

    fn solid_of<'a>(group: &'a Group, element: &str) -> &'a Solid {
        let element_path = path(element);
        group
            .solids
            .iter()
            .find(|solid| solid.element == element_path)
            .expect("element is drawn in this level")
    }

    fn interior_of<'a>(group: &'a Group, container: &str) -> &'a Group {
        let container_solid = solid_of(group, container);
        container_solid
            .interior
            .as_ref()
            .expect("planned container carries its interior")
    }

    fn dot_path_ends(connection: &Connection) -> (Point, Point) {
        let dot_path = connection
            .dot_path
            .as_ref()
            .expect("connections carry a dot path");
        let dot_segment = dot_path.segments().next().expect("dot path has a segment");
        let dot_start = dot_segment.start();
        let dot_end = dot_segment.end();
        (dot_start, dot_end)
    }

    fn assert_near(actual: Point, expected: Point) {
        let distance = (actual - expected).hypot2().sqrt();
        assert!(distance < 1e-9, "{actual:?} is not {expected:?}");
    }

    #[test]
    fn interior_frames_compose_onto_the_slab_tops_of_three_levels() {
        let scene = nested_scene();
        let outer = interior_of(&scene.root, "a");
        let inner = interior_of(outer, "a.b");
        let outer_origin = DVec3::new(100.0, 20.0, SLAB_TOP);
        let projected_outer_origin = Isometric::project(outer_origin);
        let inner_origin_in_outer = DVec3::new(30.0, 30.0, SLAB_TOP);
        let projected_inner_origin = Isometric::project(inner_origin_in_outer);
        let outer_offset = projected_outer_origin.to_vec2();
        let inner_offset = projected_inner_origin.to_vec2();
        assert_eq!(outer.frame, Affine::translate(outer_offset));
        assert_eq!(inner.frame, Affine::translate(inner_offset));
        let core = solid_of(inner, "a.b.c");
        let core_top_face = core.faces.last().expect("block has a top face");
        let core_north_corner_in_inner = core_top_face
            .outline
            .elements()
            .first()
            .and_then(|element| element.end_point())
            .expect("top face starts at its north corner");
        let inner_to_root = scene.root.frame * outer.frame * inner.frame;
        let core_north_corner = inner_to_root * core_north_corner_in_inner;
        let stacked_corner = DVec3::new(150.0, 70.0, SLAB_TOP + SLAB_TOP + BLOCK_TOP);
        let projected_stacked_corner = Isometric::project(stacked_corner);
        assert_near(core_north_corner, projected_stacked_corner);
    }

    #[test]
    fn nested_interiors_walk_every_level_in_paint_order_with_document_frames() {
        let scene = nested_scene();
        let outer = interior_of(&scene.root, "a");
        let inner = interior_of(outer, "a.b");

        let walked: Vec<_> = scene.root.nested_interiors().collect();

        let containers: Vec<String> = walked
            .iter()
            .map(|interior| interior.container.to_string())
            .collect();
        assert_eq!(containers, ["a", "a.b"]);
        let inner_to_document = scene.root.frame * outer.frame * inner.frame;
        let walked_inner = walked[1];
        assert_eq!(walked_inner.to_outer, inner_to_document);
    }

    #[test]
    fn root_edge_into_a_nested_leaf_ends_on_its_raised_top() {
        let scene = nested_scene();
        assert_eq!(scene.root.connections.len(), 1);
        let connection = &scene.root.connections[0];
        let relay_into_core = EdgeId::new(path("r"), path("a.b.c"), 0);
        assert_eq!(connection.edge, relay_into_core);
        let (dot_start, dot_end) = dot_path_ends(connection);
        let relay_top = DVec3::new(60.0, 20.0, BLOCK_TOP);
        let raised_core_top = DVec3::new(150.0, 90.0, SLAB_TOP + SLAB_TOP + BLOCK_TOP);
        let projected_relay_top = Isometric::project(relay_top);
        let projected_core_top = Isometric::project(raised_core_top);
        assert_eq!(dot_start, projected_relay_top);
        assert_eq!(dot_end, projected_core_top);
        let outer = interior_of(&scene.root, "a");
        let inner = interior_of(outer, "a.b");
        let core = solid_of(inner, "a.b.c");
        let core_top_face = core.faces.last().expect("block has a top face");
        let inner_to_root = scene.root.frame * outer.frame * inner.frame;
        let core_top_in_root = inner_to_root * core_top_face.outline.clone();
        let is_end_on_core_top = core_top_in_root
            .bounding_box()
            .inflate(1e-9, 1e-9)
            .contains(dot_end);
        assert!(is_end_on_core_top);
        let plated_label = connection.label.as_ref().expect("labelled edge is plated");
        assert_eq!(plated_label.label.text, "feeds");
    }

    #[test]
    fn edges_arc_only_in_their_home_level() {
        let scene = nested_scene();
        let outer = interior_of(&scene.root, "a");
        let inner = interior_of(outer, "a.b");
        let outer_edges: Vec<&EdgeId> = outer
            .connections
            .iter()
            .map(|connection| &connection.edge)
            .collect();
        let dealer_into_core = EdgeId::new(path("a.d"), path("a.b.c"), 0);
        assert_eq!(outer_edges, [&dealer_into_core]);
        assert!(inner.connections.is_empty());
        let (dot_start, dot_end) = dot_path_ends(&outer.connections[0]);
        let dealer_top = DVec3::new(180.0, 60.0, BLOCK_TOP);
        let raised_core_top = DVec3::new(110.0, 70.0, SLAB_TOP + BLOCK_TOP);
        let projected_dealer_top = Isometric::project(dealer_top);
        let projected_core_top = Isometric::project(raised_core_top);
        assert_eq!(dot_start, projected_dealer_top);
        assert_eq!(dot_end, projected_core_top);
    }

    #[test]
    fn a_bundled_edge_branches_on_each_containers_floor_under_one_trunk_and_one_traveler() {
        let mut builder = ModelBuilder::new();
        let app = builder.add_top_level("a", Element::new("app"));
        let ui = builder.add_child(&app, "x", Element::new("ui"));
        let backend = builder.add_top_level("b", Element::new("backend"));
        let api = builder.add_child(&backend, "y", Element::new("api"));
        let feeds = Edge::new().with_label("feeds");
        let ui_into_api = builder.add_edge(&ui, &api, feeds);
        let model = builder.build().model.expect("fixture model is valid");
        let ui_east_port = Port::new(Side::East, Point::new(60.0, 40.0));
        let app_east_slot = Port::new(Side::East, Point::new(100.0, 40.0));
        let backend_west_slot = Port::new(Side::West, Point::new(200.0, 40.0));
        let api_west_port = Port::new(Side::West, Point::new(240.0, 60.0));
        let backend_bends = vec![Point::new(220.0, 40.0), Point::new(220.0, 60.0)];
        let source_branch = Branch {
            container: path("a"),
            slot: app_east_slot,
            bends: Vec::new(),
        };
        let target_branch = Branch {
            container: path("b"),
            slot: backend_west_slot,
            bends: backend_bends,
        };
        let bundled_course = Course::Bundled {
            source_branch: Some(source_branch),
            target_branch: Some(target_branch),
        };
        let bundled_route = Route {
            source: ui_east_port,
            target: api_west_port,
            course: bundled_course,
        };
        let root_footprints = BTreeMap::from([
            (path("a"), Rect::new(0.0, 0.0, 100.0, 100.0)),
            (path("b"), Rect::new(200.0, 0.0, 300.0, 100.0)),
        ]);
        let root_routes = BTreeMap::from([(ui_into_api, bundled_route)]);
        let root_extent = Size::new(300.0, 100.0);
        let root_placed = PlacedLevel::new(root_extent, root_footprints);
        let root_plan = LevelPlan::new(root_placed, root_routes, BTreeMap::new());
        let container_extent = Size::new(100.0, 100.0);
        let app_footprints = BTreeMap::from([(path("a.x"), Rect::new(20.0, 20.0, 60.0, 60.0))]);
        let app_placed = PlacedLevel::new(container_extent, app_footprints);
        let app_plan = LevelPlan::new(app_placed, BTreeMap::new(), BTreeMap::new());
        let backend_footprints = BTreeMap::from([(path("b.y"), Rect::new(40.0, 40.0, 80.0, 80.0))]);
        let backend_placed = PlacedLevel::new(container_extent, backend_footprints);
        let backend_plan = LevelPlan::new(backend_placed, BTreeMap::new(), BTreeMap::new());
        let plans_by_level = BTreeMap::from([
            (ElementPath::root(), root_plan),
            (path("a"), app_plan),
            (path("b"), backend_plan),
        ]);
        let plans = LevelPlans::new(plans_by_level);

        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);

        let [trunk] = &scene.root.connections[..] else {
            panic!("the home level arcs the trunk alone");
        };
        assert!(!trunk.ends_in_arrowhead, "the arrowhead is never mid-route");
        assert!(
            trunk.label.is_none(),
            "the trunk is shared, so it carries no label"
        );
        let app_interior = interior_of(&scene.root, "a");
        let [source_branch_trace] = &app_interior.traces[..] else {
            panic!("the source branch is traced on a's floor, under a's children");
        };
        assert!(!source_branch_trace.ends_in_arrowhead);
        assert!(source_branch_trace.label.is_none());
        let backend_interior = interior_of(&scene.root, "b");
        let [target_branch_trace] = &backend_interior.traces[..] else {
            panic!("the target branch is traced on b's floor, under b's children");
        };
        assert!(target_branch_trace.ends_in_arrowhead);
        let target_label = target_branch_trace.label.as_ref().expect("plated");
        assert_eq!(target_label.label.text, "feeds");
        let dot_path = trunk
            .dot_path
            .as_ref()
            .expect("the trunk moves the traveler");
        let dot_path_elements = dot_path.elements();
        let move_count = dot_path_elements
            .iter()
            .filter(|element| matches!(element, PathEl::MoveTo(_point)))
            .count();
        assert_eq!(move_count, 1, "the traveler never jumps");
        let dot_start = dot_path.segments().next().expect("dot path has a segment");
        let dot_end = dot_path.segments().last().expect("dot path has a segment");
        let ui_port_on_app_floor = DVec3::new(60.0, 40.0, SLAB_TOP);
        let api_port_on_backend_floor = DVec3::new(240.0, 60.0, SLAB_TOP);
        let projected_ui_port = Isometric::project(ui_port_on_app_floor);
        let projected_api_port = Isometric::project(api_port_on_backend_floor);
        assert_near(dot_start.start(), projected_ui_port);
        assert_near(dot_end.end(), projected_api_port);
        let target_branch_start = target_branch_trace.path.segments().next().expect("segment");
        let backend_to_root = scene.root.frame * backend_interior.frame;
        let target_branch_start_in_root = backend_to_root * target_branch_start.start();
        let backend_slot_on_its_floor = DVec3::new(200.0, 40.0, SLAB_TOP);
        let projected_backend_slot = Isometric::project(backend_slot_on_its_floor);
        assert_near(target_branch_start_in_root, projected_backend_slot);
    }

    #[test]
    fn every_container_level_tags_its_stub_slots_with_the_external_endpoint() {
        let scene = nested_scene();
        let outer = interior_of(&scene.root, "a");
        let inner = interior_of(outer, "a.b");
        assert!(scene.root.stub_markers.is_empty());
        let outer_endpoints: Vec<String> =
            outer.stub_markers.keys().map(ToString::to_string).collect();
        let inner_endpoints: Vec<String> =
            inner.stub_markers.keys().map(ToString::to_string).collect();
        assert_eq!(outer_endpoints, ["r"]);
        assert_eq!(inner_endpoints, ["a.d", "r"]);
        let outer_relay_marker = &outer.stub_markers[&path("r")];
        let relay_into_core = EdgeId::new(path("r"), path("a.b.c"), 0);
        assert_eq!(outer_relay_marker.edges, BTreeSet::from([relay_into_core]));
        assert_eq!(outer_relay_marker.tag.label.text, "relay");
        let plans = LevelPlans::three_nested_levels();
        let outer_plan = plans.level(&path("a")).expect("a is planned");
        let inner_plan = plans.level(&path("a.b")).expect("a.b is planned");
        let outer_west_band = outer_plan.stub_band(Side::West);
        let inner_east_band = inner_plan.stub_band(Side::East);
        assert_plate_lies_inside(&outer_relay_marker.tag, outer_west_band);
        let inner_dealer_marker = &inner.stub_markers[&path("a.d")];
        assert_eq!(inner_dealer_marker.tag.label.text, "dealer");
        assert_plate_lies_inside(&inner_dealer_marker.tag, inner_east_band);
    }

    /// Assert that `tag`'s plate, mapped back onto its level's floor, lies inside `band`.
    fn assert_plate_lies_inside(tag: &PlatedLabel, band: Rect) {
        let floor = Isometric::horizontal_plane_at(0.0);
        let plate_to_floor = floor.inverse() * tag.label.plane;
        let plate_rect = tag.plate.rect();
        let plate_on_floor = plate_to_floor.transform_rect_bbox(plate_rect);
        let closed_band = band.inflate(1e-9, 1e-9);
        let is_inside_band = closed_band.contains_rect(plate_on_floor);
        assert!(is_inside_band, "{plate_on_floor:?} leaves {band:?}");
    }

    #[test]
    fn each_view_frames_its_level_and_the_root_view_frames_every_nested_drawable() {
        let scene = nested_scene();
        let outer = interior_of(&scene.root, "a");
        let inner = interior_of(outer, "a.b");
        let outer_view = scene.view_of(&path("a"));
        let inner_view = scene.view_of(&path("a.b"));
        let root_view = scene.root_view();
        for stub_marker in inner.stub_markers.values() {
            assert_contains(inner_view, stub_marker.tag.bounds());
        }
        let inner_to_root = scene.root.frame * outer.frame * inner.frame;
        for face in &solid_of(inner, "a.b.c").faces {
            assert_contains(inner_view, face.outline.bounding_box());
            let face_in_root = inner_to_root * face.outline.clone();
            assert_contains(root_view, face_in_root.bounding_box());
        }
        let outer_to_root = scene.root.frame * outer.frame;
        for connection in &outer.connections {
            assert_contains(outer_view, connection.path.bounding_box());
            let path_in_root = outer_to_root * connection.path.clone();
            assert_contains(root_view, path_in_root.bounding_box());
        }
    }

    #[test]
    fn repeated_nested_projections_are_identical() {
        let first_scene = nested_scene();
        let second_scene = nested_scene();
        assert_eq!(first_scene, second_scene);
    }

    fn two_boxes_model() -> Model {
        let mut builder = ModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("app").with_caption("Dioxus · wasm"));
        let b = builder.add_top_level("b", Element::new("store").with_shape(Shape::Cylinder));
        let calls = Edge::new().with_label("calls");
        builder.add_edge(&a, &b, calls);
        builder.build().model.expect("fixture model is valid")
    }

    fn assert_contains(view: Rect, bounds: Rect) {
        let enclosing = view.union(bounds);
        assert_eq!(enclosing, view);
    }

    #[test]
    fn fixture_plan_projects_both_solids_and_a_plated_arc() {
        let model = two_boxes_model();
        let plan = LevelPlan::two_boxes_and_one_route();
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        let painted: Vec<String> = scene
            .root
            .solids
            .iter()
            .map(|solid| solid.element.to_string())
            .collect();
        assert_eq!(painted, ["a", "b"]);
        assert_eq!(scene.root.solids[1].faces.len(), 2);
        assert_eq!(scene.root.connections.len(), 1);
        let connection = &scene.root.connections[0];
        let plated_label = connection.label.as_ref().expect("labelled edge is plated");
        assert_eq!(plated_label.label.text, "calls");
        assert_eq!(scene.root.frame, Affine::IDENTITY);
        assert!(!scene.ground_lines.is_empty());
    }

    #[test]
    fn arc_ends_on_the_projected_ports_at_their_solids_tops() {
        let model = two_boxes_model();
        let plan = LevelPlan::two_boxes_and_one_route();
        let plans = LevelPlans::root_only(plan.clone());
        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        let (_edge_id, route) = plan.routes().iter().next().expect("fixture has a route");
        let connection = &scene.root.connections[0];
        let dot_path = connection
            .dot_path
            .as_ref()
            .expect("connections carry a dot path");
        let dot_segment = dot_path.segments().next().expect("dot path has a segment");
        let source_top = DVec3::new(route.source.point.x, route.source.point.y, 22.5);
        let target_top = DVec3::new(route.target.point.x, route.target.point.y, 25.5);
        assert_eq!(dot_segment.start(), Isometric::project(source_top));
        assert_eq!(dot_segment.end(), Isometric::project(target_top));
        let stroked_segment = connection
            .path
            .segments()
            .next()
            .expect("path has a segment");
        assert_eq!(stroked_segment.start(), dot_segment.start());
        let shortfall = (stroked_segment.end() - dot_segment.end()).hypot2().sqrt();
        assert!((shortfall - 1.12).abs() < 0.01);
    }

    #[test]
    fn a_flat_route_is_traced_on_the_floor_port_to_port_its_stroke_stopping_under_the_arrowhead() {
        let model = two_boxes_model();
        let lifted_plan = LevelPlan::two_boxes_and_one_route();
        let lifted_routes = lifted_plan.routes();
        let (edge_id, lifted_route) = lifted_routes.iter().next().expect("fixture has a route");
        let source_port = lifted_route.source;
        let target_port = lifted_route.target;
        let plan_corners = [
            source_port.point,
            Point::new(110.0, 50.0),
            Point::new(110.0, 70.0),
            target_port.point,
        ];
        let bends = plan_corners[1..3].to_vec();
        let flat_route = Route::traced_through(source_port, bends, target_port);
        let flat_routes = BTreeMap::from([(edge_id.clone(), flat_route)]);
        let placed = lifted_plan.placed().clone();
        let flat_plan = LevelPlan::new(placed, flat_routes, BTreeMap::new());
        let plans = LevelPlans::root_only(flat_plan);

        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);

        let is_lifted_connection_drawn = !scene.root.connections.is_empty();
        assert!(
            !is_lifted_connection_drawn,
            "a flat route draws no lifted connection"
        );
        let [trace] = &scene.root.traces[..] else {
            panic!("a flat route draws exactly one trace");
        };
        let floor = Isometric::horizontal_plane_at(0.0);
        let floor_corners = plan_corners.map(|plan_corner| floor * plan_corner);
        let [
            source_on_floor,
            first_bend_on_floor,
            last_bend_on_floor,
            target_on_floor,
        ] = floor_corners;
        let mut port_to_port_on_floor = BezPath::new();
        port_to_port_on_floor.move_to(source_on_floor);
        port_to_port_on_floor.line_to(first_bend_on_floor);
        port_to_port_on_floor.line_to(last_bend_on_floor);
        port_to_port_on_floor.line_to(target_on_floor);
        let dot_path = trace.dot_path.as_ref().expect("traces carry a dot path");
        assert_eq!(
            *dot_path, port_to_port_on_floor,
            "the dot runs from port to port through the bends, all on the floor"
        );
        let stroked_segments = trace.path.segments();
        let last_stroked_segment = stroked_segments.last().expect("trace has a segment");
        let stroked_end = last_stroked_segment.end();
        let last_leg = target_on_floor - last_bend_on_floor;
        let last_leg_direction = last_leg.normalize();
        let arrowhead_tip = stroked_end + last_leg_direction * ArrowheadMarker::TIP_OVERHANG;
        let tip_miss = (arrowhead_tip - target_on_floor).hypot2().sqrt();
        assert!(
            tip_miss < 1e-9,
            "the stroke stops one arrowhead overhang short of the target port along the last leg"
        );
        let plated_label = trace.label.as_ref().expect("labelled edge is plated");
        assert_eq!(plated_label.label.text, "calls");
    }

    #[test]
    fn view_contains_every_drawable() {
        let model = two_boxes_model();
        let plan = LevelPlan::two_boxes_and_one_route();
        let measure = FixedAdvanceMeasure;
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project::<Isometric>(&model, &plans, &measure);
        for solid in &scene.root.solids {
            for face in &solid.faces {
                assert_contains(scene.root_view(), face.outline.bounding_box());
            }
            for label in &solid.labels {
                assert_contains(scene.root_view(), label.bounds());
            }
        }
        for connection in &scene.root.connections {
            assert_contains(scene.root_view(), connection.path.bounding_box());
            let plated_label = connection.label.as_ref().expect("fixture edge is labelled");
            assert_contains(scene.root_view(), plated_label.plate.rect());
            assert_contains(scene.root_view(), plated_label.label.bounds());
        }
    }

    #[test]
    fn view_frames_the_drawables_not_the_empty_floor() {
        let model = two_boxes_model();
        let two_boxes_plan = LevelPlan::two_boxes_and_one_route();
        let floor_extent = Size::new(2_000.0, 2_000.0);
        let two_boxes_placed = two_boxes_plan.placed();
        let footprints = two_boxes_placed.footprints().clone();
        let routes = two_boxes_plan.routes().clone();
        let placed = PlacedLevel::new(floor_extent, footprints);
        let placed_boundary = placed.boundary();
        let plan = LevelPlan::new(placed, routes, BTreeMap::new());
        let plans = LevelPlans::root_only(plan.clone());
        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        let floor = Isometric::horizontal_plane_at(0.0).transform_rect_bbox(placed_boundary);
        let is_floor_framed = scene.root_view().union(floor) == scene.root_view();
        assert!(
            !is_floor_framed,
            "view {:?} frames the floor",
            scene.root_view()
        );
    }

    #[test]
    fn solids_follow_the_depth_order_not_the_path_order() {
        let mut builder = ModelBuilder::new();
        builder.add_top_level("a_small_front", Element::new("front"));
        builder.add_top_level("z_long_back", Element::new("back"));
        let model = builder.build().model.expect("fixture model is valid");
        let front = ElementPath::from_segments(["a_small_front"]);
        let back = ElementPath::from_segments(["z_long_back"]);
        let footprints = BTreeMap::from([
            (front, Rect::new(10.0, 10.0, 20.0, 20.0)),
            (back, Rect::new(0.0, 0.0, 100.0, 10.0)),
        ]);
        let extent = Size::new(100.0, 20.0);
        let placed = PlacedLevel::new(extent, footprints);
        let plan = LevelPlan::new(placed, BTreeMap::new(), BTreeMap::new());
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        let painted: Vec<String> = scene
            .root
            .solids
            .iter()
            .map(|solid| solid.element.to_string())
            .collect();
        assert_eq!(painted, ["z_long_back", "a_small_front"]);
    }

    #[test]
    fn a_tall_leaf_raises_its_port_to_its_authored_top_and_the_view_frames_it() {
        let mut builder = ModelBuilder::new();
        let tall_app = Element::new("app").with_height(84.0);
        let a = builder.add_top_level("a", tall_app);
        let b = builder.add_top_level("b", Element::new("store").with_shape(Shape::Cylinder));
        builder.add_edge(&a, &b, Edge::new().with_label("calls"));
        let model = builder.build().model.expect("fixture model is valid");
        let plan = LevelPlan::two_boxes_and_one_route();
        let plans = LevelPlans::root_only(plan.clone());
        let low_scene =
            Scene::project::<Isometric>(&two_boxes_model(), &plans, &FixedAdvanceMeasure);

        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);

        let (_edge_id, route) = plan.routes().iter().next().expect("fixture has a route");
        let (dot_start, _dot_end) = dot_path_ends(&scene.root.connections[0]);
        let tall_top = DVec3::new(route.source.point.x, route.source.point.y, 84.0);
        assert_eq!(dot_start, Isometric::project(tall_top));
        let tall_solid = solid_of(&scene.root, "a");
        for face in &tall_solid.faces {
            assert_contains(scene.root_view(), face.outline.bounding_box());
        }
        let plated_label = scene.root.connections[0].label.as_ref().expect("plated");
        assert_contains(scene.root_view(), plated_label.bounds());
        assert!(scene.root_view().y0 < low_scene.root_view().y0);
    }

    #[test]
    fn a_taller_solid_behind_is_still_painted_before_a_lower_one_in_front() {
        let mut builder = ModelBuilder::new();
        builder.add_top_level("a_small_front", Element::new("front"));
        let tall_back = Element::new("back").with_height(200.0);
        builder.add_top_level("z_long_back", tall_back);
        let model = builder.build().model.expect("fixture model is valid");
        let front = ElementPath::from_segments(["a_small_front"]);
        let back = ElementPath::from_segments(["z_long_back"]);
        let footprints = BTreeMap::from([
            (front, Rect::new(10.0, 10.0, 20.0, 20.0)),
            (back, Rect::new(0.0, 0.0, 100.0, 10.0)),
        ]);
        let extent = Size::new(100.0, 20.0);
        let placed = PlacedLevel::new(extent, footprints);
        let plan = LevelPlan::new(placed, BTreeMap::new(), BTreeMap::new());
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        let painted: Vec<String> = scene
            .root
            .solids
            .iter()
            .map(|solid| solid.element.to_string())
            .collect();
        assert_eq!(painted, ["z_long_back", "a_small_front"]);
    }

    #[test]
    fn lifted_edge_rises_by_every_primitive_down_to_its_true_endpoint() {
        let mut builder = ModelBuilder::new();
        let desk = builder.add_top_level("desk", Element::new("desk"));
        let quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
        let browser = builder.add_top_level("browser", Element::new("Browser"));
        let edge_id = builder.add_edge(&quoter, &browser, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");
        let desk_path = ElementPath::from_segments(["desk"]);
        let browser_path = ElementPath::from_segments(["browser"]);
        let footprints = BTreeMap::from([
            (desk_path, Rect::new(0.0, 0.0, 100.0, 100.0)),
            (browser_path, Rect::new(200.0, 20.0, 260.0, 80.0)),
        ]);
        let source = Port::new(Side::East, Point::new(100.0, 50.0));
        let target = Port::new(Side::West, Point::new(200.0, 50.0));
        let route = Route::new(source, target);
        let routes = BTreeMap::from([(edge_id, route)]);
        let extent = Size::new(300.0, 100.0);
        let placed = PlacedLevel::new(extent, footprints);
        let plan = LevelPlan::new(placed, routes, BTreeMap::new());
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        let connection = &scene.root.connections[0];
        let dot_path = connection
            .dot_path
            .as_ref()
            .expect("connections carry a dot path");
        let dot_segment = dot_path.segments().next().expect("dot path has a segment");
        let quoter_top_on_the_slab = DVec3::new(100.0, 50.0, SLAB_TOP + BLOCK_TOP);
        let projected_quoter_top = Isometric::project(quoter_top_on_the_slab);
        assert_eq!(dot_segment.start(), projected_quoter_top);
        assert!(connection.label.is_none());
        let unlabelled_edge = EdgeId::new(
            ElementPath::from_segments(["desk", "quoter"]),
            ElementPath::from_segments(["browser"]),
            0,
        );
        assert_eq!(connection.edge, unlabelled_edge);
    }

    #[test]
    fn plates_of_arcs_sharing_an_apex_slide_apart_and_clear_every_solid_label() {
        let mut builder = ModelBuilder::new();
        let a = builder.add_top_level("a", Element::new("app"));
        let b = builder.add_top_level("b", Element::new("store"));
        let calls = Edge::new().with_label("calls");
        let replies = Edge::new().with_label("replies");
        let calls_id = builder.add_edge(&a, &b, calls);
        let replies_id = builder.add_edge(&a, &b, replies);
        let model = builder.build().model.expect("fixture model is valid");
        let a_footprint = Rect::new(20.0, 30.0, 80.0, 70.0);
        let b_footprint = Rect::new(440.0, 50.0, 500.0, 90.0);
        let footprints = BTreeMap::from([(path("a"), a_footprint), (path("b"), b_footprint)]);
        let source_point = Point::new(80.0, 50.0);
        let target_point = Point::new(440.0, 70.0);
        let source = Port::new(Side::East, source_point);
        let target = Port::new(Side::West, target_point);
        let calls_route = Route::new(source, target);
        let routes = BTreeMap::from([(calls_id, calls_route.clone()), (replies_id, calls_route)]);
        let extent = Size::new(520.0, 110.0);
        let placed = PlacedLevel::new(extent, footprints);
        let plan = LevelPlan::new(placed, routes, BTreeMap::new());
        let measure = FixedAdvanceMeasure;
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project::<Isometric>(&model, &plans, &measure);
        let plates: Vec<Rect> = scene
            .root
            .connections
            .iter()
            .filter_map(|connection| connection.label.as_ref())
            .map(PlatedLabel::bounds)
            .collect();
        let [calls_plate, replies_plate] = plates[..] else {
            panic!("both edges are plated: {plates:?}");
        };
        let is_plate_overlap = calls_plate.overlaps(replies_plate);
        assert!(
            !is_plate_overlap,
            "{calls_plate:?} overlaps {replies_plate:?}"
        );
        let solid_label_bounds = scene
            .root
            .solids
            .iter()
            .flat_map(|solid| &solid.labels)
            .map(Label::bounds);
        for label_bounds in solid_label_bounds {
            let is_calls_over_a_label = calls_plate.overlaps(label_bounds);
            let is_replies_over_a_label = replies_plate.overlaps(label_bounds);
            assert!(!is_calls_over_a_label);
            assert!(!is_replies_over_a_label);
        }
    }

    #[test]
    fn the_outermost_layer_stacks_a_peeking_sheet_under_its_closed_container_only() {
        let mut builder = ModelBuilder::new();
        let vault = builder.add_top_level("vault", Element::new("vault"));
        let ledger = builder.add_child(&vault, "ledger", Element::new("ledger"));
        let teller = builder.add_top_level("teller", Element::new("teller"));
        builder.add_edge(&teller, &ledger, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");
        let measure = FixedAdvanceMeasure;
        let layers = DisclosedModel::every_layer_of(&model);
        let outermost_layer = &layers[0];
        let outermost_model = outermost_layer.model();
        let outermost_plans = LevelPlans::lay_out(outermost_model, &measure);

        let scene =
            Scene::project_disclosed::<Isometric>(outermost_layer, &outermost_plans, &measure);

        let closed_vault = solid_of(&scene.root, "vault");
        let plain_teller = solid_of(&scene.root, "teller");
        assert!(closed_vault.interior.is_none());
        assert!(plain_teller.sheet.is_none());
        let vault_sheet = closed_vault
            .sheet
            .as_ref()
            .expect("closed container stacks a sheet");
        let vault_sheet_bounds = vault_sheet.bounding_box();
        let vault_box_bounds = Face::bounds_of(&closed_vault.faces).expect("vault has faces");
        assert!(
            vault_sheet_bounds.y1 > vault_box_bounds.y1,
            "the sheet peeks out under the box's lower edges"
        );
        assert_contains(scene.root_view(), vault_sheet_bounds);
    }

    #[test]
    fn repeated_projections_are_identical() {
        let model = two_boxes_model();
        let plan = LevelPlan::two_boxes_and_one_route();
        let plans = LevelPlans::root_only(plan);
        let first_scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        let second_scene = Scene::project::<Isometric>(&model, &plans, &FixedAdvanceMeasure);
        assert_eq!(first_scene, second_scene);
    }
}
