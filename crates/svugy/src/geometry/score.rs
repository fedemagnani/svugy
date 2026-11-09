//! Score a projected scene's readability: the crossings, occlusions and ink its connections cost, measured on screen.

use std::collections::BTreeSet;

use glam::DVec3;
use kurbo::{Affine, BezPath, ParamCurve, PathSeg, Point, Rect, Vec2};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use super::connection::PlanCourse;
use super::projection::IsometricProjection;
use super::{Connection, FaceShade, Group, Scene, Solid};

/// Counts what a projected scene costs its reader: hard violations, crossings, hidden junctions, arc lengths over solids, hidden trace length, arcs, bends and total connection length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SceneScore {
    /// Arcs running through a solid plus arrowheads hidden behind a solid outside the endpoint they point at; a readable scene has none.
    pub(crate) violations: usize,
    /// Pairs of edges whose connections cross on screen.
    pub(crate) crossings: usize,
    /// Bundle slots and trace bends hidden behind a solid, plus arrowheads hidden only by the children of the container they point at.
    pub(crate) hidden_junctions: usize,
    /// Length of arcs overlapping a solid on screen, in front of it or behind it, its own endpoints and enclosing containers exempt.
    pub(crate) arc_length_over_solids: QuantizedLength,
    /// Length of flat traces hidden behind or under a solid other than a child of their endpoints.
    pub(crate) hidden_trace_length: QuantizedLength,
    /// Arcs drawn: one per trunk or lone arc.
    pub(crate) arcs: usize,
    /// Turns along every flat trace and branch.
    pub(crate) bends: usize,
    /// Length of every drawn connection, a shared trunk once.
    pub(crate) total_length: QuantizedLength,
}

/// Holds a screen length in thousandths of a user unit, so comparing two scores never hinges on floating-point noise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct QuantizedLength(i64);

impl QuantizedLength {
    /// Thousandths of a user unit per user unit.
    const STEPS_PER_UNIT: f64 = 1000.0;

    /// Quantize `length` user units to the nearest thousandth.
    pub(crate) fn of(length: f64) -> Self {
        let steps = length * Self::STEPS_PER_UNIT;
        let rounded_steps = steps.round();
        Self(rounded_steps as i64)
    }

    /// Return this length in user units.
    pub(crate) fn units(self) -> f64 {
        let steps = self.0 as f64;
        steps / Self::STEPS_PER_UNIT
    }
}

impl Scene {
    /// Score this scene's readability as its reader sees it on screen.
    pub(crate) fn score(&self) -> SceneScore {
        let plan_scene = PlanScene::of(&self.root);
        plan_scene.score()
    }
}

/// Lifts a scene's solids and drawn connections into document plan, the one frame where a sight line tells what stands in front of what.
struct PlanScene<'s> {
    prisms: Vec<SolidPrism<'s>>,
    /// Arcs drawn, a trunk several bundled edges draw lifted once.
    arcs: Vec<DrawnCourse<'s>>,
    /// Traces drawn, a branch several bundled edges draw lifted once.
    traces: Vec<DrawnCourse<'s>>,
}

impl<'s> PlanScene<'s> {
    /// Lift every level under `root`, the root's floor at the plan origin.
    fn of(root: &'s Group) -> Self {
        let mut plan_scene = Self {
            prisms: Vec::new(),
            arcs: Vec::new(),
            traces: Vec::new(),
        };
        plan_scene.lift_level(root, DVec3::ZERO);
        plan_scene
    }

    /// Score the lifted scene: crossings, hidden traces, junctions and arrowheads, arcs over and through solids, then arcs, bends and length drawn.
    fn score(&self) -> SceneScore {
        let crossings = self.count_crossing_edge_pairs();
        let arc_edges: BTreeSet<&EdgeId> = self
            .arcs
            .iter()
            .flat_map(|arc| arc.edges.iter().copied())
            .collect();
        let mut hidden_trace_length = 0.0;
        let mut hidden_junctions = 0;
        let mut bends = 0;
        for trace in &self.traces {
            let obstructing_prisms: Vec<&SolidPrism> = self
                .obstructing_prisms_of(trace)
                .filter(|prism| !trace.has_endpoint_enclosing(prism))
                .collect();
            let depth_runs = Self::depth_runs_along(trace, &obstructing_prisms);
            let hiding_depths = [SightDepth::SolidInFront, SightDepth::InsideSolid];
            hidden_trace_length += trace.screen_length_where(&depth_runs, &hiding_depths);
            let is_branch = trace.edges.iter().any(|edge| arc_edges.contains(edge));
            let branch_slot = is_branch.then(|| trace.slot_end());
            let bend_points = trace.bend_points();
            let junctions = bend_points.chain(branch_slot);
            let hidden_junction_count = junctions
                .filter(|&junction| self.is_hidden(trace, junction))
                .count();
            hidden_junctions += hidden_junction_count;
            bends += trace.bend_count();
        }
        let mut arc_length_over_solids = 0.0;
        let mut arcs_through_solids = 0;
        for arc in &self.arcs {
            let obstructing_prisms: Vec<&SolidPrism> = self.obstructing_prisms_of(arc).collect();
            let depth_runs = Self::depth_runs_along(arc, &obstructing_prisms);
            let overlapping_depths = [SightDepth::SolidInFront, SightDepth::SolidBehind];
            arc_length_over_solids += arc.screen_length_where(&depth_runs, &overlapping_depths);
            let inside_depths = [SightDepth::InsideSolid];
            let length_inside_solids = arc.screen_length_where(&depth_runs, &inside_depths);
            let is_through_a_solid = length_inside_solids > 0.0;
            if is_through_a_solid {
                arcs_through_solids += 1;
            }
        }
        let courses = self.arcs.iter().chain(&self.traces);
        let tip_hiders: Vec<TipHider> = courses
            .clone()
            .filter_map(|course| self.arrowhead_hider_of(course))
            .collect();
        let arrowheads_hidden_by_other_solids = tip_hiders
            .iter()
            .filter(|&&tip_hider| tip_hider == TipHider::OtherSolid)
            .count();
        let arrowheads_hidden_inside_endpoints =
            tip_hiders.len() - arrowheads_hidden_by_other_solids;
        hidden_junctions += arrowheads_hidden_inside_endpoints;
        let total_length: f64 = courses.map(DrawnCourse::screen_length).sum();
        let violations = arcs_through_solids + arrowheads_hidden_by_other_solids;
        let arcs = self.arcs.len();
        let arc_length_over_solids = QuantizedLength::of(arc_length_over_solids);
        let hidden_trace_length = QuantizedLength::of(hidden_trace_length);
        let total_length = QuantizedLength::of(total_length);
        SceneScore {
            violations,
            crossings,
            hidden_junctions,
            arc_length_over_solids,
            hidden_trace_length,
            arcs,
            bends,
            total_length,
        }
    }

    /// Lift `group`'s traces, arcs and solids, its floor at `level_origin`, then every interior standing on its solids, whose floor is its container's top.
    fn lift_level(&mut self, group: &'s Group, level_origin: DVec3) {
        DrawnCourse::lift_merging_shared(&mut self.traces, &group.traces, level_origin);
        DrawnCourse::lift_merging_shared(&mut self.arcs, &group.connections, level_origin);
        for solid in &group.solids {
            let Some(plan_height) = solid.plan_height else {
                continue;
            };
            let roof = IsometricProjection::horizontal_plane_at(plan_height);
            let roof_to_level_plan = roof.inverse();
            let prism =
                SolidPrism::under_top_of(solid, roof_to_level_plan, level_origin, plan_height);
            self.prisms.extend(prism);
            let Some(interior) = &solid.interior else {
                continue;
            };
            let interior_screen_offset = interior.frame.translation();
            let interior_screen_origin = interior_screen_offset.to_point();
            let interior_ground_origin = roof_to_level_plan * interior_screen_origin;
            let interior_offset = DVec3::new(
                interior_ground_origin.x,
                interior_ground_origin.y,
                plan_height,
            );
            let interior_origin = level_origin + interior_offset;
            self.lift_level(interior, interior_origin);
        }
    }

    /// Count the pairs of distinct edges whose courses cross on screen; a course never crosses one drawn by an edge it shares.
    fn count_crossing_edge_pairs(&self) -> usize {
        let courses: Vec<&DrawnCourse> = self.arcs.iter().chain(&self.traces).collect();
        let mut crossing_edge_pairs = BTreeSet::new();
        for (course_index, course) in courses.iter().enumerate() {
            let later_courses = &courses[course_index + 1..];
            for other in later_courses {
                let is_sharing_an_edge = course.edges.iter().any(|edge| other.edges.contains(edge));
                if is_sharing_an_edge {
                    continue;
                }
                let is_crossing = course.crosses(other);
                if is_crossing {
                    let edge = course.edges[0];
                    let other_edge = other.edges[0];
                    let edge_pair = (edge.min(other_edge), edge.max(other_edge));
                    crossing_edge_pairs.insert(edge_pair);
                }
            }
        }
        crossing_edge_pairs.len()
    }

    /// Yield the solids that may obstruct `course`: all but its endpoints and the containers enclosing both.
    fn obstructing_prisms_of<'c>(
        &'c self,
        course: &'c DrawnCourse,
    ) -> impl Iterator<Item = &'c SolidPrism<'s>> + Clone + 'c {
        self.prisms
            .iter()
            .filter(|prism| course.is_obstructable_by(prism))
    }

    /// Return, per segment of `course`, the runs of it each of `obstructing_prisms` overlaps on screen, by sight depth.
    fn depth_runs_along(
        course: &DrawnCourse,
        obstructing_prisms: &[&SolidPrism],
    ) -> Vec<Vec<DepthRun>> {
        let segments = course.segments();
        segments
            .map(|segment| {
                let segment_screen_bounds = segment.screen_bounds();
                obstructing_prisms
                    .iter()
                    .filter(|prism| prism.screen_bounds.overlaps(segment_screen_bounds))
                    .flat_map(|prism| prism.depth_runs_along(&segment))
                    .collect()
            })
            .collect()
    }

    /// Tell what hides the arrowhead tip `course` ends in, its last plan point, if anything: only children of an endpoint, the reader still finding the tip by that container, or another solid.
    fn arrowhead_hider_of(&self, course: &DrawnCourse) -> Option<TipHider> {
        let is_headless = !course.ends_in_arrowhead;
        if is_headless {
            return None;
        }
        let tip = course.last_plan_point();
        let hiding_prisms: Vec<&SolidPrism> = self
            .obstructing_prisms_of(course)
            .filter(|prism| prism.hides(tip))
            .collect();
        let is_tip_visible = hiding_prisms.is_empty();
        if is_tip_visible {
            return None;
        }
        let is_hidden_inside_endpoints = hiding_prisms
            .iter()
            .all(|prism| course.has_endpoint_enclosing(prism));
        if is_hidden_inside_endpoints {
            return Some(TipHider::EndpointChildren);
        }
        Some(TipHider::OtherSolid)
    }

    /// Tell whether a solid that may obstruct `course` stands in front of or around `point` of it.
    fn is_hidden(&self, course: &DrawnCourse, point: DVec3) -> bool {
        let mut obstructing_prisms = self.obstructing_prisms_of(course);
        obstructing_prisms.any(|prism| prism.hides(point))
    }
}

/// Holds one drawn connection lifted into document plan and projected on screen, with the edges drawing it.
struct DrawnCourse<'s> {
    /// Edges drawing this course: one, or every edge sharing a bundled trunk or branch.
    edges: Vec<&'s EdgeId>,
    /// Plan points in flow order: a trace's corners, or an arc flattened.
    plan_points: Vec<DVec3>,
    screen_points: Vec<Point>,
    ends_in_arrowhead: bool,
}

impl<'s> DrawnCourse<'s> {
    /// Screen distance from a course's end within which a crossing counts as touching at a shared port or slot.
    const END_CLEARANCE: f64 = 0.5;

    /// Lift `connections`, one level's traces or arcs, its floor at `level_origin`, into `courses`, one per distinct plan course, so a branch or trunk several bundled edges draw is lifted once, drawn by all of them.
    fn lift_merging_shared(
        courses: &mut Vec<Self>,
        connections: &'s [Connection],
        level_origin: DVec3,
    ) {
        let first_level_course_index = courses.len();
        for connection in connections {
            let Some(course) = Self::lifted(connection, level_origin) else {
                continue;
            };
            let level_courses = &mut courses[first_level_course_index..];
            let shared_course = level_courses
                .iter_mut()
                .find(|level_course| level_course.plan_points == course.plan_points);
            let Some(shared_course) = shared_course else {
                courses.push(course);
                continue;
            };
            shared_course.edges.extend(course.edges);
            shared_course.ends_in_arrowhead |= course.ends_in_arrowhead;
        }
    }

    /// Lift `connection`, its level's floor at `level_origin`, or `None` when it was drawn without a plan.
    fn lifted(connection: &'s Connection, level_origin: DVec3) -> Option<Self> {
        let plan_course = connection.plan_course.as_ref()?;
        let level_plan_points = plan_course.flattened();
        let plan_points: Vec<DVec3> = level_plan_points
            .into_iter()
            .map(|level_plan_point| level_origin + level_plan_point)
            .collect();
        let screen_points = plan_points
            .iter()
            .copied()
            .map(IsometricProjection::project)
            .collect();
        let edges = vec![&connection.edge];
        let drawn_course = Self {
            edges,
            plan_points,
            screen_points,
            ends_in_arrowhead: connection.ends_in_arrowhead,
        };
        Some(drawn_course)
    }

    /// Tell whether `prism` may obstruct this course: it is neither an endpoint of an edge drawing it nor a container enclosing both its endpoints.
    fn is_obstructable_by(&self, prism: &SolidPrism) -> bool {
        let solid = prism.element;
        let is_exempt = self.edges.iter().any(|edge| {
            let source = edge.from();
            let target = edge.to();
            let is_endpoint = solid == source || solid == target;
            let is_enclosing_source = solid.is_ancestor_of(source);
            let is_enclosing_target = solid.is_ancestor_of(target);
            let is_enclosing_both = is_enclosing_source && is_enclosing_target;
            is_endpoint || is_enclosing_both
        });
        !is_exempt
    }

    /// Tell whether `prism` stands inside an endpoint of an edge drawing this course, a child of the container that edge ends on.
    fn has_endpoint_enclosing(&self, prism: &SolidPrism) -> bool {
        let solid = prism.element;
        self.edges.iter().any(|edge| {
            let source = edge.from();
            let target = edge.to();
            let is_inside_source = source.is_ancestor_of(solid);
            let is_inside_target = target.is_ancestor_of(solid);
            is_inside_source || is_inside_target
        })
    }

    /// Tell whether this course and `other` cross on screen, clear of either's ends.
    fn crosses(&self, other: &DrawnCourse) -> bool {
        let screen_bounds = self.screen_bounds();
        let other_screen_bounds = other.screen_bounds();
        let is_apart = !screen_bounds.overlaps(other_screen_bounds);
        if is_apart {
            return false;
        }
        let mut segments = self.segments();
        segments.any(|segment| {
            let mut other_segments = other.segments();
            other_segments.any(|other_segment| {
                let Some(crossing) = segment.screen_crossing_with(&other_segment) else {
                    return false;
                };
                let is_near_own_end = self.is_screen_end_near(crossing);
                let is_near_other_end = other.is_screen_end_near(crossing);
                let is_near_an_end = is_near_own_end || is_near_other_end;
                !is_near_an_end
            })
        })
    }

    /// Return the screen length of the stretches `depth_runs` marks at any of `depths`, per segment, a stretch several solids overlap counted once.
    fn screen_length_where(&self, depth_runs: &[Vec<DepthRun>], depths: &[SightDepth]) -> f64 {
        let segments = self.segments();
        segments
            .zip(depth_runs)
            .map(|(segment, segment_runs)| {
                let matching_runs = segment_runs
                    .iter()
                    .filter(|run| depths.contains(&run.depth));
                let covered_share = DepthRun::union_length(matching_runs);
                let segment_screen_length = segment.screen_length();
                covered_share * segment_screen_length
            })
            .sum()
    }

    /// Return the course's drawn length on screen.
    fn screen_length(&self) -> f64 {
        let segments = self.segments();
        segments.map(|segment| segment.screen_length()).sum()
    }

    /// Return the end where this branch meets its trunk: its first point when it reaches the target under the arrowhead, else its last.
    fn slot_end(&self) -> DVec3 {
        let slot = if self.ends_in_arrowhead {
            self.plan_points.first()
        } else {
            self.plan_points.last()
        };
        *slot.expect("a course has at least two points")
    }

    /// Yield the plan points this trace turns at: every corner but its ends.
    fn bend_points(&self) -> impl Iterator<Item = DVec3> + '_ {
        let bend_count = self.bend_count();
        let after_first_corner = self.plan_points.iter().skip(1);
        after_first_corner.take(bend_count).copied()
    }

    /// Count the turns along this trace.
    fn bend_count(&self) -> usize {
        let corner_count = self.plan_points.len();
        corner_count.saturating_sub(2)
    }

    /// Yield every segment between consecutive points, in plan and on screen.
    fn segments(&self) -> impl Iterator<Item = CourseSegment> + '_ {
        let plan_pairs = self.plan_points.windows(2);
        let screen_pairs = self.screen_points.windows(2);
        plan_pairs
            .zip(screen_pairs)
            .map(|(plan_pair, screen_pair)| CourseSegment {
                plan_start: plan_pair[0],
                plan_end: plan_pair[1],
                screen_start: screen_pair[0],
                screen_end: screen_pair[1],
            })
    }

    /// Tell whether `screen_point` lies within the end clearance of this course's first or last point.
    fn is_screen_end_near(&self, screen_point: Point) -> bool {
        let clearance_squared = Self::END_CLEARANCE * Self::END_CLEARANCE;
        let ends = [self.screen_points.first(), self.screen_points.last()];
        let mut present_ends = ends.into_iter().flatten();
        present_ends.any(|&end| {
            let offset = screen_point - end;
            offset.hypot2() < clearance_squared
        })
    }

    /// Return the screen bounds of every point.
    fn screen_bounds(&self) -> Rect {
        let point_bounds = self
            .screen_points
            .iter()
            .map(|&point| Rect::from_points(point, point));
        let bounds = point_bounds.reduce(|bounds, next_bounds| bounds.union(next_bounds));
        bounds.unwrap_or(Rect::ZERO)
    }

    /// Return the last plan point: the target port, where an arrowhead's tip lands.
    fn last_plan_point(&self) -> DVec3 {
        let last_point = self.plan_points.last();
        *last_point.expect("a course has at least two points")
    }
}

/// Pairs one straight piece of a drawn course in document plan with its projection on screen.
#[derive(Clone, Copy)]
struct CourseSegment {
    plan_start: DVec3,
    plan_end: DVec3,
    screen_start: Point,
    screen_end: Point,
}

impl CourseSegment {
    /// Return where this segment and `other` cross on screen, each straddling the other: u = s₀ / (s₀ − s₁), sᵢ = (O₁ − O₀) × (Sᵢ − O₀) for this segment's ends Sᵢ and `other`'s ends Oᵢ.
    fn screen_crossing_with(&self, other: &CourseSegment) -> Option<Point> {
        let span = self.screen_end - self.screen_start;
        let other_span = other.screen_end - other.screen_start;
        let other_start_offset = other.screen_start - self.screen_start;
        let other_end_offset = other.screen_end - self.screen_start;
        let start_offset = self.screen_start - other.screen_start;
        let end_offset = self.screen_end - other.screen_start;
        let other_start_side = span.cross(other_start_offset);
        let other_end_side = span.cross(other_end_offset);
        let start_side = other_span.cross(start_offset);
        let end_side = other_span.cross(end_offset);
        let is_other_straddling = other_start_side * other_end_side < 0.0;
        let is_straddling_other = start_side * end_side < 0.0;
        let is_crossing = is_other_straddling && is_straddling_other;
        if !is_crossing {
            return None;
        }
        let crossing_parameter = start_side / (start_side - end_side);
        let crossing = self.screen_start + span * crossing_parameter;
        Some(crossing)
    }

    /// Return the segment's screen bounds.
    fn screen_bounds(&self) -> Rect {
        Rect::from_points(self.screen_start, self.screen_end)
    }

    /// Return the segment's length on screen.
    fn screen_length(&self) -> f64 {
        let screen_span = self.screen_end - self.screen_start;
        screen_span.hypot2().sqrt()
    }
}

/// Places a solid along the sight line through a course point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SightDepth {
    /// Part of the solid lies nearer the viewer, so it hides the point.
    SolidInFront,
    /// The whole solid lies farther, so the point is drawn over it.
    SolidBehind,
    /// The solid lies both nearer and farther: the point is inside its volume.
    InsideSolid,
}

/// Marks the stretch of one course segment, between two of its parameters, that one solid overlaps on screen at one sight depth.
#[derive(Clone, Copy, Debug)]
struct DepthRun {
    start: f64,
    end: f64,
    depth: SightDepth,
}

impl DepthRun {
    /// Return the parameter length `runs` cover together, overlaps counted once.
    fn union_length<'r>(runs: impl Iterator<Item = &'r DepthRun>) -> f64 {
        let mut spans: Vec<(f64, f64)> = runs.map(|run| (run.start, run.end)).collect();
        spans.sort_by(|(start, _end), (other_start, _other_end)| start.total_cmp(other_start));
        let mut covered_length = 0.0;
        let mut covered_until = f64::NEG_INFINITY;
        for (start, end) in spans {
            let uncovered_start = start.max(covered_until);
            let is_extending = end > uncovered_start;
            if is_extending {
                covered_length += end - uncovered_start;
                covered_until = end;
            }
        }
        covered_length
    }
}

/// Bounds one solid in document plan as the prism under its top face, floor to top, so a sight line can place it against a course point; a container's prism is its slab alone, never its children.
/// Names what hides an arrowhead tip: only children of the container it points at, or another solid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TipHider {
    EndpointChildren,
    OtherSolid,
}

struct SolidPrism<'s> {
    element: &'s ElementPath,
    floor_height: f64,
    roof_height: f64,
    /// Half-planes whose intersection is the top face's plan outline.
    sides: Vec<PrismSide>,
    screen_bounds: Rect,
}

impl<'s> SolidPrism<'s> {
    /// Samples per curved segment of a top face's outline, keeping a cylinder's within 2% of its ellipse.
    const CURVE_SAMPLES: u32 = 4;

    /// Bound `solid` under its top face, which `roof_to_level_plan` maps back onto its level's plan, the level's floor at `level_origin` and the top `plan_height` above it; `None` for a solid without a top face.
    fn under_top_of(
        solid: &'s Solid,
        roof_to_level_plan: Affine,
        level_origin: DVec3,
        plan_height: f64,
    ) -> Option<Self> {
        let top_face = solid
            .faces
            .iter()
            .find(|face| face.shade == FaceShade::Top)?;
        let level_offset = Vec2::new(level_origin.x, level_origin.y);
        let level_plan_to_plan = Affine::translate(level_offset);
        let roof_to_plan = level_plan_to_plan * roof_to_level_plan;
        let plan_top = roof_to_plan * &top_face.outline;
        let outline = Self::corners_of(&plan_top);
        let sides = PrismSide::around(&outline);
        let floor_height = level_origin.z;
        let roof_height = floor_height + plan_height;
        let screen_bounds = Self::screen_bounds_of(&outline, floor_height, roof_height);
        let prism = Self {
            element: &solid.element,
            floor_height,
            roof_height,
            sides,
            screen_bounds,
        };
        Some(prism)
    }

    /// Return the corners of `outline`, each curved segment sampled `CURVE_SAMPLES` times from its start.
    fn corners_of(outline: &BezPath) -> Vec<Point> {
        let segments = outline.segments();
        segments
            .flat_map(|segment| {
                let is_straight = matches!(segment, PathSeg::Line(_line));
                let sample_count = if is_straight { 1 } else { Self::CURVE_SAMPLES };
                (0..sample_count).map(move |sample_index| {
                    let parameter = f64::from(sample_index) / f64::from(sample_count);
                    segment.eval(parameter)
                })
            })
            .collect()
    }

    /// Return the screen bounds of the prism over `outline` from `floor_height` to `roof_height`.
    fn screen_bounds_of(outline: &[Point], floor_height: f64, roof_height: f64) -> Rect {
        let heights = [floor_height, roof_height];
        let plan_corners = heights.into_iter().flat_map(|height| {
            outline
                .iter()
                .map(move |corner| DVec3::new(corner.x, corner.y, height))
        });
        let screen_corners = plan_corners.map(IsometricProjection::project);
        let corner_bounds = screen_corners.map(|corner| Rect::from_points(corner, corner));
        let bounds = corner_bounds.reduce(|bounds, next_bounds| bounds.union(next_bounds));
        bounds.unwrap_or(Rect::ZERO)
    }

    /// Return the runs of `segment` this prism overlaps on screen, by sight depth: the depth holds between consecutive flip parameters, so each run's middle classifies it.
    fn depth_runs_along(&self, segment: &CourseSegment) -> Vec<DepthRun> {
        let start_bounds = self.sight_bounds_at(segment.plan_start);
        let end_bounds = self.sight_bounds_at(segment.plan_end);
        let flip_parameters = start_bounds.flip_parameters_toward(&end_bounds);
        let mut run_ends = vec![0.0, 1.0];
        run_ends.extend(flip_parameters);
        run_ends.sort_by(f64::total_cmp);
        run_ends
            .windows(2)
            .filter_map(|run_window| {
                let start = run_window[0];
                let end = run_window[1];
                let is_empty = end <= start;
                if is_empty {
                    return None;
                }
                let middle = (start + end) / 2.0;
                let middle_point = segment.plan_start.lerp(segment.plan_end, middle);
                let depth = self.sight_depth_at(middle_point)?;
                let depth_run = DepthRun { start, end, depth };
                Some(depth_run)
            })
            .collect()
    }

    /// Tell whether this prism stands in front of `point` or around it, hiding it from the viewer.
    fn hides(&self, point: DVec3) -> bool {
        let sight_depth = self.sight_depth_at(point);
        matches!(
            sight_depth,
            Some(SightDepth::SolidInFront | SightDepth::InsideSolid)
        )
    }

    /// Place this prism along the sight line through `point`, or `None` when the line misses it.
    fn sight_depth_at(&self, point: DVec3) -> Option<SightDepth> {
        let sight_bounds = self.sight_bounds_at(point);
        sight_bounds.depth()
    }

    /// Return the bounds on t the sight line `point` + t·(1, 1, 1) keeps within inside the prism, t growing toward the viewer: the floor and roof, then each side n·q ≤ c, which bounds t·(n_x + n_y) ≤ c − n·q.
    fn sight_bounds_at(&self, point: DVec3) -> SightBounds {
        let floor_entry = self.floor_height - point.z;
        let roof_exit = self.roof_height - point.z;
        let mut sight_bounds = SightBounds {
            entry_bounds: vec![floor_entry],
            exit_bounds: vec![roof_exit],
            parallel_slacks: Vec::new(),
        };
        let plan_point = Vec2::new(point.x, point.y);
        for side in &self.sides {
            let normal_reach = side.outward_normal.dot(plan_point);
            let slack = side.offset - normal_reach;
            let sight_rate = side.outward_normal.x + side.outward_normal.y;
            if sight_rate > 0.0 {
                let exit = slack / sight_rate;
                sight_bounds.exit_bounds.push(exit);
            } else if sight_rate < 0.0 {
                let entry = slack / sight_rate;
                sight_bounds.entry_bounds.push(entry);
            } else {
                sight_bounds.parallel_slacks.push(slack);
            }
        }
        sight_bounds
    }
}

/// Bounds a prism on one side of its plan outline: plan points q inside keep outward_normal · q ≤ offset.
struct PrismSide {
    outward_normal: Vec2,
    offset: f64,
}

impl PrismSide {
    /// Bound the convex `outline` by one side per edge, facing out whichever way its corners turn: the shoelace sum Σ pᵢ × pᵢ₊₁ is positive when they turn counter-clockwise, whose outward normals are the edges d turned clockwise, (d_y, −d_x).
    fn around(outline: &[Point]) -> Vec<Self> {
        let next_corners = outline.iter().cycle().skip(1).copied();
        let edges: Vec<(Point, Point)> = outline.iter().copied().zip(next_corners).collect();
        let doubled_signed_area: f64 = edges
            .iter()
            .map(|(start, end)| start.to_vec2().cross(end.to_vec2()))
            .sum();
        let orientation = doubled_signed_area.signum();
        edges
            .into_iter()
            .filter_map(|(start, end)| {
                let along = end - start;
                let is_degenerate = along.hypot2() == 0.0;
                if is_degenerate {
                    return None;
                }
                let clockwise_turn = Vec2::new(along.y, -along.x);
                let outward_normal = clockwise_turn * orientation;
                let offset = outward_normal.dot(start.to_vec2());
                let side = Self {
                    outward_normal,
                    offset,
                };
                Some(side)
            })
            .collect()
    }
}

/// Collects the bounds the sight line through one point keeps within while inside one prism.
struct SightBounds {
    /// Lower bounds on t: the floor and every side facing away from the viewer.
    entry_bounds: Vec<f64>,
    /// Upper bounds on t: the roof and every side facing the viewer.
    exit_bounds: Vec<f64>,
    /// Slacks of the sides parallel to the sight line, non-negative while it runs inside them.
    parallel_slacks: Vec<f64>,
}

impl SightBounds {
    /// Sight-line distance within which a point touches a solid's surface, so a trace lying on a slab or a line grazing an edge is neither hidden nor inside.
    const TOUCH_TOLERANCE: f64 = 1e-6;

    /// Place the solid from these bounds: missed when a parallel side excludes the line or the line spends no more than the tolerance inside; behind when the line leaves it by the point; around the point when it also entered before it; else in front.
    fn depth(&self) -> Option<SightDepth> {
        let is_outside_a_parallel_side = self.parallel_slacks.iter().any(|&slack| slack < 0.0);
        if is_outside_a_parallel_side {
            return None;
        }
        let entry_bounds = self.entry_bounds.iter().copied();
        let exit_bounds = self.exit_bounds.iter().copied();
        let entry = entry_bounds.fold(f64::NEG_INFINITY, f64::max);
        let exit = exit_bounds.fold(f64::INFINITY, f64::min);
        let is_missed = exit - entry <= Self::TOUCH_TOLERANCE;
        if is_missed {
            return None;
        }
        let is_left_by_the_point = exit <= Self::TOUCH_TOLERANCE;
        if is_left_by_the_point {
            return Some(SightDepth::SolidBehind);
        }
        let is_entered_before_the_point = entry < -Self::TOUCH_TOLERANCE;
        if is_entered_before_the_point {
            return Some(SightDepth::InsideSolid);
        }
        Some(SightDepth::SolidInFront)
    }

    /// Return the parameters strictly inside a segment from this point to the one bounded by `end`, every bound varying linearly between them, where any comparison `depth` makes flips.
    fn flip_parameters_toward(&self, end: &SightBounds) -> Vec<f64> {
        let entry_pairs = Self::paired(&self.entry_bounds, &end.entry_bounds);
        let exit_pairs = Self::paired(&self.exit_bounds, &end.exit_bounds);
        let slack_pairs = Self::paired(&self.parallel_slacks, &end.parallel_slacks);
        let tolerance = Self::TOUCH_TOLERANCE;
        let mut compared_quantities = slack_pairs;
        for &(entry_at_start, entry_at_end) in &entry_pairs {
            for &(exit_at_start, exit_at_end) in &exit_pairs {
                let overlap_at_start = exit_at_start - entry_at_start - tolerance;
                let overlap_at_end = exit_at_end - entry_at_end - tolerance;
                compared_quantities.push((overlap_at_start, overlap_at_end));
            }
            let entry_margin = (entry_at_start + tolerance, entry_at_end + tolerance);
            compared_quantities.push(entry_margin);
        }
        for &(exit_at_start, exit_at_end) in &exit_pairs {
            let exit_margin = (exit_at_start - tolerance, exit_at_end - tolerance);
            compared_quantities.push(exit_margin);
        }
        compared_quantities
            .into_iter()
            .filter_map(|(at_start, at_end)| Self::sign_flip_parameter(at_start, at_end))
            .collect()
    }

    /// Pair each bound at a segment's start with the same bound at its end.
    fn paired(at_start: &[f64], at_end: &[f64]) -> Vec<(f64, f64)> {
        let starts = at_start.iter().copied();
        let ends = at_end.iter().copied();
        starts.zip(ends).collect()
    }

    /// Return where a quantity going linearly from `at_start` to `at_end` changes sign strictly inside the segment: u = f₀ / (f₀ − f₁).
    fn sign_flip_parameter(at_start: f64, at_end: f64) -> Option<f64> {
        let is_start_negative = at_start < 0.0;
        let is_end_negative = at_end < 0.0;
        let is_same_sign = is_start_negative == is_end_negative;
        if is_same_sign {
            return None;
        }
        let flip_parameter = at_start / (at_start - at_end);
        let is_strictly_inside = flip_parameter > 0.0 && flip_parameter < 1.0;
        is_strictly_inside.then_some(flip_parameter)
    }
}

impl PlanCourse {
    /// Screen distance an arc's flattened polyline may stray from the arc.
    const FLATTENING_TOLERANCE: f64 = 0.25;

    /// Return the course's plan points in flow order: a trace's corners, or an arc flattened.
    fn flattened(&self) -> Vec<DVec3> {
        match self {
            Self::Trace(corners) => corners.clone(),
            Self::Arc(control_points) => Self::flattened_arc(*control_points),
        }
    }

    /// Flatten the arc through `control_points` into n equal parameter steps, n = ⌈√(|S₀ − 2S₁ + S₂| / 4τ)⌉ for its screen control points Sᵢ, keeping every chord within τ of the arc on screen; B(t) = (1 − t)²P₀ + 2t(1 − t)P₁ + t²P₂.
    fn flattened_arc(control_points: [DVec3; 3]) -> Vec<DVec3> {
        let [source_end, control, target_end] = control_points;
        let screen_control_points = control_points.map(IsometricProjection::project);
        let [screen_source, screen_control, screen_target] =
            screen_control_points.map(Point::to_vec2);
        let second_difference = screen_source - screen_control * 2.0 + screen_target;
        let second_difference_length = second_difference.hypot2().sqrt();
        let squared_step_count = second_difference_length / (4.0 * Self::FLATTENING_TOLERANCE);
        let step_count_estimate = squared_step_count.sqrt().ceil();
        let step_count = step_count_estimate.max(1.0) as u32;
        (0..=step_count)
            .map(|step_index| {
                let parameter = f64::from(step_index) / f64::from(step_count);
                let remaining = 1.0 - parameter;
                let source_weight = remaining * remaining;
                let control_weight = 2.0 * parameter * remaining;
                let target_weight = parameter * parameter;
                source_end * source_weight + control * control_weight + target_end * target_weight
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kurbo::{Point, Rect, Size};
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::{Edge, Element};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::layout::{LevelPlan, LevelPlans, PlacedLevel, Port, Route, Side};

    #[test]
    fn crossing_arcs_and_a_trace_behind_a_tall_block_score_one_crossing_and_the_hidden_stretch() {
        let mut builder = ModelBuilder::new();
        let west = builder.add_top_level("west", Element::new("west"));
        let east = builder.add_top_level("east", Element::new("east"));
        let north = builder.add_top_level("north", Element::new("north"));
        let south = builder.add_top_level("south", Element::new("south"));
        let left = builder.add_top_level("left", Element::new("left"));
        let right = builder.add_top_level("right", Element::new("right"));
        let tall_block = Element::new("tall").with_height(84.0);
        builder.add_top_level("tall", tall_block);
        let west_to_east = builder.add_edge(&west, &east, Edge::new());
        let north_to_south = builder.add_edge(&north, &south, Edge::new());
        let left_to_right = builder.add_edge(&left, &right, Edge::new());
        let model = builder.build().model.expect("fixture model is valid");
        let footprints = BTreeMap::from([
            (path("west"), Rect::new(0.0, 140.0, 40.0, 180.0)),
            (path("east"), Rect::new(200.0, 140.0, 240.0, 180.0)),
            (path("north"), Rect::new(100.0, 0.0, 140.0, 40.0)),
            (path("south"), Rect::new(100.0, 200.0, 140.0, 240.0)),
            (path("left"), Rect::new(300.0, 40.0, 340.0, 80.0)),
            (path("right"), Rect::new(420.0, 40.0, 460.0, 80.0)),
            (path("tall"), Rect::new(360.0, 80.0, 400.0, 120.0)),
        ]);
        let west_east_port = Port::new(Side::East, Point::new(40.0, 160.0));
        let east_west_port = Port::new(Side::West, Point::new(200.0, 160.0));
        let west_to_east_arc = Route::new(west_east_port, east_west_port);
        let north_south_port = Port::new(Side::South, Point::new(110.0, 40.0));
        let south_north_port = Port::new(Side::North, Point::new(130.0, 200.0));
        let north_to_south_arc = Route::new(north_south_port, south_north_port);
        let left_east_port = Port::new(Side::East, Point::new(340.0, 60.0));
        let right_west_port = Port::new(Side::West, Point::new(420.0, 60.0));
        let left_to_right_trace =
            Route::traced_through(left_east_port, Vec::new(), right_west_port);
        let routes = BTreeMap::from([
            (west_to_east, west_to_east_arc),
            (north_to_south, north_to_south_arc),
            (left_to_right, left_to_right_trace),
        ]);
        let extent = Size::new(460.0, 240.0);
        let placed = PlacedLevel::new(extent, footprints);
        let plan = LevelPlan::new(placed, routes, BTreeMap::new());
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project(&model, &plans, &FixedAdvanceMeasure);

        let score = scene.score();

        assert_eq!(
            score.crossings, 1,
            "the west-east and north-south arcs cross once on screen"
        );
        let stretch_behind_tall = QuantizedLength::of(40.0);
        assert_eq!(
            score.hidden_trace_length, stretch_behind_tall,
            "the tall block in front hides the left-right trace from its source port to plan x = 380, 40 units on screen"
        );
    }

    fn path(name: &str) -> ElementPath {
        ElementPath::from_segments([name])
    }
}
