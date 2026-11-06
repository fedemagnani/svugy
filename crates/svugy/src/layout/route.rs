//! Anchor a level's routes: a port on each footprint end, on the side facing the far end.

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Line, ParamCurve, Point, Rect, Vec2};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use super::{Port, Route, Side};

/// Names the end of an edge a port belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum EdgeEnd {
    Source,
    Target,
}

/// Gathers the ends of one level's routes, faced toward footprint sides, to join them into routes.
pub(super) struct RouteEnds<'a> {
    facing_ends_by_footprint_side: BTreeMap<(&'a ElementPath, Side), FacingEnds<'a>>,
}

/// Holds the ends attaching to one side of one footprint, each with the far point it faces.
struct FacingEnds<'a> {
    footprint: Rect,
    far_points: Vec<((&'a EdgeId, EdgeEnd), Point)>,
}

impl EdgeEnd {
    pub(super) fn endpoint_of(self, edge: &EdgeId) -> &ElementPath {
        match self {
            Self::Source => edge.from(),
            Self::Target => edge.to(),
        }
    }

    pub(super) fn opposite(self) -> Self {
        match self {
            Self::Source => Self::Target,
            Self::Target => Self::Source,
        }
    }

    pub(super) fn port_of(self, route: &Route) -> Port {
        match self {
            Self::Source => route.source,
            Self::Target => route.target,
        }
    }
}

impl<'a> RouteEnds<'a> {
    pub(super) fn new() -> Self {
        Self {
            facing_ends_by_footprint_side: BTreeMap::new(),
        }
    }

    /// Face both ends of `edge` toward each other, each on the side of its own footprint.
    pub(super) fn face_each_other(
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
            .or_insert_with(|| FacingEnds {
                footprint,
                far_points: Vec::new(),
            });
        facing_ends.far_points.push(((edge, end), far_point));
    }

    /// Spread the faced ends sharing a footprint side, then join both ends of every edge into a route.
    pub(super) fn into_routes(self) -> BTreeMap<EdgeId, Route> {
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
                let route = Route::new(source, target);
                (edge_id.clone(), route)
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
    pub(super) fn spread_ports_along<K: Ord>(
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn footprints_of(named_footprints: &[(&str, Rect)]) -> BTreeMap<ElementPath, Rect> {
        named_footprints
            .iter()
            .map(|&(name, footprint)| (ElementPath::from_segments([name]), footprint))
            .collect()
    }

    fn edge_between(from: &str, to: &str) -> EdgeId {
        let from_path = ElementPath::from_segments([from]);
        let to_path = ElementPath::from_segments([to]);
        EdgeId::new(from_path, to_path, 0)
    }

    fn route_between_footprints(
        footprints: &BTreeMap<ElementPath, Rect>,
        edge_ids: &[EdgeId],
    ) -> BTreeMap<EdgeId, Route> {
        let mut route_ends = RouteEnds::new();
        for edge_id in edge_ids {
            let source_footprint = footprints[edge_id.from()];
            let target_footprint = footprints[edge_id.to()];
            route_ends.face_each_other(edge_id, source_footprint, target_footprint);
        }
        route_ends.into_routes()
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
