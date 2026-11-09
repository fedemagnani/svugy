//! Anchor every level's routes: a port on each footprint end, on the side facing the far end, then a flat course for the sibling routes that trace clear.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use kurbo::{Line, ParamCurve, Point, Rect};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use super::edge_levels::HomedEdges;
use super::trace::OrthogonalTrace;
use super::{EdgeEnd, LevelRoutes, PlacedLevels, Port, Side};

/// Routes one edge from its source port to its target port, along the course geometry draws it on.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Route {
    pub(crate) source: Port,
    pub(crate) target: Port,
    pub(crate) course: Course,
}

/// Names how a route runs between its ports: lifted over the scene as an arc, flat along its level's floor through its bends, or bundled: lifted between its trunk ends, each a branch's slot or else the port, and flat along each branch.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Course {
    Lifted,
    Flat {
        bends: Vec<Point>,
    },
    Bundled {
        source_branch: Option<Branch>,
        target_branch: Option<Branch>,
    },
}

/// Runs one end of a bundled route flat on `container`'s floor, in home-level plan space, from the boundary `slot` its bundle's trunk lands on, through `bends`, to the route's port at that end.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Branch {
    pub(crate) container: ElementPath,
    pub(crate) slot: Port,
    pub(crate) bends: Vec<Point>,
}

/// Gathers the ends of one level's routes, faced toward footprint sides, to join them into routes.
#[derive(Default)]
struct RouteEnds<'a> {
    facing_ends_by_footprint_side: BTreeMap<(&'a ElementPath, Side), FacingEnds<'a>>,
}

/// Holds the ends attaching to one side of one footprint, each with the far point it faces.
struct FacingEnds<'a> {
    footprint: Rect,
    far_points: Vec<((&'a EdgeId, EdgeEnd), Point)>,
}

/// Keeps every flat trace this far from the footprints it passes, half the level margin.
pub(super) const TRACE_CLEARANCE: f64 = 9.0;
/// Offset across their sides below which two ports snap onto one line, half the smallest block's width, so a trace never shows a jog this small.
const JOG_SNAP: f64 = 36.0;

/// Picks the course of each route in one level: flat for a route between siblings on a container's slab with a clear trace, lifted otherwise.
struct FloorCourses<'a> {
    level: &'a ElementPath,
    footprints: &'a BTreeMap<ElementPath, Rect>,
    kept_traces: Vec<OrthogonalTrace>,
}

impl Route {
    /// Route an edge from its `source` port to its `target` port, lifted over the scene.
    pub(crate) fn new(source: Port, target: Port) -> Self {
        Self {
            source,
            target,
            course: Course::Lifted,
        }
    }

    /// Route an edge from its `source` port to its `target` port flat along its level's floor, turning at each of `bends` in order.
    pub(crate) fn traced_through(source: Port, bends: Vec<Point>, target: Port) -> Self {
        let course = Course::Flat { bends };
        Self {
            source,
            target,
            course,
        }
    }

    /// Return the branch this route runs on `container`'s floor, when it is bundled into `container`.
    pub(super) fn branch_on(&self, container: &ElementPath) -> Option<&Branch> {
        let Course::Bundled {
            source_branch,
            target_branch,
        } = &self.course
        else {
            return None;
        };
        let mut branches = [source_branch, target_branch].into_iter().flatten();
        branches.find(|branch| branch.container == *container)
    }
}

impl PlacedLevels {
    /// Anchor the routes of the edges homed in every level between their true endpoints, once every footprint is fixed.
    pub(super) fn anchor_homed_routes(&self, homed_edges: &HomedEdges) -> LevelRoutes {
        let routes_by_level = self
            .by_level
            .keys()
            .map(|level| {
                let routes = self.homed_routes_of(level, homed_edges);
                let level_key = level.clone();
                (level_key, routes)
            })
            .collect();
        LevelRoutes::new(routes_by_level)
    }

    /// Route the edges homed in `level` between their true endpoints, each end facing the other.
    fn homed_routes_of(
        &self,
        level: &ElementPath,
        homed_edges: &HomedEdges,
    ) -> BTreeMap<EdgeId, Route> {
        let mut route_ends = RouteEnds::default();
        for edge in homed_edges.homed_in(level) {
            let source = edge.from();
            let target = edge.to();
            let source_footprint = self.footprint_in_level(source, level);
            let target_footprint = self.footprint_in_level(target, level);
            route_ends.face_each_other(edge, source_footprint, target_footprint);
        }
        let placed_level = &self.by_level[level];
        let footprints = placed_level.footprints();
        let floor_courses = FloorCourses::new(level, footprints);
        route_ends.into_routes(floor_courses)
    }
}

impl<'a> RouteEnds<'a> {
    /// Face both ends of `edge` toward each other, each on the side of its own footprint.
    fn face_each_other(
        &mut self,
        edge: &'a EdgeId,
        source_footprint: Rect,
        target_footprint: Rect,
    ) {
        let source_center = source_footprint.center();
        let target_center = target_footprint.center();
        self.face_footprint_end(edge, EdgeEnd::Source, source_footprint, target_center);
        self.face_footprint_end(edge, EdgeEnd::Target, target_footprint, source_center);
    }

    /// Face `edge`'s `end` toward `far_point`, on the side of `footprint` looking at it.
    fn face_footprint_end(
        &mut self,
        edge: &'a EdgeId,
        end: EdgeEnd,
        footprint: Rect,
        far_point: Point,
    ) {
        let endpoint = end.endpoint_of(edge);
        let footprint_center = footprint.center();
        let side = Side::facing(footprint_center, far_point);
        let facing_ends = self
            .facing_ends_by_footprint_side
            .entry((endpoint, side))
            .or_insert_with(|| {
                let no_far_points = Vec::new();
                FacingEnds {
                    footprint,
                    far_points: no_far_points,
                }
            });
        facing_ends.far_points.push(((edge, end), far_point));
    }

    /// Spread the faced ends sharing a footprint side, then join both ends of every edge, in edge order, into a route along the course `floor_courses` picks.
    fn into_routes(self, mut floor_courses: FloorCourses) -> BTreeMap<EdgeId, Route> {
        let mut ports = BTreeMap::new();
        for ((_endpoint, side), facing_ends) in self.facing_ends_by_footprint_side {
            let spread_ports =
                side.spread_ports_along(facing_ends.footprint, facing_ends.far_points);
            ports.extend(spread_ports);
        }
        let routed_edges: BTreeSet<&EdgeId> = ports.keys().map(|&(edge, _end)| edge).collect();
        routed_edges
            .into_iter()
            .map(|edge_id| {
                let source = ports[&(edge_id, EdgeEnd::Source)];
                let target = ports[&(edge_id, EdgeEnd::Target)];
                let route = floor_courses.route(edge_id, source, target);
                let edge_key = edge_id.clone();
                (edge_key, route)
            })
            .collect()
    }
}

impl Side {
    /// Pick the side of a box centered at `near` that faces `far`: east or west when the x offset dominates, ties to east.
    pub(super) fn facing(near: Point, far: Point) -> Self {
        let offset = far - near;
        let is_horizontal = offset.x.abs() >= offset.y.abs();
        if is_horizontal {
            let is_eastward = offset.x >= 0.0;
            return if is_eastward { Self::East } else { Self::West };
        }
        let is_southward = offset.y > 0.0;
        if is_southward {
            Self::South
        } else {
            Self::North
        }
    }

    /// Spread n ports along this side of `rect` at fractions (k + 1)/(n + 1), ordered by t = (c − p0)·(p1 − p0) of the far points c they face.
    fn spread_ports_along<K: Ord>(
        self,
        rect: Rect,
        mut far_points: Vec<(K, Point)>,
    ) -> Vec<(K, Port)> {
        let side_line = self.line_along(rect);
        let Line { p0, p1 } = side_line;
        let side_direction = p1 - p0;
        let progress_of = |far_point: Point| {
            let far_offset = far_point - p0;
            far_offset.dot(side_direction)
        };
        far_points.sort_by(|(key, far_point), (other_key, other_far_point)| {
            let progress = progress_of(*far_point);
            let other_progress = progress_of(*other_far_point);
            progress
                .total_cmp(&other_progress)
                .then_with(|| key.cmp(other_key))
        });
        let slot_count = far_points.len() + 1;
        far_points
            .into_iter()
            .enumerate()
            .map(|(index, (key, _far_point))| {
                let slot_fraction = (index + 1) as f64 / slot_count as f64;
                let point = side_line.eval(slot_fraction);
                let port = Port::new(self, point);
                (key, port)
            })
            .collect()
    }
}

impl<'a> FloorCourses<'a> {
    /// Pick courses among the child `footprints` placed in `level`, no trace kept yet.
    fn new(level: &'a ElementPath, footprints: &'a BTreeMap<ElementPath, Rect>) -> Self {
        Self {
            level,
            footprints,
            kept_traces: Vec::new(),
        }
    }

    /// Route `edge` flat when it joins two children of a container's level along a trace clear of the other footprints and every kept trace, keeping that trace; lifted otherwise, the root's open ground included.
    fn route(&mut self, edge: &EdgeId, source: Port, target: Port) -> Route {
        let is_open_ground = self.level.is_root();
        if is_open_ground {
            return Route::new(source, target);
        }
        let source_endpoint = edge.from();
        let target_endpoint = edge.to();
        let is_self_loop = source_endpoint == target_endpoint;
        if is_self_loop {
            return Route::new(source, target);
        }
        let source_level = source_endpoint.enclosing_level();
        let target_level = target_endpoint.enclosing_level();
        let is_source_off_floor = source_level != *self.level;
        let is_target_off_floor = target_level != *self.level;
        let has_endpoint_off_floor = is_source_off_floor || is_target_off_floor;
        if has_endpoint_off_floor {
            return Route::new(source, target);
        }
        let obstacles = self.obstacles_around(edge);
        let snapped_ports = self.ports_snapped_onto_one_line(edge, source, target);
        let straight_trace = snapped_ports.and_then(|(snapped_source, snapped_target)| {
            self.clear_trace_between(snapped_source, snapped_target, &obstacles)
        });
        let found_trace =
            straight_trace.or_else(|| self.clear_trace_between(source, target, &obstacles));
        let Some((traced_source, traced_target, trace)) = found_trace else {
            return Route::new(source, target);
        };
        let bends = trace.bends();
        self.kept_traces.push(trace);
        Route::traced_through(traced_source, bends, traced_target)
    }

    /// Return both ports moved onto one line, the point of both sides' shared span nearest midway between them, when they sit less than `JOG_SNAP` apart across their sides, so a near-aligned trace runs straight instead of jogging.
    fn ports_snapped_onto_one_line(
        &self,
        edge: &EdgeId,
        source: Port,
        target: Port,
    ) -> Option<(Port, Port)> {
        let is_facing_along_x = matches!(source.side, Side::East | Side::West);
        let source_across = if is_facing_along_x {
            source.point.y
        } else {
            source.point.x
        };
        let target_across = if is_facing_along_x {
            target.point.y
        } else {
            target.point.x
        };
        let jog = (target_across - source_across).abs();
        let is_aligned_or_deliberate_jog = jog == 0.0 || jog >= JOG_SNAP;
        if is_aligned_or_deliberate_jog {
            return None;
        }
        let source_footprint = self.footprints[edge.from()];
        let target_footprint = self.footprints[edge.to()];
        let source_span = Self::side_span_clear_of_corners(source_footprint, is_facing_along_x);
        let target_span = Self::side_span_clear_of_corners(target_footprint, is_facing_along_x);
        let shared_start = source_span.start().max(*target_span.start());
        let shared_end = source_span.end().min(*target_span.end());
        let is_span_shared = shared_start <= shared_end;
        if !is_span_shared {
            return None;
        }
        let midway_across = (source_across + target_across) / 2.0;
        let shared_across = midway_across.clamp(shared_start, shared_end);
        let snapped_source = Self::port_moved_across(source, shared_across, is_facing_along_x);
        let snapped_target = Self::port_moved_across(target, shared_across, is_facing_along_x);
        Some((snapped_source, snapped_target))
    }

    /// Return the trace between `source` and `target` clear of `obstacles` and every kept trace, with the ports it joins.
    fn clear_trace_between(
        &self,
        source: Port,
        target: Port,
        obstacles: &[Rect],
    ) -> Option<(Port, Port, OrthogonalTrace)> {
        let trace = OrthogonalTrace::find_clear(source, target, obstacles, &self.kept_traces)?;
        Some((source, target, trace))
    }

    /// Return the range a port may slide along a side of `footprint` facing along x (or y), `TRACE_CLEARANCE` short of its corners.
    fn side_span_clear_of_corners(footprint: Rect, is_facing_along_x: bool) -> RangeInclusive<f64> {
        let (side_start, side_end) = if is_facing_along_x {
            (footprint.y0, footprint.y1)
        } else {
            (footprint.x0, footprint.x1)
        };
        let clear_start = side_start + TRACE_CLEARANCE;
        let clear_end = side_end - TRACE_CLEARANCE;
        clear_start..=clear_end
    }

    /// Return `port` slid along its side to `across`, the coordinate across the axis it faces along.
    fn port_moved_across(port: Port, across: f64, is_facing_along_x: bool) -> Port {
        let moved_point = if is_facing_along_x {
            Point::new(port.point.x, across)
        } else {
            Point::new(across, port.point.y)
        };
        Port::new(port.side, moved_point)
    }

    /// Return every footprint but `edge`'s endpoints', inflated by the trace clearance.
    fn obstacles_around(&self, edge: &EdgeId) -> Vec<Rect> {
        let source_endpoint = edge.from();
        let target_endpoint = edge.to();
        self.footprints
            .iter()
            .filter(|&(child, _footprint)| child != source_endpoint && child != target_endpoint)
            .map(|(_child, footprint)| footprint.inflate(TRACE_CLEARANCE, TRACE_CLEARANCE))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLAB: &str = "slab";

    fn footprints_of(named_footprints: &[(&str, Rect)]) -> BTreeMap<ElementPath, Rect> {
        named_footprints
            .iter()
            .map(|&(name, footprint)| {
                let path = ElementPath::from_segments([SLAB, name]);
                (path, footprint)
            })
            .collect()
    }

    fn edge_between(from: &str, to: &str) -> EdgeId {
        let from_path = ElementPath::from_segments([SLAB, from]);
        let to_path = ElementPath::from_segments([SLAB, to]);
        EdgeId::new(from_path, to_path, 0)
    }

    fn route_between_footprints(
        footprints: &BTreeMap<ElementPath, Rect>,
        edge_ids: &[EdgeId],
    ) -> BTreeMap<EdgeId, Route> {
        let mut route_ends = RouteEnds::default();
        for edge_id in edge_ids {
            let source_footprint = footprints[edge_id.from()];
            let target_footprint = footprints[edge_id.to()];
            route_ends.face_each_other(edge_id, source_footprint, target_footprint);
        }
        let slab = ElementPath::from_segments([SLAB]);
        let floor_courses = FloorCourses::new(&slab, footprints);
        route_ends.into_routes(floor_courses)
    }

    #[test]
    fn ports_lie_on_the_sides_facing_each_other() {
        let footprints = footprints_of(&[
            ("hub", Rect::new(300.0, 300.0, 400.0, 360.0)),
            ("east", Rect::new(600.0, 290.0, 700.0, 350.0)),
            ("west", Rect::new(20.0, 320.0, 120.0, 380.0)),
            ("north", Rect::new(310.0, 20.0, 410.0, 80.0)),
            ("south", Rect::new(280.0, 600.0, 380.0, 660.0)),
        ]);
        let expected_sides = [
            ("east", Side::East, Side::West),
            ("west", Side::West, Side::East),
            ("north", Side::North, Side::South),
            ("south", Side::South, Side::North),
        ];
        let edge_ids: Vec<EdgeId> = expected_sides
            .iter()
            .map(|&(far, _hub_side, _far_side)| edge_between("hub", far))
            .collect();

        let routes = route_between_footprints(&footprints, &edge_ids);

        for (edge_id, (_far, hub_side, far_side)) in edge_ids.iter().zip(expected_sides) {
            let route = &routes[edge_id];
            let source_footprint = footprints[edge_id.from()];
            let target_footprint = footprints[edge_id.to()];
            assert_eq!(route.source.side, hub_side);
            assert_eq!(route.target.side, far_side);
            assert!(route.source.lies_on_side_of(source_footprint));
            assert!(route.target.lies_on_side_of(target_footprint));
        }
    }

    #[test]
    fn siblings_with_a_clear_path_get_a_flat_trace_and_a_sibling_standing_between_lifts_the_route()
    {
        let footprints = footprints_of(&[
            ("left", Rect::new(0.0, 0.0, 100.0, 60.0)),
            ("middle", Rect::new(200.0, 0.0, 300.0, 60.0)),
            ("right", Rect::new(400.0, 0.0, 500.0, 60.0)),
            ("aligned_west", Rect::new(0.0, 200.0, 100.0, 260.0)),
            ("aligned_east", Rect::new(200.0, 200.0, 300.0, 260.0)),
            ("offset_west", Rect::new(0.0, 400.0, 100.0, 460.0)),
            ("offset_east", Rect::new(200.0, 460.0, 300.0, 520.0)),
        ]);
        let to_neighbour = edge_between("left", "middle");
        let past_neighbour = edge_between("left", "right");
        let across_aligned_ports = edge_between("aligned_west", "aligned_east");
        let across_offset_ports = edge_between("offset_west", "offset_east");
        let edge_ids = [
            to_neighbour.clone(),
            past_neighbour.clone(),
            across_aligned_ports.clone(),
            across_offset_ports.clone(),
        ];

        let routes = route_between_footprints(&footprints, &edge_ids);

        let straight_course = Course::Flat { bends: Vec::new() };
        let neighbour_route = &routes[&to_neighbour];
        let snapped_ports_y = [
            neighbour_route.source.point.y,
            neighbour_route.target.point.y,
        ];
        assert_eq!(
            neighbour_route.course, straight_course,
            "ports less than the jog snap apart join straight instead of jogging"
        );
        assert_eq!(
            snapped_ports_y,
            [25.0, 25.0],
            "near-aligned ports snap onto the line midway between them"
        );
        let offset_bends = [Point::new(150.0, 430.0), Point::new(150.0, 490.0)];
        let offset_course = Course::Flat {
            bends: offset_bends.to_vec(),
        };
        assert_eq!(
            routes[&across_offset_ports].course, offset_course,
            "ports offset past the jog snap join through two bends midway between them"
        );
        assert_eq!(
            routes[&past_neighbour].course,
            Course::Lifted,
            "a route whose every two-bend trace crosses a standing sibling lifts over it"
        );
        assert_eq!(
            routes[&across_aligned_ports].course, straight_course,
            "aligned facing ports of adjacent siblings join straight"
        );
    }

    #[test]
    fn ports_sharing_a_side_spread_evenly_in_the_order_of_their_far_ends() {
        let footprints = footprints_of(&[
            ("hub", Rect::new(0.0, 100.0, 100.0, 160.0)),
            ("high", Rect::new(400.0, 0.0, 500.0, 60.0)),
            ("level", Rect::new(400.0, 100.0, 500.0, 160.0)),
            ("low", Rect::new(400.0, 200.0, 500.0, 260.0)),
        ]);
        let edge_ids = [
            edge_between("hub", "low"),
            edge_between("hub", "high"),
            edge_between("hub", "level"),
        ];

        let routes = route_between_footprints(&footprints, &edge_ids);

        let source_y_of = |far: &str| routes[&edge_between("hub", far)].source.point.y;
        assert_eq!(source_y_of("high"), 115.0);
        assert_eq!(source_y_of("level"), 130.0);
        assert_eq!(source_y_of("low"), 145.0);
    }

    #[test]
    fn a_self_loop_leaves_and_returns_through_two_distinct_ports_on_one_side() {
        let footprint = Rect::new(0.0, 0.0, 100.0, 60.0);
        let footprints = footprints_of(&[("a", footprint)]);
        let loop_id = edge_between("a", "a");

        let routes = route_between_footprints(&footprints, std::slice::from_ref(&loop_id));

        let route = &routes[&loop_id];
        assert!(route.source.lies_on_side_of(footprint));
        assert!(route.target.lies_on_side_of(footprint));
        assert_ne!(route.source.point, route.target.point);
    }
}
