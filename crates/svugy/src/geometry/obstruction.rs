//! Track what a plate, a badge or the legend must keep clear of in one view.

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Vec2};
use svugy_model::path::ElementPath;

use super::{
    Drawable, Face, Group, Heading, Label, NestedInterior, NestedInteriors, PlatedLabel, Scene,
    Solid,
};

/// Collects the obstructions a plate, a badge or the legend must not overlap in one view: the outlines of what that view draws and of what was placed before.
pub(super) struct Obstructions {
    outlines: Vec<ScreenPolygon>,
}

impl Obstructions {
    /// Screen gap a plate keeps from every obstruction.
    const CLEARANCE: f64 = 2.0;

    /// Collect the text `solids` draw: their labels, with every label and plate their nested interiors draw, mapped through their frames.
    pub(super) fn around_text_of(solids: &[Solid]) -> Self {
        let own_drawables = solids.iter().flat_map(Solid::drawables);
        let own_text_outlines = own_drawables
            .filter(Drawable::is_text)
            .map(|drawable| ScreenPolygon::of_drawable(drawable, Affine::IDENTITY));
        let nested_text_outlines = NestedInteriors::below(solids).flat_map(|interior| {
            let nested_drawables = interior.group.drawables();
            nested_drawables
                .filter(Drawable::is_text)
                .map(move |drawable| ScreenPolygon::of_drawable(drawable, interior.to_outer))
        });
        let outlines = own_text_outlines.chain(nested_text_outlines).collect();
        Self { outlines }
    }

    /// Collect, in `interior`'s frame, what its entered view draws over its slab: every drawable but connection paths and the faces of the slabs under its stubs, those of its container and of its container's ancestors.
    pub(super) fn around_stubs_of(root: &Group, interior: &NestedInterior) -> Self {
        let document_to_interior = interior.to_outer.inverse();
        let container = interior.container;
        let levels = root.this_and_nested_levels();
        let outlines = levels
            .flat_map(|(group, group_to_document)| {
                let to_interior = document_to_interior * group_to_document;
                let solid_drawables = group
                    .solids
                    .iter()
                    .flat_map(|solid| solid.drawables_over_slab_of(container));
                let connection_plates = group
                    .connections
                    .iter()
                    .filter_map(|connection| connection.label.as_ref())
                    .map(Drawable::Plate);
                let drawn_over_slab = solid_drawables.chain(connection_plates);
                drawn_over_slab
                    .map(move |drawable| ScreenPolygon::of_drawable(drawable, to_interior))
            })
            .collect();
        Self { outlines }
    }

    /// Collect in document space everything `scene`'s top-level view draws: every drawable of every level, and the heading's lines.
    pub(super) fn around_document(scene: &Scene) -> Self {
        let levels = scene.root.this_and_nested_levels();
        let level_outlines = levels.flat_map(|(group, group_to_document)| {
            let drawables = group.drawables();
            drawables.map(move |drawable| ScreenPolygon::of_drawable(drawable, group_to_document))
        });
        let heading_lines = scene.heading.iter().flat_map(Heading::lines);
        let heading_outlines =
            heading_lines.map(|label| ScreenPolygon::of_label(label, Affine::IDENTITY));
        let outlines = level_outlines.chain(heading_outlines).collect();
        Self { outlines }
    }

    /// Place the first of `candidates` that overlaps the fewest obstructions, else `fallback` when there is no candidate, recording it so later placements keep clear of it.
    pub(super) fn place_least_obstructed<O: Obstacle>(
        &mut self,
        candidates: impl IntoIterator<Item = O>,
        fallback: impl FnOnce() -> O,
    ) -> O {
        let candidates = candidates.into_iter();
        let least_obstructed =
            candidates.min_by_key(|candidate| self.count_overlapped_by(candidate));
        let placed = least_obstructed.unwrap_or_else(fallback);
        self.record(&placed);
        placed
    }

    /// Count the obstructions any of `obstacle`'s cleared outlines overlaps.
    pub(super) fn count_overlapped_by(&self, obstacle: &impl Obstacle) -> usize {
        let cleared_outlines = obstacle.cleared_outlines();
        self.outlines
            .iter()
            .filter(|outline| {
                let mut cleared_outline_iter = cleared_outlines.iter();
                cleared_outline_iter.any(|cleared_outline| cleared_outline.overlaps(outline))
            })
            .count()
    }

    /// Add `obstacle` to the obstructions, so later placements keep clear of it.
    pub(super) fn record(&mut self, obstacle: &impl Obstacle) {
        let screen_outlines = obstacle.screen_outlines();
        self.outlines.extend(screen_outlines);
    }
}

impl Solid {
    /// Yield what the solid draws over `container`'s slab: its text, and all it draws unless it is `container` or an ancestor of it, whose faces lie under the stubs.
    fn drawables_over_slab_of(
        &self,
        container: &ElementPath,
    ) -> impl Iterator<Item = Drawable<'_>> {
        let is_container = self.element == *container;
        let is_container_ancestor = self.element.is_ancestor_of(container);
        let is_under_stubs = is_container || is_container_ancestor;
        let is_beside_container = !is_under_stubs;
        let drawables = self.drawables();
        drawables.filter(move |drawable| {
            let is_text = drawable.is_text();
            is_text || is_beside_container
        })
    }
}

/// Outlines one obstacle checked against the obstructions: a plate, a badge's faces or an upright screen rect.
pub(super) trait Obstacle {
    /// Return the outlines the obstacle covers on the screen.
    fn screen_outlines(&self) -> Vec<ScreenPolygon>;

    /// Return the outlines the obstacle keeps clear of the obstructions, its screen outlines unless it keeps a clearance.
    fn cleared_outlines(&self) -> Vec<ScreenPolygon> {
        self.screen_outlines()
    }
}

impl Obstacle for PlatedLabel {
    fn screen_outlines(&self) -> Vec<ScreenPolygon> {
        let no_clearance = 0.0;
        let plate_outline =
            ScreenPolygon::of_plate_cleared_by(self, no_clearance, Affine::IDENTITY);
        vec![plate_outline]
    }

    /// Return the plate grown by the clearance on its plane.
    fn cleared_outlines(&self) -> Vec<ScreenPolygon> {
        let cleared_outline =
            ScreenPolygon::of_plate_cleared_by(self, Obstructions::CLEARANCE, Affine::IDENTITY);
        vec![cleared_outline]
    }
}

impl Obstacle for Vec<Face> {
    fn screen_outlines(&self) -> Vec<ScreenPolygon> {
        self.iter()
            .map(|face| ScreenPolygon::of_path(&face.outline, Affine::IDENTITY))
            .collect()
    }
}

impl Obstacle for Rect {
    fn screen_outlines(&self) -> Vec<ScreenPolygon> {
        let rect_outline = ScreenPolygon::mapped_rect(*self, Affine::IDENTITY);
        vec![rect_outline]
    }
}

/// Outlines a convex polygon in screen space, such as a label's line box set on its plane or a solid's face.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScreenPolygon {
    corners: Vec<Point>,
}

impl ScreenPolygon {
    /// Outline `drawable` as later placements keep clear of it, mapped into the view by `to_view`.
    fn of_drawable(drawable: Drawable, to_view: Affine) -> Self {
        match drawable {
            Drawable::Face(face) => Self::of_path(&face.outline, to_view),
            Drawable::Label(label) => Self::of_label(label, to_view),
            Drawable::Path(path) => Self::of_path(path, to_view),
            Drawable::Plate(plated_label) => {
                let no_clearance = 0.0;
                Self::of_plate_cleared_by(plated_label, no_clearance, to_view)
            }
        }
    }

    /// Outline `path` by its points, control points included so every projection of a curved face lies inside the outline's, mapped into the view by `to_view`.
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

    /// Outline `label`'s line box on its plane, mapped into the view by `to_view`.
    fn of_label(label: &Label, to_view: Affine) -> Self {
        let line_box = label.line_box();
        let label_to_view = to_view * label.plane;
        Self::mapped_rect(line_box, label_to_view)
    }

    /// Outline `plated_label`'s plate grown by `clearance` on its label's plane, mapped into the view by `to_view`.
    fn of_plate_cleared_by(plated_label: &PlatedLabel, clearance: f64, to_view: Affine) -> Self {
        let plate_rect = plated_label.plate.rect();
        let cleared_rect = plate_rect.inflate(clearance, clearance);
        let plate_to_view = to_view * plated_label.label.plane;
        Self::mapped_rect(cleared_rect, plate_to_view)
    }

    /// Map the corners of `rect` into screen space by `to_view`.
    fn mapped_rect(rect: Rect, to_view: Affine) -> Self {
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
                |(least, greatest), projection| {
                    let least = least.min(projection);
                    let greatest = greatest.max(projection);
                    (least, greatest)
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;
    use kurbo::{RoundedRect, Size};
    use svugy_model::path::ElementPath;

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::geometry::projection::IsometricProjection;
    use crate::metrics::{LabelRole, TextAnchor};

    fn plate_at(center: Point) -> PlatedLabel {
        PlatedLabel::centered_on(
            "calls",
            LabelRole::ConnectionLabel,
            center,
            &FixedAdvanceMeasure,
        )
    }

    #[test]
    fn a_rect_inside_a_slanted_quads_bounding_box_overlaps_it_only_on_the_quad() {
        let slanted_plane = IsometricProjection::plane(DVec3::ZERO, DVec3::X, DVec3::Y);
        let line_box = Rect::new(0.0, 0.0, 200.0, 10.0);
        let slanted = ScreenPolygon::mapped_rect(line_box, slanted_plane);
        let small_size = Size::new(4.0, 4.0);
        let off_the_quad_center = Point::new(150.0, 20.0);
        let off_the_quad_box = Rect::from_center_size(off_the_quad_center, small_size);
        let off_the_quad = ScreenPolygon::mapped_rect(off_the_quad_box, Affine::IDENTITY);
        let on_the_axis_center = Point::new(86.6, 50.0);
        let on_the_axis_box = Rect::from_center_size(on_the_axis_center, small_size);
        let on_the_axis = ScreenPolygon::mapped_rect(on_the_axis_box, Affine::IDENTITY);
        let slanted_bounds = slanted_plane.transform_rect_bbox(line_box);
        assert!(slanted_bounds.contains(off_the_quad_center));
        assert!(!slanted.overlaps(&off_the_quad));
        assert!(slanted.overlaps(&on_the_axis));
        assert!(on_the_axis.overlaps(&slanted));
    }

    #[test]
    fn recorded_plates_obstruct_later_plates_that_overlap_them() {
        let mut obstructions = Obstructions::around_text_of(&[]);
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
        let mut obstructions = Obstructions::around_text_of(&[]);
        let ground = IsometricProjection::horizontal_plane_at(0.0);
        let flat = plate_at(Point::ORIGIN).with_plane(ground);
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
        let mut obstructions = Obstructions::around_text_of(&[]);
        let blocker = plate_at(Point::ORIGIN);
        obstructions.record(&blocker);
        let blocked = plate_at(Point::new(5.0, 0.0));
        let first_clear = plate_at(Point::new(0.0, 40.0));
        let second_clear = plate_at(Point::new(0.0, 80.0));
        let candidates = [blocked, first_clear.clone(), second_clear];

        let least_obstructed =
            obstructions.place_least_obstructed(candidates, || panic!("candidates are given"));

        assert_eq!(least_obstructed, first_clear);
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
        let mut interior = Group::new(interior_frame);
        interior.solids.push(relay);
        let desk_element = ElementPath::from_segments(["desk"]);
        let desk = Solid::new(desk_element, Vec::new()).with_interior(interior);
        let obstructions = Obstructions::around_text_of(&[desk]);
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
