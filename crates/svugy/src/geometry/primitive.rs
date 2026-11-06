//! Turn an element's footprint into its primitive's faces and label site.

use std::array;

use glam::DVec3;
use kurbo::{Affine, BezPath, CubicBez, Point, Rect, Vec2};
use svugy_model::model::{Element, Shape};
use svugy_model::path::ElementPath;

use super::projection::IsometricProjection;
use super::{Face, FaceShade, Label, Solid};
use crate::metrics::{LabelBlock, LabelRole, TextAnchor, TextMeasure};

/// Resolves an element's shape to the primitive it is drawn as, the one place a model `Shape` becomes a `Primitive`.
#[derive(Clone, Copy)]
pub(super) struct ResolvedPrimitive {
    /// Height of the top face above the element's floor, where its connections attach and its interior sits.
    pub(super) height: f64,
    faces_over: fn(Rect, f64) -> Vec<Face>,
    label_site: fn(Rect, f64) -> LabelSite,
}

impl ResolvedPrimitive {
    /// Resolve a container to a slab whatever its authored shape and height, since its interior is stacked on a rectangular top, and a leaf to its authored primitive or a block.
    pub(super) fn of_element(element: &Element, is_container: bool) -> Self {
        if is_container {
            return Self::container_slab();
        }
        let shape = element.shape().unwrap_or(Shape::Box);
        match shape {
            Shape::Box => Self::leaf::<Block>(element),
            Shape::Slab => Self::leaf::<Slab>(element),
            Shape::Cylinder => Self::leaf::<Cylinder>(element),
            Shape::Disc => Self::leaf::<Disc>(element),
        }
    }

    /// Resolve a slab hanging its labels from its front face, since its interior covers its top.
    fn container_slab() -> Self {
        Self {
            height: Slab::DEFAULT_HEIGHT,
            faces_over: Slab::faces_over,
            label_site: LabelSite::hanging_from_front_left_face,
        }
    }

    /// Resolve `P` standing as tall as `element` sets, else at its default height, its labels on its own leaf site.
    fn leaf<P: Primitive>(element: &Element) -> Self {
        let authored_height = element.height();
        let height = authored_height.unwrap_or(P::DEFAULT_HEIGHT);
        Self {
            height,
            faces_over: P::faces_over,
            label_site: P::leaf_label_site,
        }
    }

    /// Draw `element` as this primitive extruded over `footprint`, its labels measured and set on the primitive's site.
    pub(super) fn draw(
        &self,
        path: &ElementPath,
        element: &Element,
        footprint: Rect,
        measure: &impl TextMeasure,
    ) -> Solid {
        let faces = self.faces_over(footprint, self.height);
        let label_site = (self.label_site)(footprint, self.height);
        let labels = label_site.labels(element, measure);
        let faced_solid = Solid::new(path.clone(), faces);
        labels.into_iter().fold(faced_solid, Solid::with_label)
    }

    /// Return the visible faces of this primitive's body extruded `height` tall over `footprint`, back to front.
    pub(super) fn faces_over(&self, footprint: Rect, height: f64) -> Vec<Face> {
        (self.faces_over)(footprint, height)
    }
}

/// Shapes an element's body over its footprint: the extrusion the viewer sees, how tall it stands unless authored otherwise, and the site its labels sit on.
pub(super) trait Primitive {
    /// Height of the top face above the ground, in plan units, when the element sets none.
    const DEFAULT_HEIGHT: f64;

    /// Body the primitive extrudes over its footprint.
    type Body: Extrusion;

    /// Return where a leaf's labels sit on its top `height` above the ground: centred on the top face, unless the primitive stands them elsewhere.
    fn leaf_label_site(footprint: Rect, height: f64) -> LabelSite {
        LabelSite::centered_on_top_face(footprint, height)
    }

    /// Return the visible faces of the body extruded `height` tall over `footprint`, back to front.
    fn faces_over(footprint: Rect, height: f64) -> Vec<Face> {
        let body = Self::Body::extrude(footprint, height);
        body.faces()
    }
}

/// Draws a component as a box.
pub(super) struct Block;

impl Primitive for Block {
    const DEFAULT_HEIGHT: f64 = 22.5;

    type Body = Cuboid;
}

/// Draws a container as a low box.
pub(super) struct Slab;

impl Primitive for Slab {
    const DEFAULT_HEIGHT: f64 = 10.5;

    type Body = Cuboid;
}

/// Draws a store as an upright cylinder.
pub(super) struct Cylinder;

impl Primitive for Cylinder {
    const DEFAULT_HEIGHT: f64 = 25.5;

    type Body = EllipticCylinder;

    fn leaf_label_site(footprint: Rect, height: f64) -> LabelSite {
        LabelSite::upright_over_top_face(footprint, height)
    }
}

/// Draws a round platform as a cylinder as low as a slab.
pub(super) struct Disc;

impl Primitive for Disc {
    const DEFAULT_HEIGHT: f64 = Slab::DEFAULT_HEIGHT;

    type Body = EllipticCylinder;
}

/// Extrudes a footprint into a body standing `height` tall and outlines the faces the viewer sees.
pub(super) trait Extrusion {
    fn extrude(footprint: Rect, height: f64) -> Self;

    /// Return the visible faces, back to front.
    fn faces(&self) -> Vec<Face>;
}

/// Extrudes a footprint into a box and outlines the three faces the viewer sees.
pub(super) struct Cuboid {
    footprint: Rect,
    height: f64,
}

impl Extrusion for Cuboid {
    fn extrude(footprint: Rect, height: f64) -> Self {
        Self { footprint, height }
    }

    /// Return the max-y face (screen left), the max-x face (screen right), then the top.
    fn faces(&self) -> Vec<Face> {
        let Rect { x0, y0, x1, y1 } = self.footprint;
        let height = self.height;
        let left_corners = [
            DVec3::new(x0, y1, 0.0),
            DVec3::new(x1, y1, 0.0),
            DVec3::new(x1, y1, height),
            DVec3::new(x0, y1, height),
        ];
        let right_corners = [
            DVec3::new(x1, y0, 0.0),
            DVec3::new(x1, y1, 0.0),
            DVec3::new(x1, y1, height),
            DVec3::new(x1, y0, height),
        ];
        let top_corners = [
            DVec3::new(x0, y0, height),
            DVec3::new(x1, y0, height),
            DVec3::new(x1, y1, height),
            DVec3::new(x0, y1, height),
        ];
        let left_face = Face::projected_quad(left_corners, FaceShade::Left);
        let right_face = Face::projected_quad(right_corners, FaceShade::Right);
        let top_face = Face::projected_quad(top_corners, FaceShade::Top);
        vec![left_face, right_face, top_face]
    }
}

impl Face {
    /// Fill the projection of the closed plan quadrilateral through `corners`.
    fn projected_quad(corners: [DVec3; 4], shade: FaceShade) -> Self {
        let [first, second, third, fourth] = corners.map(IsometricProjection::project);
        let mut outline = BezPath::new();
        outline.move_to(first);
        outline.line_to(second);
        outline.line_to(third);
        outline.line_to(fourth);
        outline.close_path();
        Self::new(outline, shade)
    }
}

/// Extrudes the ellipse inscribed in a footprint and outlines its visible side and its top.
pub(super) struct EllipticCylinder {
    base: InscribedEllipse,
    height: f64,
}

impl Extrusion for EllipticCylinder {
    fn extrude(footprint: Rect, height: f64) -> Self {
        let base = InscribedEllipse::inscribed_in(footprint);
        Self { base, height }
    }

    /// Return the curved side, bounded by the screen silhouette and both front half-ellipses, then the top.
    fn faces(&self) -> Vec<Face> {
        let ground = IsometricProjection::horizontal_plane_at(0.0);
        let roof = IsometricProjection::horizontal_plane_at(self.height);
        let quarter_arcs = self.base.quarter_arcs_from_screen_rightmost();
        let [
            rightmost_to_front,
            front_to_leftmost,
            _leftmost_to_back,
            _back_to_rightmost,
        ] = quarter_arcs;
        let bottom_rightmost_to_front = ground * rightmost_to_front;
        let bottom_front_to_leftmost = ground * front_to_leftmost;
        let top_rightmost_to_front = roof * rightmost_to_front;
        let top_front_to_leftmost = roof * front_to_leftmost;
        let mut side = BezPath::new();
        side.move_to(top_front_to_leftmost.p3);
        side.line_to(bottom_front_to_leftmost.p3);
        side.curve_to(
            bottom_front_to_leftmost.p2,
            bottom_front_to_leftmost.p1,
            bottom_front_to_leftmost.p0,
        );
        side.curve_to(
            bottom_rightmost_to_front.p2,
            bottom_rightmost_to_front.p1,
            bottom_rightmost_to_front.p0,
        );
        side.line_to(top_rightmost_to_front.p0);
        side.curve_to(
            top_rightmost_to_front.p1,
            top_rightmost_to_front.p2,
            top_rightmost_to_front.p3,
        );
        side.curve_to(
            top_front_to_leftmost.p1,
            top_front_to_leftmost.p2,
            top_front_to_leftmost.p3,
        );
        side.close_path();
        let top_arcs = quarter_arcs.map(|arc| roof * arc);
        let mut top = BezPath::new();
        top.move_to(top_arcs[0].p0);
        for arc in top_arcs {
            top.curve_to(arc.p1, arc.p2, arc.p3);
        }
        top.close_path();
        let side_face = Face::new(side, FaceShade::Left);
        let top_face = Face::new(top, FaceShade::Top);
        vec![side_face, top_face]
    }
}

/// Parameterises the plan ellipse inscribed in a footprint, split where the screen silhouette touches it.
struct InscribedEllipse {
    center: Point,
    semi_axes: Vec2,
}

impl InscribedEllipse {
    /// κ = 4/3·tan(π/8) = 4(√2 − 1)/3, the handle length per unit radius of the cubic closest to a quarter circle.
    const QUARTER_ARC_HANDLE: f64 = 0.552_284_749_830_793_4;

    fn inscribed_in(footprint: Rect) -> Self {
        let center = footprint.center();
        let semi_axes = Vec2::new(footprint.width() / 2.0, footprint.height() / 2.0);
        Self { center, semi_axes }
    }

    /// Return the quarters of E(t) = c + (a·cos t, b·sin t) from t₀ = argmax (a·cos t − b·sin t), where (cos t₀, sin t₀) = (a, −b)/√(a² + b²).
    fn quarter_arcs_from_screen_rightmost(&self) -> [CubicBez; 4] {
        let Vec2 { x: a, y: b } = self.semi_axes;
        let norm = (a * a + b * b).sqrt();
        let is_degenerate = norm == 0.0;
        let mut cos_sin = if is_degenerate {
            Vec2::new(1.0, 0.0)
        } else {
            Vec2::new(a / norm, -b / norm)
        };
        array::from_fn(|_quarter_index| {
            let arc = self.quarter_arc_from(cos_sin);
            cos_sin = cos_sin.turn_90();
            arc
        })
    }

    /// From t to t + π/2: P₀ = E(t), P₁ = P₀ + κ·E′(t), P₂ = P₃ + κ·(a·cos t, b·sin t), P₃ = E(t + π/2), with E′(t) = (−a·sin t, b·cos t).
    fn quarter_arc_from(&self, cos_sin: Vec2) -> CubicBez {
        let Vec2 { x: a, y: b } = self.semi_axes;
        let Vec2 { x: cos_t, y: sin_t } = cos_sin;
        let start_offset = Vec2::new(a * cos_t, b * sin_t);
        let start_tangent = Vec2::new(-a * sin_t, b * cos_t);
        let start = self.center + start_offset;
        let end = self.center + start_tangent;
        let start_handle = start + start_tangent * Self::QUARTER_ARC_HANDLE;
        let end_handle = end + start_offset * Self::QUARTER_ARC_HANDLE;
        CubicBez::new(start, start_handle, end_handle, end)
    }
}

/// Places an element's title on a plane of its solid, with its caption one line below.
pub(super) struct LabelSite {
    plane: Affine,
    anchor: TextAnchor,
    title_role: LabelRole,
    title_baseline_origin: Point,
}

impl LabelSite {
    /// Centre the title on the top face, reading along plan x, as the example titles its blocks.
    fn centered_on_top_face(footprint: Rect, height: f64) -> Self {
        let top_center = footprint.center();
        let origin = DVec3::new(top_center.x, top_center.y, height);
        let plane = IsometricProjection::plane(origin, DVec3::X, DVec3::Y);
        Self {
            plane,
            anchor: TextAnchor::Middle,
            title_role: LabelRole::Title,
            title_baseline_origin: LabelBlock::TOP_FACE_TITLE_BASELINE,
        }
    }

    /// Stand the title upright over the top face's centre, as the example titles its cylinders.
    fn upright_over_top_face(footprint: Rect, height: f64) -> Self {
        let top_center = footprint.center();
        let origin = DVec3::new(top_center.x, top_center.y, height);
        let projected_origin = IsometricProjection::project(origin);
        let plane = Affine::translate(projected_origin.to_vec2());
        Self {
            plane,
            anchor: TextAnchor::Middle,
            title_role: LabelRole::Title,
            title_baseline_origin: LabelBlock::UPRIGHT_TITLE_BASELINE,
        }
    }

    /// Hang the tag below the front-left face's top edge, from its west corner, as the example tags its slabs.
    fn hanging_from_front_left_face(footprint: Rect, height: f64) -> Self {
        let west_top_corner = DVec3::new(footprint.x0, footprint.y1, height);
        let plane = IsometricProjection::plane(west_top_corner, DVec3::X, DVec3::NEG_Z);
        Self {
            plane,
            anchor: TextAnchor::Start,
            title_role: LabelRole::ContainerTag,
            title_baseline_origin: LabelBlock::HANGING_TAG_BASELINE,
        }
    }

    /// Measure the element's label block and set its title, then its caption one line below.
    fn labels(&self, element: &Element, measure: &impl TextMeasure) -> Vec<Label> {
        let label_block = LabelBlock::measure(element, self.title_role, measure);
        let title_text = element.label();
        let title = Label::new(
            title_text,
            self.title_role,
            self.anchor,
            self.title_baseline_origin,
            label_block.title,
        )
        .with_plane(self.plane);
        let measured_caption = element.caption().zip(label_block.caption);
        let Some((caption_text, caption_size)) = measured_caption else {
            return vec![title];
        };
        let caption_line_offset = Vec2::new(0.0, LabelBlock::CAPTION_LINE_OFFSET);
        let caption_baseline_origin = self.title_baseline_origin + caption_line_offset;
        let caption = Label::new(
            caption_text,
            LabelRole::Caption,
            self.anchor,
            caption_baseline_origin,
            caption_size,
        )
        .with_plane(self.plane);
        vec![title, caption]
    }
}

#[cfg(test)]
mod tests {
    use kurbo::Shape as _;

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;

    const COS_30: f64 = 0.866_025_403_784_438_6;

    /// Holds a solid drawn by its resolved primitive, with that primitive's height.
    struct DrawnSolid {
        solid: Solid,
        height: f64,
    }

    impl DrawnSolid {
        fn resolved(
            path: &ElementPath,
            element: &Element,
            footprint: Rect,
            is_container: bool,
        ) -> Self {
            let primitive = ResolvedPrimitive::of_element(element, is_container);
            let solid = primitive.draw(path, element, footprint, &FixedAdvanceMeasure);
            let height = primitive.height;
            Self { solid, height }
        }
    }

    fn square_footprint(radius: f64) -> Rect {
        Rect::new(100.0, 40.0, 100.0 + 2.0 * radius, 40.0 + 2.0 * radius)
    }

    fn outline_points(face: &Face) -> Vec<Point> {
        face.outline
            .elements()
            .iter()
            .filter_map(|element| element.end_point())
            .collect()
    }

    #[test]
    fn block_draws_left_right_then_top_over_its_footprint() {
        let footprint = Rect::new(20.0, 30.0, 80.0, 70.0);
        let faces = Block::faces_over(footprint, Block::DEFAULT_HEIGHT);
        let shades: Vec<FaceShade> = faces.iter().map(|face| face.shade).collect();
        assert_eq!(shades, [FaceShade::Left, FaceShade::Right, FaceShade::Top]);
        let top_points = outline_points(&faces[2]);
        let north_corner =
            IsometricProjection::project(DVec3::new(20.0, 30.0, Block::DEFAULT_HEIGHT));
        let south_corner =
            IsometricProjection::project(DVec3::new(80.0, 70.0, Block::DEFAULT_HEIGHT));
        assert_eq!(top_points[0], north_corner);
        assert_eq!(top_points[2], south_corner);
        let left_points = outline_points(&faces[0]);
        let west_ground = IsometricProjection::project(DVec3::new(20.0, 70.0, 0.0));
        assert_eq!(left_points[0], west_ground);
    }

    #[test]
    fn slab_is_a_lower_box_than_block() {
        let footprint = Rect::new(0.0, 0.0, 200.0, 100.0);
        let slab_faces = Slab::faces_over(footprint, Slab::DEFAULT_HEIGHT);
        let block_faces = Block::faces_over(footprint, Block::DEFAULT_HEIGHT);
        let slab_top = slab_faces[2].outline.bounding_box();
        let block_top = block_faces[2].outline.bounding_box();
        assert_eq!(slab_faces.len(), 3);
        assert_eq!(
            slab_top.y0 - block_top.y0,
            Block::DEFAULT_HEIGHT - Slab::DEFAULT_HEIGHT
        );
    }

    #[test]
    fn cylinder_top_is_the_projected_circle_with_the_example_proportions() {
        let radius = 31.5;
        let footprint = square_footprint(radius);
        let faces = Cylinder::faces_over(footprint, Cylinder::DEFAULT_HEIGHT);
        let shades: Vec<FaceShade> = faces.iter().map(|face| face.shade).collect();
        assert_eq!(shades, [FaceShade::Left, FaceShade::Top]);
        let top_bounds = faces[1].outline.bounding_box();
        let center = footprint.center();
        let projected_center =
            IsometricProjection::project(DVec3::new(center.x, center.y, Cylinder::DEFAULT_HEIGHT));
        let expected_rx = radius * 2.0_f64.sqrt() * COS_30;
        let expected_ry = radius * 2.0_f64.sqrt() * 0.5;
        assert!((top_bounds.x1 - projected_center.x - expected_rx).abs() < 1e-9);
        assert!((projected_center.x - top_bounds.x0 - expected_rx).abs() < 1e-9);
        assert!((top_bounds.y1 - projected_center.y - expected_ry).abs() < 1e-3 * radius);
        assert!((top_bounds.width() / 2.0 - 38.6).abs() < 0.05);
        assert!((top_bounds.height() / 2.0 - 22.3).abs() < 0.05);
    }

    #[test]
    fn cylinder_side_spans_the_silhouette_from_ground_to_top() {
        let radius = 20.0;
        let footprint = square_footprint(radius);
        let faces = Cylinder::faces_over(footprint, Cylinder::DEFAULT_HEIGHT);
        let side_bounds = faces[0].outline.bounding_box();
        let top_bounds = faces[1].outline.bounding_box();
        assert!((side_bounds.x0 - top_bounds.x0).abs() < 1e-9);
        assert!((side_bounds.x1 - top_bounds.x1).abs() < 1e-9);
        let lowest_ground_y = top_bounds.y1 + Cylinder::DEFAULT_HEIGHT;
        assert!((side_bounds.y1 - lowest_ground_y).abs() < 1e-3 * radius);
        let side_points = outline_points(&faces[0]);
        let top_left = side_points[0];
        let ground_left = side_points[1];
        assert_eq!(top_left.x, ground_left.x);
        assert!((ground_left.y - top_left.y - Cylinder::DEFAULT_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn elongated_footprint_keeps_its_silhouette_points_on_the_ellipse_extremes() {
        let footprint = Rect::new(0.0, 0.0, 120.0, 40.0);
        let faces = Disc::faces_over(footprint, Disc::DEFAULT_HEIGHT);
        let top_bounds = faces[1].outline.bounding_box();
        let top_points = outline_points(&faces[1]);
        let rightmost = top_points[0];
        let leftmost = top_points[2];
        assert!((rightmost.x - top_bounds.x1).abs() < 1e-9);
        assert!((leftmost.x - top_bounds.x0).abs() < 1e-9);
    }

    #[test]
    fn block_titles_and_captions_sit_on_the_top_face_plane() {
        let path = ElementPath::from_segments(["quoter"]);
        let element = Element::new("quoter").with_caption("prices · signs");
        let footprint = Rect::new(0.0, 0.0, 108.0, 66.0);
        let projected = DrawnSolid::resolved(&path, &element, footprint, false);
        let labels = &projected.solid.labels;
        assert_eq!(projected.height, Block::DEFAULT_HEIGHT);
        assert_eq!(labels.len(), 2);
        let top_center =
            IsometricProjection::project(DVec3::new(54.0, 33.0, Block::DEFAULT_HEIGHT));
        let [a, b, c, d, e, f] = labels[0].plane.as_coeffs();
        assert_eq!([a, b, c, d], [COS_30, 0.5, -COS_30, 0.5]);
        assert_eq!(Point::new(e, f), top_center);
        assert_eq!(labels[0].baseline_origin, Point::new(0.0, -2.0));
        assert_eq!(labels[0].role, LabelRole::Title);
        assert_eq!(labels[1].baseline_origin, Point::new(0.0, 11.0));
        assert_eq!(labels[1].role, LabelRole::Caption);
        assert_eq!(labels[1].text, "prices · signs");
    }

    #[test]
    fn an_authored_height_raises_every_leaf_primitive_its_top_and_its_title() {
        let path = ElementPath::from_segments(["app"]);
        let footprint = square_footprint(30.0);
        let authored_height = 84.0;
        let center = footprint.center();
        let plan_raised_center = DVec3::new(center.x, center.y, authored_height);
        let raised_center = IsometricProjection::project(plan_raised_center);
        for shape in [Shape::Box, Shape::Slab, Shape::Cylinder, Shape::Disc] {
            let element = Element::new("svugy-cli")
                .with_shape(shape)
                .with_height(authored_height);
            let projected = DrawnSolid::resolved(&path, &element, footprint, false);
            let top_face = projected.solid.faces.last().expect("every body has a top");
            let top_bounds = top_face.outline.bounding_box();
            let face_bounds = projected
                .solid
                .faces
                .iter()
                .map(|face| face.outline.bounding_box())
                .reduce(|bounds, next_bounds| bounds.union(next_bounds))
                .expect("every body has faces");
            let [_a, _b, _c, _d, e, f] = projected.solid.labels[0].plane.as_coeffs();
            assert_eq!(projected.height, authored_height, "{shape:?}");
            assert!(
                (top_bounds.center().y - raised_center.y).abs() < 1e-9,
                "{shape:?}"
            );
            let side_drop = face_bounds.y1 - top_bounds.y1;
            assert!((side_drop - authored_height).abs() < 1e-6, "{shape:?}");
            assert_eq!(Point::new(e, f), raised_center, "{shape:?}");
        }
    }

    #[test]
    fn a_container_ignores_its_authored_height() {
        let path = ElementPath::from_segments(["desk"]);
        let footprint = Rect::new(0.0, 0.0, 300.0, 200.0);
        let unraised = Element::new("desk");
        let raised = Element::new("desk").with_height(84.0);
        let unraised_projected = DrawnSolid::resolved(&path, &unraised, footprint, true);
        let raised_projected = DrawnSolid::resolved(&path, &raised, footprint, true);
        assert_eq!(raised_projected.height, Slab::DEFAULT_HEIGHT);
        assert_eq!(raised_projected.solid, unraised_projected.solid);
    }

    #[test]
    fn unset_shape_resolves_to_a_slab_for_a_container() {
        let path = ElementPath::from_segments(["desk"]);
        let element = Element::new("desk");
        let footprint = Rect::new(0.0, 0.0, 300.0, 200.0);
        let projected = DrawnSolid::resolved(&path, &element, footprint, true);
        let tag = &projected.solid.labels[0];
        assert_eq!(projected.height, Slab::DEFAULT_HEIGHT);
        assert_eq!(tag.role, LabelRole::ContainerTag);
        assert_eq!(tag.anchor, TextAnchor::Start);
        let [a, b, c, d, _e, _f] = tag.plane.as_coeffs();
        assert_eq!([a, b, c, d], [COS_30, 0.5, 0.0, 1.0]);
    }

    #[test]
    fn authored_shape_wins_over_the_default() {
        let path = ElementPath::from_segments(["sqlite"]);
        let element = Element::new("SQLite").with_shape(Shape::Cylinder);
        let footprint = square_footprint(31.5);
        let projected = DrawnSolid::resolved(&path, &element, footprint, false);
        assert_eq!(projected.height, Cylinder::DEFAULT_HEIGHT);
        assert_eq!(projected.solid.faces.len(), 2);
        assert_eq!(
            projected.solid.labels[0].plane.as_coeffs()[..4],
            [1.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn a_leaf_slab_titles_its_top_face_which_no_interior_covers() {
        let path = ElementPath::from_segments(["desk", "bus"]);
        let element = Element::new("event bus")
            .with_caption("PendingOpened")
            .with_shape(Shape::Slab);
        let footprint = Rect::new(0.0, 0.0, 300.0, 66.0);
        let projected = DrawnSolid::resolved(&path, &element, footprint, false);
        let title = &projected.solid.labels[0];
        let plan_top_center = DVec3::new(150.0, 33.0, Slab::DEFAULT_HEIGHT);
        let top_center = IsometricProjection::project(plan_top_center);
        let [a, b, c, d, e, f] = title.plane.as_coeffs();
        assert_eq!(projected.height, Slab::DEFAULT_HEIGHT);
        assert_eq!(title.role, LabelRole::Title);
        assert_eq!([a, b, c, d], [COS_30, 0.5, -COS_30, 0.5]);
        assert_eq!(Point::new(e, f), top_center);
    }

    #[test]
    fn a_container_of_any_shape_is_a_slab_hanging_its_tag_from_its_front_face() {
        let path = ElementPath::from_segments(["pond"]);
        let footprint = Rect::new(0.0, 0.0, 300.0, 200.0);
        let unshaped = Element::new("pond");
        let unshaped_projected = DrawnSolid::resolved(&path, &unshaped, footprint, true);
        for shape in [Shape::Box, Shape::Cylinder, Shape::Disc] {
            let element = Element::new("pond").with_shape(shape);
            let projected = DrawnSolid::resolved(&path, &element, footprint, true);
            let tag = &projected.solid.labels[0];
            let [a, b, c, d, _e, _f] = tag.plane.as_coeffs();
            assert_eq!(projected.height, Slab::DEFAULT_HEIGHT, "{shape:?}");
            assert_eq!(projected.solid, unshaped_projected.solid, "{shape:?}");
            assert_eq!(tag.role, LabelRole::ContainerTag);
            assert_eq!([a, b, c, d], [COS_30, 0.5, 0.0, 1.0]);
        }
    }
}
