//! Side a container's stubs: one boundary slot per external endpoint, where its edges' home routes leave the boundary.

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Line, ParamCurve, ParamCurveNearest, Point, Rect, Vec2};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use super::{LevelPlan, Port, Side, Stub};

/// Gathers where the home routes of the edges toward one external endpoint leave a level's boundary.
#[derive(Default)]
pub(super) struct BoundaryCrossings {
    ports: Vec<Port>,
    edges: BTreeSet<EdgeId>,
}

impl BoundaryCrossings {
    /// Record that `edge`'s home route leaves the boundary through `port`.
    pub(super) fn add(&mut self, edge: EdgeId, port: Port) {
        self.ports.push(port);
        self.edges.insert(edge);
    }

    /// Slot the stub on the side most crossings leave through, ties to the first side, at their mean point along it.
    fn into_stub(self) -> Option<Stub> {
        let mut crossing_counts_by_side: BTreeMap<Side, usize> = BTreeMap::new();
        for port in &self.ports {
            let crossing_count = crossing_counts_by_side.entry(port.side).or_default();
            *crossing_count += 1;
        }
        let busiest_side = crossing_counts_by_side
            .iter()
            .max_by(|(side, count), (other_side, other_count)| {
                count.cmp(other_count).then_with(|| other_side.cmp(side))
            })
            .map(|(&side, _count)| side)?;
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
        let stub = Stub::new(slot, self.edges);
        Some(stub)
    }
}

impl LevelPlan {
    /// Slot one stub per external endpoint where most of its edges' home routes leave the boundary.
    pub(super) fn side_stubs(
        &mut self,
        crossings_by_external_endpoint: BTreeMap<ElementPath, BoundaryCrossings>,
    ) {
        for (external_endpoint, crossings) in crossings_by_external_endpoint {
            let Some(stub) = crossings.into_stub() else {
                continue;
            };
            self.stubs.insert(external_endpoint, stub);
        }
    }
}

impl Port {
    /// Return where the ray from `inside` through `outside` leaves `rect`: on the side it reaches at the least t = (side − inside) / (outside − inside) > 0.
    pub(super) fn leaving(rect: Rect, inside: Point, outside: Point) -> Self {
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
                let is_reached_ahead = reach > 0.0 && reach.is_finite();
                is_reached_ahead.then_some((side, reach))
            })
            .min_by(|(_side, reach), (_other_side, other_reach)| reach.total_cmp(other_reach));
        let (side, reach) =
            first_reached.expect("a ray from inside a rectangle to a distinct point leaves it");
        let reached_point = inside + offset * reach;
        let side_line = side.line_along(rect);
        let point = Self::snapped_onto(side_line, reached_point);
        Self::new(side, point)
    }

    /// Return the point of `side_line` nearest `point`, so it lies exactly on the side.
    fn snapped_onto(side_line: Line, point: Point) -> Point {
        let nearest = side_line.nearest(point, 0.0);
        side_line.eval(nearest.t)
    }
}

#[cfg(test)]
mod tests {
    use kurbo::Size;

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
        let mut plan = LevelPlan::new(Size::new(200.0, 100.0));
        let external = ElementPath::from_segments(["far"]);
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
        let crossings_by_external_endpoint = BTreeMap::from([(external.clone(), crossings)]);

        plan.side_stubs(crossings_by_external_endpoint);

        let stub = &plan.stubs[&external];
        assert_eq!(stub.slot, Port::new(Side::East, Point::new(200.0, 35.0)));
        assert_eq!(stub.edges.len(), 3);
        assert!(stub.slot.lies_on_side_of(plan.boundary()));
    }
}
