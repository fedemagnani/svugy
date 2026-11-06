//! Track what a plate, a badge or the legend must keep clear of in one view.

use std::iter;

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Vec2};
use svugy_model::path::ElementPath;

use super::{Face, Group, Heading, Label, NestedInterior, NestedInteriors, PlatedLabel, Solid};

/// Holds the screen outlines a plate, a badge or the legend must not overlap in one view: what that view draws and what was placed before.
pub(super) struct Obstructions {
    outlines: Vec<ScreenPolygon>,
}

impl Obstructions {
    /// Screen gap a plate keeps from every obstruction.
    const CLEARANCE: f64 = 2.0;

    /// Collect the labels of `solids`, with every label and plate the solids' nested interiors draw, mapped through their frames.
    pub(super) fn around_level(solids: &[Solid]) -> Self {
        let own_label_outlines = Self::solid_label_outlines(solids, Affine::IDENTITY);
        let nested_outlines = NestedInteriors::below(solids).flat_map(|interior| {
            let group = interior.group;
            let label_outlines = Self::solid_label_outlines(&group.solids, interior.to_outer);
            let plate_outlines = Self::plate_outlines(group, interior.to_outer);
            label_outlines.chain(plate_outlines)
        });
        let outlines = own_label_outlines.chain(nested_outlines).collect();
        Self { outlines }
    }

    /// Collect, in `container`'s frame, what its entered view draws over its slab: every solid label, every connection plate, every face but those of the slabs under its stubs.
    pub(super) fn around_stubs_of(root: &Group, container: &NestedInterior) -> Self {
        let document_to_container = container.to_outer.inverse();
        let root_level = (root, Affine::IDENTITY);
        let nested_levels = NestedInteriors::below(&root.solids)
            .map(|interior| (interior.group, interior.to_outer));
        let outlines = iter::once(root_level)
            .chain(nested_levels)
            .flat_map(|(group, group_to_document)| {
                let to_container = document_to_container * group_to_document;
                let face_outlines =
                    Self::face_outlines_beside(&group.solids, container.container, to_container);
                let label_outlines = Self::solid_label_outlines(&group.solids, to_container);
                let plate_outlines = Self::plate_outlines(group, to_container);
                face_outlines.chain(label_outlines).chain(plate_outlines)
            })
            .collect();
        Self { outlines }
    }

    /// Collect in document space everything the top-level view draws: every face, badge, solid label, arc and plate of every level, and the heading's lines.
    pub(super) fn around_document(root: &Group, heading: &Heading) -> Self {
        let root_level = (root, Affine::IDENTITY);
        let nested_levels = NestedInteriors::below(&root.solids)
            .map(|interior| (interior.group, interior.to_outer));
        let level_outlines =
            iter::once(root_level)
                .chain(nested_levels)
                .flat_map(|(group, group_to_document)| {
                    let solids = group.solids.iter();
                    let face_outlines = Self::solid_face_outlines(solids, group_to_document);
                    let label_outlines =
                        Self::solid_label_outlines(&group.solids, group_to_document);
                    let arc_outlines = Self::arc_outlines(group, group_to_document);
                    let plate_outlines = Self::plate_outlines(group, group_to_document);
                    face_outlines
                        .chain(label_outlines)
                        .chain(arc_outlines)
                        .chain(plate_outlines)
                });
        let heading_lines = heading.title.iter().chain(&heading.subtitle);
        let heading_outlines =
            heading_lines.map(|label| ScreenPolygon::of_label(label, Affine::IDENTITY));
        let outlines = level_outlines.chain(heading_outlines).collect();
        Self { outlines }
    }

    /// Yield the outline of every face and badge face of `solids` but those of `container` and its ancestors, mapped into the view by `to_view`.
    fn face_outlines_beside<'s>(
        solids: &'s [Solid],
        container: &'s ElementPath,
        to_view: Affine,
    ) -> impl Iterator<Item = ScreenPolygon> + 's {
        let solids_beside = solids.iter().filter(move |solid| {
            let is_container = solid.element == *container;
            let is_ancestor = solid.element.is_ancestor_of(container);
            !is_container && !is_ancestor
        });
        Self::solid_face_outlines(solids_beside, to_view)
    }

    /// Yield the outline of every face of `solids` and of the badges on them, mapped into the view by `to_view`.
    fn solid_face_outlines<'s>(
        solids: impl Iterator<Item = &'s Solid> + 's,
        to_view: Affine,
    ) -> impl Iterator<Item = ScreenPolygon> + 's {
        let faces = solids.flat_map(|solid| {
            let badge_faces = solid.badges.iter().flat_map(|badge| &badge.faces);
            solid.faces.iter().chain(badge_faces)
        });
        faces.map(move |face| ScreenPolygon::of_face(face, to_view))
    }

    /// Yield the outline of every connection arc `group` draws, the hull of its control points, mapped into the view by `to_view`.
    fn arc_outlines(group: &Group, to_view: Affine) -> impl Iterator<Item = ScreenPolygon> {
        let arcs = group.connections.iter().map(|connection| &connection.path);
        arcs.map(move |arc| ScreenPolygon::of_path(arc, to_view))
    }

    /// Yield the outline of every label of `solids`, mapped into the view by `to_view`.
    fn solid_label_outlines(
        solids: &[Solid],
        to_view: Affine,
    ) -> impl Iterator<Item = ScreenPolygon> {
        let labels = solids.iter().flat_map(|solid| &solid.labels);
        labels.map(move |label| ScreenPolygon::of_label(label, to_view))
    }

    /// Yield the outline of every connection plate `group` draws, mapped into the view by `to_view`.
    fn plate_outlines(group: &Group, to_view: Affine) -> impl Iterator<Item = ScreenPolygon> {
        let plated_labels = group
            .connections
            .iter()
            .filter_map(|connection| connection.label.as_ref());
        plated_labels.map(move |plated_label| ScreenPolygon::of_plate(plated_label, to_view, 0.0))
    }

    /// Pick the first of `candidates` whose plate overlaps the fewest obstructions, or `None` when there is no candidate.
    pub(super) fn least_obstructed(
        &self,
        candidates: impl IntoIterator<Item = PlatedLabel>,
    ) -> Option<PlatedLabel> {
        let candidates = candidates.into_iter();
        candidates.min_by_key(|candidate| self.count_overlapped_by(candidate))
    }

    /// Count the obstructions `plated_label`'s plate, grown by the clearance on its plane, overlaps.
    pub(super) fn count_overlapped_by(&self, plated_label: &PlatedLabel) -> usize {
        let cleared_outline =
            ScreenPolygon::of_plate(plated_label, Affine::IDENTITY, Self::CLEARANCE);
        self.outlines
            .iter()
            .filter(|outline| outline.overlaps(&cleared_outline))
            .count()
    }

    /// Add `plated_label` to the obstructions, so later plates keep clear of it.
    pub(super) fn record(&mut self, plated_label: &PlatedLabel) {
        let plate_outline = ScreenPolygon::of_plate(plated_label, Affine::IDENTITY, 0.0);
        self.outlines.push(plate_outline);
    }

    /// Tell whether `rect`, upright on the screen, overlaps any obstruction.
    pub(super) fn is_overlapped_by_rect(&self, rect: Rect) -> bool {
        let rect_outline = ScreenPolygon::mapped_rect(Affine::IDENTITY, rect);
        let mut outlines = self.outlines.iter();
        outlines.any(|outline| outline.overlaps(&rect_outline))
    }

    /// Pick the first of `candidates`, each a mini's faces, that overlaps the fewest obstructions, or `None` when there is no candidate.
    pub(super) fn least_obstructed_faces(
        &self,
        candidates: impl IntoIterator<Item = Vec<Face>>,
    ) -> Option<Vec<Face>> {
        let candidates = candidates.into_iter();
        candidates.min_by_key(|faces| self.count_overlapped_by_faces(faces))
    }

    /// Count the obstructions any of `faces` overlaps.
    pub(super) fn count_overlapped_by_faces(&self, faces: &[Face]) -> usize {
        let face_outlines: Vec<ScreenPolygon> = faces
            .iter()
            .map(|face| ScreenPolygon::of_face(face, Affine::IDENTITY))
            .collect();
        self.outlines
            .iter()
            .filter(|outline| {
                let mut overlapping_faces = face_outlines.iter();
                overlapping_faces.any(|face_outline| face_outline.overlaps(outline))
            })
            .count()
    }

    /// Add `faces` to the obstructions, so later minis keep clear of them.
    pub(super) fn record_faces(&mut self, faces: &[Face]) {
        let face_outlines = faces
            .iter()
            .map(|face| ScreenPolygon::of_face(face, Affine::IDENTITY));
        self.outlines.extend(face_outlines);
    }
}

/// Outlines a convex polygon in screen space, such as a label's line box set on its plane or a solid's face.
#[derive(Clone, Debug, PartialEq)]
struct ScreenPolygon {
    corners: Vec<Point>,
}

impl ScreenPolygon {
    /// Map the corners of `rect` into screen space by `to_view`.
    fn mapped_rect(to_view: Affine, rect: Rect) -> Self {
        let Rect { x0, y0, x1, y1 } = rect;
        let rect_corners = [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ];
        let corners = rect_corners.map(|corner| to_view * corner).to_vec();
        Self { corners }
    }

    /// Outline `label`'s line box on its plane, mapped into the view by `to_view`.
    fn of_label(label: &Label, to_view: Affine) -> Self {
        let line_box = label.line_box();
        let label_to_view = to_view * label.plane;
        Self::mapped_rect(label_to_view, line_box)
    }

    /// Outline `plated_label`'s plate grown by `clearance` on its label's plane, mapped into the view by `to_view`.
    fn of_plate(plated_label: &PlatedLabel, to_view: Affine, clearance: f64) -> Self {
        let plate_rect = plated_label.plate.rect();
        let cleared_rect = plate_rect.inflate(clearance, clearance);
        let plate_to_view = to_view * plated_label.label.plane;
        Self::mapped_rect(plate_to_view, cleared_rect)
    }

    /// Outline `face` by the points of its path, control points included, so every projection of a curved face lies inside the outline's.
    fn of_face(face: &Face, to_view: Affine) -> Self {
        Self::of_path(&face.outline, to_view)
    }

    /// Outline `path` by its points, control points included, mapped into the view by `to_view`.
    fn of_path(path: &BezPath, to_view: Affine) -> Self {
        let path_points = path.elements().iter().flat_map(|element| match *element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => vec![point],
            PathEl::QuadTo(control, point) => vec![control, point],
            PathEl::CurveTo(first_control, second_control, point) => {
                vec![first_control, second_control, point]
            }
            PathEl::ClosePath => Vec::new(),
        });
        let corners = path_points.map(|point| to_view * point).collect();
        Self { corners }
    }

    /// Tell whether the two polygons overlap: no edge normal of either separates their projections (separating axis theorem).
    fn overlaps(&self, other: &Self) -> bool {
        let self_normals = self.edge_normals();
        let other_normals = other.edge_normals();
        let mut edge_normals = self_normals.chain(other_normals);
        edge_normals.all(|normal| {
            let (self_min, self_max) = self.span_along(normal);
            let (other_min, other_max) = other.span_along(normal);
            let is_self_before_other_ends = self_min < other_max;
            let is_other_before_self_ends = other_min < self_max;
            is_self_before_other_ends && is_other_before_self_ends
        })
    }

    /// Yield the normal of every edge between consecutive corners, skipping the degenerate ones that separate nothing.
    fn edge_normals(&self) -> impl Iterator<Item = Vec2> + '_ {
        let next_corners = self.corners.iter().cycle().skip(1);
        let edges = self
            .corners
            .iter()
            .zip(next_corners)
            .map(|(&corner, &next_corner)| next_corner - corner);
        edges
            .filter(|edge| edge.hypot2() > 0.0)
            .map(|edge| edge.turn_90())
    }

    /// Return the least and greatest projection of the corners onto `axis`.
    fn span_along(&self, axis: Vec2) -> (f64, f64) {
        self.corners
            .iter()
            .map(|corner| corner.to_vec2().dot(axis))
            .fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(min, max), projection| (min.min(projection), max.max(projection)),
            )
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;
    use kurbo::{RoundedRect, Size};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::geometry::projection::IsometricProjection;
    use crate::metrics::{LabelRole, TextAnchor};

    fn plate_at(center: Point) -> PlatedLabel {
        PlatedLabel::centered_on("calls", center, &FixedAdvanceMeasure)
    }

    #[test]
    fn a_rect_inside_a_slanted_quads_bounding_box_overlaps_it_only_on_the_quad() {
        let slanted_plane = IsometricProjection::plane(DVec3::ZERO, DVec3::X, DVec3::Y);
        let line_box = Rect::new(0.0, 0.0, 200.0, 10.0);
        let slanted = ScreenPolygon::mapped_rect(slanted_plane, line_box);
        let small_size = Size::new(4.0, 4.0);
        let off_the_quad_center = Point::new(150.0, 20.0);
        let off_the_quad_box = Rect::from_center_size(off_the_quad_center, small_size);
        let off_the_quad = ScreenPolygon::mapped_rect(Affine::IDENTITY, off_the_quad_box);
        let on_the_axis_center = Point::new(86.6, 50.0);
        let on_the_axis_box = Rect::from_center_size(on_the_axis_center, small_size);
        let on_the_axis = ScreenPolygon::mapped_rect(Affine::IDENTITY, on_the_axis_box);
        let slanted_bounds = slanted_plane.transform_rect_bbox(line_box);
        assert!(slanted_bounds.contains(off_the_quad_center));
        assert!(!slanted.overlaps(&off_the_quad));
        assert!(slanted.overlaps(&on_the_axis));
        assert!(on_the_axis.overlaps(&slanted));
    }

    #[test]
    fn recorded_plates_obstruct_later_plates_that_overlap_them() {
        let mut obstructions = Obstructions::around_level(&[]);
        let first = plate_at(Point::ORIGIN);
        let first_obstruction_count = obstructions.count_overlapped_by(&first);
        obstructions.record(&first);
        let overlapping_center = Point::new(10.0, 5.0);
        let far_below_center = Point::new(0.0, 40.0);
        let overlapping = plate_at(overlapping_center);
        let far_below = plate_at(far_below_center);
        let overlapping_obstruction_count = obstructions.count_overlapped_by(&overlapping);
        let far_below_obstruction_count = obstructions.count_overlapped_by(&far_below);
        assert_eq!(first_obstruction_count, 0);
        assert_eq!(overlapping_obstruction_count, 1);
        assert_eq!(far_below_obstruction_count, 0);
    }

    /// Shrink the plate of a plated label centred on `center` to a 1 × 1 square.
    fn tiny_plate_at(center: Point) -> PlatedLabel {
        let plated_label = plate_at(center);
        let tiny_rect = Rect::from_center_size(center, Size::new(1.0, 1.0));
        let tiny_plate = RoundedRect::from_rect(tiny_rect, 0.5);
        PlatedLabel::new(plated_label.label, tiny_plate)
    }

    #[test]
    fn a_plate_laid_on_a_plane_obstructs_along_its_plane_not_its_bounding_box() {
        let mut obstructions = Obstructions::around_level(&[]);
        let ground = IsometricProjection::horizontal_plane_at(0.0);
        let flat = plate_at(Point::ORIGIN).on_plane(ground);
        obstructions.record(&flat);
        let flat_bounds = flat.bounds();
        let bounds_corner_off_the_plate = Point::new(flat_bounds.x0 + 1.0, flat_bounds.y0 + 1.0);
        let off_the_plate = tiny_plate_at(bounds_corner_off_the_plate);
        let on_the_plate = tiny_plate_at(Point::ORIGIN);
        let off_the_plate_count = obstructions.count_overlapped_by(&off_the_plate);
        let on_the_plate_count = obstructions.count_overlapped_by(&on_the_plate);
        assert_eq!(off_the_plate_count, 0);
        assert_eq!(on_the_plate_count, 1);
    }

    #[test]
    fn the_least_obstructed_candidate_is_the_first_of_those_overlapping_fewest() {
        let mut obstructions = Obstructions::around_level(&[]);
        let blocker = plate_at(Point::ORIGIN);
        obstructions.record(&blocker);
        let blocked = plate_at(Point::new(5.0, 0.0));
        let first_clear = plate_at(Point::new(0.0, 40.0));
        let second_clear = plate_at(Point::new(0.0, 80.0));
        let candidates = [blocked, first_clear.clone(), second_clear];

        let least_obstructed = obstructions.least_obstructed(candidates);

        assert_eq!(least_obstructed, Some(first_clear));
    }

    #[test]
    fn interior_labels_obstruct_through_the_interior_frame() {
        let element = ElementPath::from_segments(["desk", "relay"]);
        let caption_size = LabelRole::Caption.measure_text("co-signs", &FixedAdvanceMeasure);
        let caption = Label::new(
            "co-signs",
            LabelRole::Caption,
            TextAnchor::Middle,
            Point::ORIGIN,
            caption_size,
        );
        let relay = Solid::new(element, Vec::new()).with_label(caption);
        let interior_offset = Vec2::new(300.0, 100.0);
        let interior_frame = Affine::translate(interior_offset);
        let mut interior = Group::new(interior_frame, Rect::ZERO);
        interior.solids.push(relay);
        let desk_element = ElementPath::from_segments(["desk"]);
        let desk = Solid::new(desk_element, Vec::new()).with_interior(interior);
        let obstructions = Obstructions::around_level(&[desk]);
        let mapped_caption_center = Point::new(300.0, 97.0);
        let unmapped_caption_center = Point::new(0.0, -3.0);
        let over_the_mapped_caption = plate_at(mapped_caption_center);
        let over_the_unmapped_caption = plate_at(unmapped_caption_center);
        let mapped_obstruction_count = obstructions.count_overlapped_by(&over_the_mapped_caption);
        let unmapped_obstruction_count =
            obstructions.count_overlapped_by(&over_the_unmapped_caption);
        assert_eq!(mapped_obstruction_count, 1);
        assert_eq!(unmapped_obstruction_count, 0);
    }
}
