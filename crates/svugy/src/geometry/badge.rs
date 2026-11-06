//! Collapse edges into badges: minis of their targets seated on their sources' top faces.

use std::iter;
use std::slice;

use glam::DVec3;
use kurbo::{Affine, BezPath, Point, Rect, Shape as _, Size};
use svugy_model::model::{EdgeId, Element};

use super::obstruction::Obstructions;
use super::primitive::ResolvedPrimitive;
use super::projection::IsometricProjection;
use super::{Badge, Face, FaceShade, Solid};

/// Shapes a badge's icon: its target's primitive shrunk to a fixed screen width over a footprint of the target's proportions.
#[derive(Clone, Copy)]
pub(super) struct BadgeIcon {
    primitive: ResolvedPrimitive,
    footprint_size: Size,
    height: f64,
}

impl BadgeIcon {
    /// Screen width of every icon, as wide as the reference's SQLite badges (2 × 9.6).
    const SCREEN_WIDTH: f64 = 19.2;

    /// Shrink leaf `target`, laid out over `target_footprint`, by k = SCREEN_WIDTH / w, w the screen width of its full-size faces.
    pub(super) fn of_target(target: &Element, target_footprint: Rect) -> Self {
        let primitive = ResolvedPrimitive::of_element(target, false);
        let full_size_faces = primitive.faces_over(target_footprint, primitive.height);
        let full_size_bounds = Face::bounds_of(&full_size_faces);
        let full_size_width = full_size_bounds.map_or(0.0, |bounds| bounds.width());
        let is_degenerate = full_size_width <= 0.0;
        let scale = if is_degenerate {
            0.0
        } else {
            Self::SCREEN_WIDTH / full_size_width
        };
        let footprint_size = target_footprint.size() * scale;
        let height = primitive.height * scale;
        Self {
            primitive,
            footprint_size,
            height,
        }
    }

    /// Return the icon's faces over the plan footprint centred on `center`, its floor `floor` plan units up, back to front.
    pub(super) fn faces_centered_on(&self, center: Point, floor: f64) -> Vec<Face> {
        let footprint = Rect::from_center_size(center, self.footprint_size);
        let ground_faces = self.primitive.faces_over(footprint, self.height);
        let plan_floor_origin = DVec3::new(0.0, 0.0, floor);
        let floor_origin = IsometricProjection::project(plan_floor_origin);
        let raise = Affine::translate(floor_origin.to_vec2());
        ground_faces
            .into_iter()
            .map(|face| Face::new(raise * face.outline, face.shade))
            .collect()
    }

    /// Return the icon's faces with their screen bounding box centred on `screen_center`, upright in its own space, as a legend draws a sample.
    pub(super) fn faces_around(&self, screen_center: Point) -> Vec<Face> {
        let unplaced_faces = self.faces_centered_on(Point::ORIGIN, 0.0);
        let unplaced_bounds = Face::bounds_of(&unplaced_faces).unwrap_or(Rect::ZERO);
        let offset = screen_center - unplaced_bounds.center();
        let shift = Affine::translate(offset);
        unplaced_faces
            .into_iter()
            .map(|face| Face::new(shift * face.outline, face.shade))
            .collect()
    }
}

impl Solid {
    /// Seat a badge per edge of `badge_icons` on this solid's top face, standing `top_height` over `footprint`, clear of its labels and of each other.
    pub(super) fn with_badges(
        mut self,
        footprint: Rect,
        top_height: f64,
        badge_icons: Vec<(EdgeId, BadgeIcon)>,
    ) -> Self {
        let obstructions = Obstructions::around_level(slice::from_ref(&self));
        let top_face = self.faces.iter().find(|face| face.shade == FaceShade::Top);
        let top_outline = top_face.map(|face| face.outline.clone());
        let mut seater = BadgeSeater {
            footprint,
            top_height,
            top_outline,
            obstructions,
        };
        let badges = badge_icons.into_iter().map(|(edge, icon)| {
            let faces = seater.seat(&icon);
            Badge::new(edge, faces)
        });
        self.badges = badges.collect();
        self
    }
}

/// Seats icons one by one on a solid's top face, each clear of what is already seated or labelled there.
struct BadgeSeater {
    footprint: Rect,
    top_height: f64,
    top_outline: Option<BezPath>,
    obstructions: Obstructions,
}

impl BadgeSeater {
    /// Plan gap an icon keeps from the footprint's border and from its neighbours.
    const GAP: f64 = 3.0;

    /// Seat `icon` at the first spot from the east corner that stays on the top face and overlaps the fewest obstructions, else on the footprint's centre.
    fn seat(&mut self, icon: &BadgeIcon) -> Vec<Face> {
        let seats = self.seats_from_the_east_corner(icon.footprint_size);
        let seats_on_top = seats
            .into_iter()
            .filter(|&seat| self.is_on_top_face(seat, icon.footprint_size));
        let candidates = seats_on_top.map(|seat| icon.faces_centered_on(seat, self.top_height));
        let least_obstructed = self.obstructions.least_obstructed_faces(candidates);
        let footprint_center = self.footprint.center();
        let faces = least_obstructed
            .unwrap_or_else(|| icon.faces_centered_on(footprint_center, self.top_height));
        self.obstructions.record_faces(&faces);
        faces
    }

    /// Return plan centres on a lattice one icon plus a gap apart, row by row from the back edge (min y), each row from the east corner (max x) westward.
    fn seats_from_the_east_corner(&self, icon_size: Size) -> Vec<Point> {
        let Rect { x0, y0, x1, y1 } = self.footprint;
        let pitch_x = icon_size.width + Self::GAP;
        let pitch_y = icon_size.height + Self::GAP;
        let first_x = x1 - Self::GAP - icon_size.width / 2.0;
        let first_y = y0 + Self::GAP + icon_size.height / 2.0;
        let last_x = x0 + Self::GAP + icon_size.width / 2.0;
        let last_y = y1 - Self::GAP - icon_size.height / 2.0;
        let column_xs =
            iter::successors(Some(first_x), |x| Some(x - pitch_x)).take_while(|&x| x >= last_x);
        let row_ys: Vec<f64> = iter::successors(Some(first_y), |y| Some(y + pitch_y))
            .take_while(|&y| y <= last_y)
            .collect();
        let column_xs: Vec<f64> = column_xs.collect();
        row_ys
            .iter()
            .flat_map(|&y| column_xs.iter().map(move |&x| Point::new(x, y)))
            .collect()
    }

    /// Tell whether every corner of the icon's footprint centred on `seat`, raised to the top, projects inside the top face.
    fn is_on_top_face(&self, seat: Point, icon_size: Size) -> bool {
        let Some(top_outline) = &self.top_outline else {
            return false;
        };
        let icon_footprint = Rect::from_center_size(seat, icon_size);
        let Rect { x0, y0, x1, y1 } = icon_footprint;
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        corners.into_iter().all(|(x, y)| {
            let raised_corner = DVec3::new(x, y, self.top_height);
            let projected_corner = IsometricProjection::project(raised_corner);
            top_outline.contains(projected_corner)
        })
    }
}

impl Face {
    /// Return the union of the bounding boxes of `faces`, or `None` when there is none.
    pub(super) fn bounds_of(faces: &[Face]) -> Option<Rect> {
        let face_bounds = faces.iter().map(|face| face.outline.bounding_box());
        face_bounds.reduce(|bounds, next_bounds| bounds.union(next_bounds))
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::model::Shape;
    use svugy_model::path::ElementPath;

    use super::*;
    use crate::geometry::Scene;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::layout::LevelPlans;
    use crate::metrics::BundledFont;
    use crate::svugy_example::SvugyExample;

    const TOLERANCE: f64 = 1e-9;

    fn sqlite_icon() -> BadgeIcon {
        let sqlite = Element::new("SQLite").with_shape(Shape::Cylinder);
        let sqlite_footprint = Rect::new(0.0, 0.0, 63.0, 63.0);
        BadgeIcon::of_target(&sqlite, sqlite_footprint)
    }

    /// Return the plan centre of a badge's floor, found under its top face's centre one icon height down, at `top_height`.
    fn floor_center_of(badge: &Badge, icon: &BadgeIcon, top_height: f64) -> Point {
        let top_face = badge
            .faces
            .iter()
            .find(|face| face.shade == FaceShade::Top)
            .expect("every icon has a top");
        let top_center = top_face.outline.bounding_box().center();
        let floor_center = top_center + kurbo::Vec2::new(0.0, icon.height);
        let top_plane = IsometricProjection::horizontal_plane_at(top_height);
        top_plane.inverse() * floor_center
    }

    #[test]
    fn an_icon_is_its_targets_primitive_shrunk_to_the_badge_width_in_its_proportions() {
        let icon = sqlite_icon();

        let faces = icon.faces_centered_on(Point::ORIGIN, 0.0);

        let bounds = Face::bounds_of(&faces).expect("an icon has faces");
        assert!((bounds.width() - BadgeIcon::SCREEN_WIDTH).abs() < 1e-6);
        assert!((icon.footprint_size.width - icon.footprint_size.height).abs() < TOLERANCE);
        let height_per_side = icon.height / icon.footprint_size.width;
        assert!((height_per_side - 25.5 / 63.0).abs() < TOLERANCE);
        let shades: Vec<FaceShade> = faces.iter().map(|face| face.shade).collect();
        assert_eq!(shades, [FaceShade::Left, FaceShade::Top]);
    }

    #[test]
    fn badges_line_up_from_the_east_corner_on_the_top_face_clear_of_its_labels_and_each_other() {
        let path = ElementPath::from_segments(["quoter"]);
        let element = Element::new("quoter").with_caption("prices · signs");
        let footprint = Rect::new(0.0, 0.0, 132.0, 66.0);
        let primitive = ResolvedPrimitive::of_element(&element, false);
        let solid = primitive.draw(&path, &element, footprint, &FixedAdvanceMeasure);
        let labels_only = Obstructions::around_level(slice::from_ref(&solid));
        let icon = sqlite_icon();
        let edges = ["a", "b", "c"].map(|target| {
            let target_path = ElementPath::from_segments([target]);
            EdgeId::new(path.clone(), target_path, 0)
        });
        let badge_icons = edges.iter().map(|edge| (edge.clone(), icon)).collect();

        let badged_solid = solid.with_badges(footprint, primitive.height, badge_icons);

        let top_face = badged_solid.faces.last().expect("a block has a top");
        let mut seated = labels_only;
        let mut floor_centers = Vec::new();
        for badge in &badged_solid.badges {
            let overlap_count = seated.count_overlapped_by_faces(&badge.faces);
            assert_eq!(overlap_count, 0, "{}", badge.edge);
            seated.record_faces(&badge.faces);
            let floor_center = floor_center_of(badge, &icon, primitive.height);
            let floor = Rect::from_center_size(floor_center, icon.footprint_size);
            let corners = [floor.origin(), Point::new(floor.x1, floor.y1)];
            for corner in corners {
                let raised_corner = DVec3::new(corner.x, corner.y, primitive.height);
                let projected_corner = IsometricProjection::project(raised_corner);
                assert!(
                    top_face.outline.contains(projected_corner),
                    "{}",
                    badge.edge
                );
            }
            floor_centers.push(floor_center);
        }
        let first_center = floor_centers[0];
        let east_corner_center = Point::new(
            footprint.x1 - BadgeSeater::GAP - icon.footprint_size.width / 2.0,
            footprint.y0 + BadgeSeater::GAP + icon.footprint_size.height / 2.0,
        );
        assert!((first_center - east_corner_center).hypot2().sqrt() < 1e-6);
        for pair in floor_centers.windows(2) {
            let is_same_row = (pair[0].y - pair[1].y).abs() < 1e-6;
            let is_westward = pair[1].x < pair[0].x;
            assert!(is_same_row && is_westward, "{pair:?}");
        }
    }

    #[test]
    fn every_badge_of_the_example_stays_clear_of_its_sources_labels_and_other_badges() {
        let model = SvugyExample::model();
        let font = BundledFont::new();
        let plans = LevelPlans::lay_out(&model, &font);

        let scene = Scene::project(&model, &plans, &font);

        let interiors = scene.root.nested_interiors();
        let solids = interiors.flat_map(|interior| &interior.group.solids);
        let badged_solids: Vec<&Solid> = solids.filter(|solid| !solid.badges.is_empty()).collect();
        assert_eq!(badged_solids.len(), 2);
        for solid in badged_solids {
            let unbadged_solid = Solid {
                badges: Vec::new(),
                ..solid.clone()
            };
            let mut seated = Obstructions::around_level(slice::from_ref(&unbadged_solid));
            for badge in &solid.badges {
                let overlap_count = seated.count_overlapped_by_faces(&badge.faces);
                assert_eq!(overlap_count, 0, "{}", badge.edge);
                seated.record_faces(&badge.faces);
            }
        }
    }
}
