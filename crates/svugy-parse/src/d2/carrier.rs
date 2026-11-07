//! Read the `svugy` carrier of the root or an element into the typed attributes it sets, rejecting unknown attributes, lists and malformed values one by one.

use std::iter;
use std::str::FromStr;

use strum::VariantNames;
use svugy_model::legend::DashKey;
use svugy_model::model::{Position, Shape};
use svugy_model::path::ElementPath;

use super::ast::{Scalar, Span, SvugyProperty, SvugyValue};
use super::diagnostic::Diagnostics;
use crate::prose::ProseList;

/// Address the root carrier's one list attribute, the legend's lines.
const LEGEND_ENTRIES_PATH: [&str; 2] = ["legend", "entries"];

/// Read carriers into the typed attributes they set, recording an error for every property their holder does not take.
pub(super) struct CarrierReader<'sink> {
    diagnostics: &'sink mut Diagnostics,
}

/// Name one attribute the root's carrier sets.
pub(super) enum RootAttribute {
    Subtitle(String),
    LegendTitle(String),
    LegendEntries(Vec<String>),
    LegendDash(DashKey, String),
    LegendBadge(LegendBadge),
}

/// Name one attribute an element's carrier sets.
pub(super) enum ElementAttribute {
    Caption(String),
    Primitive(Shape),
    Height(f64),
    BadgeMark { is_marked: bool, span: Span },
    Position(Position),
}

/// Describe the badges of the shape at `target_path`, kept as written since that shape may be declared after the legend.
pub(super) struct LegendBadge {
    pub(super) target_path: Vec<String>,
    pub(super) description: String,
    pub(super) span: Span,
}

impl<'sink> CarrierReader<'sink> {
    /// Create a reader recording its errors into `diagnostics`.
    pub(super) fn new(diagnostics: &'sink mut Diagnostics) -> Self {
        Self { diagnostics }
    }

    /// Read the root's carrier into its subtitle and legend attributes in write order, rejecting every other attribute: the title is the root's own `label`.
    pub(super) fn read_root(&mut self, properties: Vec<SvugyProperty>) -> Vec<RootAttribute> {
        let mut attributes = Vec::new();
        for property in properties {
            let SvugyProperty {
                attribute_path,
                value,
                span: property_span,
            } = property;
            let scalar = match value {
                SvugyValue::Scalar(scalar) => scalar,
                SvugyValue::List { items, span } => {
                    let legend_entries = self.read_legend_entries(&attribute_path, items, span);
                    attributes.extend(legend_entries);
                    continue;
                }
            };
            let attribute_segments: Vec<&str> = attribute_path.iter().map(String::as_str).collect();
            match attribute_segments.as_slice() {
                ["subtitle"] => {
                    let subtitle = RootAttribute::Subtitle(scalar.text);
                    attributes.push(subtitle);
                }
                ["legend", "title"] => {
                    let legend_title = RootAttribute::LegendTitle(scalar.text);
                    attributes.push(legend_title);
                }
                entries_path if entries_path == LEGEND_ENTRIES_PATH => {
                    let entries_carrier_path = Self::spell_carrier_path(&LEGEND_ENTRIES_PATH);
                    let message = format!(
                        "`{entries_carrier_path}` is a list of lines, e.g. `[\"① …\"; \"② …\"]`"
                    );
                    self.diagnostics.record_error(message, scalar.span);
                }
                ["legend", "dashes"] | ["legend", "badges"] => {
                    self.reject_root_attribute(&attribute_path, property_span);
                }
                ["legend", "dashes", dash_key_path @ ..] => {
                    let legend_dash = self.read_legend_dash(dash_key_path, scalar, property_span);
                    attributes.extend(legend_dash);
                }
                ["legend", "badges", target_path @ ..] => {
                    let target_path = target_path.iter().map(ToString::to_string).collect();
                    let legend_badge = LegendBadge {
                        target_path,
                        description: scalar.text,
                        span: property_span,
                    };
                    let legend_badge = RootAttribute::LegendBadge(legend_badge);
                    attributes.push(legend_badge);
                }
                _unknown_attribute => self.reject_root_attribute(&attribute_path, property_span),
            }
        }
        attributes
    }

    /// Read the legend's entries from the one list attribute, the latest list replacing any earlier one as in d2; any other list is an error.
    fn read_legend_entries(
        &mut self,
        attribute_path: &[String],
        items: Vec<Scalar>,
        span: Span,
    ) -> Option<RootAttribute> {
        let is_entries = attribute_path == LEGEND_ENTRIES_PATH;
        if is_entries {
            let entries = items.into_iter().map(|item| item.text).collect();
            let legend_entries = RootAttribute::LegendEntries(entries);
            return Some(legend_entries);
        }
        self.reject_list(attribute_path, span);
        None
    }

    /// Read the legend's description of the arcs dashed as long as `dash_key_path` spells, rejecting a key that is not a plain number.
    fn read_legend_dash(
        &mut self,
        dash_key_path: &[&str],
        description: Scalar,
        span: Span,
    ) -> Option<RootAttribute> {
        let dash_key_segments = dash_key_path.iter().copied();
        let dash_key = ElementPath::from_segments(dash_key_segments).to_string();
        let Ok(parsed_dash_key) = dash_key.parse::<DashKey>() else {
            let message = format!(
                "`{dash_key}` is not a dash length: a `svugy.legend.dashes` key is a plain number like `5`, an arc's `stroke-dash`"
            );
            self.diagnostics.record_error(message, span);
            return None;
        };
        let legend_dash = RootAttribute::LegendDash(parsed_dash_key, description.text);
        Some(legend_dash)
    }

    /// Reject an attribute the root carrier does not hold.
    fn reject_root_attribute(&mut self, attribute_path: &[String], span: Span) {
        let carrier_path = Self::spell_carrier_path(attribute_path);
        let message = format!(
            "`{carrier_path}` is not a svugy attribute of the root: expected `svugy.subtitle` or `svugy.legend` with `title`, `entries`, `dashes` or `badges`, and the title is the root's own `label`"
        );
        self.diagnostics.record_error(message, span);
    }

    /// Read an element's carrier into its caption, primitive, height and badge mark in write order, then its position, written whole within the carrier's `span`.
    pub(super) fn read_element(
        &mut self,
        properties: Vec<SvugyProperty>,
        span: Span,
    ) -> Vec<ElementAttribute> {
        let mut attributes = Vec::new();
        let mut position_x = None;
        let mut position_y = None;
        for property in properties {
            let SvugyProperty {
                attribute_path,
                value,
                span: property_span,
            } = property;
            let Some(value) = self.scalar_or_reject_list(&attribute_path, value) else {
                continue;
            };
            let attribute_segments: Vec<&str> = attribute_path.iter().map(String::as_str).collect();
            let attribute = match attribute_segments.as_slice() {
                ["caption"] => {
                    let caption = ElementAttribute::Caption(value.text);
                    Some(caption)
                }
                ["shape"] => self.read_primitive(value).map(ElementAttribute::Primitive),
                ["height"] => self
                    .parse_decimal_or_reject(value, "`svugy.height`")
                    .map(ElementAttribute::Height),
                ["badge"] => {
                    let mark_span = value.span;
                    let is_marked = self.read_badge_mark(value);
                    is_marked.map(|is_marked| ElementAttribute::BadgeMark {
                        is_marked,
                        span: mark_span,
                    })
                }
                ["position", "x"] => {
                    position_x = Some(value);
                    None
                }
                ["position", "y"] => {
                    position_y = Some(value);
                    None
                }
                _unknown_attribute => {
                    self.reject_element_attribute(&attribute_path, property_span);
                    None
                }
            };
            attributes.extend(attribute);
        }
        let position = self.read_position(position_x, position_y, span);
        let position = position.map(ElementAttribute::Position);
        attributes.extend(position);
        attributes
    }

    /// Read the primitive the carrier names, spelled as in the model's JSON in any case; it outranks d2's `shape`, which d2 itself still reads.
    fn read_primitive(&mut self, primitive: Scalar) -> Option<Shape> {
        let lowercase_primitive = primitive.text.to_ascii_lowercase();
        let Ok(shape) = Shape::from_str(&lowercase_primitive) else {
            let expected = ProseList::join_quoted(Shape::VARIANTS, "or");
            let message = format!(
                "`{}` is not a svugy primitive: expected {expected}",
                primitive.text
            );
            self.diagnostics.record_error(message, primitive.span);
            return None;
        };
        Some(shape)
    }

    /// Read a badge mark, `true` or `false`; any other value is rejected.
    fn read_badge_mark(&mut self, mark: Scalar) -> Option<bool> {
        let lowercase_mark = mark.text.to_ascii_lowercase();
        match lowercase_mark.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _not_a_boolean => {
                let message = format!(
                    "`{}` is not a boolean: `svugy.badge` is `true` or `false`",
                    mark.text
                );
                self.diagnostics.record_error(message, mark.span);
                None
            }
        }
    }

    /// Read the position from the carrier's `x` and `y`, written together in one `vars` map at `span` so a position is never half-set.
    fn read_position(
        &mut self,
        x: Option<Scalar>,
        y: Option<Scalar>,
        span: Span,
    ) -> Option<Position> {
        let is_x_absent = x.is_none();
        let is_y_absent = y.is_none();
        let is_position_absent = is_x_absent && is_y_absent;
        if is_position_absent {
            return None;
        }
        let (Some(x), Some(y)) = (x, y) else {
            let message = "`svugy.position` needs both `x` and `y` in the same `vars` map";
            self.diagnostics.record_error(message, span);
            return None;
        };
        let coordinate_attribute = "a `svugy.position` coordinate";
        let x = self.parse_decimal_or_reject(x, coordinate_attribute)?;
        let y = self.parse_decimal_or_reject(y, coordinate_attribute)?;
        let position = Position { x, y };
        Some(position)
    }

    /// Reject an attribute an element's carrier does not hold.
    fn reject_element_attribute(&mut self, attribute_path: &[String], span: Span) {
        let carrier_path = Self::spell_carrier_path(attribute_path);
        let message = format!(
            "`{carrier_path}` is not a svugy attribute: expected `svugy.caption`, `svugy.shape`, `svugy.height`, `svugy.badge` or `svugy.position` with `x` and `y`"
        );
        self.diagnostics.record_error(message, span);
    }

    /// Return the scalar of an attribute that takes one value, rejecting a list.
    fn scalar_or_reject_list(
        &mut self,
        attribute_path: &[String],
        value: SvugyValue,
    ) -> Option<Scalar> {
        match value {
            SvugyValue::Scalar(scalar) => Some(scalar),
            SvugyValue::List {
                items: _items,
                span: list_span,
            } => {
                self.reject_list(attribute_path, list_span);
                None
            }
        }
    }

    /// Reject a list given to an attribute that takes one value.
    fn reject_list(&mut self, attribute_path: &[String], span: Span) {
        let carrier_path = Self::spell_carrier_path(attribute_path);
        let entries_carrier_path = Self::spell_carrier_path(&LEGEND_ENTRIES_PATH);
        let message = format!(
            "`{carrier_path}` is not a svugy list attribute: a list carries only the root's `{entries_carrier_path}`"
        );
        self.diagnostics.record_error(message, span);
    }

    /// Parse `number` as the value of `attribute`, rejecting it at its span unless it is a decimal.
    fn parse_decimal_or_reject(&mut self, number: Scalar, attribute: &str) -> Option<f64> {
        let Ok(value) = number.text.parse() else {
            let message = format!(
                "`{}` is not a number: {attribute} is a plain decimal like `91.5`",
                number.text
            );
            self.diagnostics.record_error(message, number.span);
            return None;
        };
        Some(value)
    }

    /// Spell the carrier path of `attribute_path` as a model path does, e.g. `svugy.position.x`, quoting a segment that holds a dot.
    fn spell_carrier_path(attribute_path: &[impl AsRef<str>]) -> String {
        let attribute_segments = attribute_path.iter().map(AsRef::as_ref);
        let variable_segment = iter::once(SvugyProperty::VARIABLE);
        let carrier_segments = variable_segment.chain(attribute_segments);
        let carrier_path = ElementPath::from_segments(carrier_segments);
        carrier_path.to_string()
    }
}
