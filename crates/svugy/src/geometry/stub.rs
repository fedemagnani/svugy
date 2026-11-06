//! Plate every container's stub markers flat on its slab, in the band its layout reserves along each slot's side.

use std::collections::BTreeMap;
use std::iter;

use glam::DVec3;
use kurbo::{Affine, Point, Size};
use svugy_model::model::Model;
use svugy_model::path::ElementPath;

use super::obstruction::Obstructions;
use super::projection::IsometricProjection;
use super::{Group, NestedInteriors, PlatedLabel, StubMarker};
use crate::layout::{LevelPlan, LevelPlans, Port, Side};
use crate::metrics::TextMeasure;

impl Group {
    /// Overlay every nested container's stub markers once the whole scene is projected, so their plates keep clear of everything its entered view draws.
    pub(super) fn overlay_stub_markers(
        &mut self,
        model: &Model,
        plans: &LevelPlans,
        measure: &impl TextMeasure,
    ) {
        let mut stub_markers_by_container = self.plate_stub_markers(model, plans, measure);
        self.hand_stub_markers_to_interiors(&mut stub_markers_by_container);
    }

    /// Plate the stub markers of every nested container its plan stubs edges for, keyed by container.
    fn plate_stub_markers(
        &self,
        model: &Model,
        plans: &LevelPlans,
        measure: &impl TextMeasure,
    ) -> BTreeMap<ElementPath, BTreeMap<ElementPath, StubMarker>> {
        NestedInteriors::below(&self.solids)
            .filter_map(|interior| {
                let plan = plans.level(interior.container)?;
                let is_stubless = plan.stubs.is_empty();
                if is_stubless {
                    return None;
                }
                let obstructions = Obstructions::around_stubs_of(self, &interior);
                let stub_plater = StubPlater::new(plan, obstructions, measure);
                let stub_markers = stub_plater.plate_stub_markers(model);
                Some((interior.container.clone(), stub_markers))
            })
            .collect()
    }

    /// Hand every interior nested under this level its stub markers, at any depth.
    fn hand_stub_markers_to_interiors(
        &mut self,
        stub_markers_by_container: &mut BTreeMap<ElementPath, BTreeMap<ElementPath, StubMarker>>,
    ) {
        for solid in &mut self.solids {
            let Some(interior) = solid.interior.as_mut() else {
                continue;
            };
            let stub_markers = stub_markers_by_container
                .remove(&solid.element)
                .unwrap_or_default();
            interior.overlay(stub_markers);
            interior.hand_stub_markers_to_interiors(stub_markers_by_container);
        }
    }

    /// Overlay `stub_markers` on this level, its view widened to frame their plates.
    fn overlay(&mut self, stub_markers: BTreeMap<ElementPath, StubMarker>) {
        for stub_marker in stub_markers.values() {
            let tag_bounds = stub_marker.tag.bounds();
            let framed_tag = tag_bounds.inflate(Self::VIEW_MARGIN, Self::VIEW_MARGIN);
            self.view = self.view.union(framed_tag);
        }
        self.stub_markers = stub_markers;
    }
}

/// Plates one container's stub tags flat on its slab, each in its slot's stub band as near the slot as the entered view's obstructions allow.
struct StubPlater<'a, M> {
    plan: &'a LevelPlan,
    obstructions: Obstructions,
    measure: &'a M,
}

impl<'a, M: TextMeasure> StubPlater<'a, M> {
    /// Plan gap between a plate and the slab borders it rests against.
    const BORDER_GAP: f64 = 1.5;
    /// Plan distance between neighbouring plate centres along a side.
    const SLIDE_STEP: f64 = 6.0;

    fn new(plan: &'a LevelPlan, obstructions: Obstructions, measure: &'a M) -> Self {
        Self {
            plan,
            obstructions,
            measure,
        }
    }

    /// Plate every stub's tag in turn, each clear of the tags plated before it.
    fn plate_stub_markers(mut self, model: &Model) -> BTreeMap<ElementPath, StubMarker> {
        let mut stub_markers = BTreeMap::new();
        for (external_endpoint, stub) in &self.plan.stubs {
            let Some(external_element) = model.element(external_endpoint) else {
                continue;
            };
            let tag_text = external_element.label();
            let tag = self.plate_flat_beside(stub.slot, tag_text);
            self.obstructions.record(&tag);
            let stubbed_edges = stub.edges.clone();
            let stub_marker = StubMarker::new(stubbed_edges, tag);
            stub_markers.insert(external_endpoint.clone(), stub_marker);
        }
        stub_markers
    }

    /// Plate `text` flat against `slot`'s side, at the first spot sliding away from the slot whose plate overlaps the fewest obstructions.
    fn plate_flat_beside(&self, slot: Port, text: &str) -> PlatedLabel {
        let upright_tag = PlatedLabel::centered_on(text, Point::ORIGIN, self.measure);
        let plate_size = upright_tag.plate.rect().size();
        let plate_centers = self.plate_centers_sliding_from(slot, plate_size);
        let candidates = plate_centers.into_iter().map(|plate_center| {
            let plate_plane = slot.side.slab_top_plane_reading_along(plate_center);
            upright_tag.clone().on_plane(plate_plane)
        });
        let least_obstructed = self.obstructions.least_obstructed(candidates);
        least_obstructed.expect("a side offers at least the spot nearest its slot")
    }

    /// Return plan centres resting a plate of `plate_size` against `slot`'s side: nearest the slot first, then a slide step further either way in turn, each plate clear of the side's ends; the side's midpoint alone when the plate is longer than the side.
    fn plate_centers_sliding_from(&self, slot: Port, plate_size: Size) -> Vec<Point> {
        let boundary = self.plan.boundary();
        let side_line = slot.side.line_along(boundary);
        let side_vector = side_line.p1 - side_line.p0;
        let side_length = side_vector.hypot2().sqrt();
        let side_direction = side_vector / side_length;
        let rest_depth = Self::BORDER_GAP + plate_size.height / 2.0;
        let rest_offset = slot.side.inward_normal() * rest_depth;
        let center_at = |progress: f64| side_line.p0 + side_direction * progress + rest_offset;
        let end_clearance = Self::BORDER_GAP + plate_size.width / 2.0;
        let is_plate_longer_than_side = 2.0 * end_clearance > side_length;
        if is_plate_longer_than_side {
            let side_midpoint_center = center_at(side_length / 2.0);
            return vec![side_midpoint_center];
        }
        let first_progress = end_clearance;
        let last_progress = side_length - end_clearance;
        let slot_offset = slot.point - side_line.p0;
        let slot_progress = slot_offset.dot(side_direction);
        let nearest_progress = slot_progress.clamp(first_progress, last_progress);
        let step_count = (side_length / Self::SLIDE_STEP).ceil() as usize;
        let slid_progresses = (1..=step_count).flat_map(|step_index| {
            let slide = step_index as f64 * Self::SLIDE_STEP;
            [nearest_progress + slide, nearest_progress - slide]
        });
        iter::once(nearest_progress)
            .chain(slid_progresses)
            .filter(|progress| (first_progress..=last_progress).contains(progress))
            .map(center_at)
            .collect()
    }
}

impl Side {
    /// Map plate coordinates onto the slab's top around plan point `center`, the text reading along this side: toward +x on north and south, toward −y on east and west.
    fn slab_top_plane_reading_along(self, center: Point) -> Affine {
        let origin = DVec3::new(center.x, center.y, 0.0);
        match self {
            Self::North | Self::South => IsometricProjection::plane(origin, DVec3::X, DVec3::Y),
            Self::East | Self::West => IsometricProjection::plane(origin, DVec3::NEG_Y, DVec3::X),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kurbo::{Rect, RoundedRect};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::geometry::{NestedInterior, Scene};
    use crate::layout::Stub;
    use crate::metrics::BundledFont;
    use crate::svugy_example::SvugyExample;

    const TOLERANCE: f64 = 1e-9;

    /// Return the plan rectangle `tag`'s plate covers on its level's floor, the slab's top.
    fn plate_plan_rect(tag: &PlatedLabel) -> Rect {
        let floor = IsometricProjection::horizontal_plane_at(0.0);
        let plate_to_plan = floor.inverse() * tag.label.plane;
        let plate_rect = tag.plate.rect();
        plate_to_plan.transform_rect_bbox(plate_rect)
    }

    fn assert_inside(outer: Rect, inner: Rect, context: &str) {
        let closed_outer = outer.inflate(TOLERANCE, TOLERANCE);
        let is_inside = closed_outer.contains_rect(inner);
        assert!(is_inside, "{context}: {inner:?} leaves {outer:?}");
    }

    struct ProjectedExample {
        plans: LevelPlans,
        scene: Scene,
    }

    impl ProjectedExample {
        fn new() -> Self {
            let model = SvugyExample::model();
            let font = BundledFont::new();
            let plans = LevelPlans::lay_out(&model, &font);
            let scene = Scene::project(&model, &plans, &font);
            Self { plans, scene }
        }

        fn stubbed_interiors(&self) -> Vec<NestedInterior<'_>> {
            let interiors = self.scene.root.nested_interiors();
            let stubbed_interiors =
                interiors.filter(|interior| !interior.group.stub_markers.is_empty());
            stubbed_interiors.collect()
        }
    }

    #[test]
    fn every_stub_plate_of_the_example_lies_flat_inside_its_slot_band_on_the_slab_top() {
        let example = ProjectedExample::new();

        let stubbed_interiors = example.stubbed_interiors();

        assert!(!stubbed_interiors.is_empty());
        for interior in stubbed_interiors {
            let plan = example.plans.level(interior.container).expect("planned");
            for (external_endpoint, stub_marker) in &interior.group.stub_markers {
                let context = format!("{external_endpoint} in {}", interior.container);
                let slot_side = plan.stubs[external_endpoint].slot.side;
                let band = plan.stub_band(slot_side);
                let plan_rect = plate_plan_rect(&stub_marker.tag);
                assert_inside(band, plan_rect, &context);
                let plate_area = stub_marker.tag.plate.rect().area();
                let plan_area = plan_rect.area();
                let is_plate_flat_on_the_floor = (plan_area - plate_area).abs() < 1e-6;
                assert!(is_plate_flat_on_the_floor, "{context}");
            }
        }
    }

    /// Count, per stub of `interior`, what its plate overlaps among what its entered view draws and the plates placed before it.
    fn overlap_counts_in_placement_order(
        example: &ProjectedExample,
        interior: &NestedInterior,
    ) -> Vec<(ElementPath, usize, usize)> {
        let plan = example.plans.level(interior.container).expect("planned");
        let font = BundledFont::new();
        let mut obstructions = Obstructions::around_stubs_of(&example.scene.root, interior);
        let mut overlap_counts = Vec::new();
        for (external_endpoint, stub_marker) in &interior.group.stub_markers {
            let tag = &stub_marker.tag;
            let slot = plan.stubs[external_endpoint].slot;
            let overlap_count = obstructions.count_overlapped_by(tag);
            let stub_plater = StubPlater::new(plan, obstructions, &font);
            let plate_size = tag.plate.rect().size();
            let candidate_centers = stub_plater.plate_centers_sliding_from(slot, plate_size);
            let least_overlap_count = candidate_centers
                .into_iter()
                .map(|center| {
                    let plane = slot.side.slab_top_plane_reading_along(center);
                    let candidate = tag.clone().on_plane(plane);
                    stub_plater.obstructions.count_overlapped_by(&candidate)
                })
                .min()
                .expect("a side offers at least one spot");
            obstructions = stub_plater.obstructions;
            obstructions.record(tag);
            overlap_counts.push((
                external_endpoint.clone(),
                overlap_count,
                least_overlap_count,
            ));
        }
        overlap_counts
    }

    #[test]
    fn every_stub_plate_of_the_example_takes_the_least_obstructed_spot_of_its_side() {
        let example = ProjectedExample::new();

        let stubbed_interiors = example.stubbed_interiors();

        for interior in &stubbed_interiors {
            let overlap_counts = overlap_counts_in_placement_order(&example, interior);
            for (external_endpoint, overlap_count, least_overlap_count) in overlap_counts {
                let context = format!("{external_endpoint} in {}", interior.container);
                assert_eq!(overlap_count, least_overlap_count, "{context}");
            }
        }
    }

    #[test]
    fn no_stub_plate_of_the_example_overlaps_another() {
        let example = ProjectedExample::new();

        let stubbed_interiors = example.stubbed_interiors();

        for interior in &stubbed_interiors {
            let mut placed_plates = Obstructions::around_level(&[]);
            for (external_endpoint, stub_marker) in &interior.group.stub_markers {
                let overlap_count = placed_plates.count_overlapped_by(&stub_marker.tag);
                assert_eq!(
                    overlap_count, 0,
                    "{external_endpoint} in {}",
                    interior.container
                );
                placed_plates.record(&stub_marker.tag);
            }
        }
    }

    #[test]
    fn every_stub_plate_of_the_example_render_keeps_clear_of_faces_labels_and_plates() {
        let example = ProjectedExample::new();
        let render = ElementPath::from_segments(["pipeline", "render"]);

        let stubbed_interiors = example.stubbed_interiors();

        let render_interior = stubbed_interiors
            .iter()
            .find(|interior| *interior.container == render)
            .expect("render stubs its edges");
        let overlap_counts = overlap_counts_in_placement_order(&example, render_interior);
        assert_eq!(overlap_counts.len(), 2);
        for (external_endpoint, overlap_count, _least_overlap_count) in overlap_counts {
            assert_eq!(overlap_count, 0, "{external_endpoint} in render");
        }
    }

    /// Plan a 300 × 120 level stubbing one edge toward `far` through `slot`.
    fn plan_with_one_stub(slot: Port) -> LevelPlan {
        let mut plan = LevelPlan::new(Size::new(300.0, 120.0));
        let far = ElementPath::from_segments(["far"]);
        let stub = Stub::new(slot, BTreeSet::new());
        plan.stubs.insert(far, stub);
        plan
    }

    fn north_slot_at(x: f64) -> Port {
        let slot_point = Point::new(x, 0.0);
        Port::new(Side::North, slot_point)
    }

    /// Return the plan centre of `tag`'s plate.
    fn plate_plan_center(tag: &PlatedLabel) -> Point {
        let plan_rect = plate_plan_rect(tag);
        plan_rect.center()
    }

    #[test]
    fn a_plate_rests_against_its_slot_side_centred_on_the_slot() {
        let slot = north_slot_at(150.0);
        let plan = plan_with_one_stub(slot);
        let obstructions = Obstructions::around_level(&[]);
        let stub_plater = StubPlater::new(&plan, obstructions, &FixedAdvanceMeasure);

        let tag = stub_plater.plate_flat_beside(slot, "browser");

        let plan_rect = plate_plan_rect(&tag);
        let plate_center = plan_rect.center();
        assert!((plate_center.x - 150.0).abs() < TOLERANCE);
        assert!((plan_rect.y0 - StubPlater::<FixedAdvanceMeasure>::BORDER_GAP).abs() < TOLERANCE);
        assert_inside(plan.stub_band(Side::North), plan_rect, "north");
    }

    #[test]
    fn an_east_plate_reads_along_its_side_inside_the_east_band() {
        let slot = Port::new(Side::East, Point::new(300.0, 60.0));
        let plan = plan_with_one_stub(slot);
        let obstructions = Obstructions::around_level(&[]);
        let stub_plater = StubPlater::new(&plan, obstructions, &FixedAdvanceMeasure);

        let tag = stub_plater.plate_flat_beside(slot, "browser");

        let plan_rect = plate_plan_rect(&tag);
        assert!(plan_rect.height() > plan_rect.width());
        assert_inside(plan.stub_band(Side::East), plan_rect, "east");
    }

    #[test]
    fn a_plate_slides_from_its_slot_to_the_nearest_clear_spot() {
        let slot = north_slot_at(150.0);
        let plan = plan_with_one_stub(slot);
        let mut obstructions = Obstructions::around_level(&[]);
        let blocking_stub_plater =
            StubPlater::new(&plan, Obstructions::around_level(&[]), &FixedAdvanceMeasure);
        let blocking_tag = blocking_stub_plater.plate_flat_beside(slot, "browser");
        obstructions.record(&blocking_tag);
        let stub_plater = StubPlater::new(&plan, obstructions, &FixedAdvanceMeasure);

        let tag = stub_plater.plate_flat_beside(slot, "browser");

        let plate_size = tag.plate.rect().size();
        let candidate_centers = stub_plater.plate_centers_sliding_from(slot, plate_size);
        let first_clear_center = candidate_centers
            .into_iter()
            .find(|&center| {
                let plane = Side::North.slab_top_plane_reading_along(center);
                let candidate = tag.clone().on_plane(plane);
                stub_plater.obstructions.count_overlapped_by(&candidate) == 0
            })
            .expect("the north band has a clear spot");
        let plate_center = plate_plan_center(&tag);
        let center_offset = plate_center - first_clear_center;
        assert!(center_offset.hypot2().sqrt() < TOLERANCE);
        assert_eq!(stub_plater.obstructions.count_overlapped_by(&tag), 0);
        assert_inside(plan.stub_band(Side::North), plate_plan_rect(&tag), "north");
    }

    #[test]
    fn with_no_clear_spot_a_plate_falls_back_to_the_spot_nearest_its_slot() {
        let slot = north_slot_at(150.0);
        let plan = plan_with_one_stub(slot);
        let mut obstructions = Obstructions::around_level(&[]);
        let anything = PlatedLabel::centered_on("x", Point::ORIGIN, &FixedAdvanceMeasure);
        let everywhere = Rect::new(-10_000.0, -10_000.0, 10_000.0, 10_000.0);
        let everywhere_plate = RoundedRect::from_rect(everywhere, 0.0);
        let covering_everything = PlatedLabel::new(anything.label, everywhere_plate);
        obstructions.record(&covering_everything);
        let stub_plater = StubPlater::new(&plan, obstructions, &FixedAdvanceMeasure);

        let tag = stub_plater.plate_flat_beside(slot, "browser");

        let plate_center = plate_plan_center(&tag);
        assert!((plate_center.x - 150.0).abs() < TOLERANCE);
        assert_inside(plan.stub_band(Side::North), plate_plan_rect(&tag), "north");
    }

    #[test]
    fn a_slot_near_a_corner_keeps_its_plate_clear_of_the_side_end() {
        let slot = north_slot_at(1.0);
        let plan = plan_with_one_stub(slot);
        let obstructions = Obstructions::around_level(&[]);
        let stub_plater = StubPlater::new(&plan, obstructions, &FixedAdvanceMeasure);

        let tag = stub_plater.plate_flat_beside(slot, "browser");

        let plan_rect = plate_plan_rect(&tag);
        let border_gap = StubPlater::<FixedAdvanceMeasure>::BORDER_GAP;
        assert!((plan_rect.x0 - border_gap).abs() < TOLERANCE);
        assert_inside(plan.stub_band(Side::North), plan_rect, "north");
    }

    #[test]
    fn a_plate_longer_than_its_side_centres_on_the_side_midpoint() {
        let slot = north_slot_at(300.0);
        let plan = plan_with_one_stub(slot);
        let obstructions = Obstructions::around_level(&[]);
        let stub_plater = StubPlater::new(&plan, obstructions, &FixedAdvanceMeasure);
        let long_text = "a label far longer than the three hundred units of its side";

        let tag = stub_plater.plate_flat_beside(slot, long_text);

        let plate_center = plate_plan_center(&tag);
        assert!((plate_center.x - 150.0).abs() < TOLERANCE);
    }
}
