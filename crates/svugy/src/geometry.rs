//! Project plan-space boxes into the isometric scene graph.

use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Affine, BezPath, Line, Point, Rect, RoundedRect};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;

use crate::metrics::{LabelRole, TextAnchor, TextSize, TextSpec};

mod badge;
mod connection;
mod depth;
mod ground;
mod heading;
mod legend;
mod level;
mod obstruction;
mod primitive;
mod projection;
mod stub;

/// Draws the whole diagram in document space: the ground grid, the root level's group, the diagram's heading and its legend.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Scene {
    /// Projected ground-grid lines under the root level, painted before everything else.
    pub(crate) ground_lines: Vec<Line>,
    /// Root level, whose view is the document viewBox.
    pub(crate) root: Group,
    /// Diagram title and subtitle, upright in document space.
    pub(crate) heading: Heading,
    /// Legend, upright in document space, when the model has one with anything to draw.
    pub(crate) legend: Option<LegendPanel>,
}

impl Scene {
    /// Draw `root` on the ground `ground_lines` rule, with no heading and no legend yet.
    pub(crate) fn new(ground_lines: Vec<Line>, root: Group) -> Self {
        Self {
            ground_lines,
            root,
            heading: Heading::default(),
            legend: None,
        }
    }
}

/// Groups one hierarchy level's drawables in its local screen frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Group {
    /// Maps this level's local screen space into its parent's; identity for the root.
    pub(crate) frame: Affine,
    /// Local rectangle the viewBox frames once this level is entered.
    pub(crate) view: Rect,
    /// Solids back to front, painted before every connection.
    pub(crate) solids: Vec<Solid>,
    /// Connections back to front, painted over the solids, their labels over every path.
    pub(crate) connections: Vec<Connection>,
    /// Stub markers keyed by external endpoint, painted last, plated once the whole scene is projected.
    pub(crate) stub_markers: BTreeMap<ElementPath, StubMarker>,
}

impl Group {
    pub(crate) fn new(frame: Affine, view: Rect) -> Self {
        Self {
            frame,
            view,
            solids: Vec::new(),
            connections: Vec::new(),
            stub_markers: BTreeMap::new(),
        }
    }

    /// Walk every interior nested in this level, each with its frame composed into this level's space.
    pub(crate) fn nested_interiors(&self) -> NestedInteriors<'_> {
        NestedInteriors::below(&self.solids)
    }
}

/// Walks every interior nested under one level's solids, depth-first in paint order, composing frames on the way down.
pub(crate) struct NestedInteriors<'g> {
    pending: Vec<NestedInterior<'g>>,
}

/// Pairs one nested interior with the frame mapping its local space into the space of the level the walk started from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NestedInterior<'g> {
    /// Container whose solid the interior sits on.
    pub(crate) container: &'g ElementPath,
    pub(crate) group: &'g Group,
    /// Every frame from the starting level down to this interior, composed: the document's when the walk starts at the scene.
    pub(crate) to_outer: Affine,
}

impl<'g> NestedInteriors<'g> {
    /// Start below `solids`, the solids of the level every yielded frame maps into.
    pub(crate) fn below(solids: &'g [Solid]) -> Self {
        let mut walk = Self {
            pending: Vec::new(),
        };
        walk.push_interiors_of(solids, Affine::IDENTITY);
        walk
    }

    /// Queue the interiors of `solids`, last first so they pop in paint order.
    fn push_interiors_of(&mut self, solids: &'g [Solid], solids_to_outer: Affine) {
        let interiors = solids.iter().rev().filter_map(|solid| {
            let group = solid.interior.as_ref()?;
            let to_outer = solids_to_outer * group.frame;
            let nested_interior = NestedInterior {
                container: &solid.element,
                group,
                to_outer,
            };
            Some(nested_interior)
        });
        self.pending.extend(interiors);
    }
}

impl<'g> Iterator for NestedInteriors<'g> {
    type Item = NestedInterior<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        let nested_interior = self.pending.pop()?;
        let group = nested_interior.group;
        self.push_interiors_of(&group.solids, nested_interior.to_outer);
        Some(nested_interior)
    }
}

impl NestedInterior<'_> {
    /// Return the interior's view in the starting level's space, the document's when the walk starts at the scene.
    pub(crate) fn outer_view(&self) -> Rect {
        let view = self.group.view;
        self.to_outer.transform_rect_bbox(view)
    }
}

/// Draws one element as a projected solid: its faces, then its interior level, then its labels, then the badges on its top.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Solid {
    pub(crate) element: ElementPath,
    /// Faces back to front.
    pub(crate) faces: Vec<Face>,
    /// Group of the container's own level, sitting on its top face.
    pub(crate) interior: Option<Group>,
    pub(crate) labels: Vec<Label>,
    /// Badges of the edges collapsed onto this solid's top face, in edge order.
    pub(crate) badges: Vec<Badge>,
}

impl Solid {
    pub(crate) fn new(element: ElementPath, faces: Vec<Face>) -> Self {
        Self {
            element,
            faces,
            interior: None,
            labels: Vec::new(),
            badges: Vec::new(),
        }
    }

    pub(crate) fn with_interior(mut self, interior: Group) -> Self {
        self.interior = Some(interior);
        self
    }

    pub(crate) fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }
}

/// Draws one edge collapsed into a badge: a mini of its target's primitive standing on its source's top face, in the source's level frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Badge {
    pub(crate) edge: EdgeId,
    /// Faces of the mini, back to front, shaded like the target's own.
    pub(crate) faces: Vec<Face>,
}

impl Badge {
    pub(crate) fn new(edge: EdgeId, faces: Vec<Face>) -> Self {
        Self { edge, faces }
    }
}

/// Fills one face outline with a shade of its solid's base color.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Face {
    pub(crate) outline: BezPath,
    pub(crate) shade: FaceShade,
}

impl Face {
    pub(crate) fn new(outline: BezPath, shade: FaceShade) -> Self {
        Self { outline, shade }
    }
}

/// Selects which of the owner's three base-color shades fills a face; a curved side takes `Left`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FaceShade {
    Top,
    Left,
    Right,
}

/// Draws one edge's connection path in screen space, its arrowhead at the target.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Connection {
    pub(crate) edge: EdgeId,
    /// Smoothed path, already shortened so the arrowhead tip lands on the target port.
    pub(crate) path: BezPath,
    /// Path the animated dot travels, which the static profile drops.
    pub(crate) dot_path: Option<BezPath>,
    pub(crate) label: Option<PlatedLabel>,
}

impl Connection {
    /// Screen width every connection is stroked at, which also scales its arrowhead.
    pub(crate) const STROKE_WIDTH: f64 = 1.6;

    pub(crate) fn new(edge: EdgeId, path: BezPath) -> Self {
        Self {
            edge,
            path,
            dot_path: None,
            label: None,
        }
    }

    pub(crate) fn with_dot_path(mut self, dot_path: BezPath) -> Self {
        self.dot_path = Some(dot_path);
        self
    }

    pub(crate) fn with_label(mut self, label: PlatedLabel) -> Self {
        self.label = Some(label);
        self
    }
}

/// Shapes the arrowhead marker every connection ends in, in marker units, so geometry trims paths by its tip and render draws the same triangle.
pub(crate) struct ArrowheadMarker;

impl ArrowheadMarker {
    /// Side of the marker's square viewBox.
    pub(crate) const VIEW_BOX_SIZE: f64 = 10.0;
    /// Triangle corners: the upper back corner, the tip, the lower back corner.
    pub(crate) const TRIANGLE: [Point; 3] = [
        Point::new(0.0, 1.0),
        Point::new(9.0, 5.0),
        Point::new(0.0, 9.0),
    ];
    /// Marker point placed on the path's end.
    pub(crate) const REFERENCE: Point = Point::new(8.0, 5.0);
    /// Marker width and height, in stroke widths.
    pub(crate) const SIZE: f64 = 7.0;
    /// Screen distance the tip reaches past the path end: (tip.x − reference.x) / viewBox · size · stroke width.
    pub(crate) const TIP_OVERHANG: f64 = (Self::TRIANGLE[1].x - Self::REFERENCE.x)
        / Self::VIEW_BOX_SIZE
        * Self::SIZE
        * Connection::STROKE_WIDTH;
}

/// Marks a boundary slot where edges leave the view toward one external endpoint.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StubMarker {
    pub(crate) edges: BTreeSet<EdgeId>,
    /// Plate lying flat on the slab inside the slot's stub band, labelled with the external endpoint.
    pub(crate) tag: PlatedLabel,
}

impl StubMarker {
    pub(crate) fn new(edges: BTreeSet<EdgeId>, tag: PlatedLabel) -> Self {
        Self { edges, tag }
    }
}

/// Sets the diagram's title over its subtitle, upright and start-anchored, each line optional.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Heading {
    pub(crate) title: Option<Label>,
    pub(crate) subtitle: Option<Label>,
}

/// Sets the legend upright and start-anchored: its title, its entries, then one key row per described line style and badge kind the diagram draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LegendPanel {
    pub(crate) title: Option<Label>,
    pub(crate) entries: Vec<Label>,
    pub(crate) key_rows: Vec<KeyRow>,
}

/// Pairs a sample of one way the diagram draws its edges with the description the legend gives it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeyRow {
    pub(crate) sample: KeySample,
    pub(crate) description: Label,
    /// Screen bounds of the sample and the description together.
    pub(crate) bounds: Rect,
}

/// Samples how a set of edges is drawn: a stroke dashed like their arcs, or the icon of the badges of their target.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum KeySample {
    Stroke {
        line: Line,
        stroke_dash: f64,
        /// Edges whose arcs are dashed `stroke_dash` long, whose paint the sample shares when they all share one.
        edges: Vec<EdgeId>,
    },
    Badge {
        target: ElementPath,
        /// Faces of the target's mini, back to front.
        faces: Vec<Face>,
    },
}

/// Sets a label over a rounded plate on the label's plane, so it stays legible above paths.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlatedLabel {
    pub(crate) label: Label,
    /// Plate in the label's plane coordinates, mapped into screen space by the label's plane.
    pub(crate) plate: RoundedRect,
}

impl PlatedLabel {
    pub(crate) fn new(label: Label, plate: RoundedRect) -> Self {
        Self { label, plate }
    }

    /// Lay the label and its plate on `plane`, which maps their plane coordinates into the group's local screen space.
    pub(crate) fn on_plane(mut self, plane: Affine) -> Self {
        self.label.plane = plane;
        self
    }
}

/// Sets one line of label text, measured once, on its plane.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Label {
    pub(crate) text: String,
    pub(crate) role: LabelRole,
    pub(crate) anchor: TextAnchor,
    /// Baseline origin in plane coordinates.
    pub(crate) baseline_origin: Point,
    /// Line box of `text` set in `role`'s font, measured when the label was built.
    pub(crate) size: TextSize,
    /// Maps plane coordinates into the group's local screen space; identity keeps the label upright.
    pub(crate) plane: Affine,
}

impl Label {
    /// Set `text`, measured as `size` in `role`'s font, with its baseline origin at `baseline_origin`.
    pub(crate) fn new(
        text: impl Into<String>,
        role: LabelRole,
        anchor: TextAnchor,
        baseline_origin: Point,
        size: TextSize,
    ) -> Self {
        let text = text.into();
        Self {
            text,
            role,
            anchor,
            baseline_origin,
            size,
            plane: Affine::IDENTITY,
        }
    }

    pub(crate) fn with_plane(mut self, plane: Affine) -> Self {
        self.plane = plane;
        self
    }

    /// Return the request the label was measured with, so the outline matches the measurement.
    pub(crate) fn text_spec(&self) -> TextSpec<'_> {
        self.role.text_spec(&self.text)
    }

    /// Return the screen bounding box of the line box, set on its plane.
    pub(crate) fn bounds(&self) -> Rect {
        let line_box = self.line_box();
        self.plane.transform_rect_bbox(line_box)
    }

    /// Return the line box in plane coordinates, around the baseline origin as the anchor places it.
    pub(crate) fn line_box(&self) -> Rect {
        let TextSize {
            width,
            line_height,
            baseline_from_top,
        } = self.size;
        let width_before_origin = self.anchor.width_share_before_origin() * width;
        let left = self.baseline_origin.x - width_before_origin;
        let top = self.baseline_origin.y - baseline_from_top;
        let right = left + width;
        let bottom = top + line_height;
        Rect::new(left, top, right, bottom)
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use glam::DVec3;
    use kurbo::Vec2;

    use super::projection::IsometricProjection;
    use super::*;
    use crate::metrics::TextMeasure;

    /// Measures every character as 0.6 em wide, so geometry tests need no font.
    pub(crate) struct FixedAdvanceMeasure;

    impl TextMeasure for FixedAdvanceMeasure {
        fn measure(&self, spec: &TextSpec) -> TextSize {
            let character_count = spec.text.chars().count() as f64;
            TextSize {
                width: character_count * 0.6 * spec.size,
                line_height: 1.2 * spec.size,
                baseline_from_top: 0.95 * spec.size,
            }
        }
    }

    const BLOCK_HEIGHT: f64 = 20.0;

    impl Face {
        /// Fill the closed quadrilateral through `corners`.
        fn quad(corners: [Point; 4], shade: FaceShade) -> Self {
            let [first, second, third, fourth] = corners;
            let mut outline = BezPath::new();
            outline.move_to(first);
            outline.line_to(second);
            outline.line_to(third);
            outline.line_to(fourth);
            outline.close_path();
            Self::new(outline, shade)
        }
    }

    impl Solid {
        /// Draw a box over `footprint` as its left, right and top faces, titled with its path on the top face.
        fn box_over(element: ElementPath, footprint: Rect) -> Self {
            let Rect { x0, y0, x1, y1 } = footprint;
            let left_corners = [
                DVec3::new(x0, y1, 0.0),
                DVec3::new(x1, y1, 0.0),
                DVec3::new(x1, y1, BLOCK_HEIGHT),
                DVec3::new(x0, y1, BLOCK_HEIGHT),
            ]
            .map(IsometricProjection::project);
            let right_corners = [
                DVec3::new(x1, y0, 0.0),
                DVec3::new(x1, y1, 0.0),
                DVec3::new(x1, y1, BLOCK_HEIGHT),
                DVec3::new(x1, y0, BLOCK_HEIGHT),
            ]
            .map(IsometricProjection::project);
            let top_corners = [
                DVec3::new(x0, y0, BLOCK_HEIGHT),
                DVec3::new(x1, y0, BLOCK_HEIGHT),
                DVec3::new(x1, y1, BLOCK_HEIGHT),
                DVec3::new(x0, y1, BLOCK_HEIGHT),
            ]
            .map(IsometricProjection::project);
            let left_face = Face::quad(left_corners, FaceShade::Left);
            let right_face = Face::quad(right_corners, FaceShade::Right);
            let top_face = Face::quad(top_corners, FaceShade::Top);
            let faces = vec![left_face, right_face, top_face];
            let top_center = footprint.center();
            let title_plane_origin = DVec3::new(top_center.x, top_center.y, BLOCK_HEIGHT);
            let title_plane = IsometricProjection::plane(title_plane_origin, DVec3::X, DVec3::Y);
            let title_text = element.to_string();
            let title_baseline_origin = Point::new(0.0, -2.0);
            let title_size = LabelRole::Title.measure_text(&title_text, &FixedAdvanceMeasure);
            let title = Label::new(
                title_text,
                LabelRole::Title,
                TextAnchor::Middle,
                title_baseline_origin,
                title_size,
            )
            .with_plane(title_plane);
            Self::new(element, faces).with_label(title)
        }
    }

    impl Scene {
        /// Draw the root level with boxes `a` and `b` and one labelled, animated connection `a -> b`, on no ground grid.
        pub(crate) fn two_solids_and_one_connection() -> Self {
            let a = ElementPath::from_segments(["a"]);
            let b = ElementPath::from_segments(["b"]);
            let edge = EdgeId::new(a.clone(), b.clone(), 0);
            let a_footprint = Rect::new(20.0, 30.0, 80.0, 70.0);
            let b_footprint = Rect::new(140.0, 50.0, 200.0, 90.0);
            let a_solid = Solid::box_over(a, a_footprint);
            let b_solid = Solid::box_over(b, b_footprint);
            let plan_source_point = DVec3::new(80.0, 50.0, BLOCK_HEIGHT / 2.0);
            let source_point = IsometricProjection::project(plan_source_point);
            let plan_control_point = DVec3::new(110.0, 60.0, BLOCK_HEIGHT * 2.0);
            let control_point = IsometricProjection::project(plan_control_point);
            let plan_target_point = DVec3::new(140.0, 70.0, BLOCK_HEIGHT);
            let target_point = IsometricProjection::project(plan_target_point);
            let mut path = BezPath::new();
            path.move_to(source_point);
            path.quad_to(control_point, target_point);
            let label_size = LabelRole::ConnectionLabel.measure_text("calls", &FixedAdvanceMeasure);
            let label = Label::new(
                "calls",
                LabelRole::ConnectionLabel,
                TextAnchor::Middle,
                control_point,
                label_size,
            );
            let plate_half_size = Vec2::new(18.0, 8.0);
            let plate_center = control_point - Vec2::new(0.0, 4.0);
            let plate_rect = Rect::from_points(
                plate_center - plate_half_size,
                plate_center + plate_half_size,
            );
            let plate = RoundedRect::from_rect(plate_rect, plate_half_size.y);
            let plated_label = PlatedLabel::new(label, plate);
            let connection = Connection::new(edge, path.clone())
                .with_dot_path(path)
                .with_label(plated_label);
            let plan_west_corner = DVec3::new(0.0, 110.0, 0.0);
            let west_corner = IsometricProjection::project(plan_west_corner);
            let plan_east_corner = DVec3::new(220.0, 0.0, 0.0);
            let east_corner = IsometricProjection::project(plan_east_corner);
            let plan_north_corner = DVec3::new(0.0, 0.0, BLOCK_HEIGHT * 2.0);
            let north_corner = IsometricProjection::project(plan_north_corner);
            let plan_south_corner = DVec3::new(220.0, 110.0, 0.0);
            let south_corner = IsometricProjection::project(plan_south_corner);
            let view = Rect::from_points(west_corner, east_corner)
                .union_pt(north_corner)
                .union_pt(south_corner);
            let mut root = Group::new(Affine::IDENTITY, view);
            root.solids.push(a_solid);
            root.solids.push(b_solid);
            root.connections.push(connection);
            Self::new(Vec::new(), root)
        }
    }
}

#[cfg(test)]
mod tests {
    use kurbo::{ParamCurve, Shape};

    use super::*;

    #[test]
    fn fixture_view_contains_every_drawable() {
        let root = Scene::two_solids_and_one_connection().root;
        let face_bounds = root
            .solids
            .iter()
            .flat_map(|solid| &solid.faces)
            .map(|face| face.outline.bounding_box());
        let path_bounds = root
            .connections
            .iter()
            .map(|connection| connection.path.bounding_box());
        for bounds in face_bounds.chain(path_bounds) {
            let enclosing = root.view.union(bounds);
            assert_eq!(enclosing, root.view);
        }
    }

    #[test]
    fn fixture_connection_ends_on_its_target_solid() {
        let root = Scene::two_solids_and_one_connection().root;
        for connection in &root.connections {
            let last_segment = connection
                .path
                .segments()
                .last()
                .expect("connection has a segment");
            let end_point = last_segment.end();
            let target_solid = root
                .solids
                .iter()
                .find(|solid| &solid.element == connection.edge.to())
                .expect("target solid is drawn");
            let is_end_on_a_face = target_solid.faces.iter().any(|face| {
                face.outline
                    .bounding_box()
                    .inflate(1e-9, 1e-9)
                    .contains(end_point)
            });
            assert!(is_end_on_a_face);
        }
    }

    #[test]
    fn label_spec_is_its_role_font_applied_to_its_text() {
        let root = Scene::two_solids_and_one_connection().root;
        let label = &root.solids[0].labels[0];
        let spec = label.text_spec();
        let role_spec = label.role.text_spec("a");
        assert_eq!(spec, role_spec);
    }
}
