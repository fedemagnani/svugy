//! Project plan-space boxes into the isometric scene graph.

use std::collections::{BTreeMap, BTreeSet};
use std::iter;

use kurbo::{Affine, BezPath, Line, Point, Rect, RoundedRect, Shape as _, Size, Vec2};
use svugy_model::disclose::DisclosedModel;
use svugy_model::model::{EdgeId, Model};
use svugy_model::path::ElementPath;

use crate::layout::{LevelPlacement, LevelPlan, LevelPlans};
use crate::metrics::{LabelRole, TextAnchor, TextMeasure, TextSize, TextSpec};

use self::ground::GroundGrid;
use self::level::LevelProjector;
use self::projection::IsometricProjection;

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

/// Draws the whole diagram in document space as one scene: the ground grid, the root level's group, the diagram's heading and its legend.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Scene {
    /// Projected ground-grid lines under the root level, painted before everything else.
    pub(crate) ground_lines: Vec<Line>,
    pub(crate) root: Group,
    /// Local rectangle the viewBox frames once each level is entered, keyed by level; the root's is the document viewBox.
    views_by_level: BTreeMap<ElementPath, Rect>,
    /// Diagram title and subtitle, upright in document space, when the model sets either.
    pub(crate) heading: Option<Heading>,
    /// Legend, upright in document space, when the model has one with anything to draw.
    pub(crate) legend: Option<LegendPanel>,
}

impl Scene {
    /// Screen margin a view keeps around what it frames.
    const VIEW_MARGIN: f64 = 24.0;

    /// Draw `root` on the ground `ground_lines` rule, each level framed by its view in `views_by_level`, with no heading and no legend.
    pub(crate) fn new(
        ground_lines: Vec<Line>,
        root: Group,
        views_by_level: BTreeMap<ElementPath, Rect>,
    ) -> Self {
        Self {
            ground_lines,
            root,
            views_by_level,
            heading: None,
            legend: None,
        }
    }

    /// Project every level `plans` lays out for `model` into one scene, closing no container.
    pub(crate) fn project(model: &Model, plans: &LevelPlans, measure: &impl TextMeasure) -> Self {
        let no_closed_containers = BTreeSet::new();
        Self::project_closing(model, &no_closed_containers, plans, measure)
    }

    /// Project one disclosure layer, laid out by `plans`, into one scene, stacking a sheet under each container it closes.
    pub(crate) fn project_disclosed(
        disclosed: &DisclosedModel,
        plans: &LevelPlans,
        measure: &impl TextMeasure,
    ) -> Self {
        let disclosed_model = disclosed.model();
        let closed_containers = disclosed.closed_containers();
        Self::project_closing(disclosed_model, closed_containers, plans, measure)
    }

    /// Project every level `plans` lays out into one scene: the ground grid under the root, each container's interior stacked on its solid, a sheet under each of `closed_containers`, each edge arced in its home level or badged on its source, every container's stub markers over it, every level's view framed around what it draws, then the diagram's heading and legend above it all.
    fn project_closing(
        model: &Model,
        closed_containers: &BTreeSet<ElementPath>,
        plans: &LevelPlans,
        measure: &impl TextMeasure,
    ) -> Self {
        let root_path = ElementPath::root();
        let root_plan = plans
            .level(&root_path)
            .expect("layout plans the root level");
        let projector = LevelProjector::new(model, closed_containers, plans, measure);
        let mut root = projector.project_level(&root_path, root_plan, Affine::IDENTITY);
        let mut stub_markers_by_container =
            root.plate_stub_markers_per_container(model, plans, measure);
        root.hand_stub_markers_to_interiors(&mut stub_markers_by_container);
        let views_by_level = Self::frame_every_level(&root, plans);
        let root_placed = root_plan.placed();
        let root_boundary = root_placed.boundary();
        let ground_grid = GroundGrid::covering(root_boundary);
        let ground_lines = ground_grid.lines();
        let mut scene = Self::new(ground_lines, root, views_by_level);
        let content_view = scene.root_view();
        let heading = Heading::set_above(content_view, model, measure);
        if let Some(heading) = heading {
            let heading_bounds = heading.bounds();
            scene.widen_root_view_to_frame(heading_bounds);
            scene = scene.with_heading(heading);
        }
        let legend = LegendPanel::set_top_right_of(&scene, model, plans, measure);
        if let Some(legend) = legend {
            scene.widen_root_view_to_frame(legend.bounds);
            scene = scene.with_legend(legend);
        }
        scene
    }

    /// Frame the view of `root` and of every level nested in it, each laid out by `plans`, keyed by level.
    fn frame_every_level(root: &Group, plans: &LevelPlans) -> BTreeMap<ElementPath, Rect> {
        let root_level = (ElementPath::root(), root);
        let nested_levels = root.nested_interiors().map(|interior| {
            let level = interior.container.clone();
            (level, interior.group)
        });
        let levels = iter::once(root_level).chain(nested_levels);
        levels
            .map(|(level, group)| {
                let plan = plans
                    .level(&level)
                    .expect("layout plans every projected level");
                let view = group.framed_view(plan);
                (level, view)
            })
            .collect()
    }

    /// Widen the root view to frame `bounds` plus the view margin.
    fn widen_root_view_to_frame(&mut self, bounds: Rect) {
        let framed_bounds = bounds.inflate(Self::VIEW_MARGIN, Self::VIEW_MARGIN);
        let root_path = ElementPath::root();
        let root_view = self
            .views_by_level
            .get_mut(&root_path)
            .expect("every scene frames its root level");
        *root_view = root_view.union(framed_bounds);
    }

    /// Head the scene with `heading`.
    fn with_heading(mut self, heading: Heading) -> Self {
        self.heading = Some(heading);
        self
    }

    /// Set `legend` on the scene.
    fn with_legend(mut self, legend: LegendPanel) -> Self {
        self.legend = Some(legend);
        self
    }

    /// Return the root level's view, the document viewBox.
    pub(crate) fn root_view(&self) -> Rect {
        let root_path = ElementPath::root();
        self.view_of(&root_path)
    }

    /// Return the local rectangle the viewBox frames once `level` is entered.
    pub(crate) fn view_of(&self, level: &ElementPath) -> Rect {
        self.views_by_level[level]
    }
}

/// Groups one hierarchy level's drawables in its local screen frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Group {
    /// Maps this level's local screen space into its parent's; identity for the root.
    pub(crate) frame: Affine,
    /// Solids back to front, painted before every connection.
    pub(crate) solids: Vec<Solid>,
    /// Connections back to front, painted over the solids, their labels over every path.
    pub(crate) connections: Vec<Connection>,
    /// Stub markers keyed by external endpoint, painted last, plated once the whole scene is projected.
    pub(crate) stub_markers: BTreeMap<ElementPath, StubMarker>,
}

impl Group {
    /// Group no drawable yet in `frame`.
    pub(crate) fn new(frame: Affine) -> Self {
        Self {
            frame,
            solids: Vec::new(),
            connections: Vec::new(),
            stub_markers: BTreeMap::new(),
        }
    }

    /// Frame this level's view, laid out by `plan`: a margin around what it draws, nested levels included, and its stub tags, or around its floor when it draws nothing; the ground grid may bleed past the view.
    fn framed_view(&self, plan: &LevelPlan) -> Rect {
        let content_bounds = self.content_bounds();
        let drawn_bounds = content_bounds.unwrap_or_else(|| {
            let placed = plan.placed();
            let boundary = placed.boundary();
            let floor = IsometricProjection::horizontal_plane_at(0.0);
            floor.transform_rect_bbox(boundary)
        });
        let stub_tag_bounds = self
            .stub_markers
            .values()
            .map(|stub_marker| stub_marker.tag.bounds());
        let framed_bounds =
            stub_tag_bounds.fold(drawn_bounds, |framed, tag_bounds| framed.union(tag_bounds));
        framed_bounds.inflate(Scene::VIEW_MARGIN, Scene::VIEW_MARGIN)
    }

    /// Return the bounds of this level's content: the union of every drawable of it and of its nested interiors, mapped through their frames, or `None` for an empty level.
    fn content_bounds(&self) -> Option<Rect> {
        let levels = self.this_and_nested_levels();
        let drawable_bounds = levels.flat_map(|(group, group_to_this)| {
            let drawables = group.drawables();
            drawables.map(move |drawable| {
                let local_bounds = drawable.bounds();
                group_to_this.transform_rect_bbox(local_bounds)
            })
        });
        drawable_bounds.reduce(|framed, bounds| framed.union(bounds))
    }

    /// Walk this level, then every interior nested in it, each with the frame mapping it into this level's space.
    pub(crate) fn this_and_nested_levels(&self) -> impl Iterator<Item = (&Group, Affine)> {
        let this_level = (self, Affine::IDENTITY);
        let nested_levels = self
            .nested_interiors()
            .map(|interior| (interior.group, interior.to_outer));
        iter::once(this_level).chain(nested_levels)
    }

    /// Walk every interior nested in this level, each with its frame composed into this level's space.
    pub(crate) fn nested_interiors(&self) -> NestedInteriors<'_> {
        NestedInteriors::below(&self.solids)
    }

    /// Yield every drawable this level draws itself: each solid's, then each connection's, interiors left out.
    fn drawables(&self) -> impl Iterator<Item = Drawable<'_>> {
        let solid_drawables = self.solids.iter().flat_map(Solid::drawables);
        let connection_drawables = self.connections.iter().flat_map(Connection::drawables);
        solid_drawables.chain(connection_drawables)
    }
}

/// Names one drawable shape a level draws itself, so framing its view and keeping placements clear of it list the same shapes.
#[derive(Clone, Copy)]
enum Drawable<'g> {
    Face(&'g Face),
    Label(&'g Label),
    Path(&'g BezPath),
    Plate(&'g PlatedLabel),
}

impl Drawable<'_> {
    /// Return the shape's screen bounds in its level's local space.
    fn bounds(&self) -> Rect {
        match self {
            Self::Face(face) => face.outline.bounding_box(),
            Self::Label(label) => label.bounds(),
            Self::Path(path) => path.bounding_box(),
            Self::Plate(plated_label) => plated_label.bounds(),
        }
    }

    /// Tell whether the shape sets text: a label or a plate.
    fn is_text(&self) -> bool {
        let is_label = matches!(self, Self::Label(_label));
        let is_plate = matches!(self, Self::Plate(_plate));
        is_label || is_plate
    }
}

/// Walks every interior nested under one level's solids, depth-first in paint order, composing frames on the way down.
pub(crate) struct NestedInteriors<'g> {
    pending: Vec<NestedInterior<'g>>,
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

/// Pairs one nested interior with the frame mapping its local space into the space of the level the walk started from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NestedInterior<'g> {
    /// Container whose solid the interior sits on.
    pub(crate) container: &'g ElementPath,
    pub(crate) group: &'g Group,
    /// Every frame from the starting level down to this interior, composed: the document's when the walk starts at the scene.
    pub(crate) to_outer: Affine,
}

/// Draws one element as a projected solid: its faces, then its interior level, then its labels, then the badges on its top.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Solid {
    pub(crate) element: ElementPath,
    /// Outline of the plate stacked under a closed container, hinting at the children it hides; painted before the faces.
    pub(crate) sheet: Option<BezPath>,
    /// Faces back to front.
    pub(crate) faces: Vec<Face>,
    /// Group of the container's own level, sitting on its top face.
    pub(crate) interior: Option<Group>,
    pub(crate) labels: Vec<Label>,
    /// Badges of the edges collapsed onto this solid's top face, in edge order.
    pub(crate) badges: Vec<Badge>,
}

impl Solid {
    /// Build the solid of `element` from its `faces`, with no sheet, interior, label or badge.
    pub(crate) fn new(element: ElementPath, faces: Vec<Face>) -> Self {
        Self {
            element,
            sheet: None,
            faces,
            interior: None,
            labels: Vec::new(),
            badges: Vec::new(),
        }
    }

    /// Stack a sheet with `outline` under the solid, marking it as a closed container.
    pub(crate) fn with_sheet(mut self, outline: BezPath) -> Self {
        self.sheet = Some(outline);
        self
    }

    /// Stack `interior` on the solid's top.
    pub(crate) fn with_interior(mut self, interior: Group) -> Self {
        self.interior = Some(interior);
        self
    }

    /// Add `label` after the solid's other labels.
    pub(crate) fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// Seat `badges` on the solid's top.
    fn with_badges(mut self, badges: Vec<Badge>) -> Self {
        self.badges = badges;
        self
    }

    /// Yield every drawable the solid draws itself: its sheet, its faces, its badges' faces, then its labels, its interior left out.
    fn drawables(&self) -> impl Iterator<Item = Drawable<'_>> {
        let sheet = self.sheet.iter().map(Drawable::Path);
        let badge_faces = self.badges.iter().flat_map(|badge| &badge.faces);
        let faces = self.faces.iter().chain(badge_faces).map(Drawable::Face);
        let labels = self.labels.iter().map(Drawable::Label);
        sheet.chain(faces).chain(labels)
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
    /// Badge `edge` as the `faces` of its target's mini.
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
    /// Fill `outline` in `shade` as one face.
    pub(crate) fn new(outline: BezPath, shade: FaceShade) -> Self {
        Self { outline, shade }
    }

    /// Return the bounds of `faces`: the union of their bounding boxes, or `None` when there is none.
    fn bounds_of(faces: &[Face]) -> Option<Rect> {
        let face_bounds = faces.iter().map(|face| face.outline.bounding_box());
        face_bounds.reduce(|bounds, next_bounds| bounds.union(next_bounds))
    }

    /// Return the face with its outline mapped by `affine`.
    fn transformed(&self, affine: Affine) -> Self {
        let outline = affine * &self.outline;
        Self::new(outline, self.shade)
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

    /// Stroke `edge`'s connection along `path`, with no dot path and no label.
    pub(crate) fn new(edge: EdgeId, path: BezPath) -> Self {
        Self {
            edge,
            path,
            dot_path: None,
            label: None,
        }
    }

    /// Move the connection's dot along `dot_path`.
    pub(crate) fn with_dot_path(mut self, dot_path: BezPath) -> Self {
        self.dot_path = Some(dot_path);
        self
    }

    /// Plate `label` on the connection.
    pub(crate) fn with_label(mut self, label: PlatedLabel) -> Self {
        self.label = Some(label);
        self
    }

    /// Yield the drawables the connection draws: its path, then its plate, if any.
    fn drawables(&self) -> impl Iterator<Item = Drawable<'_>> {
        let path = Drawable::Path(&self.path);
        let plate = self.label.as_ref().map(Drawable::Plate);
        iter::once(path).chain(plate)
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

/// Marks a boundary stub slot where edges leave the view toward one external endpoint.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StubMarker {
    pub(crate) edges: BTreeSet<EdgeId>,
    /// Plate lying flat on the slab inside the slot's stub band, labelled with the external endpoint.
    pub(crate) tag: PlatedLabel,
}

impl StubMarker {
    /// Tag the stub marker of `edges` with its plated `tag`.
    pub(crate) fn new(edges: BTreeSet<EdgeId>, tag: PlatedLabel) -> Self {
        Self { edges, tag }
    }
}

/// Sets the diagram's heading upright and start-anchored: its title over an optional subtitle, or its subtitle alone.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Heading {
    Titled {
        title: Label,
        subtitle: Option<Label>,
    },
    Subtitled(Label),
}

/// Sets the legend upright and start-anchored: its title, its entries, then one key row per described line style and badge kind the diagram draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LegendPanel {
    pub(crate) title: Option<Label>,
    pub(crate) entries: Vec<Label>,
    pub(crate) key_rows: Vec<KeyRow>,
    /// Screen bounds of every line and key row.
    pub(crate) bounds: Rect,
}

/// Keys one legend row: a sample of one way the diagram draws its edges, paired with the description the legend gives it.
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
    /// Gap between the measured text and the plate's edge, along and across the text.
    const PLATE_PADDING: Size = Size::new(10.0, 1.5);

    /// Plate `label` on `plate`.
    pub(crate) fn new(label: Label, plate: RoundedRect) -> Self {
        Self { label, plate }
    }

    /// Set `text` in `role`'s font and its pill-shaped plate, sized from the measured text, centered on `center`.
    fn centered_on(text: &str, role: LabelRole, center: Point, measure: &impl TextMeasure) -> Self {
        let text_size = role.measure_text(text, measure);
        let baseline_below_center = text_size.baseline_from_top - text_size.line_height / 2.0;
        let baseline_offset = Vec2::new(0.0, baseline_below_center);
        let baseline_origin = center + baseline_offset;
        let label = Label::new(text, role, TextAnchor::Middle, baseline_origin, text_size);
        let plate_width = text_size.width + 2.0 * Self::PLATE_PADDING.width;
        let plate_height = text_size.line_height + 2.0 * Self::PLATE_PADDING.height;
        let plate_size = Size::new(plate_width, plate_height);
        let plate_rect = Rect::from_center_size(center, plate_size);
        let plate = RoundedRect::from_rect(plate_rect, plate_height / 2.0);
        Self::new(label, plate)
    }

    /// Lay the label and its plate on `plane`, which maps their plane coordinates into the group's local screen space.
    fn with_plane(mut self, plane: Affine) -> Self {
        self.label.plane = plane;
        self
    }

    /// Return the screen bounding box of the plate and its label, both set on the label's plane.
    fn bounds(&self) -> Rect {
        let plate_rect = self.plate.rect();
        let plate_bounds = self.label.plane.transform_rect_bbox(plate_rect);
        let label_bounds = self.label.bounds();
        plate_bounds.union(label_bounds)
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

    /// Set the label on `plane`.
    pub(crate) fn with_plane(mut self, plane: Affine) -> Self {
        self.plane = plane;
        self
    }

    /// Return the label translated by `offset` in plane coordinates.
    fn translated(mut self, offset: Vec2) -> Self {
        self.baseline_origin += offset;
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
            let mut root = Group::new(Affine::IDENTITY);
            root.solids.push(a_solid);
            root.solids.push(b_solid);
            root.connections.push(connection);
            let views_by_level = BTreeMap::from([(ElementPath::root(), view)]);
            Self::new(Vec::new(), root, views_by_level)
        }
    }
}

#[cfg(test)]
mod tests {
    use kurbo::{ParamCurve, Shape};

    use super::*;

    #[test]
    fn fixture_view_contains_every_drawable() {
        let scene = Scene::two_solids_and_one_connection();
        let root_view = scene.root_view();
        let root = scene.root;
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
            let enclosing = root_view.union(bounds);
            assert_eq!(enclosing, root_view);
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
