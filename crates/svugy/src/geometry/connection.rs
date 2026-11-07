//! Arc connections over the scene and plate their labels.

use std::iter;
use std::ops::Range;

use glam::DVec3;
use kurbo::{BezPath, ParamCurve, ParamCurveDeriv, Point, QuadBez};

use super::obstruction::Obstructions;
use super::projection::IsometricProjection;
use super::{ArrowheadMarker, PlatedLabel};
use crate::layout::Route;
use crate::metrics::{LabelRole, TextMeasure};

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

    /// Lift `route` from its ports, which sit `source_height` and `target_height` above the level's floor, through the apex at t = ½: C = 2·apex − (source + target)/2.
    pub(super) fn over_route(route: &Route, source_height: f64, target_height: f64) -> Self {
        let source_port = route.source.point;
        let target_port = route.target.point;
        let apex_ground_point = source_port.midpoint(target_port);
        let port_span = (target_port - source_port).hypot2().sqrt();
        let higher_end_height = source_height.max(target_height);
        let apex_height = higher_end_height + Self::BASE_LIFT + Self::LIFT_PER_SPAN * port_span;
        let source_plan_point = DVec3::new(source_port.x, source_port.y, source_height);
        let target_plan_point = DVec3::new(target_port.x, target_port.y, target_height);
        let apex_plan_point = DVec3::new(apex_ground_point.x, apex_ground_point.y, apex_height);
        let source = IsometricProjection::project(source_plan_point);
        let apex = IsometricProjection::project(apex_plan_point);
        let target = IsometricProjection::project(target_plan_point);
        let ends_midpoint = source.midpoint(target);
        let control = apex + (apex - ends_midpoint);
        let curve = QuadBez::new(source, control, target);
        Self { curve }
    }

    /// Return the arc from port to port, which the animated dot travels.
    pub(super) fn port_to_port_path(&self) -> BezPath {
        Self::path_along(self.curve)
    }

    /// Return the arc cut short by the arrowhead's overhang at t_end = 1 − δ/|B′(1)|, B′(1) = 2(P₂ − C), so the tip lands on the target port.
    pub(super) fn path_under_target_arrowhead(&self) -> BezPath {
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

    /// Return the arc's apex, its point at the apex parameter.
    fn apex(&self) -> Point {
        self.curve.eval(Self::APEX_PARAMETER)
    }

    /// Return the plate positions along the arc: the apex, then a slide step further toward the target and the source in turn, within the slide span.
    fn plate_positions_sliding_from_apex(&self) -> Vec<Point> {
        let arc = self.curve;
        let toward_target = Self::parameters_stepping_from_apex(arc, Self::SLIDE_STEP);
        let toward_source = Self::parameters_stepping_from_apex(arc, -Self::SLIDE_STEP);
        let toward_target_count = toward_target.len();
        let toward_source_count = toward_source.len();
        let step_count = toward_target_count.max(toward_source_count);
        let alternating_sides = (0..step_count).flat_map(|step_index| {
            let target_side = toward_target.get(step_index).copied();
            let source_side = toward_source.get(step_index).copied();
            target_side.into_iter().chain(source_side)
        });
        let apex_then_alternating_sides = iter::once(Self::APEX_PARAMETER).chain(alternating_sides);
        apex_then_alternating_sides
            .map(|parameter| arc.eval(parameter))
            .collect()
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

impl PlatedLabel {
    /// Place a connection label at the first plate position along `arc` whose plate overlaps the fewest `obstructions`, else at its apex, recording it among them.
    pub(super) fn place_least_obstructed_along(
        text: &str,
        arc: &LiftedArc,
        obstructions: &mut Obstructions,
        measure: &impl TextMeasure,
    ) -> Self {
        let role = LabelRole::ConnectionLabel;
        let positions = arc.plate_positions_sliding_from_apex();
        let candidates = positions
            .into_iter()
            .map(|position| Self::centered_on(text, role, position, measure));
        let apex = arc.apex();
        let centered_on_apex = || Self::centered_on(text, role, apex, measure);
        obstructions.place_least_obstructed(candidates, centered_on_apex)
    }
}

#[cfg(test)]
mod tests {
    use kurbo::{ParamCurve, PathSeg};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::layout::{Port, Side};
    use crate::metrics::TextAnchor;

    fn route_between(source: Point, target: Point) -> Route {
        let source_port = Port::new(Side::East, source);
        let target_port = Port::new(Side::West, target);
        Route::new(source_port, target_port)
    }

    fn straight_route_300_long() -> Route {
        let source = Point::new(0.0, 0.0);
        let target = Point::new(300.0, 0.0);
        route_between(source, target)
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
        let route = route_between(Point::new(80.0, 50.0), Point::new(140.0, 70.0));
        let arc = LiftedArc::over_route(&route, 22.5, 10.5);
        let dot_arc = only_quad(&arc.port_to_port_path());
        let projected_source = IsometricProjection::project(DVec3::new(80.0, 50.0, 22.5));
        let projected_target = IsometricProjection::project(DVec3::new(140.0, 70.0, 10.5));
        assert_eq!(dot_arc.p0, projected_source);
        assert_eq!(dot_arc.p2, projected_target);
    }

    #[test]
    fn stroked_path_stops_where_the_arrowhead_tip_lands_on_the_target_port() {
        let route = route_between(Point::new(0.0, 0.0), Point::new(200.0, 40.0));
        let arc = LiftedArc::over_route(&route, 22.5, 22.5);
        let stroked = only_quad(&arc.path_under_target_arrowhead());
        let dot_arc = only_quad(&arc.port_to_port_path());
        let end_tangent = (stroked.p2 - stroked.p1).normalize();
        let tip = stroked.p2 + end_tangent * ArrowheadMarker::TIP_OVERHANG;
        assert_eq!(stroked.p0, dot_arc.p0);
        assert!((tip - dot_arc.p2).hypot2().sqrt() < 0.01);
    }

    #[test]
    fn apex_is_the_arc_midpoint_above_the_ports_midpoint() {
        let route = route_between(Point::new(80.0, 50.0), Point::new(140.0, 70.0));
        let arc = LiftedArc::over_route(&route, 22.5, 22.5);
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
        let short_route = route_between(Point::new(0.0, 0.0), Point::new(50.0, 0.0));
        let long_route = route_between(Point::new(0.0, 0.0), Point::new(500.0, 0.0));
        let short_arc = LiftedArc::over_route(&short_route, 0.0, 0.0);
        let long_arc = LiftedArc::over_route(&long_route, 0.0, 0.0);
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
        let route = straight_route_300_long();
        let arc = LiftedArc::over_route(&route, 0.0, 0.0);
        let positions = arc.plate_positions_sliding_from_apex();
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
        let route = straight_route_300_long();
        let arc = LiftedArc::over_route(&route, 0.0, 0.0);
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
