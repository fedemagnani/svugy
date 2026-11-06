//! Write SVG markup with escaped values and fixed-decimal numbers.

pub(crate) mod css;
pub(crate) mod path_data;
pub(crate) mod url;

use std::collections::BTreeMap;
use std::fmt;

use css::{CssValue, StyleSheet};
use kurbo::{Affine, Rect};
use url::SafeUrl;

const FRACTION_DIGITS: usize = 3;
const FRACTION_SCALE: f64 = 1_000.0;
const MAX_FORMATTED_MAGNITUDE: f64 = 1e12;
const SCRIPT_CDATA_OPENING: &str = "//<![CDATA[\n";
const SCRIPT_CDATA_CLOSING: &str = "\n//]]>";
const STYLE_CDATA_OPENING: &str = "/*<![CDATA[*/";
const STYLE_CDATA_CLOSING: &str = "/*]]>*/";
const CDATA_END: &str = "]]>";
const CDATA_END_SPLIT: &str = "]]]]><![CDATA[>";

/// Writes SVG markup into a string, one element at a time.
pub(crate) struct SvgWriter {
    markup: String,
    open_elements: Vec<OpenElement>,
}

struct OpenElement {
    name: &'static str,
    is_start_tag_unterminated: bool,
    puts_children_on_own_lines: bool,
}

/// Collects the attributes of one element, emitted sorted by name.
#[derive(Default)]
pub(crate) struct Attributes {
    values_by_name: BTreeMap<&'static str, String>,
}

/// Formats a number with at most three fraction digits, trailing zeros trimmed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FixedDecimal(pub(crate) f64);

/// Formats a rectangle as a viewBox value, `min-x min-y width height` in fixed decimals.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewBox(pub(crate) Rect);

/// Formats an affine map as a `matrix(a b c d e f)` transform in fixed decimals.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Matrix(pub(crate) Affine);

impl SvgWriter {
    pub(crate) fn new() -> Self {
        SvgWriter {
            markup: String::new(),
            open_elements: Vec::new(),
        }
    }

    pub(crate) fn open_element(&mut self, name: &'static str, attributes: &Attributes) {
        self.prepare_parent_for_child_element();
        self.markup.push('<');
        self.markup.push_str(name);
        for (attribute_name, attribute_value) in &attributes.values_by_name {
            self.markup.push(' ');
            self.markup.push_str(attribute_name);
            self.markup.push_str("=\"");
            Self::push_escaped_attribute_value(&mut self.markup, attribute_value);
            self.markup.push('"');
        }
        self.open_elements.push(OpenElement {
            name,
            is_start_tag_unterminated: true,
            puts_children_on_own_lines: true,
        });
    }

    /// Write an element with no children as one self-closed tag.
    pub(crate) fn empty_element(&mut self, name: &'static str, attributes: &Attributes) {
        self.open_element(name, attributes);
        self.close_element();
    }

    pub(crate) fn text(&mut self, text: &str) {
        self.prepare_parent_for_text();
        Self::push_escaped_text(&mut self.markup, text);
    }

    pub(crate) fn style_element(&mut self, style_sheet: &StyleSheet) {
        let no_attributes = Attributes::new();
        self.open_element("style", &no_attributes);
        self.cdata(
            STYLE_CDATA_OPENING,
            style_sheet.as_str(),
            STYLE_CDATA_CLOSING,
        );
        self.close_element();
    }

    pub(crate) fn script_element(&mut self, source: &'static str) {
        let no_attributes = Attributes::new();
        self.open_element("script", &no_attributes);
        self.cdata(SCRIPT_CDATA_OPENING, source, SCRIPT_CDATA_CLOSING);
        self.close_element();
    }

    pub(crate) fn close_element(&mut self) {
        let Some(element) = self.open_elements.pop() else {
            debug_assert!(false, "close_element without an open element");
            return;
        };
        if element.is_start_tag_unterminated {
            self.markup.push_str("/>");
            return;
        }
        if element.puts_children_on_own_lines {
            self.markup.push('\n');
        }
        self.markup.push_str("</");
        self.markup.push_str(element.name);
        self.markup.push('>');
    }

    pub(crate) fn finish(mut self) -> String {
        debug_assert!(self.open_elements.is_empty(), "unclosed elements");
        self.markup.push('\n');
        self.markup
    }

    fn prepare_parent_for_child_element(&mut self) {
        let Some(parent) = self.open_elements.last_mut() else {
            debug_assert!(self.markup.is_empty(), "a second root element");
            return;
        };
        if parent.is_start_tag_unterminated {
            self.markup.push('>');
            parent.is_start_tag_unterminated = false;
        }
        if parent.puts_children_on_own_lines {
            self.markup.push('\n');
        }
    }

    fn prepare_parent_for_text(&mut self) {
        let Some(parent) = self.open_elements.last_mut() else {
            debug_assert!(false, "text outside the root element");
            return;
        };
        if parent.is_start_tag_unterminated {
            self.markup.push('>');
            parent.is_start_tag_unterminated = false;
        }
        parent.puts_children_on_own_lines = false;
    }

    fn cdata(&mut self, opening_guard: &str, content: &str, closing_guard: &str) {
        self.prepare_parent_for_text();
        self.markup.push_str(opening_guard);
        for (piece_index, piece) in content.split(CDATA_END).enumerate() {
            if piece_index > 0 {
                self.markup.push_str(CDATA_END_SPLIT);
            }
            self.markup.push_str(piece);
        }
        self.markup.push_str(closing_guard);
    }

    fn push_escaped_text(markup: &mut String, raw: &str) {
        for character in raw.chars().filter(Self::is_xml_char) {
            match character {
                '&' => markup.push_str("&amp;"),
                '<' => markup.push_str("&lt;"),
                '>' => markup.push_str("&gt;"),
                '\r' => markup.push_str("&#13;"),
                _ => markup.push(character),
            }
        }
    }

    fn push_escaped_attribute_value(markup: &mut String, raw: &str) {
        for character in raw.chars().filter(Self::is_xml_char) {
            match character {
                '&' => markup.push_str("&amp;"),
                '<' => markup.push_str("&lt;"),
                '>' => markup.push_str("&gt;"),
                '"' => markup.push_str("&quot;"),
                '\t' => markup.push_str("&#9;"),
                '\n' => markup.push_str("&#10;"),
                '\r' => markup.push_str("&#13;"),
                _ => markup.push(character),
            }
        }
    }

    /// Tell whether XML 1.0 allows a character at all, escaped or not.
    fn is_xml_char(character: &char) -> bool {
        matches!(
            character,
            '\t' | '\n' | '\r' | '\u{20}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
        )
    }
}

impl Attributes {
    pub(crate) fn new() -> Self {
        Attributes::default()
    }

    pub(crate) fn with_text(self, name: &'static str, value: &str) -> Self {
        let is_link_attribute = name == "href" || name == "xlink:href";
        debug_assert!(!is_link_attribute, "links go through with_url");
        let owned_value = value.to_owned();
        self.with_value(name, owned_value)
    }

    pub(crate) fn with_number(self, name: &'static str, value: f64) -> Self {
        let formatted = FixedDecimal(value).to_string();
        self.with_value(name, formatted)
    }

    pub(crate) fn with_url(self, name: &'static str, url: &SafeUrl) -> Self {
        let owned_url = url.as_str().to_owned();
        self.with_value(name, owned_url)
    }

    pub(crate) fn with_css_value(self, name: &'static str, value: &CssValue) -> Self {
        let owned_value = value.as_str().to_owned();
        self.with_value(name, owned_value)
    }

    fn with_value(mut self, name: &'static str, value: String) -> Self {
        let previous_value = self.values_by_name.insert(name, value);
        debug_assert!(previous_value.is_none(), "duplicate attribute {name}");
        self
    }
}

impl fmt::Display for FixedDecimal {
    /// Round to the nearest multiple of 10⁻³, halves away from zero.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let FixedDecimal(value) = *self;
        let is_formattable = value.is_finite() && value.abs() < MAX_FORMATTED_MAGNITUDE;
        debug_assert!(is_formattable, "unformattable number {value}");
        let scaled = (value * FRACTION_SCALE).round();
        let thousandths = if is_formattable { scaled as i64 } else { 0 };
        let sign = if thousandths < 0 { "-" } else { "" };
        let magnitude = thousandths.unsigned_abs();
        let scale = FRACTION_SCALE as u64;
        let integer_part = magnitude / scale;
        let fraction_part = magnitude % scale;
        if fraction_part == 0 {
            return write!(formatter, "{sign}{integer_part}");
        }
        let fraction_digits = format!("{fraction_part:0FRACTION_DIGITS$}");
        let trimmed_fraction_digits = fraction_digits.trim_end_matches('0');
        write!(formatter, "{sign}{integer_part}.{trimmed_fraction_digits}")
    }
}

impl fmt::Display for ViewBox {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ViewBox(view) = *self;
        let min_x = FixedDecimal(view.x0);
        let min_y = FixedDecimal(view.y0);
        let width = FixedDecimal(view.width());
        let height = FixedDecimal(view.height());
        write!(formatter, "{min_x} {min_y} {width} {height}")
    }
}

impl fmt::Display for Matrix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Matrix(affine) = *self;
        let [a, b, c, d, e, f] = affine.as_coeffs().map(FixedDecimal);
        write!(formatter, "matrix({a} {b} {c} {d} {e} {f})")
    }
}

#[cfg(test)]
mod tests {
    use svugy_model::color::Rgba;

    use super::css::{CssDeclaration, CssIdentifier, CssSelector};
    use super::*;

    const FORBIDDEN_XML_CHARS: &str = "\u{0}\u{1}\u{8}\u{B}\u{C}\u{E}\u{1F}\u{FFFE}\u{FFFF}";

    fn parse_as_svg(markup: &str) -> Result<usvg::Tree, usvg::Error> {
        let options = usvg::Options::default();
        usvg::Tree::from_str(markup, &options)
    }

    fn svg_root_attributes() -> Attributes {
        Attributes::new()
            .with_text("xmlns", "http://www.w3.org/2000/svg")
            .with_text("viewBox", "0 0 10 10")
    }

    fn svg_with_labelled_path(path_id: &str, label: &str) -> String {
        let mut writer = SvgWriter::new();
        let root_attributes = svg_root_attributes();
        writer.open_element("svg", &root_attributes);
        let path_attributes = Attributes::new()
            .with_text("id", path_id)
            .with_text("d", "M0 0 L10 10")
            .with_text("stroke", "#000000")
            .with_text("aria-label", label);
        writer.empty_element("path", &path_attributes);
        let text_attributes = Attributes::new().with_text("data-label", label);
        writer.open_element("text", &text_attributes);
        writer.text(label);
        writer.close_element();
        writer.close_element();
        writer.finish()
    }

    #[test]
    fn fixed_decimal_rounds_to_three_digits_and_trims_trailing_zeros() {
        let cases = [
            (0.0, "0"),
            (1.0, "1"),
            (1.5, "1.5"),
            (1.25, "1.25"),
            (0.866_025_403_784_438_6, "0.866"),
            (-12.345_6, "-12.346"),
            (0.000_6, "0.001"),
            (2.999_9, "3"),
            (-0.5, "-0.5"),
            (123_456_789.0, "123456789"),
            (-1_000.001, "-1000.001"),
        ];
        for (value, expected) in cases {
            assert_eq!(FixedDecimal(value).to_string(), expected, "{value}");
        }
    }

    #[test]
    fn fixed_decimal_normalises_negative_zero() {
        assert_eq!(FixedDecimal(-0.0).to_string(), "0");
        assert_eq!(FixedDecimal(-0.000_4).to_string(), "0");
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "unformattable number")]
    fn fixed_decimal_rejects_nan() {
        let _formatted = FixedDecimal(f64::NAN).to_string();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "unformattable number")]
    fn fixed_decimal_rejects_infinity() {
        let _formatted = FixedDecimal(f64::NEG_INFINITY).to_string();
    }

    #[test]
    fn writer_sorts_attributes_self_closes_empty_elements_and_keeps_text_inline() {
        let mut writer = SvgWriter::new();
        let root_attributes = svg_root_attributes();
        writer.open_element("svg", &root_attributes);
        let group_attributes = Attributes::new()
            .with_text("id", "g")
            .with_number("opacity", 0.5)
            .with_text("class", "c");
        writer.open_element("g", &group_attributes);
        let rect_attributes = Attributes::new()
            .with_number("y", 2.0)
            .with_number("x", -0.0);
        writer.empty_element("rect", &rect_attributes);
        let no_attributes = Attributes::new();
        writer.open_element("text", &no_attributes);
        writer.text("a < b");
        writer.close_element();
        writer.close_element();
        writer.close_element();
        let markup = writer.finish();
        let expected = "<svg viewBox=\"0 0 10 10\" xmlns=\"http://www.w3.org/2000/svg\">\n\
            <g class=\"c\" id=\"g\" opacity=\"0.5\">\n\
            <rect x=\"0\" y=\"2\"/>\n\
            <text>a &lt; b</text>\n\
            </g>\n\
            </svg>\n";
        assert_eq!(markup, expected);
        assert!(parse_as_svg(&markup).is_ok());
    }

    #[test]
    fn hostile_labels_are_stripped_and_escaped_into_parseable_xml() {
        let hostile_labels = [
            FORBIDDEN_XML_CHARS.to_owned(),
            "]]> <![CDATA[ x ]]>".to_owned(),
            "\"quoted\" and 'single' & <b>".to_owned(),
            "<script>alert(1)</script>".to_owned(),
            "</text></svg><svg onload=\"alert(1)\">".to_owned(),
            "tab\tline\ncarriage\r".to_owned(),
            format!("mixed{FORBIDDEN_XML_CHARS}&amp;"),
        ];
        for label in &hostile_labels {
            let markup = svg_with_labelled_path("p", label);
            let parse_result = parse_as_svg(&markup);
            assert!(parse_result.is_ok(), "{label:?} -> {markup}");
            assert!(!markup.contains("<script"), "{markup}");
        }
    }

    #[test]
    fn hostile_ids_round_trip_through_usvg_without_forbidden_chars() {
        let hostile_id = format!("a\"<&>'{FORBIDDEN_XML_CHARS}b");
        let markup = svg_with_labelled_path(&hostile_id, "label");
        let tree = parse_as_svg(&markup).expect("parseable");
        assert!(tree.node_by_id("a\"<&>'b").is_some(), "{markup}");
    }

    #[test]
    fn style_cdata_is_comment_guarded_split_and_applied() {
        let mut style_sheet = StyleSheet::new();
        style_sheet.push_static("/* a < b && ]]> c */\n");
        let selector = CssSelector::attribute("data-target");
        let fill_property = CssIdentifier::escape("fill");
        let red_color = Rgba::from_packed(0xff0000ff);
        let red = CssValue::color(red_color);
        let fill_declaration = CssDeclaration {
            property: fill_property,
            value: red,
        };
        style_sheet.push_rule(&selector, &[fill_declaration]);
        let mut writer = SvgWriter::new();
        let root_attributes = svg_root_attributes();
        writer.open_element("svg", &root_attributes);
        writer.style_element(&style_sheet);
        let rect_attributes = Attributes::new()
            .with_text("id", "target")
            .with_text("data-target", "")
            .with_number("width", 5.0)
            .with_number("height", 5.0);
        writer.empty_element("rect", &rect_attributes);
        writer.close_element();
        let markup = writer.finish();
        assert!(markup.contains("<style>/*<![CDATA[*//* a < b && ]]]]><![CDATA[> c */"));
        assert!(markup.contains("[data-target]{fill:#ff0000}\n/*]]>*/</style>"));
        let tree = parse_as_svg(&markup).expect("parseable");
        let target = tree.node_by_id("target").expect("kept");
        let usvg::Node::Path(path) = target else {
            panic!("rect becomes a path");
        };
        let fill = path.fill().expect("filled");
        let is_red = matches!(fill.paint(), usvg::Paint::Color(color) if color.red == 255 && color.green == 0);
        assert!(is_red, "{markup}");
    }

    #[test]
    fn hostile_identifiers_in_style_sheets_parse_as_xml() {
        let hostile_property = format!("--x</style><script>]]>{FORBIDDEN_XML_CHARS}&\"'");
        let property_identifier = CssIdentifier::escape(&hostile_property);
        let selector = CssSelector::attribute("data-x");
        let blue_color = Rgba::from_packed(0x0000ffff);
        let blue = CssValue::color(blue_color);
        let fill_declaration = CssDeclaration {
            property: property_identifier,
            value: blue,
        };
        let mut style_sheet = StyleSheet::new();
        style_sheet.push_rule(&selector, &[fill_declaration]);
        let mut writer = SvgWriter::new();
        let root_attributes = svg_root_attributes();
        writer.open_element("svg", &root_attributes);
        writer.style_element(&style_sheet);
        writer.close_element();
        let markup = writer.finish();
        assert!(parse_as_svg(&markup).is_ok(), "{markup}");
        assert!(!markup.contains("</style><script>"), "{markup}");
    }

    #[test]
    fn script_cdata_starts_on_its_own_line_and_splits_cdata_end() {
        let mut writer = SvgWriter::new();
        let root_attributes = svg_root_attributes();
        writer.open_element("svg", &root_attributes);
        writer.script_element("if (a < b && c) { s = \"]]>\"; }");
        writer.close_element();
        let markup = writer.finish();
        let expected_script =
            "<script>//<![CDATA[\nif (a < b && c) { s = \"]]]]><![CDATA[>\"; }\n//]]></script>";
        assert!(markup.contains(expected_script), "{markup}");
        assert!(parse_as_svg(&markup).is_ok());
    }

    #[test]
    fn accepted_links_are_written_as_parseable_href_attributes() {
        let accepted_links = [
            "https://example.com/?a=1&b=<2>",
            "mailto:a@b.c",
            "docs/a.md#x",
        ];
        for link in accepted_links {
            let url = SafeUrl::parse(link).expect("accepted");
            let mut writer = SvgWriter::new();
            let root_attributes = svg_root_attributes();
            writer.open_element("svg", &root_attributes);
            let anchor_attributes = Attributes::new().with_url("href", &url);
            writer.empty_element("a", &anchor_attributes);
            writer.close_element();
            let markup = writer.finish();
            assert!(parse_as_svg(&markup).is_ok(), "{markup}");
        }
    }
}
