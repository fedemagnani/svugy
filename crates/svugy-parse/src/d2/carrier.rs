//! Read the `svugy` carrier of the root or an element into the typed attributes it sets, rejecting unknown attributes and malformed values one by one.

use std::iter;
use std::str::FromStr;

use strum::VariantNames;
use svugy_model::model::{Position, Shape};
use svugy_model::path::ElementPath;

use super::ast::{Scalar, Span, SvugyProperty};
use super::diagnostic::Diagnostics;
use crate::prose::ProseList;

/// Address the root carrier's one attribute, the subtitle.
const SUBTITLE_PATH: [&str; 1] = ["subtitle"];

/// Read carriers into the typed attributes they set, recording an error for every property their holder does not take.
pub(super) struct CarrierReader<'sink> {
    diagnostics: &'sink mut Diagnostics,
}

/// Name one attribute an element's carrier sets.
pub(super) enum ElementAttribute {
    Caption(String),
    Primitive(Shape),
    Height(f64),
    BadgeMark { is_marked: bool, span: Span },
    Position(Position),
}

impl<'sink> CarrierReader<'sink> {
    /// Create a reader recording its errors into `diagnostics`.
    pub(super) fn new(diagnostics: &'sink mut Diagnostics) -> Self {
        Self { diagnostics }
    }

    /// Read the root's carrier into its subtitle, the latest write winning as in d2, rejecting every other attribute: the title is the root's own `label`.
    pub(super) fn read_root_subtitle(&mut self, properties: Vec<SvugyProperty>) -> Option<String> {
        let mut subtitle = None;
        for property in properties {
            let SvugyProperty {
                attribute_path,
                value: scalar,
                span: property_span,
            } = property;
            let is_subtitle = attribute_path == SUBTITLE_PATH;
            if is_subtitle {
                subtitle = Some(scalar.text);
                continue;
            }
            self.reject_root_attribute(&attribute_path, property_span);
        }
        subtitle
    }

    /// Reject an attribute the root carrier does not hold.
    fn reject_root_attribute(&mut self, attribute_path: &[String], span: Span) {
        let carrier_path = Self::spell_carrier_path(attribute_path);
        let message = format!(
            "`{carrier_path}` is not a svugy attribute of the root: expected `svugy.subtitle`, and the title is the root's own `label`"
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
