//! Define one symbol per traveler a scene's animated connections carry, each glyph painted by the `<use>` that moves it.

use std::collections::BTreeMap;

use kurbo::{BezPath, Point, Rect, Vec2};
use svugy_model::model::{Edge, EdgeId, Model, Traveler};

use crate::geometry::Group;
use crate::render::contract::RuntimeContract;
use crate::render::writer::css::CssValue;
use crate::render::writer::path_data::PathData;
use crate::render::writer::url::SafeUrl;
use crate::render::writer::{Attributes, SvgWriter, ViewBox};

/// Prefix every symbol's id, followed by its traveler's name.
const SYMBOL_ID_PREFIX: &str = "traveler-";
/// Frame every glyph in this square centred on the path point, wide enough for its stroke.
const SYMBOL_FRAME: Rect = Rect::new(-5.0, -5.0, 5.0, 5.0);
/// Stroke every glyph's body and marks at this width.
const GLYPH_STROKE_WIDTH: f64 = 1.0;
/// Fill an outlined glyph's body at this opacity, so its marks read over it.
const TINTED_BODY_OPACITY: f64 = 0.35;
/// Size the dot by this radius, its stroke included.
const DOT_RADIUS: f64 = 3.0;
/// Lengthen a quarter arc's handles by this fraction of the radius: 4(√2 − 1)/3.
const QUARTER_ARC_HANDLE: f64 = 0.552_284_749_830_793_4;
/// Move a traveler along its whole path in this duration.
const TRAVEL_DURATION: &str = "2.4s";

/// Draws one built-in traveler as a glyph centred on the origin, about eight units across.
trait TravelerSymbol {
    /// Name the traveler this symbol draws, whose spelling `data-svugy-traveler` and the symbol's id use.
    const TRAVELER: Traveler;
    /// Fill the body at this opacity: tinted under the marks of an outlined glyph.
    const BODY_OPACITY: f64 = TINTED_BODY_OPACITY;

    /// Outline the glyph's body, filled and stroked.
    fn body() -> BezPath;

    /// Draw the marks set over the body, stroked only.
    fn marks() -> BezPath;

    /// Outline the circle of `radius` around `center` as four cubic quarter arcs, each handle `QUARTER_ARC_HANDLE · radius` long.
    fn circle(center: Point, radius: f64) -> BezPath {
        let handle = QUARTER_ARC_HANDLE * radius;
        let handle_down = Vec2::new(0.0, handle);
        let handle_right = Vec2::new(handle, 0.0);
        let radius_down = Vec2::new(0.0, radius);
        let radius_right = Vec2::new(radius, 0.0);
        let right = center + radius_right;
        let bottom = center + radius_down;
        let left = center - radius_right;
        let top = center - radius_down;
        let mut circle = BezPath::new();
        circle.move_to(right);
        circle.curve_to(right + handle_down, bottom + handle_right, bottom);
        circle.curve_to(bottom - handle_right, left + handle_down, left);
        circle.curve_to(left - handle_down, top - handle_right, top);
        circle.curve_to(top + handle_right, right - handle_down, right);
        circle.close_path();
        circle
    }
}

/// Draws the default traveler: a solid dot.
struct Dot;

/// Draws an envelope: a landscape rectangle crossed by its flap's V.
struct Envelope;

/// Draws a parcel: an isometric box whose lid is tied by a tape cross.
struct Parcel;

/// Draws a document: a portrait page with a folded corner and two text lines.
struct Document;

/// Draws a key: a ring, a shaft and two teeth.
struct Key;

/// Resolves an edge's traveler to the symbol it is drawn as, the one place a model `Traveler` becomes a `TravelerSymbol`.
#[derive(Clone, Copy)]
pub(crate) struct ResolvedTraveler {
    name: &'static str,
    body_opacity: f64,
    body: fn() -> BezPath,
    marks: fn() -> BezPath,
}

/// Defines the symbol of every traveler a scene's animated connections carry.
pub(crate) struct TravelerSymbols {
    travelers_by_name: BTreeMap<&'static str, ResolvedTraveler>,
}

impl TravelerSymbols {
    /// Collect the traveler of every connection of `drawn_layers` that moves one, interiors included, each resolved in the model its layer is drawn from.
    pub(crate) fn collect(drawn_layers: &[(&Group, &Model)]) -> Self {
        let mut travelers_by_name = BTreeMap::new();
        for &(root, model) in drawn_layers {
            let levels = root.this_and_nested_levels();
            let connections = levels.flat_map(|(group, _to_root)| &group.connections);
            let moving_connections = connections.filter(|connection| connection.dot_path.is_some());
            let layer_travelers = moving_connections.map(|connection| {
                let traveler = ResolvedTraveler::carried_along(&connection.edge, model);
                (traveler.name, traveler)
            });
            travelers_by_name.extend(layer_travelers);
        }
        Self { travelers_by_name }
    }

    /// Write one `<symbol>` per collected traveler, in name order.
    pub(crate) fn write_symbols(&self, writer: &mut SvgWriter) {
        for traveler in self.travelers_by_name.values() {
            traveler.write_symbol(writer);
        }
    }

    /// Tell whether no connection moves a traveler.
    pub(crate) fn is_empty(&self) -> bool {
        self.travelers_by_name.is_empty()
    }
}

impl ResolvedTraveler {
    /// Resolve the traveler `edge` carries in `model`, else the dot.
    pub(crate) fn carried_along(edge: &EdgeId, model: &Model) -> Self {
        let authored_edge = model.edge(edge);
        let authored_traveler = authored_edge.and_then(Edge::traveler);
        let traveler = authored_traveler.unwrap_or(Traveler::Dot);
        match traveler {
            Traveler::Dot => Self::symbol::<Dot>(),
            Traveler::Envelope => Self::symbol::<Envelope>(),
            Traveler::Parcel => Self::symbol::<Parcel>(),
            Traveler::Document => Self::symbol::<Document>(),
            Traveler::Key => Self::symbol::<Key>(),
        }
    }

    /// Resolve the symbol `S` draws.
    fn symbol<S: TravelerSymbol>() -> Self {
        let name: &'static str = S::TRAVELER.into();
        Self {
            name,
            body_opacity: S::BODY_OPACITY,
            body: S::body,
            marks: S::marks,
        }
    }

    /// Write this traveler's `<use>`: its symbol framed around the origin, named for the runtime, painted in `paint`, moved by SMIL along `motion_path` forever.
    pub(crate) fn write_moving_along(
        &self,
        writer: &mut SvgWriter,
        motion_path: &BezPath,
        paint: &CssValue,
    ) {
        let symbol_id = self.symbol_id();
        let symbol_reference = SafeUrl::fragment(&symbol_id);
        let frame_width = SYMBOL_FRAME.width();
        let frame_height = SYMBOL_FRAME.height();
        let use_attributes = Attributes::new()
            .with_text(RuntimeContract::TRAVELER, self.name)
            .with_url("href", &symbol_reference)
            .with_number("x", SYMBOL_FRAME.x0)
            .with_number("y", SYMBOL_FRAME.y0)
            .with_number("width", frame_width)
            .with_number("height", frame_height)
            .with_css_value("fill", paint)
            .with_css_value("stroke", paint);
        let motion_path_data = PathData(motion_path).to_string();
        let motion_attributes = Attributes::new()
            .with_text("path", &motion_path_data)
            .with_text("dur", TRAVEL_DURATION)
            .with_text("repeatCount", "indefinite");
        writer.open_element("use", &use_attributes);
        writer.empty_element("animateMotion", &motion_attributes);
        writer.close_element();
    }

    /// Write the `<symbol>`: its body filled and stroked, then its marks stroked, both in the paint the `<use>` passes down.
    fn write_symbol(&self, writer: &mut SvgWriter) {
        let symbol_id = self.symbol_id();
        let view_box = ViewBox(SYMBOL_FRAME).to_string();
        let symbol_attributes = Attributes::new()
            .with_text("id", &symbol_id)
            .with_text("viewBox", &view_box)
            .with_number("stroke-width", GLYPH_STROKE_WIDTH)
            .with_text("stroke-linecap", "round")
            .with_text("stroke-linejoin", "round");
        let body = (self.body)();
        let body_data = PathData(&body).to_string();
        let body_attributes = Attributes::new()
            .with_text("d", &body_data)
            .with_number("fill-opacity", self.body_opacity);
        writer.open_element("symbol", &symbol_attributes);
        writer.empty_element("path", &body_attributes);
        let marks = (self.marks)();
        let has_marks = !marks.is_empty();
        if has_marks {
            let marks_data = PathData(&marks).to_string();
            let marks_attributes = Attributes::new()
                .with_text("d", &marks_data)
                .with_text("fill", "none");
            writer.empty_element("path", &marks_attributes);
        }
        writer.close_element();
    }

    /// Name the symbol of this traveler.
    fn symbol_id(&self) -> String {
        format!("{SYMBOL_ID_PREFIX}{}", self.name)
    }
}

impl TravelerSymbol for Dot {
    const TRAVELER: Traveler = Traveler::Dot;
    const BODY_OPACITY: f64 = 1.0;

    fn body() -> BezPath {
        let half_stroke = GLYPH_STROKE_WIDTH / 2.0;
        let radius = DOT_RADIUS - half_stroke;
        Self::circle(Point::ORIGIN, radius)
    }

    fn marks() -> BezPath {
        BezPath::new()
    }
}

impl TravelerSymbol for Envelope {
    const TRAVELER: Traveler = Traveler::Envelope;

    fn body() -> BezPath {
        let mut sheet = BezPath::new();
        sheet.move_to((-3.5, -2.5));
        sheet.line_to((3.5, -2.5));
        sheet.line_to((3.5, 2.5));
        sheet.line_to((-3.5, 2.5));
        sheet.close_path();
        sheet
    }

    fn marks() -> BezPath {
        let mut flap = BezPath::new();
        flap.move_to((-3.5, -2.5));
        flap.line_to((0.0, 0.5));
        flap.line_to((3.5, -2.5));
        flap
    }
}

impl TravelerSymbol for Parcel {
    const TRAVELER: Traveler = Traveler::Parcel;

    fn body() -> BezPath {
        let mut cube = BezPath::new();
        cube.move_to((0.0, -3.75));
        cube.line_to((3.25, -1.875));
        cube.line_to((3.25, 1.875));
        cube.line_to((0.0, 3.75));
        cube.line_to((-3.25, 1.875));
        cube.line_to((-3.25, -1.875));
        cube.close_path();
        cube
    }

    fn marks() -> BezPath {
        let mut edges_and_tape = BezPath::new();
        edges_and_tape.move_to((-3.25, -1.875));
        edges_and_tape.line_to((0.0, 0.0));
        edges_and_tape.line_to((3.25, -1.875));
        edges_and_tape.move_to((0.0, 0.0));
        edges_and_tape.line_to((0.0, 3.75));
        edges_and_tape.move_to((-1.625, -2.8125));
        edges_and_tape.line_to((1.625, -0.9375));
        edges_and_tape.move_to((1.625, -2.8125));
        edges_and_tape.line_to((-1.625, -0.9375));
        edges_and_tape
    }
}

impl TravelerSymbol for Document {
    const TRAVELER: Traveler = Traveler::Document;

    fn body() -> BezPath {
        let mut page = BezPath::new();
        page.move_to((-2.75, -3.5));
        page.line_to((1.0, -3.5));
        page.line_to((2.75, -1.75));
        page.line_to((2.75, 3.5));
        page.line_to((-2.75, 3.5));
        page.close_path();
        page
    }

    fn marks() -> BezPath {
        let mut fold_and_lines = BezPath::new();
        fold_and_lines.move_to((1.0, -3.5));
        fold_and_lines.line_to((1.0, -1.75));
        fold_and_lines.line_to((2.75, -1.75));
        fold_and_lines.move_to((-1.25, 0.25));
        fold_and_lines.line_to((1.25, 0.25));
        fold_and_lines.move_to((-1.25, 2.0));
        fold_and_lines.line_to((1.25, 2.0));
        fold_and_lines
    }
}

impl TravelerSymbol for Key {
    const TRAVELER: Traveler = Traveler::Key;

    fn body() -> BezPath {
        let ring_center = Point::new(-2.0, 0.0);
        Self::circle(ring_center, 1.75)
    }

    fn marks() -> BezPath {
        let mut shaft_and_teeth = BezPath::new();
        shaft_and_teeth.move_to((-0.25, 0.0));
        shaft_and_teeth.line_to((3.75, 0.0));
        shaft_and_teeth.move_to((2.25, 0.0));
        shaft_and_teeth.line_to((2.25, 1.5));
        shaft_and_teeth.move_to((3.5, 0.0));
        shaft_and_teeth.line_to((3.5, 1.5));
        shaft_and_teeth
    }
}
