//! Project every laid-out level into nested scene groups.

use std::collections::BTreeSet;
use std::iter;

use glam::DVec3;
use kurbo::{Affine, Rect};
use svugy_model::model::{Edge, EdgeId, Model};
use svugy_model::path::ElementPath;

use super::badge::{BadgeIcon, BadgeSeater};
use super::connection::LiftedArc;
use super::depth::DepthOrder;
use super::obstruction::Obstructions;
use super::primitive::ResolvedPrimitive;
use super::projection::IsometricProjection;
use super::{Connection, Group, PlatedLabel, Solid};
use crate::layout::{LevelPlacement, LevelPlan, LevelPlans, Route};
use crate::metrics::TextMeasure;

/// Projects levels top-down, each container's interior onto its solid's top, a sheet under each closed container.
pub(super) struct LevelProjector<'a, M> {
    model: &'a Model,
    closed_containers: &'a BTreeSet<ElementPath>,
    plans: &'a LevelPlans,
    measure: &'a M,
}

impl<'a, M: TextMeasure> LevelProjector<'a, M> {
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
        }
    }

    /// Project `level` as `plan` lays it out, mapped into its parent by `frame`: solids back to front with their interiors, then arcs of the edges homed here with plates clear of the solids' labels.
    pub(super) fn project_level(
        &self,
        level: &ElementPath,
        plan: &LevelPlan,
        frame: Affine,
    ) -> Group {
        let solids = self.project_solids_back_to_front(plan);
        let obstructions = Obstructions::around_text_of(&solids);
        let connections = self.arc_connections_homed_at(level, plan, obstructions);
        let empty_group = Group::new(frame);
        Group {
            solids,
            connections,
            ..empty_group
        }
    }

    /// Project each footprint's element back to front, stacking a sheet under it when closed, seating the badges of the edges it is the source of on its top, stacking the interior of every container the plans lay out there.
    fn project_solids_back_to_front(&self, plan: &LevelPlan) -> Vec<Solid> {
        let placed = plan.placed();
        let footprints = placed.footprints();
        let back_to_front = DepthOrder::back_to_front(footprints);
        back_to_front
            .into_iter()
            .filter_map(|path| {
                let element = self.model.element(path)?;
                let primitive = ResolvedPrimitive::of_path(self.model, path)?;
                let footprint = footprints[path];
                let drawn_solid = primitive.draw(path, element, footprint, self.measure);
                let sheeted_solid =
                    self.stack_sheet_if_closed(path, footprint, &primitive, drawn_solid);
                let badge_icons = self.badge_icons_from(path);
                let badge_seater =
                    BadgeSeater::on_top_of(&sheeted_solid, footprint, primitive.height);
                let badges = badge_seater.seat_badges(badge_icons);
                let badged_solid = sheeted_solid.with_badges(badges);
                let solid =
                    self.stack_planned_interior(path, footprint, primitive.height, badged_solid);
                Some(solid)
            })
            .collect()
    }

    /// Stack a sheet outlined from `primitive` over `footprint` under the solid of `path`, if `path` is a closed container.
    fn stack_sheet_if_closed(
        &self,
        path: &ElementPath,
        footprint: Rect,
        primitive: &ResolvedPrimitive,
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
    fn badge_icons_from(&self, source: &ElementPath) -> Vec<(EdgeId, BadgeIcon)> {
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

    /// Stack the planned interior of `path`, if any, on its solid: the interior's origin is the footprint's top-left corner raised to the primitive's top, `primitive_height` up.
    fn stack_planned_interior(
        &self,
        path: &ElementPath,
        footprint: Rect,
        primitive_height: f64,
        drawn_solid: Solid,
    ) -> Solid {
        let Some(interior_plan) = self.plans.level(path) else {
            return drawn_solid;
        };
        let raised_origin = DVec3::new(footprint.x0, footprint.y0, primitive_height);
        let projected_origin = IsometricProjection::project(raised_origin);
        let interior_offset = projected_origin.to_vec2();
        let interior_frame = Affine::translate(interior_offset);
        let interior = self.project_level(path, interior_plan, interior_frame);
        drawn_solid.with_interior(interior)
    }

    /// Arc the route of every edge homed at `level`, plating each label where its arc is least obstructed, clear of the plates before it, in route order.
    fn arc_connections_homed_at(
        &self,
        level: &ElementPath,
        plan: &LevelPlan,
        mut obstructions: Obstructions,
    ) -> Vec<Connection> {
        let routes = plan.routes();
        routes
            .iter()
            .map(|(edge_id, route)| {
                self.arc_connection_homed_at(level, edge_id, route, &mut obstructions)
            })
            .collect()
    }

    /// Arc `edge_id`'s `route`, homed at `level`, from its ports raised by the solids under them, its label, if any, plated where the arc is least obstructed and recorded among `obstructions`.
    fn arc_connection_homed_at(
        &self,
        level: &ElementPath,
        edge_id: &EdgeId,
        route: &Route,
        obstructions: &mut Obstructions,
    ) -> Connection {
        let source = edge_id.from();
        let target = edge_id.to();
        let source_elevation = self.elevation_above(level, source);
        let target_elevation = self.elevation_above(level, target);
        let arc = LiftedArc::over_route(route, source_elevation, target_elevation);
        let stroked_path = arc.path_under_target_arrowhead();
        let dot_path = arc.port_to_port_path();
        let connection_edge_id = edge_id.clone();
        let connection = Connection::new(connection_edge_id, stroked_path).with_dot_path(dot_path);
        let edge = self.model.edge(edge_id);
        let edge_label = edge.and_then(Edge::label);
        let Some(label_text) = edge_label else {
            return connection;
        };
        let plated_label =
            PlatedLabel::place_least_obstructed_along(label_text, &arc, obstructions, self.measure);
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
                let primitive = ResolvedPrimitive::of_path(self.model, &path)?;
                Some(primitive.height)
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use kurbo::{ParamCurve, Point, Shape as _, Size};
    use svugy_model::builder::ModelBuilder;
    use svugy_model::disclose::DisclosedModel;
    use svugy_model::model::{Element, Shape};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::geometry::{Face, Label, Scene};
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
        Scene::project(&model, &plans, &FixedAdvanceMeasure)
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
        let projected_outer_origin = IsometricProjection::project(outer_origin);
        let inner_origin_in_outer = DVec3::new(30.0, 30.0, SLAB_TOP);
        let projected_inner_origin = IsometricProjection::project(inner_origin_in_outer);
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
        let projected_stacked_corner = IsometricProjection::project(stacked_corner);
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
        let projected_relay_top = IsometricProjection::project(relay_top);
        let projected_core_top = IsometricProjection::project(raised_core_top);
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
        let projected_dealer_top = IsometricProjection::project(dealer_top);
        let projected_core_top = IsometricProjection::project(raised_core_top);
        assert_eq!(dot_start, projected_dealer_top);
        assert_eq!(dot_end, projected_core_top);
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
        let floor = IsometricProjection::horizontal_plane_at(0.0);
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
        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
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
        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
        let (_edge_id, route) = plan.routes().iter().next().expect("fixture has a route");
        let connection = &scene.root.connections[0];
        let dot_path = connection
            .dot_path
            .as_ref()
            .expect("connections carry a dot path");
        let dot_segment = dot_path.segments().next().expect("dot path has a segment");
        let source_top = DVec3::new(route.source.point.x, route.source.point.y, 22.5);
        let target_top = DVec3::new(route.target.point.x, route.target.point.y, 25.5);
        assert_eq!(
            dot_segment.start(),
            IsometricProjection::project(source_top)
        );
        assert_eq!(dot_segment.end(), IsometricProjection::project(target_top));
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
    fn view_contains_every_drawable() {
        let model = two_boxes_model();
        let plan = LevelPlan::two_boxes_and_one_route();
        let measure = FixedAdvanceMeasure;
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project(&model, &plans, &measure);
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
        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
        let floor =
            IsometricProjection::horizontal_plane_at(0.0).transform_rect_bbox(placed_boundary);
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
        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
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
        let low_scene = Scene::project(&two_boxes_model(), &plans, &FixedAdvanceMeasure);

        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);

        let (_edge_id, route) = plan.routes().iter().next().expect("fixture has a route");
        let (dot_start, _dot_end) = dot_path_ends(&scene.root.connections[0]);
        let tall_top = DVec3::new(route.source.point.x, route.source.point.y, 84.0);
        assert_eq!(dot_start, IsometricProjection::project(tall_top));
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
        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
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
        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
        let connection = &scene.root.connections[0];
        let dot_path = connection
            .dot_path
            .as_ref()
            .expect("connections carry a dot path");
        let dot_segment = dot_path.segments().next().expect("dot path has a segment");
        let quoter_top_on_the_slab = DVec3::new(100.0, 50.0, SLAB_TOP + BLOCK_TOP);
        let projected_quoter_top = IsometricProjection::project(quoter_top_on_the_slab);
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
        let scene = Scene::project(&model, &plans, &measure);
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

        let scene = Scene::project_disclosed(outermost_layer, &outermost_plans, &measure);

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
        let first_scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
        let second_scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);
        assert_eq!(first_scene, second_scene);
    }
}
