//! Pick the paint of every solid and connection from the model's styles.

use std::fmt;

use svugy_model::color::Rgba;
use svugy_model::model::{Edge, EdgeId, Element, Model};
use svugy_model::path::ElementPath;
use svugy_model::style::{ColorKey, NumberKey};

use crate::geometry::FaceShade;
use crate::render::theme::color::Oklab;
use crate::render::theme::{BaseShades, FaceShades, Surface, Token};
use crate::render::writer::FixedDecimal;

/// Shade unfilled leaves from this surface.
const LEAF_SURFACE: Surface = Surface::Neutral;
/// Shade unfilled containers from this surface.
const CONTAINER_SURFACE: Surface = Surface::ContainerBlue;
/// Width the seam between filled faces is stroked at.
const SEAM_OUTLINE_WIDTH: f64 = 0.8;
/// Width an authored or external outline is stroked at.
const STROKED_OUTLINE_WIDTH: f64 = 1.0;
/// Ratio of an outline's gap to its dash.
const DASH_GAP_RATIO: f64 = 0.8;
/// Lengthen a connection's gap past its dash by this much.
const CONNECTION_DASH_GAP_EXCESS: f64 = 2.0;

/// Picks paints from the model's `fill`, `stroke` and `stroke-dash` styles, falling back to theme surfaces.
pub(crate) struct ModelPaints<'m> {
    model: &'m Model,
}

/// Names a color by theme token or by literal, or paints nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Paint {
    Token(Token),
    Literal(Rgba),
    None,
}

/// Holds the paints of one solid's three face shades, of the ink its labels are set in, and of its outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SolidPaint {
    top: Paint,
    left: Paint,
    right: Paint,
    ink: Paint,
    outline: Outline,
}

/// Holds the stroke drawn around every face of a solid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Outline {
    pub(crate) paint: Paint,
    pub(crate) width: f64,
    pub(crate) dashes: Option<Dashes>,
}

/// Holds the dash and gap lengths of a dashed outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dashes {
    pub(crate) dash: f64,
    pub(crate) gap: f64,
}

/// Holds the paints of the ground a level's content lies on: its fill, and the ink of text set on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GroundPaint {
    pub(crate) fill: Paint,
    pub(crate) ink: Paint,
}

/// Holds the paints of a plated label: its plate's fill and outline, and its text's ink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlatePaint {
    pub(crate) fill: Paint,
    pub(crate) outline: Paint,
    pub(crate) ink: Paint,
}

impl<'m> ModelPaints<'m> {
    /// Pick paints from `model`'s styles.
    pub(crate) fn new(model: &'m Model) -> Self {
        Self { model }
    }

    /// Paint a solid from its element's `fill`, `stroke` and `stroke-dash`, else from the leaf or container surface.
    pub(crate) fn solid(&self, path: &ElementPath) -> SolidPaint {
        let element = self.model.element(path);
        let filled = self.filled_solid(path, element);
        let style = element.map(Element::style);
        let authored_stroke = style.and_then(|style| ColorKey::STROKE.read_from(style));
        let stroked = authored_stroke.map_or(filled, |stroke| filled.outlined_in(stroke));
        let authored_dash = style.and_then(|style| NumberKey::STROKE_DASH.read_from(style));
        let Some(stroke_dash) = authored_dash else {
            return stroked;
        };
        let Some(dashes) = Dashes::from_stroke_dash(stroke_dash) else {
            return stroked;
        };
        stroked.with_dashes(dashes)
    }

    /// Paint a connection from its edge's `stroke` and its source's `fill`: stroke ?? (opaque(fill) ∧ L(fill) ≥ 0.5 ? fill : muted text).
    pub(crate) fn connection(&self, edge: &EdgeId) -> Paint {
        let authored_edge = self.model.edge(edge);
        let edge_style = authored_edge.map(Edge::style);
        let authored_stroke = edge_style.and_then(|style| ColorKey::STROKE.read_from(style));
        if let Some(stroke) = authored_stroke {
            return Paint::from_color(stroke);
        }
        let source_path = edge.from();
        let source = self.model.element(source_path);
        let source_style = source.map(Element::style);
        let source_fill = source_style.and_then(|style| ColorKey::FILL.read_from(style));
        let Some(fill) = source_fill else {
            return Paint::Token(Token::TextMuted);
        };
        let is_opaque_fill = fill.is_opaque();
        let fill_oklab = Oklab::from_rgb(fill.rgb);
        let is_readable_fill = fill_oklab.is_readable_as_stroke();
        let is_readable_on_canvas = is_opaque_fill && is_readable_fill;
        if is_readable_on_canvas {
            return Paint::Literal(fill);
        }
        Paint::Token(Token::TextMuted)
    }

    /// Dash a connection by its edge's `stroke-dash`, else by the default dash every unstyled edge flows with.
    pub(crate) fn connection_dashes(&self, edge: &EdgeId) -> Option<Dashes> {
        let authored_edge = self.model.edge(edge);
        let stroke_dash = authored_edge.map_or(Edge::DEFAULT_STROKE_DASH, Edge::stroke_dash);
        Dashes::along_connection(stroke_dash)
    }

    /// Paint a legend's stroke sample in the one paint every connection of `edges` shares, else in text ink.
    pub(crate) fn stroke_sample(&self, edges: &[EdgeId]) -> Paint {
        let mut connection_paints = edges.iter().map(|edge| self.connection(edge));
        let first_paint = connection_paints.next();
        let Some(first_paint) = first_paint else {
            return Paint::Token(Token::Text);
        };
        let is_shared = connection_paints.all(|paint| paint == first_paint);
        if is_shared {
            return first_paint;
        }
        Paint::Token(Token::Text)
    }

    /// Paint the ground of `level`: the canvas and its text at the root, else the top face and ink of the container whose top that ground is.
    pub(crate) fn ground(&self, level: &ElementPath) -> GroundPaint {
        let is_root = level.is_root();
        if is_root {
            return GroundPaint::CANVAS;
        }
        let container_paint = self.solid(level);
        let fill = container_paint.face(FaceShade::Top);
        let ink = container_paint.ink();
        GroundPaint { fill, ink }
    }

    /// Fill a solid's faces from its `fill`: shaded from an authored color, none when transparent, else its default surface.
    fn filled_solid(&self, path: &ElementPath, element: Option<&Element>) -> SolidPaint {
        let style = element.map(Element::style);
        let authored_fill = style.and_then(|style| ColorKey::FILL.read_from(style));
        let Some(fill) = authored_fill else {
            let is_container = self.model.is_container(path);
            let default_surface = if is_container {
                CONTAINER_SURFACE
            } else {
                LEAF_SURFACE
            };
            return SolidPaint::from_surface(default_surface);
        };
        let is_transparent_fill = fill.is_transparent();
        if is_transparent_fill {
            return SolidPaint::unfilled();
        }
        SolidPaint::from_base(fill)
    }
}

impl Paint {
    /// Paint `color` as a literal, or nothing when it is fully transparent.
    fn from_color(color: Rgba) -> Self {
        let is_transparent = color.is_transparent();
        if is_transparent {
            return Paint::None;
        }
        Paint::Literal(color)
    }
}

impl SolidPaint {
    /// Paint a solid's faces, ink and outline from `surface`'s tokens.
    pub(crate) fn from_surface(surface: Surface) -> Self {
        let top_token = Token::SurfaceTop(surface);
        let left_token = Token::SurfaceLeft(surface);
        let right_token = Token::SurfaceRight(surface);
        let ink_token = Token::SurfaceInk(surface);
        let top = Paint::Token(top_token);
        let left = Paint::Token(left_token);
        let right = Paint::Token(right_token);
        let ink = Paint::Token(ink_token);
        let seam = Outline::seam();
        Self {
            top,
            left,
            right,
            ink,
            outline: seam,
        }
    }

    /// Paint a solid from an authored base shaded by the theme, its faces keeping the base's alpha, its ink opaque.
    pub(crate) fn from_base(base: Rgba) -> Self {
        let Rgba { rgb, alpha } = base;
        let BaseShades { faces, ink } = BaseShades::from_base(rgb);
        let FaceShades { top, left, right } = faces;
        let opaque_ink = Rgba::opaque(ink);
        let top_color = Rgba { rgb: top, alpha };
        let left_color = Rgba { rgb: left, alpha };
        let right_color = Rgba { rgb: right, alpha };
        let top_paint = Paint::Literal(top_color);
        let left_paint = Paint::Literal(left_color);
        let right_paint = Paint::Literal(right_color);
        let ink_paint = Paint::Literal(opaque_ink);
        let seam = Outline::seam();
        Self {
            top: top_paint,
            left: left_paint,
            right: right_paint,
            ink: ink_paint,
            outline: seam,
        }
    }

    /// Paint no face, so the outline alone draws the solid, in the external outline color.
    pub(crate) fn unfilled() -> Self {
        let external_outline = Paint::Token(Token::ExternalOutline);
        let stroked_outline = Outline::stroked(external_outline);
        let muted_ink = Paint::Token(Token::TextMuted);
        Self {
            top: Paint::None,
            left: Paint::None,
            right: Paint::None,
            ink: muted_ink,
            outline: stroked_outline,
        }
    }

    /// Outline every face in `stroke`, keeping the outline's dashes.
    fn outlined_in(mut self, stroke: Rgba) -> Self {
        self.outline.paint = Paint::from_color(stroke);
        self.outline.width = STROKED_OUTLINE_WIDTH;
        self
    }

    /// Dash every face's outline by `dashes`.
    fn with_dashes(mut self, dashes: Dashes) -> Self {
        self.outline.dashes = Some(dashes);
        self
    }

    /// Return the paint of the face shaded `shade`.
    pub(crate) fn face(&self, shade: FaceShade) -> Paint {
        match shade {
            FaceShade::Top => self.top,
            FaceShade::Left => self.left,
            FaceShade::Right => self.right,
        }
    }

    /// Return the ink the solid's labels are set in.
    pub(crate) fn ink(&self) -> Paint {
        self.ink
    }

    /// Return the outline stroked around every face of the solid.
    pub(crate) fn outline(&self) -> Outline {
        self.outline
    }
}

impl Outline {
    /// Seam filled faces apart with a thin line in the canvas color.
    fn seam() -> Self {
        let face_outline = Paint::Token(Token::FaceOutline);
        Self {
            paint: face_outline,
            width: SEAM_OUTLINE_WIDTH,
            dashes: None,
        }
    }

    /// Stroke an undashed outline in `paint` at the stroked width.
    fn stroked(paint: Paint) -> Self {
        Self {
            paint,
            width: STROKED_OUTLINE_WIDTH,
            dashes: None,
        }
    }
}

impl Dashes {
    /// Dash an outline by `dash`: gap = 0.8 · dash (the reference's "5 4").
    fn from_stroke_dash(dash: f64) -> Option<Self> {
        let gap = dash * DASH_GAP_RATIO;
        Self::unless_solid(dash, gap)
    }

    /// Dash a connection by `dash`: gap = dash + 2 (the reference's flowing "5 7").
    pub(crate) fn along_connection(dash: f64) -> Option<Self> {
        let gap = dash + CONNECTION_DASH_GAP_EXCESS;
        Self::unless_solid(dash, gap)
    }

    /// Dash by `dash` and `gap`, unless a zero dash draws a solid line, which has none.
    fn unless_solid(dash: f64, gap: f64) -> Option<Self> {
        let is_solid = dash == 0.0;
        if is_solid {
            return None;
        }
        let dashes = Dashes { dash, gap };
        Some(dashes)
    }
}

impl fmt::Display for Dashes {
    /// Spell the dashes as a `stroke-dasharray` value, `dash gap` in fixed decimals.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dash = FixedDecimal(self.dash);
        let gap = FixedDecimal(self.gap);
        write!(formatter, "{dash} {gap}")
    }
}

impl GroundPaint {
    /// Paint the root's ground: the canvas, with text inked on it.
    const CANVAS: GroundPaint = GroundPaint {
        fill: Paint::Token(Token::Canvas),
        ink: Paint::Token(Token::Text),
    };
}

impl PlatePaint {
    /// Plate a connection label over the arcs: the canvas under canvas text, outlined in the connection's paint.
    pub(crate) fn over_arcs(connection: Paint) -> Self {
        Self {
            fill: GroundPaint::CANVAS.fill,
            outline: connection,
            ink: GroundPaint::CANVAS.ink,
        }
    }

    /// Plate a label lying on `ground`: filled like the ground, outlined and inked in its ink.
    pub(crate) fn on_ground(ground: GroundPaint) -> Self {
        Self {
            fill: ground.fill,
            outline: ground.ink,
            ink: ground.ink,
        }
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::builder::ModelBuilder;
    use svugy_model::model::Edge;

    use super::*;
    use crate::render::theme::color::Oklab;

    const RED: Rgba = Rgba::from_packed(0xff0000ff);

    /// Paint the solid of one top-level element carrying `style`.
    fn solid_paint_with_style(style: &[(&str, &str)]) -> SolidPaint {
        let mut element = Element::new("styled");
        for &(key, value) in style {
            element = element.with_style(key, value);
        }
        let mut builder = ModelBuilder::new();
        let styled = builder.add_top_level("styled", element);
        let model = builder.build().model.expect("valid model");
        let paints = ModelPaints::new(&model);
        paints.solid(styled.path())
    }

    /// Paint the one connection from a source filled with `source_fill` along an edge carrying `edge_style`.
    fn connection_paint_with_styles(source_fill: &str, edge_style: &[(&str, &str)]) -> Paint {
        let mut edge = Edge::new();
        for &(key, value) in edge_style {
            edge = edge.with_style(key, value);
        }
        let mut builder = ModelBuilder::new();
        let source_element = Element::new("source").with_style(ColorKey::FILL.name(), source_fill);
        let source = builder.add_top_level("source", source_element);
        let target = builder.add_top_level("target", Element::new("target"));
        let edge_id = builder.add_edge(&source, &target, edge);
        let model = builder.build().model.expect("valid model");
        let paints = ModelPaints::new(&model);
        paints.connection(&edge_id)
    }

    #[test]
    fn unstyled_leaves_and_containers_take_their_default_surfaces() {
        let mut builder = ModelBuilder::new();
        let desk = builder.add_top_level("desk", Element::new("desk"));
        let quoter = builder.add_child(&desk, "quoter", Element::new("quoter"));
        let model = builder.build().model.expect("valid model");
        let paints = ModelPaints::new(&model);
        let desk_paint = paints.solid(desk.path());
        let quoter_paint = paints.solid(quoter.path());
        assert_eq!(desk_paint, SolidPaint::from_surface(CONTAINER_SURFACE));
        assert_eq!(quoter_paint, SolidPaint::from_surface(LEAF_SURFACE));
    }

    #[test]
    fn the_ground_is_the_canvas_at_the_root_and_the_container_top_inside_one() {
        let dark_fill = "#1b2436";
        let mut builder = ModelBuilder::new();
        let desk_element = Element::new("desk").with_style(ColorKey::FILL.name(), dark_fill);
        let desk = builder.add_top_level("desk", desk_element);
        let core = builder.add_child(&desk, "core", Element::new("core"));
        builder.add_child(&core, "engine", Element::new("engine"));
        let model = builder.build().model.expect("valid model");
        let paints = ModelPaints::new(&model);
        let root = ElementPath::root();
        let root_ground = paints.ground(&root);
        let desk_ground = paints.ground(desk.path());
        let core_ground = paints.ground(core.path());
        let dark_base = Rgba::from_packed(0x1b2436ff);
        let desk_paint = SolidPaint::from_base(dark_base);
        let core_paint = SolidPaint::from_surface(CONTAINER_SURFACE);
        assert_eq!(root_ground, GroundPaint::CANVAS);
        assert_eq!(desk_ground.fill, desk_paint.face(FaceShade::Top));
        assert_eq!(desk_ground.ink, desk_paint.ink());
        assert_eq!(core_ground.fill, core_paint.face(FaceShade::Top));
        assert_eq!(core_ground.ink, core_paint.ink());
    }

    #[test]
    fn a_plate_on_the_ground_takes_its_fill_and_ink_and_a_plate_over_arcs_the_canvas() {
        let connection = Paint::Token(Token::TextMuted);
        let slab_ground = GroundPaint {
            fill: Paint::Token(Token::SurfaceTop(CONTAINER_SURFACE)),
            ink: Paint::Token(Token::SurfaceInk(CONTAINER_SURFACE)),
        };
        let over_arcs = PlatePaint::over_arcs(connection);
        let on_slab = PlatePaint::on_ground(slab_ground);
        assert_eq!(over_arcs.fill, GroundPaint::CANVAS.fill);
        assert_eq!(over_arcs.outline, connection);
        assert_eq!(over_arcs.ink, GroundPaint::CANVAS.ink);
        assert_eq!(on_slab.fill, slab_ground.fill);
        assert_eq!(on_slab.outline, slab_ground.ink);
        assert_eq!(on_slab.ink, slab_ground.ink);
    }

    #[test]
    fn light_bases_get_a_dark_ink_and_dark_bases_white() {
        let green = Rgba::from_packed(0x14f195ff);
        let violet = Rgba::from_packed(0x9945ffff);
        let green_ink = SolidPaint::from_base(green).ink();
        let violet_ink = SolidPaint::from_base(violet).ink();
        let Paint::Literal(green_ink_color) = green_ink else {
            panic!("authored bases resolve to literals");
        };
        let green_ink_lightness = Oklab::from_rgb(green_ink_color.rgb).lightness;
        assert!(green_ink_lightness < 0.3, "{green_ink_color:?}");
        let opaque_white = Rgba::from_packed(0xffffffff);
        assert_eq!(violet_ink, Paint::Literal(opaque_white));
    }

    #[test]
    fn every_opaque_color_form_fills_a_solid_alike() {
        let red_paint = SolidPaint::from_base(RED);
        for red_fill in ["#f00", "#ff0000", "#F00F", "#ff0000ff", "red", "RED"] {
            let styled_paint = solid_paint_with_style(&[(ColorKey::FILL.name(), red_fill)]);
            assert_eq!(styled_paint, red_paint, "{red_fill:?}");
        }
    }

    #[test]
    fn translucent_fills_keep_their_alpha_on_faces_but_not_on_ink() {
        let cases = [("#ff000080", 0x80), ("#f008", 0x88)];
        for (translucent_fill, alpha) in cases {
            let paint = solid_paint_with_style(&[(ColorKey::FILL.name(), translucent_fill)]);
            let top = paint.face(FaceShade::Top);
            let translucent_red = Rgba {
                rgb: RED.rgb,
                alpha,
            };
            assert_eq!(top, Paint::Literal(translucent_red), "{translucent_fill:?}");
            let opaque_paint = SolidPaint::from_base(RED);
            assert_eq!(paint.ink(), opaque_paint.ink(), "{translucent_fill:?}");
        }
    }

    #[test]
    fn transparent_fills_leave_faces_unfilled_and_outlined() {
        for transparent_fill in ["transparent", "#0000", "#ff000000"] {
            let paint = solid_paint_with_style(&[(ColorKey::FILL.name(), transparent_fill)]);
            assert_eq!(paint, SolidPaint::unfilled(), "{transparent_fill:?}");
            assert_eq!(paint.face(FaceShade::Left), Paint::None);
        }
    }

    #[test]
    fn authored_stroke_and_dash_outline_an_unfilled_solid() {
        let style = [
            (ColorKey::FILL.name(), "transparent"),
            (ColorKey::STROKE.name(), "#5b6170"),
            (NumberKey::STROKE_DASH.name(), "5"),
        ];
        let paint = solid_paint_with_style(&style);
        let stroke = Rgba::from_packed(0x5b6170ff);
        let expected_dashes = Dashes {
            dash: 5.0,
            gap: 4.0,
        };
        let expected_outline = Outline {
            paint: Paint::Literal(stroke),
            width: STROKED_OUTLINE_WIDTH,
            dashes: Some(expected_dashes),
        };
        assert_eq!(paint.outline(), expected_outline);
        assert_eq!(paint.face(FaceShade::Top), Paint::None);
        assert_eq!(paint.ink(), Paint::Token(Token::TextMuted));
    }

    #[test]
    fn zero_dash_draws_a_solid_outline() {
        let paint = solid_paint_with_style(&[(NumberKey::STROKE_DASH.name(), "0")]);
        assert_eq!(paint, SolidPaint::from_surface(LEAF_SURFACE));
    }

    #[test]
    fn connections_fall_back_from_edge_stroke_to_source_fill_to_muted_text() {
        let mut builder = ModelBuilder::new();
        let app_element = Element::new("app").with_style(ColorKey::FILL.name(), "#9945ff");
        let app = builder.add_top_level("app", app_element);
        let api = builder.add_top_level("api", Element::new("api"));
        let stroked_edge = Edge::new().with_style(ColorKey::STROKE.name(), "#14f195");
        let stroked = builder.add_edge(&app, &api, stroked_edge);
        let from_app = builder.add_edge(&app, &api, Edge::new());
        let from_api = builder.add_edge(&api, &app, Edge::new());
        let model = builder.build().model.expect("valid model");
        let paints = ModelPaints::new(&model);
        let stroked_paint = paints.connection(&stroked);
        let from_app_paint = paints.connection(&from_app);
        let from_api_paint = paints.connection(&from_api);
        let green = Rgba::from_packed(0x14f195ff);
        let violet = Rgba::from_packed(0x9945ffff);
        assert_eq!(stroked_paint, Paint::Literal(green));
        assert_eq!(from_app_paint, Paint::Literal(violet));
        assert_eq!(from_api_paint, Paint::Token(Token::TextMuted));
    }

    #[test]
    fn connections_from_dark_translucent_or_transparent_fills_fall_back_to_muted_text() {
        for source_fill in ["#3a3f4b", "#9945ff80", "transparent"] {
            let paint = connection_paint_with_styles(source_fill, &[]);
            assert_eq!(paint, Paint::Token(Token::TextMuted), "{source_fill:?}");
        }
    }

    #[test]
    fn transparent_and_translucent_edge_strokes_keep_their_alpha() {
        let transparent =
            connection_paint_with_styles("#3a3f4b", &[(ColorKey::STROKE.name(), "transparent")]);
        assert_eq!(transparent, Paint::None);
        let translucent =
            connection_paint_with_styles("#3a3f4b", &[(ColorKey::STROKE.name(), "#14f19580")]);
        let translucent_green = Rgba::from_packed(0x14f19580);
        assert_eq!(translucent, Paint::Literal(translucent_green));
    }

    #[test]
    fn connections_are_dashed_by_their_stroke_dash_two_units_apart_and_solid_at_zero() {
        let mut builder = ModelBuilder::new();
        let app = builder.add_top_level("app", Element::new("app"));
        let api = builder.add_top_level("api", Element::new("api"));
        let flowing = builder.add_edge(&app, &api, Edge::new());
        let dotted = builder.add_edge(&app, &api, Edge::new().with_style("stroke-dash", "2"));
        let solid = builder.add_edge(&app, &api, Edge::new().with_style("stroke-dash", "0"));
        let model = builder.build().model.expect("valid model");
        let paints = ModelPaints::new(&model);

        let flowing_dashes = paints.connection_dashes(&flowing);
        let dotted_dashes = paints.connection_dashes(&dotted);
        let solid_dashes = paints.connection_dashes(&solid);

        let flowing_dash_array = flowing_dashes.map(|dashes| dashes.to_string());
        let dotted_dash_array = dotted_dashes.map(|dashes| dashes.to_string());
        assert_eq!(flowing_dash_array.as_deref(), Some("5 7"));
        assert_eq!(dotted_dash_array.as_deref(), Some("2 4"));
        assert_eq!(solid_dashes, None);
        let shared_paint = paints.stroke_sample(&[flowing.clone(), dotted]);
        let no_paint = paints.stroke_sample(&[]);
        assert_eq!(shared_paint, paints.connection(&flowing));
        assert_eq!(no_paint, Paint::Token(Token::Text));
    }
}
