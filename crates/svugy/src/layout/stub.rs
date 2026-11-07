//! Side every container's stubs: one boundary slot per external endpoint, where its edges' home routes leave the boundary.

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Line, ParamCurve, ParamCurveNearest, Point, Rect, Vec2};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use super::edge_levels::{LeavingEdge, LeavingEdges};
use super::{LevelRoutes, LevelStubs, PlacedLevels, Port, Side};

/// Stubs the edges that leave this level toward one external endpoint through one boundary slot.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stub {
    pub(crate) slot: Port,
    pub(crate) edges: BTreeSet<EdgeId>,
}

/// Gathers where the home routes of the edges toward one external endpoint leave a level's boundary.
#[derive(Default)]
struct BoundaryCrossings {
    ports: Vec<Port>,
    edges: BTreeSet<EdgeId>,
}

impl Stub {
    /// Stub `edges` through the boundary `slot`.
    pub(crate) fn new(slot: Port, edges: BTreeSet<EdgeId>) -> Self {
        Self { slot, edges }
    }
}

impl PlacedLevels {
    /// Side the stubs of every container left by edges where the home routes of those edges leave its boundary.
    pub(super) fn side_stubs_at_home_route_crossings(
        &self,
        leaving_edges: &LeavingEdges,
        level_routes: &LevelRoutes,
    ) -> LevelStubs {
        let mut stubs_by_container = BTreeMap::new();
        for container in leaving_edges.containers_left_by_edges() {
            let container_leaving_edges = leaving_edges.leaving(container);
            let crossings_by_external_endpoint =
                self.boundary_crossings_of(container, container_leaving_edges, level_routes);
            let stubs = crossings_by_external_endpoint
                .into_iter()
                .map(|(external_endpoint, crossings)| {
                    let stub = crossings.into_stub();
                    (external_endpoint, stub)
                })
                .collect();
            let container_key = container.clone();
            stubs_by_container.insert(container_key, stubs);
        }
        LevelStubs::new(stubs_by_container)
    }

    /// Cross the home line of every edge leaving `container` with its boundary, gathering the crossings per external endpoint.
    fn boundary_crossings_of(
        &self,
        container: &ElementPath,
        leaving_edges: &[LeavingEdge],
        level_routes: &LevelRoutes,
    ) -> BTreeMap<ElementPath, BoundaryCrossings> {
        let boundary = self.by_level[container].boundary();
        let mut crossings_by_external_endpoint: BTreeMap<ElementPath, BoundaryCrossings> =
            BTreeMap::new();
        for leaving_edge in leaving_edges {
            let edge = leaving_edge.edge();
            let home = edge.home_level();
            let container_footprint = self.footprint_in_level(container, &home);
            let container_origin = container_footprint.origin();
            let home_to_container = container_origin.to_vec2();
            let home_line = self.home_line_of(leaving_edge, level_routes);
            let inner_point = home_line.p0 - home_to_container;
            let outer_point = home_line.p1 - home_to_container;
            let crossing = Port::leaving(boundary, inner_point, outer_point);
            let external_endpoint = leaving_edge.external_endpoint();
            let external_endpoint_key = external_endpoint.clone();
            let crossings = crossings_by_external_endpoint
                .entry(external_endpoint_key)
                .or_default();
            let crossed_edge = edge.clone();
            crossings.add(crossed_edge, crossing);
        }
        crossings_by_external_endpoint
    }

    /// Return the straight line `leaving_edge` runs along in its home level, from its inner end out: between its endpoints' footprint centres for a badge, which has no route, between its route's ports otherwise.
    fn home_line_of(&self, leaving_edge: &LeavingEdge, level_routes: &LevelRoutes) -> Line {
        let edge = leaving_edge.edge();
        let home = edge.home_level();
        let inner_end = leaving_edge.inner_end();
        let outer_end = inner_end.opposite();
        let is_badge = leaving_edge.is_badge();
        if is_badge {
            let inner_endpoint = inner_end.endpoint_of(edge);
            let outer_endpoint = outer_end.endpoint_of(edge);
            let inner_footprint = self.footprint_in_level(inner_endpoint, &home);
            let outer_footprint = self.footprint_in_level(outer_endpoint, &home);
            let inner_center = inner_footprint.center();
            let outer_center = outer_footprint.center();
            return Line::new(inner_center, outer_center);
        }
        let home_route = &level_routes.by_level[&home][edge];
        let inner_port = inner_end.port_of(home_route);
        let outer_port = outer_end.port_of(home_route);
        Line::new(inner_port.point, outer_port.point)
    }
}

impl Port {
    /// Return where the ray from `inside` through `outside` leaves `rect`: on the side it reaches at the least t = (side − inside) / (outside − inside) > 0, snapped onto that side.
    fn leaving(rect: Rect, inside: Point, outside: Point) -> Self {
        let offset = outside - inside;
        let gaps_and_steps_by_side = [
            (Side::North, rect.y0 - inside.y, offset.y),
            (Side::East, rect.x1 - inside.x, offset.x),
            (Side::South, rect.y1 - inside.y, offset.y),
            (Side::West, rect.x0 - inside.x, offset.x),
        ];
        let first_reached = gaps_and_steps_by_side
            .into_iter()
            .filter_map(|(side, gap, step)| {
                let reach = gap / step;
                let is_ahead = reach > 0.0;
                let is_finite = reach.is_finite();
                let is_reached_ahead = is_ahead && is_finite;
                is_reached_ahead.then_some((side, reach))
            })
            .min_by(|(_side, reach), (_other_side, other_reach)| reach.total_cmp(other_reach));
        let (side, reach) =
            first_reached.expect("a ray from inside a rectangle to a distinct point leaves it");
        let reached_point = inside + offset * reach;
        let side_line = side.line_along(rect);
        let nearest_on_side = side_line.nearest(reached_point, 0.0);
        let point = side_line.eval(nearest_on_side.t);
        Self::new(side, point)
    }
}

impl BoundaryCrossings {
    /// Record that `edge`'s home route leaves the boundary through `port`.
    fn add(&mut self, edge: EdgeId, port: Port) {
        self.ports.push(port);
        self.edges.insert(edge);
    }

    /// Slot the stub on side* = argmax_s #{crossings on s} (ties to the least s), at slot = mean{crossing points on side*}.
    fn into_stub(self) -> Stub {
        let mut crossing_counts_by_side: BTreeMap<Side, usize> = BTreeMap::new();
        for port in &self.ports {
            let crossing_count = crossing_counts_by_side.entry(port.side).or_default();
            *crossing_count += 1;
        }
        let (&busiest_side, _busiest_count) = crossing_counts_by_side
            .iter()
            .max_by(|(side, count), (other_side, other_count)| {
                count.cmp(other_count).then_with(|| other_side.cmp(side))
            })
            .expect("every boundary crossing records at least one port");
        let crossing_points: Vec<Point> = self
            .ports
            .iter()
            .filter(|port| port.side == busiest_side)
            .map(|port| port.point)
            .collect();
        let first_point = crossing_points[0];
        let summed_offset = crossing_points
            .iter()
            .fold(Vec2::ZERO, |summed, &point| summed + (point - first_point));
        let mean_offset = summed_offset / crossing_points.len() as f64;
        let mean_point = first_point + mean_offset;
        let slot = Port::new(busiest_side, mean_point);
        Stub::new(slot, self.edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge_from_inside_to(external: &str, ordinal: u32) -> EdgeId {
        let inner = ElementPath::from_segments(["c", "x"]);
        let external_path = ElementPath::from_segments([external]);
        EdgeId::new(inner, external_path, ordinal)
    }

    #[test]
    fn a_ray_leaves_through_the_side_it_reaches_first_not_the_side_its_direction_favours() {
        let wide = Rect::new(0.0, 0.0, 300.0, 100.0);
        let center = wide.center();
        let east_northeast = Point::new(350.0, -50.0);

        let port = Port::leaving(wide, center, east_northeast);

        assert_eq!(port.side, Side::North);
        assert!(port.lies_on_side_of(wide));
        assert_eq!(port.point, Point::new(250.0, 0.0));
    }

    #[test]
    fn every_crossing_lies_exactly_on_its_side() {
        let boundary = Rect::new(0.0, 0.0, 673.5, 300.0);
        let inside = Point::new(108.0, 231.0);
        let outsides = [
            Point::new(81.4, 396.0),
            Point::new(-155.0, 201.3),
            Point::new(700.1, 117.7),
            Point::new(301.3, -400.9),
        ];
        for outside in outsides {
            let port = Port::leaving(boundary, inside, outside);
            assert!(port.lies_on_side_of(boundary), "{outside:?} gives {port:?}");
        }
    }

    #[test]
    fn a_merged_stub_slots_on_the_busiest_side_at_its_crossings_mean() {
        let boundary = Rect::new(0.0, 0.0, 200.0, 100.0);
        let mut crossings = BoundaryCrossings::default();
        let east_ports = [
            Port::new(Side::East, Point::new(200.0, 20.0)),
            Port::new(Side::East, Point::new(200.0, 50.0)),
        ];
        let north_port = Port::new(Side::North, Point::new(190.0, 0.0));
        let ports = east_ports.into_iter().chain([north_port]);
        for (ordinal, port) in (0..).zip(ports) {
            let edge = edge_from_inside_to("far", ordinal);
            crossings.add(edge, port);
        }

        let stub = crossings.into_stub();

        assert_eq!(stub.slot, Port::new(Side::East, Point::new(200.0, 35.0)));
        assert_eq!(stub.edges.len(), 3);
        assert!(stub.slot.lies_on_side_of(boundary));
    }
}
