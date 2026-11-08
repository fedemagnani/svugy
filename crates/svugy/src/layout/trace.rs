//! Find clear orthogonal traces across a level's floor: from a source port to a target port, around obstacles and earlier traces.

use kurbo::{Point, Rect, Vec2};

use super::Port;

/// Traces one axis-aligned polyline in plan space from a source port to a target port, turning at most twice.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct OrthogonalTrace {
    points: Vec<Point>,
}

impl OrthogonalTrace {
    /// Trace the axis-aligned polyline through `points`, ports included.
    pub(super) fn through(points: Vec<Point>) -> Self {
        Self { points }
    }

    /// Find the first trace from `source` to `target` that leaves and enters perpendicular to their sides, enters no obstacle and touches no `earlier_traces`, trying channels nearest midway first.
    pub(super) fn find_clear(
        source: Port,
        target: Port,
        obstacles: &[Rect],
        earlier_traces: &[Self],
    ) -> Option<Self> {
        let channels = Self::channels_between(source, target, obstacles);
        channels
            .into_iter()
            .map(|channel| Self::through_channel(source, target, channel))
            .filter(|trace| trace.is_perpendicular_to_port_sides(source, target))
            .find(|trace| trace.is_clear_of(obstacles, earlier_traces))
    }

    /// Return the points the trace turns at, ports left out.
    pub(super) fn bends(&self) -> Vec<Point> {
        let last_index = self.points.len() - 1;
        self.points[1..last_index].to_vec()
    }

    /// List the coordinates, along the axis leaving `source`'s side, of the channels a trace may cross over in: the quarter points between the ports, the target itself for a single-bend trace into a side across that axis, and every obstacle edge, nearest midway first.
    fn channels_between(source: Port, target: Port, obstacles: &[Rect]) -> Vec<f64> {
        let leaving_axis = Self::axis_of(source);
        let source_coordinate = source.point.to_vec2().dot(leaving_axis);
        let target_coordinate = target.point.to_vec2().dot(leaving_axis);
        let port_gap = target_coordinate - source_coordinate;
        let midway = source_coordinate + port_gap / 2.0;
        let port_channels =
            [0.25, 0.5, 0.75, 1.0].map(|fraction| source_coordinate + port_gap * fraction);
        let obstacle_edges = obstacles.iter().flat_map(|obstacle| {
            let near_corner = obstacle.origin().to_vec2();
            let far_corner = Vec2::new(obstacle.x1, obstacle.y1);
            let near_edge = near_corner.dot(leaving_axis);
            let far_edge = far_corner.dot(leaving_axis);
            [near_edge, far_edge]
        });
        let mut channels: Vec<f64> = port_channels.into_iter().chain(obstacle_edges).collect();
        channels.sort_by(|channel, other_channel| {
            let distance = (channel - midway).abs();
            let other_distance = (other_channel - midway).abs();
            distance
                .total_cmp(&other_distance)
                .then_with(|| channel.total_cmp(other_channel))
        });
        channels.dedup();
        channels
    }

    /// Trace `source` → across to `channel` → along it → across to `target`, dropping the points that do not turn.
    fn through_channel(source: Port, target: Port, channel: f64) -> Self {
        let leaving_axis = Self::axis_of(source);
        let channel_axis = Vec2::new(leaving_axis.y, leaving_axis.x);
        let channel_offset = leaving_axis * channel;
        let source_lane = source.point.to_vec2().dot(channel_axis);
        let target_lane = target.point.to_vec2().dot(channel_axis);
        let source_side_offset = channel_axis * source_lane;
        let target_side_offset = channel_axis * target_lane;
        let source_side_bend_offset = channel_offset + source_side_offset;
        let target_side_bend_offset = channel_offset + target_side_offset;
        let source_side_bend = source_side_bend_offset.to_point();
        let target_side_bend = target_side_bend_offset.to_point();
        let mut points: Vec<Point> = Vec::new();
        for point in [
            source.point,
            source_side_bend,
            target_side_bend,
            target.point,
        ] {
            let is_repeated = points.last() == Some(&point);
            if is_repeated {
                continue;
            }
            if let [.., before_last, last] = points[..] {
                let last_step = last - before_last;
                let next_step = point - last;
                let is_straight_on = last_step.cross(next_step) == 0.0;
                if is_straight_on {
                    points.pop();
                }
            }
            points.push(point);
        }
        Self { points }
    }

    /// Tell whether the trace leaves `source` straight out of its side and enters `target` straight into its side.
    fn is_perpendicular_to_port_sides(&self, source: Port, target: Port) -> bool {
        let [first, second, ..] = self.points[..] else {
            return false;
        };
        let [.., second_last, last] = self.points[..] else {
            return false;
        };
        let leaving_step = second - first;
        let entering_step = last - second_last;
        let outward_of_source = -source.side.inward_normal();
        let inward_of_target = target.side.inward_normal();
        let is_leaving_outward = Self::runs_along(leaving_step, outward_of_source);
        let is_entering_inward = Self::runs_along(entering_step, inward_of_target);
        is_leaving_outward && is_entering_inward
    }

    /// Tell whether no segment enters the interior of any obstacle and none touches a segment of any `earlier_traces`.
    fn is_clear_of(&self, obstacles: &[Rect], earlier_traces: &[Self]) -> bool {
        let segment_bounds: Vec<Rect> = self.segment_bounds().collect();
        let enters_obstacle = segment_bounds.iter().any(|&segment| {
            obstacles
                .iter()
                .any(|&obstacle| Self::enters_interior(segment, obstacle))
        });
        if enters_obstacle {
            return false;
        }
        let touches_earlier_trace =
            earlier_traces
                .iter()
                .flat_map(Self::segment_bounds)
                .any(|earlier_segment| {
                    segment_bounds
                        .iter()
                        .any(|segment| segment.overlaps(earlier_segment))
                });
        !touches_earlier_trace
    }

    /// Return the bounds of every segment, each degenerate to a line since segments are axis-aligned.
    fn segment_bounds(&self) -> impl Iterator<Item = Rect> + '_ {
        self.points
            .windows(2)
            .map(|segment_ends| Rect::from_points(segment_ends[0], segment_ends[1]))
    }

    /// Return the unit axis `port` leaves its side along, as (1, 0) or (0, 1).
    fn axis_of(port: Port) -> Vec2 {
        let normal = port.side.inward_normal();
        Vec2::new(normal.x.abs(), normal.y.abs())
    }

    /// Tell whether `step` runs in the same direction as `direction`.
    fn runs_along(step: Vec2, direction: Vec2) -> bool {
        let is_parallel = step.cross(direction) == 0.0;
        let is_forward = step.dot(direction) > 0.0;
        is_parallel && is_forward
    }

    /// Tell whether `segment` reaches into the open interior of `obstacle`; grazing its boundary does not count.
    fn enters_interior(segment: Rect, obstacle: Rect) -> bool {
        let is_within_x_span = segment.x0 < obstacle.x1 && segment.x1 > obstacle.x0;
        let is_within_y_span = segment.y0 < obstacle.y1 && segment.y1 > obstacle.y0;
        is_within_x_span && is_within_y_span
    }
}
