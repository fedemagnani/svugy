//! Render the static and interactive profiles from one scene graph.

mod arrowhead;
mod interactive;
mod level_view;
mod paint;
mod static_profile;
mod traveler;

use kurbo::{BezPath, Line, Rect};
use svugy_model::disclose::DisclosedModel;
use svugy_model::model::Model;
use svugy_model::path::ElementPath;

use crate::geometry::{
    Badge, Connection, Face, FaceShade, Group, Heading, KeySample, Label, LegendPanel, PlatedLabel,
    Scene, Solid,
};
use crate::metrics::LabelRole;
use crate::render::contract::{ContractAnnotator, DiagramDetails, RuntimeContract};
use crate::render::theme::Token;
use crate::render::writer::css::CssValue;
use crate::render::writer::path_data::PathData;
use crate::render::writer::{Attributes, SvgWriter, ViewBox};
use arrowhead::ArrowheadMarkers;
pub(crate) use interactive::Interactive;
use level_view::LevelViews;
pub(crate) use paint::Paint;
use paint::{Dashes, ModelPaints, PlatePaint, SolidPaint};
pub(crate) use static_profile::Static;
use traveler::{ResolvedTraveler, TravelerSymbols};

/// Name the SVG namespace the root declares.
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
/// Round the canvas corners by this radius.
const CANVAS_CORNER_RADIUS: f64 = 14.0;
/// Draw connection strokes at this opacity.
const CONNECTION_OPACITY: f64 = 0.9;
/// Draw plate outlines at this stroke opacity.
const PLATE_STROKE_OPACITY: f64 = 0.55;
/// Draw captions at this opacity.
const CAPTION_OPACITY: f64 = 0.85;
/// Fill a closed container's sheet at this opacity.
const SHEET_FILL_OPACITY: f64 = 0.35;
/// Outline a closed container's sheet at this stroke opacity.
const SHEET_STROKE_OPACITY: f64 = 0.3;

/// Express an output profile: how a paint is written, how a label is drawn, whether live-only content is shown, what runtime embeds the document.
pub(crate) trait Profile {
    /// Write `token` as the CSS value this profile gives it.
    fn token_value(&self, token: Token) -> CssValue;

    /// Write `paint` as a CSS value: a token as the profile writes it, a literal as its color, no paint as `none`.
    fn paint_value(&self, paint: Paint) -> CssValue {
        match paint {
            Paint::Token(token) => self.token_value(token),
            Paint::Literal(color) => CssValue::color(color),
            Paint::None => CssValue::from_static("none"),
        }
    }

    /// Draw `label` on its plane, carrying the renderer's `presentation` attributes.
    fn write_label(&self, writer: &mut SvgWriter, label: &Label, presentation: Attributes);

    /// Emit what only a live viewer can show, which `write_live_content` draws.
    fn write_live_only(
        &self,
        writer: &mut SvgWriter,
        write_live_content: impl FnOnce(&mut SvgWriter),
    );

    /// Write the document's content through `write_content`, embedded in the runtime that drives it, if the profile has one.
    fn embed_in_runtime(&self, writer: &mut SvgWriter, write_content: impl FnOnce(&mut SvgWriter));
}

/// Renders scenes as SVG documents carrying the runtime contract, expressed in profile `P`.
pub(crate) struct Renderer<P> {
    profile: P,
}

/// Renders the interactive profile, the product: themed in both palettes, labels in the viewer's font, the runtime embedded.
pub(crate) type InteractiveRenderer = Renderer<Interactive>;

/// Renders the self-contained static profile in the dark palette.
pub(crate) type StaticRenderer<'f> = Renderer<Static<'f>>;

/// Pairs a disclosure layer with the scene projected from it: one layer of the interactive document.
pub(crate) struct LayerScene<'d> {
    disclosure: &'d DisclosedModel,
    scene: Scene,
}

/// Holds the definitions every layer of a document references: one arrowhead marker per connection paint, one symbol per traveler.
struct Definitions {
    arrowheads: ArrowheadMarkers,
    travelers: TravelerSymbols,
}

/// Writes one layer's scene in a single pass, painted from the model it draws, ending each arc in its shared arrowhead, moving each traveler as its shared symbol and framing each level in its collected view.
struct LayerPass<'r, 'm, P> {
    profile: &'r P,
    scene: &'r Scene,
    model: &'m Model,
    paints: ModelPaints<'m>,
    annotator: ContractAnnotator<'m>,
    definitions: &'r Definitions,
    level_views: LevelViews,
}

impl<P: Profile> Renderer<P> {
    /// Render scenes expressed in `profile`.
    pub(crate) fn new(profile: P) -> Self {
        Self { profile }
    }

    /// Write `scene`, projected whole from `model`, as a document of one unwrapped layer sized to its root view: the definitions, the canvas, then the layer's ground grid, heading, legend and root level.
    pub(crate) fn write_document(&self, model: &Model, scene: &Scene) -> String {
        let drawn_layer = (&scene.root, model);
        let definitions = Definitions::collect(&[drawn_layer]);
        let annotator = ContractAnnotator::new(model);
        let layer_pass = LayerPass::new(&self.profile, scene, model, annotator, &definitions);
        let document_view = scene.root_view();
        self.write_root(document_view, |writer| {
            definitions.write(writer, &self.profile);
            self.write_canvas(writer, document_view);
            layer_pass.write_layer(writer);
        })
    }

    /// Write the root sized to `view` around `write_content`, embedded in the profile's runtime.
    fn write_root(&self, view: Rect, write_content: impl FnOnce(&mut SvgWriter)) -> String {
        let mut writer = SvgWriter::new();
        let svg_attributes = Self::svg_attributes(view);
        writer.open_element("svg", &svg_attributes);
        self.profile.embed_in_runtime(&mut writer, write_content);
        writer.close_element();
        writer.finish()
    }

    /// Declare the root's namespace and contract version, its viewBox `view` and its size the view's.
    fn svg_attributes(view: Rect) -> Attributes {
        let view_box = ViewBox(view).to_string();
        let view_width = view.width();
        let view_height = view.height();
        Attributes::new()
            .with_text("xmlns", SVG_NAMESPACE)
            .with_text(RuntimeContract::CONTRACT, RuntimeContract::VERSION)
            .with_text("viewBox", &view_box)
            .with_number("width", view_width)
            .with_number("height", view_height)
    }

    /// Paint the canvas: a rounded rectangle filling `view`.
    fn write_canvas(&self, writer: &mut SvgWriter, view: Rect) {
        let canvas_paint = Paint::Token(Token::Canvas);
        let canvas_fill = self.profile.paint_value(canvas_paint);
        let view_width = view.width();
        let view_height = view.height();
        let canvas_attributes = Attributes::new()
            .with_number("x", view.x0)
            .with_number("y", view.y0)
            .with_number("width", view_width)
            .with_number("height", view_height)
            .with_number("rx", CANVAS_CORNER_RADIUS)
            .with_css_value("fill", &canvas_fill);
        writer.empty_element("rect", &canvas_attributes);
    }
}

impl InteractiveRenderer {
    /// Write `layer_scenes`, disclosed from `model`, as the interactive document sized to the canvas covering every layer's root view, so changing layer never changes its aspect ratio: the model's title, its diagram details, the definitions every layer shares, one canvas under every layer's root view, then one group per layer, the deepest shown so a viewer without scripts sees the whole diagram.
    pub(crate) fn write_layered_document(
        &self,
        model: &Model,
        layer_scenes: &[LayerScene],
    ) -> String {
        let drawn_layers: Vec<(&Group, &Model)> = layer_scenes
            .iter()
            .map(|layer_scene| {
                let layer_model = layer_scene.disclosure.model();
                (&layer_scene.scene.root, layer_model)
            })
            .collect();
        let definitions = Definitions::collect(&drawn_layers);
        let layer_passes: Vec<LayerPass<'_, '_, Interactive>> = layer_scenes
            .iter()
            .map(|layer_scene| {
                let layer_model = layer_scene.disclosure.model();
                let annotator = ContractAnnotator::new(model)
                    .with_disclosure(layer_scene.disclosure)
                    .with_descriptions();
                let scene = &layer_scene.scene;
                LayerPass::new(&self.profile, scene, layer_model, annotator, &definitions)
            })
            .collect();
        let root_views = layer_scenes
            .iter()
            .map(|layer_scene| layer_scene.scene.root_view());
        let canvas_view = root_views
            .reduce(|covered_view, root_view| covered_view.union(root_view))
            .expect("disclosure yields layer 0 at least");
        let deepest_layer_index = layer_scenes.len() - 1;
        self.write_root(canvas_view, |writer| {
            if let Some(title) = model.title() {
                let no_attributes = Attributes::new();
                writer.text_element("title", &no_attributes, title);
            }
            if let Some(diagram_details) = DiagramDetails::of(model) {
                diagram_details.write(writer);
            }
            definitions.write(writer, &self.profile);
            self.write_canvas(writer, canvas_view);
            for (layer_index, layer_pass) in layer_passes.iter().enumerate() {
                let layer_value = layer_index.to_string();
                let layer_attributes =
                    Attributes::new().with_text(RuntimeContract::LAYER, &layer_value);
                let is_deepest_layer = layer_index == deepest_layer_index;
                let shown_layer_attributes = if is_deepest_layer {
                    layer_attributes.with_text(RuntimeContract::SHOWN, "")
                } else {
                    layer_attributes
                };
                writer.open_element("g", &shown_layer_attributes);
                layer_pass.write_layer(writer);
                writer.close_element();
            }
        })
    }
}

impl<'d> LayerScene<'d> {
    /// Pair `disclosure` with `scene`, projected from its model.
    pub(crate) fn new(disclosure: &'d DisclosedModel, scene: Scene) -> Self {
        Self { disclosure, scene }
    }
}

impl Definitions {
    /// Collect the arrowheads and travelers of every layer of `drawn_layers`, each a root level and the model it is drawn from.
    fn collect(drawn_layers: &[(&Group, &Model)]) -> Self {
        let arrowheads = ArrowheadMarkers::collect(drawn_layers);
        let travelers = TravelerSymbols::collect(drawn_layers);
        Self {
            arrowheads,
            travelers,
        }
    }

    /// Write a `<defs>` holding the arrowhead markers, then the traveler symbols a live viewer moves, or nothing when neither was collected.
    fn write<P: Profile>(&self, writer: &mut SvgWriter, profile: &P) {
        let has_no_marker = self.arrowheads.is_empty();
        let has_no_traveler = self.travelers.is_empty();
        let has_no_definition = has_no_marker && has_no_traveler;
        if has_no_definition {
            return;
        }
        let no_attributes = Attributes::new();
        writer.open_element("defs", &no_attributes);
        self.arrowheads.write_markers(writer, profile);
        profile.write_live_only(writer, |writer| self.travelers.write_symbols(writer));
        writer.close_element();
    }
}

impl<'r, 'm, P: Profile> LayerPass<'r, 'm, P> {
    /// Set up the pass over layer `scene`, painted from `model`, annotated by `annotator`, referencing `definitions`.
    fn new(
        profile: &'r P,
        scene: &'r Scene,
        model: &'m Model,
        annotator: ContractAnnotator<'m>,
        definitions: &'r Definitions,
    ) -> Self {
        let paints = ModelPaints::new(model);
        let level_views = LevelViews::collect(scene);
        Self {
            profile,
            scene,
            model,
            paints,
            annotator,
            definitions,
            level_views,
        }
    }

    /// Write the layer's scene: its ground grid, heading, legend, then root level.
    fn write_layer(&self, writer: &mut SvgWriter) {
        for ground_line in &self.scene.ground_lines {
            self.write_ground_line(writer, ground_line);
        }
        if let Some(heading) = &self.scene.heading {
            self.write_heading(writer, heading);
        }
        if let Some(legend) = &self.scene.legend {
            self.write_legend(writer, legend);
        }
        let root = ElementPath::root();
        self.write_level(writer, &root, &self.scene.root);
    }

    /// Draw one line of the ground grid in the grid-line color.
    fn write_ground_line(&self, writer: &mut SvgWriter, ground_line: &Line) {
        let grid_paint = Paint::Token(Token::GridLine);
        let grid_stroke = self.profile.paint_value(grid_paint);
        let line_attributes = Attributes::new()
            .with_number("x1", ground_line.p0.x)
            .with_number("y1", ground_line.p0.y)
            .with_number("x2", ground_line.p1.x)
            .with_number("y2", ground_line.p1.y)
            .with_css_value("stroke", &grid_stroke);
        writer.empty_element("line", &line_attributes);
    }

    /// Write the heading: the diagram's title in text ink and its subtitle muted, outside every level group, so they stay in document space whatever level is entered.
    fn write_heading(&self, writer: &mut SvgWriter, heading: &Heading) {
        let title_ink = Paint::Token(Token::Text);
        let subtitle_ink = Paint::Token(Token::TextMuted);
        match heading {
            Heading::Titled { title, subtitle } => {
                self.write_label_in_ink(writer, title, title_ink);
                if let Some(subtitle) = subtitle {
                    self.write_label_in_ink(writer, subtitle, subtitle_ink);
                }
            }
            Heading::Subtitled(subtitle) => {
                self.write_label_in_ink(writer, subtitle, subtitle_ink);
            }
        }
    }

    /// Draw `label` in `ink`, a caption at the caption opacity.
    fn write_label_in_ink(&self, writer: &mut SvgWriter, label: &Label, ink: Paint) {
        let ink_fill = self.profile.paint_value(ink);
        let ink_presentation = Attributes::new().with_css_value("fill", &ink_fill);
        let is_caption = label.role == LabelRole::Caption;
        let presentation = if is_caption {
            ink_presentation.with_number("opacity", CAPTION_OPACITY)
        } else {
            ink_presentation
        };
        self.profile.write_label(writer, label, presentation);
    }

    /// Set the legend's title in text ink and its entries muted, then each key row's sample and its muted description, outside every level group like the heading.
    fn write_legend(&self, writer: &mut SvgWriter, legend: &LegendPanel) {
        let title_ink = Paint::Token(Token::Text);
        let line_ink = Paint::Token(Token::TextMuted);
        if let Some(title) = &legend.title {
            self.write_label_in_ink(writer, title, title_ink);
        }
        for entry in &legend.entries {
            self.write_label_in_ink(writer, entry, line_ink);
        }
        for key_row in &legend.key_rows {
            self.write_key_sample(writer, &key_row.sample);
            self.write_label_in_ink(writer, &key_row.description, line_ink);
        }
    }

    /// Draw a key sample as the diagram draws what it samples: a stroke dashed and painted like its arcs, or a badge shaded like its target.
    fn write_key_sample(&self, writer: &mut SvgWriter, sample: &KeySample) {
        match sample {
            KeySample::Stroke {
                line,
                stroke_dash,
                edges,
            } => {
                let sample_paint = self.paints.stroke_sample(edges);
                let dashes = Dashes::along_connection(*stroke_dash);
                let mut sample_path = BezPath::new();
                sample_path.move_to(line.p0);
                sample_path.line_to(line.p1);
                let path_data = PathData(&sample_path).to_string();
                let sample_attributes = self
                    .stroke_attributes(sample_paint, dashes)
                    .with_text("d", &path_data);
                writer.empty_element("path", &sample_attributes);
            }
            KeySample::Badge { target, faces } => {
                let target_paint = self.paints.solid(target);
                for face in faces {
                    self.write_face(writer, face, &target_paint);
                }
            }
        }
    }

    /// Stroke an unfilled path like a connection: in `paint`, at the connection width and opacity, dashed by `dashes` unless solid.
    fn stroke_attributes(&self, paint: Paint, dashes: Option<Dashes>) -> Attributes {
        let stroke = self.profile.paint_value(paint);
        Attributes::new()
            .with_text("fill", "none")
            .with_css_value("stroke", &stroke)
            .with_number("stroke-width", Connection::STROKE_WIDTH)
            .with_number("opacity", CONNECTION_OPACITY)
            .with_dash_array(dashes)
    }

    /// Paint one face of a solid in `solid_paint`: filled in its shade, outlined in the solid's outline.
    fn write_face(&self, writer: &mut SvgWriter, face: &Face, solid_paint: &SolidPaint) {
        let face_paint = solid_paint.face(face.shade);
        let face_fill = self.profile.paint_value(face_paint);
        let outline = solid_paint.outline();
        let outline_stroke = self.profile.paint_value(outline.paint);
        let outline_data = PathData(&face.outline).to_string();
        let face_attributes = Attributes::new()
            .with_text("d", &outline_data)
            .with_css_value("fill", &face_fill)
            .with_css_value("stroke", &outline_stroke)
            .with_number("stroke-width", outline.width)
            .with_dash_array(outline.dashes);
        writer.empty_element("path", &face_attributes);
    }

    /// Paint `group` as level `level`'s group, back to front: solids, connection arcs, their plated labels over every arc, then its overlay.
    fn write_level(&self, writer: &mut SvgWriter, level: &ElementPath, group: &Group) {
        let level_attributes = self.level_attributes(level, group);
        writer.open_element("g", &level_attributes);
        for solid in &group.solids {
            self.write_solid(writer, solid);
        }
        for connection in &group.connections {
            self.write_connection_arc(writer, connection);
        }
        for connection in &group.connections {
            self.write_connection_label(writer, connection);
        }
        self.profile
            .write_live_only(writer, |writer| self.write_overlay(writer, level, group));
        writer.close_element();
    }

    /// Carry level `level`'s path, its document view and its frame.
    fn level_attributes(&self, level: &ElementPath, group: &Group) -> Attributes {
        let level_value = level.to_string();
        let view = self.level_views.view_of(level);
        let view_value = ViewBox(view).to_string();
        Attributes::new()
            .with_text(RuntimeContract::LEVEL, &level_value)
            .with_text(RuntimeContract::VIEW, &view_value)
            .with_transform(group.frame)
    }

    /// Paint a solid as its element's group: its sheet, then its faces, then its interior level, then its labels, then its badges.
    fn write_solid(&self, writer: &mut SvgWriter, solid: &Solid) {
        let element_group = self.annotator.element_group(&solid.element);
        element_group.write_around(writer, |writer| {
            let solid_paint = self.paints.solid(&solid.element);
            if let Some(sheet) = &solid.sheet {
                self.write_sheet(writer, sheet, &solid_paint);
            }
            for face in &solid.faces {
                self.write_face(writer, face, &solid_paint);
            }
            if let Some(interior) = &solid.interior {
                self.write_level(writer, &solid.element, interior);
            }
            let label_ink = self.label_ink(solid, &solid_paint);
            for label in &solid.labels {
                self.write_label_in_ink(writer, label, label_ink);
            }
            for badge in &solid.badges {
                self.write_badge(writer, badge);
            }
        });
    }

    /// Paint a closed container's sheet faintly in `solid_paint`: its top face's fill and its ink's outline, both at low opacity.
    fn write_sheet(&self, writer: &mut SvgWriter, sheet: &BezPath, solid_paint: &SolidPaint) {
        let top_paint = solid_paint.face(FaceShade::Top);
        let sheet_fill = self.profile.paint_value(top_paint);
        let ink_paint = solid_paint.ink();
        let sheet_stroke = self.profile.paint_value(ink_paint);
        let outline = solid_paint.outline();
        let sheet_data = PathData(sheet).to_string();
        let sheet_attributes = Attributes::new()
            .with_text("d", &sheet_data)
            .with_css_value("fill", &sheet_fill)
            .with_number("fill-opacity", SHEET_FILL_OPACITY)
            .with_css_value("stroke", &sheet_stroke)
            .with_number("stroke-opacity", SHEET_STROKE_OPACITY)
            .with_number("stroke-width", outline.width);
        writer.empty_element("path", &sheet_attributes);
    }

    /// Ink a solid's labels: a hanging tag and its caption lie on the level's ground and take its ink, any other label lies on a face and takes the solid's.
    fn label_ink(&self, solid: &Solid, solid_paint: &SolidPaint) -> Paint {
        let is_tag_hanging = solid
            .labels
            .iter()
            .any(|label| label.role == LabelRole::ContainerTag);
        if is_tag_hanging {
            let enclosing_level = solid.element.enclosing_level();
            let ground = self.paints.ground(&enclosing_level);
            return ground.ink;
        }
        solid_paint.ink()
    }

    /// Paint a badge as its edge's group, so it carries the edge's details: its target's mini, shaded like the target.
    fn write_badge(&self, writer: &mut SvgWriter, badge: &Badge) {
        let edge_group = self.annotator.edge_group(&badge.edge);
        edge_group.write_around(writer, |writer| {
            let target = badge.edge.to();
            let target_paint = self.paints.solid(target);
            for face in &badge.faces {
                self.write_face(writer, face, &target_paint);
            }
        });
    }

    /// Paint a connection's arc as an edge group: its flow path, then its traveler.
    fn write_connection_arc(&self, writer: &mut SvgWriter, connection: &Connection) {
        let edge_group = self.annotator.edge_group(&connection.edge);
        edge_group.write_around(writer, |writer| {
            let connection_paint = self.paints.connection(&connection.edge);
            self.write_flow_path(writer, connection, connection_paint);
            self.profile.write_live_only(writer, |writer| {
                self.write_traveler(writer, connection, connection_paint);
            });
        });
    }

    /// Stroke a connection's flow path in `connection_paint`, dashed like its edge and ending in its arrowhead.
    fn write_flow_path(
        &self,
        writer: &mut SvgWriter,
        connection: &Connection,
        connection_paint: Paint,
    ) {
        let path_data = PathData(&connection.path).to_string();
        let marker_reference = self
            .definitions
            .arrowheads
            .marker_reference(connection_paint);
        let dashes = self.paints.connection_dashes(&connection.edge);
        let path_attributes = self
            .stroke_attributes(connection_paint, dashes)
            .with_text(RuntimeContract::FLOW, "")
            .with_text("d", &path_data)
            .with_text("marker-end", &marker_reference);
        writer.empty_element("path", &path_attributes);
    }

    /// Draw the traveler the connection's edge carries moving upright along its port-to-port path, painted in `connection_paint`, when the connection has one.
    fn write_traveler(
        &self,
        writer: &mut SvgWriter,
        connection: &Connection,
        connection_paint: Paint,
    ) {
        let Some(dot_path) = &connection.dot_path else {
            return;
        };
        let traveler = ResolvedTraveler::carried_along(&connection.edge, self.model);
        let traveler_paint = self.profile.paint_value(connection_paint);
        traveler.write_moving_along(writer, dot_path, &traveler_paint);
    }

    /// Paint a connection's plated label, if any, as a second group of the same edge, so it lies over every arc of the level.
    fn write_connection_label(&self, writer: &mut SvgWriter, connection: &Connection) {
        let Some(plated_label) = &connection.label else {
            return;
        };
        let edge_group = self.annotator.edge_group(&connection.edge);
        edge_group.write_around(writer, |writer| {
            let connection_paint = self.paints.connection(&connection.edge);
            let plate_paint = PlatePaint::over_arcs(connection_paint);
            self.write_plated_label(writer, plated_label, plate_paint);
        });
    }

    /// Paint a plate on its label's plane in `plate_paint`, then its label in the plate's ink.
    fn write_plated_label(
        &self,
        writer: &mut SvgWriter,
        plated_label: &PlatedLabel,
        plate_paint: PlatePaint,
    ) {
        let plate_rect = plated_label.plate.rect();
        let plate_width = plate_rect.width();
        let plate_height = plate_rect.height();
        let corner_radius = plated_label.plate.radii().top_left;
        let plate_fill = self.profile.paint_value(plate_paint.fill);
        let plate_outline = self.profile.paint_value(plate_paint.outline);
        let plate_attributes = Attributes::new()
            .with_number("x", plate_rect.x0)
            .with_number("y", plate_rect.y0)
            .with_number("width", plate_width)
            .with_number("height", plate_height)
            .with_number("rx", corner_radius)
            .with_css_value("fill", &plate_fill)
            .with_css_value("stroke", &plate_outline)
            .with_number("stroke-opacity", PLATE_STROKE_OPACITY)
            .with_transform(plated_label.label.plane);
        writer.empty_element("rect", &plate_attributes);
        self.write_label_in_ink(writer, &plated_label.label, plate_paint.ink);
    }

    /// Draw level `level`'s overlay, the last child of its group: one stub group per stub marker, its plate painted like the ground it lies on, shown once the level is entered.
    fn write_overlay(&self, writer: &mut SvgWriter, level: &ElementPath, group: &Group) {
        let overlay_attributes = Attributes::new().with_text(RuntimeContract::OVERLAY, "");
        writer.open_element("g", &overlay_attributes);
        let ground = self.paints.ground(level);
        let plate_paint = PlatePaint::on_ground(ground);
        for (external_endpoint, stub_marker) in &group.stub_markers {
            let stub_group = self
                .annotator
                .stub_group(external_endpoint, &stub_marker.edges);
            stub_group.write_around(writer, |writer| {
                self.write_plated_label(writer, &stub_marker.tag, plate_paint);
            });
        }
        writer.close_element();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::iter;

    use kurbo::{Affine, BezPath, Point, RoundedRect, Shape};
    use svugy_model::builder::{ElementRef, ModelBuilder};
    use svugy_model::color::Rgb;
    use svugy_model::model::{Edge, EdgeId, Element};
    use svugy_model::path::ElementPath;
    use svugy_parse::diagnostic::SourceLanguage;
    use svugy_parse::json::{JsonLanguage, JsonPrinter};

    use super::*;
    use crate::geometry::fixtures::FixedAdvanceMeasure;
    use crate::geometry::{FaceShade, NestedInterior, StubMarker};
    use crate::layout::{LevelPlan, LevelPlans};
    use crate::metrics::{BundledFont, TextAnchor};
    use crate::render::theme::FaceShades;
    use crate::{InteractiveSvg, RenderSvg, StaticSvg};

    const FORBIDDEN_STATIC_MARKUP: [&str; 6] = [
        "<text",
        "var(",
        "context-stroke",
        "<animate",
        "<style",
        "<script",
    ];
    const NESTED_DEPTH: usize = 3;
    const DEEP_NESTED_DEPTH: usize = 7;
    const INTERIOR_FRAME_COEFFICIENTS: [f64; 6] = [
        0.866_025_403_784_438_6,
        0.5,
        -0.866_025_403_784_438_6,
        0.5,
        60.123_45,
        10.000_4,
    ];
    const INTERIOR_FRAME_TRANSFORM: &str = " transform=\"matrix(0.866 0.5 -0.866 0.5 60.123 10)\">";
    const PEER_ORIGIN: Point = Point::new(0.0, 0.0);
    const PEER_SIZE: f64 = 20.0;
    const NEST_ORIGIN: Point = Point::new(40.0, 0.0);
    const NEST_SIZE: f64 = 120.0;
    const BOUNDS_TOLERANCE: f64 = 0.05;
    const HALF_OPACITY: f32 = 128.0 / 255.0;

    /// Build elements `a` and `b` joined by one labelled edge, as the fixture scene draws them.
    fn model_with_a_and_b(a_element: Element) -> Model {
        let mut builder = ModelBuilder::new();
        let a = builder.add_top_level("a", a_element);
        let b = builder.add_top_level("b", Element::new("b"));
        let calls = Edge::new().with_label("calls");
        builder.add_edge(&a, &b, calls);
        builder.build().model.expect("valid model")
    }

    /// Render `scene`, painted from `model`'s styles, in `profile`.
    fn render_scene(model: &Model, scene: &Scene, profile: impl Profile) -> String {
        let renderer = Renderer::new(profile);
        renderer.write_document(model, scene)
    }

    fn render_fixture_scene(model: &Model) -> String {
        let font = BundledFont::new();
        let scene = Scene::two_solids_and_one_connection();
        let profile = Static::new(&font);
        render_scene(model, &scene, profile)
    }

    fn render_interactive_fixture_scene(model: &Model) -> String {
        let scene = Scene::two_solids_and_one_connection();
        render_scene(model, &scene, Interactive)
    }

    #[test]
    fn fixture_scene_renders_as_a_static_document() {
        let a_element = Element::new("a");
        let model = model_with_a_and_b(a_element);
        let markup = render_fixture_scene(&model);
        insta::assert_snapshot!(markup);
    }

    #[test]
    fn static_document_parses_as_svg() {
        let a_element = Element::new("a");
        let model = model_with_a_and_b(a_element);
        let markup = render_fixture_scene(&model);
        let options = usvg::Options::default();
        let parse_result = usvg::Tree::from_str(&markup, &options);
        assert!(parse_result.is_ok(), "{parse_result:?}");
    }

    #[test]
    fn static_document_needs_no_css_script_font_or_animation() {
        let a_element = Element::new("a");
        let model = model_with_a_and_b(a_element);
        let markup = render_fixture_scene(&model);
        for forbidden in FORBIDDEN_STATIC_MARKUP {
            assert!(!markup.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn two_renders_are_byte_identical() {
        let a_element = Element::new("a");
        let model = model_with_a_and_b(a_element);
        let first_markup = render_fixture_scene(&model);
        let second_markup = render_fixture_scene(&model);
        assert_eq!(first_markup, second_markup);
    }

    #[test]
    fn hex_fill_style_recolors_its_solid_faces() {
        let red = Rgb::from_packed(0xff0000);
        let FaceShades { top, left, right } = FaceShades::from_base(red);
        let red_shade_fills = [top, left, right].map(|shade| format!("fill=\"{shade}\""));
        let plain_model = model_with_a_and_b(Element::new("a"));
        let red_element = Element::new("a").with_style("fill", "#ff0000");
        let red_model = model_with_a_and_b(red_element);
        let plain_markup = render_fixture_scene(&plain_model);
        let red_markup = render_fixture_scene(&red_model);
        for red_shade_fill in &red_shade_fills {
            assert!(red_markup.contains(red_shade_fill), "{red_shade_fill}");
            assert!(!plain_markup.contains(red_shade_fill), "{red_shade_fill}");
        }
    }

    /// Stroke level `level`'s connection in a color whose red channel grows with depth, so markers sort by level.
    fn level_stroke(level: usize) -> String {
        let red = level * 32;
        format!("#{red:02x}00ff")
    }

    /// Build a `depth`-level chain: each level holds a peer `p{level}` and a nest `n{level}`, joined by one stroked edge.
    fn nested_model(depth: usize) -> Model {
        let mut builder = ModelBuilder::new();
        let mut parent: Option<ElementRef> = None;
        for level in 0..depth {
            let peer_id = format!("p{level}");
            let nest_id = format!("n{level}");
            let peer_element = Element::new(&peer_id);
            let nest_element = Element::new(&nest_id);
            let (peer, nest) = match &parent {
                None => {
                    let peer = builder.add_top_level(&peer_id, peer_element);
                    let nest = builder.add_top_level(&nest_id, nest_element);
                    (peer, nest)
                }
                Some(parent) => {
                    let peer = builder.add_child(parent, &peer_id, peer_element);
                    let nest = builder.add_child(parent, &nest_id, nest_element);
                    (peer, nest)
                }
            };
            let stroke = level_stroke(level);
            let edge = Edge::new().with_style("stroke", stroke);
            builder.add_edge(&peer, &nest, edge);
            parent = Some(nest);
        }
        builder.build().model.expect("valid model")
    }

    fn nested_scene(depth: usize) -> Scene {
        let root_path = ElementPath::root();
        let root = nested_level(0, depth, &root_path);
        let nested_levels = root
            .nested_interiors()
            .map(|interior| interior.container.clone());
        let levels = iter::once(root_path).chain(nested_levels);
        let view = Rect::new(-20.0, -20.0, 180.0, 140.0);
        let views_by_level = levels.map(|level| (level, view)).collect();
        Scene::new(Vec::new(), root, views_by_level)
    }

    /// Draw level `level` under `parent`: peer, nest (holding the next level), their connection, and a stub below the root.
    fn nested_level(level: usize, depth: usize, parent: &ElementPath) -> Group {
        let peer_id = format!("p{level}");
        let nest_id = format!("n{level}");
        let peer_path = parent.child(peer_id);
        let nest_path = parent.child(nest_id);
        let peer_face = square_face(PEER_ORIGIN, PEER_SIZE);
        let peer_solid = Solid::new(peer_path.clone(), vec![peer_face]);
        let nest_face = square_face(NEST_ORIGIN, NEST_SIZE);
        let leaf_nest_solid = Solid::new(nest_path.clone(), vec![nest_face]);
        let is_deepest = level + 1 == depth;
        let nest_solid = if is_deepest {
            leaf_nest_solid
        } else {
            let interior = nested_level(level + 1, depth, &nest_path);
            leaf_nest_solid.with_interior(interior)
        };
        let peer_center = square_center(PEER_ORIGIN, PEER_SIZE);
        let nest_center = square_center(NEST_ORIGIN, NEST_SIZE);
        let mut connection_path = BezPath::new();
        connection_path.move_to(peer_center);
        connection_path.line_to(nest_center);
        let edge = EdgeId::new(peer_path, nest_path, 0);
        let connection = Connection::new(edge.clone(), connection_path);
        let is_root = level == 0;
        let frame = if is_root {
            Affine::IDENTITY
        } else {
            Affine::new(INTERIOR_FRAME_COEFFICIENTS)
        };
        let mut group = Group::new(frame);
        group.solids.push(peer_solid);
        group.solids.push(nest_solid);
        group.connections.push(connection);
        if is_root {
            return group;
        }
        let external_endpoint = ElementPath::root().child("outside");
        let stub_marker = stub_marker_for(edge);
        group.stub_markers.insert(external_endpoint, stub_marker);
        group
    }

    fn stub_marker_for(edge: EdgeId) -> StubMarker {
        let tag_baseline_origin = Point::new(12.0, 9.0);
        let tag_size = LabelRole::ConnectionLabel.measure_text("outside", &FixedAdvanceMeasure);
        let tag_label = Label::new(
            "outside",
            LabelRole::ConnectionLabel,
            TextAnchor::Middle,
            tag_baseline_origin,
            tag_size,
        );
        let tag_plate = RoundedRect::new(0.0, 0.0, 24.0, 12.0, 6.0);
        let tag = PlatedLabel::new(tag_label, tag_plate);
        let edges = BTreeSet::from([edge]);
        StubMarker::new(edges, tag)
    }

    fn square_face(origin: Point, size: f64) -> Face {
        let mut outline = BezPath::new();
        outline.move_to(origin);
        outline.line_to((origin.x + size, origin.y));
        outline.line_to((origin.x + size, origin.y + size));
        outline.line_to((origin.x, origin.y + size));
        outline.close_path();
        Face::new(outline, FaceShade::Top)
    }

    fn square_center(origin: Point, size: f64) -> Point {
        let half_size = size / 2.0;
        Point::new(origin.x + half_size, origin.y + half_size)
    }

    fn render_nested(depth: usize) -> String {
        let font = BundledFont::new();
        let model = nested_model(depth);
        let scene = nested_scene(depth);
        let profile = Static::new(&font);
        render_scene(&model, &scene, profile)
    }

    fn nested_path_nodes(group: &usvg::Group) -> Vec<&usvg::Path> {
        let mut paths = Vec::new();
        for child in group.children() {
            match child {
                usvg::Node::Group(child_group) => {
                    let child_paths = nested_path_nodes(child_group);
                    paths.extend(child_paths);
                }
                usvg::Node::Path(path) => paths.push(path),
                _other_node => {}
            }
        }
        paths
    }

    fn is_within_tolerance(bounds: &usvg::Rect, expected: Rect) -> bool {
        let left = f64::from(bounds.left());
        let top = f64::from(bounds.top());
        let right = f64::from(bounds.right());
        let bottom = f64::from(bounds.bottom());
        let edge_offsets = [
            left - expected.x0,
            top - expected.y0,
            right - expected.x1,
            bottom - expected.y1,
        ];
        edge_offsets
            .iter()
            .all(|offset| offset.abs() < BOUNDS_TOLERANCE)
    }

    #[test]
    fn transparent_dashed_element_renders_as_an_unfilled_dashed_outline() {
        let a_element = Element::new("a")
            .with_style("fill", "transparent")
            .with_style("stroke", "#5b6170")
            .with_style("stroke-dash", "5");
        let model = model_with_a_and_b(a_element);
        let markup = render_fixture_scene(&model);
        let outlined_face_attributes =
            "fill=\"none\" stroke=\"#5b6170\" stroke-dasharray=\"5 4\" stroke-width=\"1\"";
        let outlined_face_count = markup.matches(outlined_face_attributes).count();
        assert_eq!(outlined_face_count, 3, "{markup}");
        let options = usvg::Options::default();
        let parse_result = usvg::Tree::from_str(&markup, &options);
        assert!(parse_result.is_ok(), "{parse_result:?}");
    }

    #[test]
    fn translucent_fill_renders_as_eight_digit_hex_that_usvg_reads_as_opacity() {
        let a_element = Element::new("a").with_style("fill", "#ff000080");
        let model = model_with_a_and_b(a_element);
        let markup = render_fixture_scene(&model);
        assert!(markup.contains("fill=\"#ff000080\""), "{markup}");
        let options = usvg::Options::default();
        let tree = usvg::Tree::from_str(&markup, &options).expect("parseable");
        let root = tree.root();
        let paths = nested_path_nodes(root);
        let has_half_opaque_fill = paths.iter().filter_map(|path| path.fill()).any(|fill| {
            let opacity = fill.opacity().get();
            (opacity - HALF_OPACITY).abs() < 1e-3
        });
        assert!(has_half_opaque_fill, "{markup}");
    }

    #[test]
    fn nested_scene_renders_as_a_static_document() {
        let markup = render_nested(NESTED_DEPTH);
        insta::assert_snapshot!(markup);
    }

    #[test]
    fn interiors_nest_one_fixed_decimal_frame_group_per_level_at_any_depth() {
        for depth in [1, NESTED_DEPTH, DEEP_NESTED_DEPTH] {
            let markup = render_nested(depth);
            let level_tags: Vec<&str> = markup
                .lines()
                .filter(|line| line.starts_with("<g data-svugy-level="))
                .collect();
            let (root_tags, interior_tags): (Vec<&str>, Vec<&str>) = level_tags
                .into_iter()
                .partition(|tag| tag.starts_with("<g data-svugy-level=\"\""));
            assert_eq!(root_tags.len(), 1, "depth {depth}");
            let is_root_unframed = !root_tags[0].contains("transform=");
            assert!(is_root_unframed, "{}", root_tags[0]);
            let interior_count = depth - 1;
            assert_eq!(interior_tags.len(), interior_count, "depth {depth}");
            for interior_tag in interior_tags {
                let is_frame_in_fixed_decimals = interior_tag.ends_with(INTERIOR_FRAME_TRANSFORM);
                assert!(is_frame_in_fixed_decimals, "{interior_tag}");
            }
        }
    }

    #[test]
    fn every_nested_level_carries_its_outer_view_as_its_data_view() {
        let model = nested_model(DEEP_NESTED_DEPTH);
        let scene = nested_scene(DEEP_NESTED_DEPTH);
        let markup = render_scene(&model, &scene, Interactive);
        let nested_interiors: Vec<NestedInterior> = scene.root.nested_interiors().collect();
        assert_eq!(nested_interiors.len(), DEEP_NESTED_DEPTH - 1);
        for nested_interior in nested_interiors {
            let container = nested_interior.container;
            let view = scene.view_of(container);
            let outer_view = nested_interior.to_outer.transform_rect_bbox(view);
            let view_value = ViewBox(outer_view);
            let level_attributes =
                format!("data-svugy-level=\"{container}\" data-svugy-view=\"{view_value}\"");
            assert!(markup.contains(&level_attributes), "{level_attributes}");
        }
        let root_view = ViewBox(scene.root_view());
        let root_level_attributes =
            format!("data-svugy-level=\"\" data-svugy-view=\"{root_view}\"");
        let root_view_box = format!("viewBox=\"{root_view}\"");
        assert!(markup.contains(&root_level_attributes), "{markup}");
        assert!(markup.contains(&root_view_box), "{markup}");
    }

    #[test]
    fn a_path_link_enters_its_level_and_only_safe_urls_become_anchors() {
        let path_linked_model = model_with_a_and_b(Element::new("a").with_link("b"));
        let script_linked_model =
            model_with_a_and_b(Element::new("a").with_link("javascript:alert(1)"));
        let url_linked_model =
            model_with_a_and_b(Element::new("a").with_link("https://example.com/?a=1&b=2"));
        let path_linked_markup = render_interactive_fixture_scene(&path_linked_model);
        let script_linked_markup = render_interactive_fixture_scene(&script_linked_model);
        let url_linked_markup = render_interactive_fixture_scene(&url_linked_model);
        let leaf_target_entering_root = "<g data-svugy-element=\"a\" data-svugy-goto=\"\">";
        let unlinked_element = "<g data-svugy-element=\"a\">";
        assert!(
            path_linked_markup.contains(leaf_target_entering_root),
            "{path_linked_markup}"
        );
        assert!(
            !path_linked_markup.contains("<a href="),
            "{path_linked_markup}"
        );
        assert!(
            !script_linked_markup.contains("<a href="),
            "{script_linked_markup}"
        );
        assert!(
            !script_linked_markup.contains("javascript"),
            "{script_linked_markup}"
        );
        assert!(
            script_linked_markup.contains(unlinked_element),
            "{script_linked_markup}"
        );
        let anchored_element =
            "<a href=\"https://example.com/?a=1&amp;b=2\">\n<g data-svugy-element=\"a\">";
        assert!(
            url_linked_markup.contains(anchored_element),
            "{url_linked_markup}"
        );
    }

    #[test]
    fn hostile_label_and_metadata_round_trip_through_title_and_details() {
        let hostile_text = "\"q\" <b> & ]]> '\u{FFFF}\u{1}end";
        let xml_safe_text = "\"q\" <b> & ]]> 'end";
        let json_escaped_text = "\"q\" <b> & ]]> '\u{1}end";
        let hostile_element = Element::new(hostile_text)
            .with_caption(hostile_text)
            .with_metadata("note", hostile_text);
        let model = model_with_a_and_b(hostile_element);
        let markup = render_interactive_fixture_scene(&model);
        let options = usvg::Options::default();
        let parse_result = usvg::Tree::from_str(&markup, &options);
        assert!(parse_result.is_ok(), "{parse_result:?}");
        let document = roxmltree::Document::parse(&markup).expect("well-formed XML");
        let element_group = document
            .descendants()
            .find(|node| node.attribute(RuntimeContract::ELEMENT) == Some("a"))
            .expect("element group of a");
        let mut children = element_group.children().filter(roxmltree::Node::is_element);
        let title = children.next().expect("title first");
        let details = children.next().expect("details second");
        assert_eq!(title.tag_name().name(), "title");
        assert_eq!(title.text(), Some(xml_safe_text));
        assert_eq!(details.tag_name().name(), "desc");
        assert_eq!(details.attribute(RuntimeContract::DETAILS), Some(""));
        let details_text = details.text().expect("details text");
        let details_json: serde_json::Value =
            serde_json::from_str(details_text).expect("details are JSON");
        let expected_json = serde_json::json!({
            "kind": "element",
            "path": "a",
            "label": json_escaped_text,
            "caption": json_escaped_text,
            "metadata": { "note": json_escaped_text },
        });
        assert!(details_text.contains("\\u0001"), "{details_text}");
        assert_eq!(details_json, expected_json);
    }

    #[test]
    fn interactive_document_themes_both_palettes_and_embeds_the_runtime_last() {
        let model = model_with_a_and_b(Element::new("a"));
        let markup = render_interactive_fixture_scene(&model);
        let canvas_property = Token::Canvas.css_custom_property();
        let canvas_reference = CssValue::custom_property_reference(&canvas_property);
        let canvas_fill = format!("fill=\"{}\"", canvas_reference.as_str());
        assert!(markup.contains(&canvas_fill), "{markup}");
        assert!(
            markup.contains("@media (prefers-color-scheme: light){"),
            "{markup}"
        );
        assert!(markup.contains("<style>/*<![CDATA[*/"), "{markup}");
        assert!(markup.contains("textLength=\""), "{markup}");
        assert!(markup.contains("<animateMotion "), "{markup}");
        let script_tail = "\n//]]></script>\n</svg>\n";
        assert!(markup.ends_with(script_tail), "{markup}");
    }

    fn tagged_solid(path: &ElementPath, tag: &str) -> Solid {
        let face = square_face(NEST_ORIGIN, NEST_SIZE);
        let tag_size = LabelRole::ContainerTag.measure_text(tag, &FixedAdvanceMeasure);
        let tag_baseline_origin = Point::new(9.0, 21.5);
        let tag_label = Label::new(
            tag,
            LabelRole::ContainerTag,
            TextAnchor::Start,
            tag_baseline_origin,
            tag_size,
        );
        let untagged_solid = Solid::new(path.clone(), vec![face]);
        untagged_solid.with_label(tag_label)
    }

    /// Return the line drawing the label `text`.
    fn label_line<'a>(markup: &'a str, text: &str) -> &'a str {
        let label_closing = format!(">{text}</text>");
        markup
            .lines()
            .find(|line| line.ends_with(&label_closing))
            .unwrap_or_else(|| panic!("label {text} is drawn"))
    }

    #[test]
    fn hanging_tags_take_the_ink_of_the_ground_they_hang_onto() {
        let mut builder = ModelBuilder::new();
        let desk_element = Element::new("desk").with_style("fill", "#1b2436");
        let desk = builder.add_top_level("desk", desk_element);
        let core = builder.add_child(&desk, "core", Element::new("core"));
        builder.add_child(&core, "engine", Element::new("engine"));
        let model = builder.build().model.expect("valid model");
        let view = Rect::new(-20.0, -20.0, 180.0, 140.0);
        let mut desk_interior = Group::new(Affine::IDENTITY);
        let core_solid = tagged_solid(core.path(), "core");
        desk_interior.solids.push(core_solid);
        let desk_solid = tagged_solid(desk.path(), "desk").with_interior(desk_interior);
        let mut root = Group::new(Affine::IDENTITY);
        root.solids.push(desk_solid);
        let views_by_level =
            BTreeMap::from([(ElementPath::root(), view), (desk.path().clone(), view)]);
        let scene = Scene::new(Vec::new(), root, views_by_level);
        let markup = render_scene(&model, &scene, Interactive);
        let desk_tag = label_line(&markup, "desk");
        let core_tag = label_line(&markup, "core");
        assert!(
            desk_tag.starts_with("<text fill=\"var(--svugy-text)\""),
            "{desk_tag}"
        );
        assert!(core_tag.starts_with("<text fill=\"#ffffff\""), "{core_tag}");
    }

    /// Build the fixture plan's `a -> b`, titled and subtitled, projected in `font`.
    fn headed_fixture(font: &BundledFont) -> (Model, Scene) {
        let mut builder = ModelBuilder::new()
            .with_title("svugy · how it is built")
            .with_subtitle("full Rust stack");
        let a = builder.add_top_level("a", Element::new("a"));
        let b = builder.add_top_level("b", Element::new("b"));
        builder.add_edge(&a, &b, Edge::new().with_label("calls"));
        let model = builder.build().model.expect("valid model");
        let plan = LevelPlan::two_boxes_and_one_route();
        let plans = LevelPlans::root_only(plan);
        let scene = Scene::project(&model, &plans, font);
        (model, scene)
    }

    /// Return the lines between the canvas and the root level group, where the heading is drawn, leaving out the ground grid painted beneath it.
    fn lines_before_the_root_level(markup: &str) -> Vec<&str> {
        let lines = markup.lines();
        let after_canvas = lines.skip_while(|line| !line.starts_with("<rect "));
        let before_root_level = after_canvas
            .skip(1)
            .take_while(|line| !line.starts_with("<g data-svugy-level=\"\""));
        let heading_lines = before_root_level.filter(|line| !line.starts_with("<line "));
        heading_lines.collect()
    }

    #[test]
    fn interactive_heading_sets_upright_text_outside_every_level_in_text_and_muted_ink() {
        let font = BundledFont::new();
        let (model, scene) = headed_fixture(&font);
        let markup = render_scene(&model, &scene, Interactive);

        let heading_lines = lines_before_the_root_level(&markup);

        let [title_line, subtitle_line] = heading_lines[..] else {
            panic!("expected a title and a subtitle line: {heading_lines:?}");
        };
        let text_ink = format!(
            "fill=\"var({})\"",
            Token::Text.css_custom_property().as_str()
        );
        let muted_ink = format!(
            "fill=\"var({})\"",
            Token::TextMuted.css_custom_property().as_str()
        );
        assert!(title_line.starts_with("<text "), "{title_line}");
        assert!(title_line.contains(&text_ink), "{title_line}");
        assert!(title_line.contains("font-size=\"17\""), "{title_line}");
        assert!(title_line.contains("font-weight=\"700\""), "{title_line}");
        assert!(title_line.contains("text-anchor=\"start\""), "{title_line}");
        assert!(!title_line.contains("transform="), "{title_line}");
        assert!(
            title_line.ends_with(">svugy · how it is built</text>"),
            "{title_line}"
        );
        assert!(subtitle_line.contains(&muted_ink), "{subtitle_line}");
        assert!(
            subtitle_line.contains("font-size=\"11.5\""),
            "{subtitle_line}"
        );
        assert!(
            subtitle_line.ends_with(">full Rust stack</text>"),
            "{subtitle_line}"
        );
    }

    #[test]
    fn static_heading_outlines_both_lines_in_palette_literals_before_the_root_level() {
        let font = BundledFont::new();
        let (model, scene) = headed_fixture(&font);
        let profile = Static::new(&font);
        let markup = render_scene(&model, &scene, profile);

        let heading_lines = lines_before_the_root_level(&markup);

        let [title_line, subtitle_line] = heading_lines[..] else {
            panic!("expected a title and a subtitle outline: {heading_lines:?}");
        };
        assert!(title_line.starts_with("<path d=\"M"), "{title_line}");
        assert!(title_line.contains("fill=\"#e6e8ee\""), "{title_line}");
        assert!(subtitle_line.starts_with("<path d=\"M"), "{subtitle_line}");
        assert!(
            subtitle_line.contains("fill=\"#8b93a5\""),
            "{subtitle_line}"
        );
        for forbidden in FORBIDDEN_STATIC_MARKUP {
            assert!(!markup.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn arrowheads_are_collected_from_every_depth() {
        let markup = render_nested(NESTED_DEPTH);
        let lines: Vec<&str> = markup.lines().collect();
        for level in 0..NESTED_DEPTH {
            let stroke = level_stroke(level);
            let marker_opening = format!("<marker id=\"arrowhead-{level}\"");
            let marker_line_index = lines
                .iter()
                .position(|line| line.starts_with(&marker_opening))
                .unwrap_or_else(|| panic!("marker of level {level} is defined"));
            let marker_path = lines[marker_line_index + 1];
            let marker_fill = format!("fill=\"{stroke}\"");
            assert!(marker_path.contains(&marker_fill), "{marker_path}");
            let marker_reference = format!("marker-end=\"url(#arrowhead-{level})\"");
            let connection_stroke = format!("stroke=\"{stroke}\"");
            let is_connection_referencing_its_marker = lines
                .iter()
                .any(|line| line.contains(&marker_reference) && line.contains(&connection_stroke));
            assert!(is_connection_referencing_its_marker, "level {level}");
        }
    }

    #[test]
    fn static_profile_emits_no_stub_marker_at_any_depth() {
        let markup = render_nested(DEEP_NESTED_DEPTH);
        let rect_count = markup.matches("<rect").count();
        assert_eq!(rect_count, 1, "only the canvas is a rect");
        assert!(!markup.contains(RuntimeContract::STUB), "{markup}");
    }

    #[test]
    fn interactive_profile_overlays_every_level_with_its_stub_markers() {
        let model = nested_model(NESTED_DEPTH);
        let scene = nested_scene(NESTED_DEPTH);
        let markup = render_scene(&model, &scene, Interactive);
        let overlay_group_count = markup.matches("<g data-svugy-overlay=\"\"").count();
        assert_eq!(overlay_group_count, NESTED_DEPTH);
        let stub_group =
            "<g data-svugy-goto=\"\" data-svugy-stub=\"outside\">\n<title>to outside</title>";
        let stub_group_count = markup.matches(stub_group).count();
        assert_eq!(stub_group_count, NESTED_DEPTH - 1);
        let stub_plate_count = markup.matches("<rect").count() - 1;
        assert_eq!(stub_plate_count, NESTED_DEPTH - 1);
    }

    #[test]
    fn nested_static_document_rasterizes_with_the_deepest_face_under_the_composed_frames() {
        let markup = render_nested(NESTED_DEPTH);
        let options = usvg::Options::default();
        let tree = usvg::Tree::from_str(&markup, &options).expect("parseable");
        let size = tree.size().to_int_size();
        let mut pixmap =
            resvg::tiny_skia::Pixmap::new(size.width(), size.height()).expect("non-empty size");
        let identity = resvg::tiny_skia::Transform::default();
        let mut pixmap_view = pixmap.as_mut();
        resvg::render(&tree, identity, &mut pixmap_view);
        let scene = nested_scene(NESTED_DEPTH);
        let view_offset = scene.root_view().origin().to_vec2();
        let view_to_pixels = Affine::translate(-view_offset);
        let interior_frame = Affine::new(INTERIOR_FRAME_COEFFICIENTS);
        let deepest_frame = view_to_pixels * interior_frame * interior_frame;
        let deepest_nest_face = square_face(NEST_ORIGIN, NEST_SIZE);
        let deepest_nest_outline = deepest_frame * deepest_nest_face.outline;
        let expected_bounds = deepest_nest_outline.bounding_box();
        let root = tree.root();
        let paths = nested_path_nodes(root);
        let is_deepest_face_drawn_in_place = paths.iter().any(|path| {
            let bounds = path.abs_bounding_box();
            is_within_tolerance(&bounds, expected_bounds)
        });
        assert!(is_deepest_face_drawn_in_place, "{expected_bounds:?}");
    }

    /// Load the svugy golden, with its legend unless `is_unlegended`.
    fn golden_model(is_unlegended: bool) -> Model {
        let model = crate::svugy_example::SvugyExample::model();
        if is_unlegended {
            let saved = JsonPrinter::print(&model);
            let mut model_value: serde_json::Value = serde_json::from_str(&saved).expect("JSON");
            let fields = model_value
                .as_object_mut()
                .expect("a model saves as an object");
            fields.remove("legend");
            let unlegended = model_value.to_string();
            let import = JsonLanguage::import(&unlegended);
            let import_parts = import.into_admitted_parts();
            import_parts
                .model
                .expect("dropping the legend keeps a sound model")
        } else {
            model
        }
    }

    #[test]
    fn the_legend_is_text_in_the_interactive_profile_and_glyph_paths_in_the_static_one() {
        let legended_model = golden_model(false);
        let unlegended_model = golden_model(true);
        let interactive_svg = InteractiveSvg::render(&legended_model);
        let static_svg = StaticSvg::render(&legended_model);
        let unlegended_static_svg = StaticSvg::render(&unlegended_model);

        assert!(interactive_svg.contains(">Read it as a story</text>"));
        assert!(interactive_svg.contains(">reads the bundled font</text>"));
        assert!(!static_svg.contains("Read it as a story"));
        let static_path_count = static_svg.matches("<path ").count();
        let unlegended_path_count = unlegended_static_svg.matches("<path ").count();
        let legend_lines = 1 + 4 + 3;
        let legend_samples = 2 + 2;
        assert_eq!(
            static_path_count - unlegended_path_count,
            legend_lines + legend_samples
        );
        let root_level_start = static_svg
            .find("<g data-svugy-level=")
            .expect("the root level is drawn");
        let embedded_sample_start = static_svg
            .find(r#"stroke-dasharray="2 4""#)
            .expect("the embedded line is dotted");
        assert!(embedded_sample_start < root_level_start);
    }

    #[test]
    fn a_badge_is_its_edges_one_group_with_its_details_inside_its_sources_group_in_both_profiles() {
        let source_group = r#"<g data-svugy-element="pipeline.render.profile">"#;
        let next_source_group = r#"<g data-svugy-element="pipeline.render.writer">"#;
        let badge_group_opening = r#"<g data-svugy-edge="(pipeline.render.profile -&gt; pipeline.inter)[0]" data-svugy-from="pipeline.render.profile" data-svugy-to="pipeline.inter">"#;
        let badge_details = r#"<title>outlines</title>
<desc data-svugy-details="">{"kind":"edge","edge":"(pipeline.render.profile -&gt; pipeline.inter)[0]","from":"pipeline.render.profile","to":"pipeline.inter","label":"outlines"}</desc>"#;
        let badge_group = format!("{badge_group_opening}\n{badge_details}");
        let model = golden_model(false);
        let interactive_svg = InteractiveSvg::render(&model);
        let static_svg = StaticSvg::render(&model);
        for svg in [interactive_svg, static_svg] {
            let source_start = svg.find(source_group).expect("profile is drawn");
            let next_source_start = svg.find(next_source_group).expect("writer is drawn");
            let badge_start = svg
                .find(&badge_group)
                .expect("the badge carries its details");
            let edge_group_count = svg.matches(badge_group_opening).count();
            assert!(source_start < badge_start && badge_start < next_source_start);
            assert_eq!(edge_group_count, 1);
        }
    }
}
