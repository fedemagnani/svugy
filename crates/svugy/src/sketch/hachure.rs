use std::cmp::Ordering;

use kurbo::{Affine, Line, Point};

/// Spaces parallel hachure lines across a polygon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hachure {
    /// Direction of the lines, in degrees counter-clockwise from the x axis.
    angle_degrees: f64,
    /// Distance between neighbouring lines.
    gap: f64,
}

impl Hachure {
    /// Smallest gap honoured, so a near-zero gap cannot flood the fill with lines.
    const MIN_GAP: f64 = 0.1;
    /// Shortest hachure line kept, so a scanline grazing a vertex draws no dot.
    const MIN_LINE_LENGTH: f64 = 1.0;

    /// Hachure at `angle_degrees` with lines `gap` apart.
    pub(crate) fn new(angle_degrees: f64, gap: f64) -> Self {
        Self { angle_degrees, gap }
    }

    /// Clip the hachure lines to the inside of `polygon`, an implicitly closed vertex ring.
    pub(crate) fn lines_inside(&self, polygon: &[Point]) -> Vec<Line> {
        let scan_angle_degrees = self.angle_degrees + 90.0;
        let to_scan_space = Self::rotation_by_degrees(scan_angle_degrees);
        let from_scan_space = Self::rotation_by_degrees(-scan_angle_degrees);
        let scan_space_polygon: Vec<Point> = polygon
            .iter()
            .map(|&vertex| to_scan_space * vertex)
            .collect();
        let gap = self.gap.max(Self::MIN_GAP);
        let scan_space_lines = Self::horizontal_lines_inside(&scan_space_polygon, gap);
        scan_space_lines
            .into_iter()
            .map(|line| from_scan_space * line)
            .collect()
    }

    /// Rotate counter-clockwise by `degrees`: [cos θ, sin θ, −sin θ, cos θ] with θ in radians.
    fn rotation_by_degrees(degrees: f64) -> Affine {
        let radians = degrees.to_radians();
        let cos = libm::cos(radians);
        let sin = libm::sin(radians);
        Affine::new([cos, sin, -sin, cos, 0.0, 0.0])
    }

    /// Clip horizontal lines `gap` apart to the inside of `polygon`, pairing edge crossings by even-odd scanline.
    fn horizontal_lines_inside(polygon: &[Point], gap: f64) -> Vec<Line> {
        let mut pending_edges = ScanEdge::sorted_edges_of(polygon);
        let Some(first_edge) = pending_edges.first() else {
            return Vec::new();
        };
        let mut scan_y = first_edge.y_min;
        let mut active_edges: Vec<ScanEdge> = Vec::new();
        let mut lines = Vec::new();
        loop {
            let reached_count = pending_edges.partition_point(|edge| edge.y_min <= scan_y);
            let reached_edges = pending_edges.drain(..reached_count);
            active_edges.extend(reached_edges);
            active_edges.retain(|edge| edge.y_max > scan_y);
            active_edges.sort_by(|left, right| left.x.total_cmp(&right.x));
            let (crossing_pairs, _unpaired_crossing) = active_edges.as_chunks::<2>();
            for [entry_edge, exit_edge] in crossing_pairs {
                let crossing_width = exit_edge.x - entry_edge.x;
                let is_shorter_than_the_pen = crossing_width < Self::MIN_LINE_LENGTH;
                if is_shorter_than_the_pen {
                    continue;
                }
                let entry = Point::new(entry_edge.x, scan_y);
                let exit = Point::new(exit_edge.x, scan_y);
                lines.push(Line::new(entry, exit));
            }
            scan_y += gap;
            for edge in &mut active_edges {
                edge.x += gap * edge.inverse_slope;
            }
            let is_scan_done = pending_edges.is_empty() && active_edges.is_empty();
            if is_scan_done {
                return lines;
            }
        }
    }
}

/// Tracks where one non-horizontal polygon edge crosses the current scanline.
#[derive(Clone, Copy, Debug)]
struct ScanEdge {
    y_min: f64,
    y_max: f64,
    /// Crossing at the current scanline, starting at the `y_min` end.
    x: f64,
    /// dx/dy, the crossing's shift per unit of scanline advance.
    inverse_slope: f64,
}

impl ScanEdge {
    /// Collect the non-horizontal edges of `polygon` in scan order.
    fn sorted_edges_of(polygon: &[Point]) -> Vec<Self> {
        let Some(&first_vertex) = polygon.first() else {
            return Vec::new();
        };
        let closing_vertex = std::iter::once(first_vertex);
        let ring: Vec<Point> = polygon.iter().copied().chain(closing_vertex).collect();
        let mut edges: Vec<Self> = ring
            .windows(2)
            .filter_map(|edge_ends| Self::spanning(edge_ends[0], edge_ends[1]))
            .collect();
        edges.sort_by(Self::scan_order);
        edges
    }

    /// Edge from `start` to `end`, or `None` when it is horizontal and no scanline crosses it.
    fn spanning(start: Point, end: Point) -> Option<Self> {
        let is_horizontal = start.y == end.y;
        if is_horizontal {
            return None;
        }
        let lower_end = if start.y < end.y { start } else { end };
        let y_max = start.y.max(end.y);
        let inverse_slope = (end.x - start.x) / (end.y - start.y);
        Some(Self {
            y_min: lower_end.y,
            y_max,
            x: lower_end.x,
            inverse_slope,
        })
    }

    /// Order edges by `y_min`, then `x`, then `y_max`.
    fn scan_order(left: &Self, right: &Self) -> Ordering {
        let by_y_min = left.y_min.total_cmp(&right.y_min);
        let by_x = left.x.total_cmp(&right.x);
        let by_y_max = left.y_max.total_cmp(&right.y_max);
        by_y_min.then(by_x).then(by_y_max)
    }
}
