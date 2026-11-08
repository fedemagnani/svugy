//! Shape connections as arcs over the scene or traces on the floor, and plate their labels.

use std::iter;
use std::ops::Range;

use glam::DVec3;
use kurbo::{BezPath, Line, ParamCurve, ParamCurveDeriv, PathSeg, Point, QuadBez};

use super::obstruction::Obstructions;
use super::projection::IsometricProjection;
use super::{ArrowheadMarker, PlatedLabel};
use crate::metrics::{LabelRole, TextMeasure};

/// Shapes one connection on the screen: the path its dot travels, the path it is stroked along and the positions its label plate may take.
pub(super) trait ConnectionShape {
    /// Return the path from port to port, which the animated dot travels.
    fn port_to_port_path(&self) -> BezPath;

    /// Return the path cut short by the arrowhead's overhang δ, so the tip lands on the target port.
    fn path_under_target_arrowhead(&self) -> BezPath;

    /// Return the positions a label plate may take along the path, most preferred first.
    fn plate_positions_preferred_first(&self) -> Vec<Point>;

    /// Return the position a label plate takes when no other position is offered.
    fn fallback_plate_position(&self) -> Point;
}

/// Arcs one route over the scene: a quadratic from its projected source port to its projected target port, peaking above their midpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LiftedArc {
    curve: QuadBez,
}

impl LiftedArc {
    /// Lift of the apex above the arc's higher end, in plan units, for the shortest arcs.
    const BASE_LIFT: f64 = 20.0;
    /// Extra lift per plan unit between the ports, so long arcs rise over what they cross.
    const LIFT_PER_SPAN: f64 = 0.2;
    /// Parameter of the apex, the arc's midpoint.
    const APEX_PARAMETER: f64 = 0.5;
    /// Screen distance between neighbouring plate positions along the arc.
    const SLIDE_STEP: f64 = 6.0;
    /// Parameters a plate may slide within, clear of the ports and the arrowhead.
    const SLIDE_SPAN: Range<f64> = 0.15..0.85;

    /// Lift an arc from `source_end` to `target_end`, plan points raised above the level's floor, through the apex at t = ½: C = 2·apex − (source + target)/2.
    pub(super) fn between(source_end: DVec3, target_end: DVec3) -> Self {
        let source_ground_point = Point::new(source_end.x, source_end.y);
        let target_ground_point = Point::new(target_end.x, target_end.y);
        let apex_ground_point = source_ground_point.midpoint(target_ground_point);
        let ground_span = (target_ground_point - source_ground_point).hypot2().sqrt();
        let higher_end_height = source_end.z.max(target_end.z);
        let apex_height = higher_end_height + Self::BASE_LIFT + Self::LIFT_PER_SPAN * ground_span;
        let apex_plan_point = DVec3::new(apex_ground_point.x, apex_ground_point.y, apex_height);
        let source = IsometricProjection::project(source_end);
        let apex = IsometricProjection::project(apex_plan_point);
        let target = IsometricProjection::project(target_end);
        let ends_midpoint = source.midpoint(target);
        let control = apex + (apex - ends_midpoint);
        let curve = QuadBez::new(source, control, target);
        Self { curve }
    }

    /// Return the path a bundled route's dot travels: from the true source port along `source_branch`, over this arc as the trunk, then along `target_branch` to the true target port.
    pub(super) fn port_to_port_path_via(
        &self,
        source_branch: Option<&FlatTrace>,
        target_branch: Option<&FlatTrace>,
    ) -> BezPath {
        let source_branch_path = source_branch.map(FlatTrace::port_to_port_path);
        let trunk_path = self.port_to_port_path();
        let trunk_piece = iter::once(trunk_path);
        let target_branch_path = target_branch.map(FlatTrace::port_to_port_path);
        let mut piece_paths = source_branch_path
            .into_iter()
            .chain(trunk_piece)
            .chain(target_branch_path);
        let mut dot_path = piece_paths.next().expect("the trunk is always a piece");
        for piece_path in piece_paths {
            let elements_after_move = piece_path.elements().iter().skip(1).copied();
            dot_path.extend(elements_after_move);
        }
        dot_path
    }

    /// Return the parameters a signed screen `step` apart from the apex while inside the slide span: tₖ₊₁ = tₖ + step/|B′(tₖ)|.
    fn parameters_stepping_from_apex(arc: QuadBez, step: f64) -> Vec<f64> {
        let velocity_curve = arc.deriv();
        let apex_parameter = Some(Self::APEX_PARAMETER);
        let parameters = iter::successors(apex_parameter, |&parameter| {
            let velocity = velocity_curve.eval(parameter);
            let speed = velocity.to_vec2().hypot2().sqrt();
            let next_parameter = parameter + step / speed;
            Some(next_parameter)
        });
        parameters
            .skip(1)
            .take_while(|parameter| Self::SLIDE_SPAN.contains(parameter))
            .collect()
    }

    /// Return `arc` as a one-segment path.
    fn path_along(arc: QuadBez) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(arc.p0);
        path.quad_to(arc.p1, arc.p2);
        path
    }
}

impl ConnectionShape for LiftedArc {
    fn port_to_port_path(&self) -> BezPath {
        Self::path_along(self.curve)
    }

    /// Return the arc cut short at t_end = 1 − δ/|B′(1)|, B′(1) = 2(P₂ − C).
    fn path_under_target_arrowhead(&self) -> BezPath {
        let arc = self.curve;
        let end_velocity = (arc.p2 - arc.p1) * 2.0;
        let end_speed = end_velocity.hypot2().sqrt();
        let is_too_short_to_trim = end_speed <= ArrowheadMarker::TIP_OVERHANG;
        if is_too_short_to_trim {
            return Self::path_along(arc);
        }
        let trimmed_end = 1.0 - ArrowheadMarker::TIP_OVERHANG / end_speed;
        let trimmed_arc = arc.subsegment(0.0..trimmed_end);
        Self::path_along(trimmed_arc)
    }

    /// Return the apex, then a slide step further toward the target and the source in turn, within the slide span.
    fn plate_positions_preferred_first(&self) -> Vec<Point> {
        let arc = self.curve;
        let toward_target = Self::parameters_stepping_from_apex(arc, Self::SLIDE_STEP);
        let toward_source = Self::parameters_stepping_from_apex(arc, -Self::SLIDE_STEP);
        let alternating_sides = alternate_target_and_source_sides(toward_target, toward_source);
        let apex_then_alternating_sides = iter::once(Self::APEX_PARAMETER).chain(alternating_sides);
        apex_then_alternating_sides
            .map(|parameter| arc.eval(parameter))
            .collect()
    }

    /// Return the arc's apex.
    fn fallback_plate_position(&self) -> Point {
        self.curve.eval(Self::APEX_PARAMETER)
    }
}

/// Traces one polyline flat on a floor: a flat route on its level's, from its source port through its bends to its target port, or a bundled route's branch on its container's.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct FlatTrace {
    /// Screen corners in flow order, from where the edge enters the trace to where it leaves; at least two.
    corners: Vec<Point>,
}

impl FlatTrace {
    /// Screen distance between neighbouring plate positions along the trace, the same as along an arc.
    const SLIDE_STEP: f64 = LiftedArc::SLIDE_STEP;
    /// Fractions of the trace's length a plate may slide within, clear of the ports and the arrowhead.
    const SLIDE_SPAN: Range<f64> = 0.15..0.85;

    /// Trace the polyline through `plan_corners`, in flow order, on the floor `floor_height` above the projected level's.
    pub(super) fn on_floor_through(
        floor_height: f64,
        plan_corners: impl IntoIterator<Item = Point>,
    ) -> Self {
        let floor = IsometricProjection::horizontal_plane_at(floor_height);
        let corners = plan_corners
            .into_iter()
            .map(|plan_corner| floor * plan_corner)
            .collect();
        Self { corners }
    }

    /// Return how far along the trace the midpoint of its longest segment lies, the earliest segment winning ties.
    fn distance_to_longest_segment_midpoint(&self) -> f64 {
        let mut distance_to_segment_start = 0.0;
        let mut longest_length = f64::NEG_INFINITY;
        let mut distance_to_longest_midpoint = 0.0;
        for segment in Self::segments_through(&self.corners) {
            let segment_length = Self::length_of(segment);
            let is_longer = segment_length > longest_length;
            if is_longer {
                longest_length = segment_length;
                distance_to_longest_midpoint = distance_to_segment_start + segment_length / 2.0;
            }
            distance_to_segment_start += segment_length;
        }
        distance_to_longest_midpoint
    }

    /// Return the distances a signed screen `step` apart from `start` while inside `slide_span`.
    fn distances_stepping_from(start: f64, step: f64, slide_span: &Range<f64>) -> Vec<f64> {
        let step_counts = 1_u32..;
        step_counts
            .map(|step_count| start + f64::from(step_count) * step)
            .take_while(|distance| slide_span.contains(distance))
            .collect()
    }

    /// Return the point `distance` along the trace from its source port, or its target port past the end.
    fn point_at_distance(&self, distance: f64) -> Point {
        let mut distance_to_segment_start = 0.0;
        for segment in Self::segments_through(&self.corners) {
            let segment_length = Self::length_of(segment);
            let distance_into_segment = distance - distance_to_segment_start;
            let is_within_segment = segment_length > 0.0 && distance_into_segment <= segment_length;
            if is_within_segment {
                let segment_parameter = distance_into_segment / segment_length;
                return segment.eval(segment_parameter);
            }
            distance_to_segment_start += segment_length;
        }
        *self
            .corners
            .last()
            .expect("a trace has at least two corners")
    }

    /// Return the trace's length, its segments' lengths summed.
    fn length(&self) -> f64 {
        let segments = Self::segments_through(&self.corners);
        segments.map(Self::length_of).sum()
    }

    /// Return the polyline through `corners` as a path.
    fn path_through(corners: &[Point]) -> BezPath {
        let segments = Self::segments_through(corners);
        let path_segments = segments.map(PathSeg::Line);
        BezPath::from_path_segments(path_segments)
    }

    /// Yield the segment between every two consecutive `corners`.
    fn segments_through(corners: &[Point]) -> impl Iterator<Item = Line> + '_ {
        let next_corners = corners.iter().skip(1);
        corners
            .iter()
            .zip(next_corners)
            .map(|(&start, &end)| Line::new(start, end))
    }

    /// Return `segment`'s length.
    fn length_of(segment: Line) -> f64 {
        let span = segment.p1 - segment.p0;
        span.hypot2().sqrt()
    }
}

impl ConnectionShape for FlatTrace {
    fn port_to_port_path(&self) -> BezPath {
        Self::path_through(&self.corners)
    }

    /// Return the trace with its last segment shortened by δ, or whole when that segment is no longer than δ.
    fn path_under_target_arrowhead(&self) -> BezPath {
        let [.., last_bend, target_port] = self.corners[..] else {
            return self.port_to_port_path();
        };
        let last_leg = target_port - last_bend;
        let last_leg_length = last_leg.hypot2().sqrt();
        let is_too_short_to_trim = last_leg_length <= ArrowheadMarker::TIP_OVERHANG;
        if is_too_short_to_trim {
            return self.port_to_port_path();
        }
        let overhang_along_last_leg = last_leg * (ArrowheadMarker::TIP_OVERHANG / last_leg_length);
        let trimmed_target = target_port - overhang_along_last_leg;
        let mut trimmed_corners = self.corners.clone();
        trimmed_corners.pop();
        trimmed_corners.push(trimmed_target);
        Self::path_through(&trimmed_corners)
    }

    /// Return the midpoint of the longest segment, then a slide step further toward the target and the source in turn, within the slide span.
    fn plate_positions_preferred_first(&self) -> Vec<Point> {
        let start = self.distance_to_longest_segment_midpoint();
        let trace_length = self.length();
        let slide_span_start = Self::SLIDE_SPAN.start * trace_length;
        let slide_span_end = Self::SLIDE_SPAN.end * trace_length;
        let slide_span = slide_span_start..slide_span_end;
        let toward_target = Self::distances_stepping_from(start, Self::SLIDE_STEP, &slide_span);
        let toward_source = Self::distances_stepping_from(start, -Self::SLIDE_STEP, &slide_span);
        let alternating_sides = alternate_target_and_source_sides(toward_target, toward_source);
        let midpoint_then_alternating_sides = iter::once(start).chain(alternating_sides);
        midpoint_then_alternating_sides
            .map(|distance| self.point_at_distance(distance))
            .collect()
    }

    /// Return the midpoint of the longest segment.
    fn fallback_plate_position(&self) -> Point {
        let distance_to_longest_midpoint = self.distance_to_longest_segment_midpoint();
        self.point_at_distance(distance_to_longest_midpoint)
    }
}

impl PlatedLabel {
    /// Place a connection label at the first plate position along `shape` whose plate overlaps the fewest `obstructions`, else at its fallback position, recording it among them.
    pub(super) fn place_least_obstructed_along(
        text: &str,
        shape: &impl ConnectionShape,
        obstructions: &mut Obstructions,
        measure: &impl TextMeasure,
    ) -> Self {
        let role = LabelRole::ConnectionLabel;
        let positions = shape.plate_positions_preferred_first();
        let candidates = positions
            .into_iter()
            .map(|position| Self::centered_on(text, role, position, measure));
        let fallback_position = shape.fallback_plate_position();
        let centered_on_fallback = || Self::centered_on(text, role, fallback_position, measure);
        obstructions.place_least_obstructed(candidates, centered_on_fallback)
    }
}

/// Interleave slide coordinates outward from a preferred one: the first toward the target, the first toward the source, the second toward the target, and so on.
fn alternate_target_and_source_sides(
    toward_target: Vec<f64>,
    toward_source: Vec<f64>,
) -> impl Iterator<Item = f64> {
    let toward_target_count = toward_target.len();
    let toward_source_count = toward_source.len();
    let step_count = toward_target_count.max(toward_source_count);
    (0..step_count).flat_map(move |step_index| {
        let target_side = toward_target.get(step_index).copied();
        let source_side = toward_source.get(step_index).copied();
        target_side.into_iter().chain(source_side)
    })
}

#[cfg(test)]
mod tests {
    use kurbo::{ParamCurve, PathSeg};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::metrics::TextAnchor;

    fn straight_arc_300_long_on_the_floor() -> LiftedArc {
        let source_end = DVec3::new(0.0, 0.0, 0.0);
        let target_end = DVec3::new(300.0, 0.0, 0.0);
        LiftedArc::between(source_end, target_end)
    }

    fn only_quad(path: &BezPath) -> QuadBez {
        let segments: Vec<PathSeg> = path.segments().collect();
        let [PathSeg::Quad(quad)] = segments[..] else {
            panic!("arc is one quadratic: {segments:?}");
        };
        quad
    }

    #[test]
    fn dot_path_runs_from_projected_port_to_projected_port() {
        let source_end = DVec3::new(80.0, 50.0, 22.5);
        let target_end = DVec3::new(140.0, 70.0, 10.5);
        let arc = LiftedArc::between(source_end, target_end);
        let dot_arc = only_quad(&arc.port_to_port_path());
        let projected_source = IsometricProjection::project(DVec3::new(80.0, 50.0, 22.5));
        let projected_target = IsometricProjection::project(DVec3::new(140.0, 70.0, 10.5));
        assert_eq!(dot_arc.p0, projected_source);
        assert_eq!(dot_arc.p2, projected_target);
    }

    #[test]
    fn stroked_path_stops_where_the_arrowhead_tip_lands_on_the_target_port() {
        let source_end = DVec3::new(0.0, 0.0, 22.5);
        let target_end = DVec3::new(200.0, 40.0, 22.5);
        let arc = LiftedArc::between(source_end, target_end);
        let stroked = only_quad(&arc.path_under_target_arrowhead());
        let dot_arc = only_quad(&arc.port_to_port_path());
        let end_tangent = (stroked.p2 - stroked.p1).normalize();
        let tip = stroked.p2 + end_tangent * ArrowheadMarker::TIP_OVERHANG;
        assert_eq!(stroked.p0, dot_arc.p0);
        assert!((tip - dot_arc.p2).hypot2().sqrt() < 0.01);
    }

    #[test]
    fn apex_is_the_arc_midpoint_above_the_ports_midpoint() {
        let source_end = DVec3::new(80.0, 50.0, 22.5);
        let target_end = DVec3::new(140.0, 70.0, 22.5);
        let arc = LiftedArc::between(source_end, target_end);
        let dot_arc = only_quad(&arc.port_to_port_path());
        let midpoint = dot_arc.eval(0.5);
        let port_span = (Point::new(140.0, 70.0) - Point::new(80.0, 50.0))
            .hypot2()
            .sqrt();
        let apex_height = 22.5 + LiftedArc::BASE_LIFT + LiftedArc::LIFT_PER_SPAN * port_span;
        let apex = IsometricProjection::project(DVec3::new(110.0, 60.0, apex_height));
        assert!((midpoint - apex).hypot2().sqrt() < 1e-9);
        let ground_anchor = IsometricProjection::project(DVec3::new(110.0, 60.0, 0.0));
        assert!((apex.x - ground_anchor.x).abs() < 1e-9);
        assert!(apex.y < dot_arc.p0.y);
        assert!(apex.y < dot_arc.p2.y);
    }

    /// Return how far `arc`'s apex stands above its ports' midpoint on the screen.
    fn rise_of(arc: LiftedArc) -> f64 {
        let ends_midpoint = arc.curve.p0.midpoint(arc.curve.p2);
        let apex = arc.curve.eval(LiftedArc::APEX_PARAMETER);
        ends_midpoint.y - apex.y
    }

    #[test]
    fn longer_arcs_rise_higher() {
        let source_end = DVec3::new(0.0, 0.0, 0.0);
        let short_target_end = DVec3::new(50.0, 0.0, 0.0);
        let long_target_end = DVec3::new(500.0, 0.0, 0.0);
        let short_arc = LiftedArc::between(source_end, short_target_end);
        let long_arc = LiftedArc::between(source_end, long_target_end);
        let short_rise = rise_of(short_arc);
        let long_rise = rise_of(long_arc);
        assert!(long_rise > short_rise);
    }

    #[test]
    fn plate_wraps_the_measured_label_around_the_apex() {
        let measure = FixedAdvanceMeasure;
        let center = Point::new(10.0, 20.0);
        let plated =
            PlatedLabel::centered_on("markets", LabelRole::ConnectionLabel, center, &measure);
        let spec = LabelRole::ConnectionLabel.text_spec("markets");
        let text_size = measure.measure(&spec);
        assert_eq!(plated.plate.center(), center);
        assert!(plated.plate.width() > text_size.width);
        assert!(plated.plate.height() > text_size.line_height);
        let pill_radius = plated.plate.height() / 2.0;
        assert!((plated.plate.radii().top_left - pill_radius).abs() < 1e-9);
        assert_eq!(plated.label.anchor, TextAnchor::Middle);
        assert_eq!(plated.label.plane, kurbo::Affine::IDENTITY);
    }

    #[test]
    fn plate_positions_start_at_the_apex_and_alternate_a_slide_step_apart_toward_each_port() {
        let arc = straight_arc_300_long_on_the_floor();
        let positions = arc.plate_positions_preferred_first();
        let apex = arc.curve.eval(LiftedArc::APEX_PARAMETER);
        let span_start = arc.curve.eval(LiftedArc::SLIDE_SPAN.start);
        let span_end = arc.curve.eval(LiftedArc::SLIDE_SPAN.end);
        assert_eq!(positions[0], apex);
        assert!(positions[1].x > apex.x);
        assert!(positions[2].x < apex.x);
        let first_target_step = (positions[1] - positions[0]).hypot2().sqrt();
        let first_source_step = (positions[2] - positions[0]).hypot2().sqrt();
        assert!((first_target_step - LiftedArc::SLIDE_STEP).abs() < 0.1);
        assert!((first_source_step - LiftedArc::SLIDE_STEP).abs() < 0.1);
        for position in &positions {
            let is_within_span = (span_start.x..=span_end.x).contains(&position.x);
            assert!(is_within_span, "{position:?} leaves the slide span");
        }
    }

    #[test]
    fn plate_stays_at_the_apex_unless_obstructed_there() {
        let measure = FixedAdvanceMeasure;
        let arc = straight_arc_300_long_on_the_floor();
        let apex = arc.curve.eval(LiftedArc::APEX_PARAMETER);
        let mut obstructions = Obstructions::around_text_of(&[]);
        let free =
            PlatedLabel::place_least_obstructed_along("calls", &arc, &mut obstructions, &measure);
        let free_center = free.plate.center();
        assert_eq!(free_center, apex);
        let mut free_only = Obstructions::around_text_of(&[]);
        free_only.record(&free);
        let slid =
            PlatedLabel::place_least_obstructed_along("calls", &arc, &mut obstructions, &measure);
        let slid_center = slid.plate.center();
        let slid_obstruction_count = free_only.count_overlapped_by(&slid);
        let is_slid_off_the_apex = slid_center != apex;
        assert!(is_slid_off_the_apex);
        assert_eq!(slid_obstruction_count, 0);
    }
}
