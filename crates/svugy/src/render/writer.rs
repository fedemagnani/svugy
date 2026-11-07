//! Write SVG markup with escaped values and fixed-decimal numbers.

pub(crate) mod css;
pub(crate) mod path_data;
pub(crate) mod url;

use std::collections::BTreeMap;
use std::fmt;

use css::{CssValue, StyleSheet};
use kurbo::{Affine, Rect};
use url::SafeUrl;

/// Count the fraction digits a fixed decimal keeps.
const FRACTION_DIGITS: usize = 3;
/// Scale a number so its kept fraction digits become integer units: 10^FRACTION_DIGITS.
const FRACTION_SCALE: u64 = 10_u64.pow(FRACTION_DIGITS as u32);
/// Bound the magnitude a fixed decimal formats exactly.
const MAX_FORMATTED_MAGNITUDE: f64 = 1e12;
/// Open a script's CDATA behind a line comment, so a script-unaware parser ignores the guard.
const SCRIPT_CDATA_OPENING: &str = "//<![CDATA[\n";
/// Close a script's CDATA behind a line comment.
const SCRIPT_CDATA_CLOSING: &str = "\n//]]>";
/// Open a style sheet's CDATA inside a CSS comment.
const STYLE_CDATA_OPENING: &str = "/*<![CDATA[*/";
/// Close a style sheet's CDATA inside a CSS comment.
const STYLE_CDATA_CLOSING: &str = "/*]]>*/";
/// Mark the end of a CDATA section.
const CDATA_END: &str = "]]>";
/// Split a CDATA end across two sections, so content can carry it.
const CDATA_END_SPLIT: &str = "]]]]><![CDATA[>";

/// Writes SVG markup into a string, one element at a time.
pub(crate) struct SvgWriter {
    markup: String,
    open_elements: Vec<OpenElement>,
}

/// Tracks an open element: its name, whether its start tag still awaits its `>`, and whether its children go on their own lines.
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

/// Formats a number as a fixed decimal with at most three fraction digits, trailing zeros trimmed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FixedDecimal(pub(crate) f64);

/// Formats a rectangle as a viewBox value, `min-x min-y width height` in fixed decimals.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewBox(pub(crate) Rect);

/// Formats an affine map as a `matrix(a b c d e f)` transform in fixed decimals.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Matrix(pub(crate) Affine);

/// Tells which characters XML 1.0 allows at all, escaped or not.
struct XmlChar;

impl SvgWriter {
    /// Start a writer with no markup and no open element.
    pub(crate) fn new() -> Self {
        SvgWriter {
            markup: String::new(),
            open_elements: Vec::new(),
        }
    }

    /// Open element `name` with `attributes`, leaving its start tag unterminated until its content or its close.
    pub(crate) fn open_element(&mut self, name: &'static str, attributes: &Attributes) {
        let parent = self.terminate_parent_start_tag();
        let has_parent = parent.is_some();
        let is_on_own_line = parent.is_some_and(|parent| parent.puts_children_on_own_lines);
        let is_child_or_first_root = has_parent || self.markup.is_empty();
        debug_assert!(is_child_or_first_root, "a second root element");
        if is_on_own_line {
            self.markup.push('\n');
        }
        self.markup.push('<');
        self.markup.push_str(name);
        for (attribute_name, attribute_value) in &attributes.values_by_name {
            self.markup.push(' ');
            self.markup.push_str(attribute_name);
            self.markup.push_str("=\"");
            self.push_escaped_attribute_value(attribute_value);
            self.markup.push('"');
        }
        let open_element = OpenElement {
            name,
            is_start_tag_unterminated: true,
            puts_children_on_own_lines: true,
        };
        self.open_elements.push(open_element);
    }

    /// Write an element with no children as one self-closed tag.
    pub(crate) fn empty_element(&mut self, name: &'static str, attributes: &Attributes) {
        self.open_element(name, attributes);
        self.close_element();
    }

    /// Write `text`, escaped, as content of the open element.
    pub(crate) fn text(&mut self, text: &str) {
        self.inline_parent_children();
        self.push_escaped_text(text);
    }

    /// Write a style element holding `style_sheet` in CDATA.
    pub(crate) fn style_element(&mut self, style_sheet: &StyleSheet) {
        let css = style_sheet.as_str();
        self.cdata_element("style", STYLE_CDATA_OPENING, css, STYLE_CDATA_CLOSING);
    }

    /// Write a script element holding `source` in CDATA.
    pub(crate) fn script_element(&mut self, source: &'static str) {
        self.cdata_element("script", SCRIPT_CDATA_OPENING, source, SCRIPT_CDATA_CLOSING);
    }

    /// Close the innermost open element, self-closing it when it has no content.
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

    /// Finish the markup with a final newline and return it.
    pub(crate) fn finish(mut self) -> String {
        debug_assert!(self.open_elements.is_empty(), "unclosed elements");
        self.markup.push('\n');
        self.markup
    }

    /// Terminate the parent's start tag, if still awaiting its `>`, and return the parent.
    fn terminate_parent_start_tag(&mut self) -> Option<&mut OpenElement> {
        let parent = self.open_elements.last_mut()?;
        if parent.is_start_tag_unterminated {
            self.markup.push('>');
            parent.is_start_tag_unterminated = false;
        }
        Some(parent)
    }

    /// Keep the parent's children inline with the text about to be written.
    fn inline_parent_children(&mut self) {
        let Some(parent) = self.terminate_parent_start_tag() else {
            debug_assert!(false, "text outside the root element");
            return;
        };
        parent.puts_children_on_own_lines = false;
    }

    /// Write element `name` holding `content` in CDATA between comment guards, every CDATA end inside it split.
    fn cdata_element(
        &mut self,
        name: &'static str,
        opening_guard: &str,
        content: &str,
        closing_guard: &str,
    ) {
        let no_attributes = Attributes::new();
        self.open_element(name, &no_attributes);
        self.inline_parent_children();
        let split_content = content.replace(CDATA_END, CDATA_END_SPLIT);
        self.markup.push_str(opening_guard);
        self.markup.push_str(&split_content);
        self.markup.push_str(closing_guard);
        self.close_element();
    }

    /// Escape `raw` as text content, dropping what XML 1.0 forbids.
    fn push_escaped_text(&mut self, raw: &str) {
        for character in raw.chars().filter(XmlChar::is_allowed) {
            self.push_escaped_text_character(character);
        }
    }

    /// Escape like text, plus the quote and the whitespace attribute-value normalisation would fold.
    fn push_escaped_attribute_value(&mut self, raw: &str) {
        for character in raw.chars().filter(XmlChar::is_allowed) {
            match character {
                '"' => self.markup.push_str("&quot;"),
                '\t' => self.markup.push_str("&#9;"),
                '\n' => self.markup.push_str("&#10;"),
                text_character => self.push_escaped_text_character(text_character),
            }
        }
    }

    /// Escape one text character: the markup delimiters and the carriage return a parser would fold.
    fn push_escaped_text_character(&mut self, character: char) {
        match character {
            '&' => self.markup.push_str("&amp;"),
            '<' => self.markup.push_str("&lt;"),
            '>' => self.markup.push_str("&gt;"),
            '\r' => self.markup.push_str("&#13;"),
            plain_character => self.markup.push(plain_character),
        }
    }
}

impl XmlChar {
    /// Tell whether XML 1.0 allows a character at all, escaped or not.
    fn is_allowed(character: &char) -> bool {
        matches!(
            character,
            '\t' | '\n' | '\r' | '\u{20}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
        )
    }
}

impl Attributes {
    /// Start with no attributes.
    pub(crate) fn new() -> Self {
        Attributes::default()
    }

    /// Set attribute `name` to the text `value`, never a link.
    pub(crate) fn with_text(self, name: &'static str, value: &str) -> Self {
        let is_plain_attribute = name != "href" && name != "xlink:href";
        debug_assert!(is_plain_attribute, "links go through with_url");
        let owned_value = value.to_owned();
        self.with_value(name, owned_value)
    }

    /// Set attribute `name` to the number `value`, in fixed decimals.
    pub(crate) fn with_number(self, name: &'static str, value: f64) -> Self {
        let formatted = FixedDecimal(value).to_string();
        self.with_value(name, formatted)
    }

    /// Set link attribute `name` to `url`, which passed the URL policy.
    pub(crate) fn with_url(self, name: &'static str, url: &SafeUrl) -> Self {
        let owned_url = url.as_str().to_owned();
        self.with_value(name, owned_url)
    }

    /// Set attribute `name` to the CSS `value`.
    pub(crate) fn with_css_value(self, name: &'static str, value: &CssValue) -> Self {
        let owned_value = value.as_str().to_owned();
        self.with_value(name, owned_value)
    }

    /// Transform the element by `affine`, written only when it moves anything.
    pub(crate) fn with_transform(self, affine: Affine) -> Self {
        let is_identity = affine == Affine::IDENTITY;
        if is_identity {
            return self;
        }
        let matrix = Matrix(affine).to_string();
        self.with_value("transform", matrix)
    }

    /// Dash the element's stroke by `dash_array`, written only when the stroke is dashed.
    pub(crate) fn with_dash_array(self, dash_array: Option<impl fmt::Display>) -> Self {
        let Some(dash_array) = dash_array else {
            return self;
        };
        let dash_array_value = dash_array.to_string();
        self.with_value("stroke-dasharray", dash_array_value)
    }

    /// Set attribute `name` to the already formatted `value`, each name set once.
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
        let is_finite = value.is_finite();
        let absolute_value = value.abs();
        let is_within_formatted_magnitude = absolute_value < MAX_FORMATTED_MAGNITUDE;
        let is_formattable = is_finite && is_within_formatted_magnitude;
        debug_assert!(is_formattable, "unformattable number {value}");
        let scale = FRACTION_SCALE as f64;
        let scaled = (value * scale).round();
        let thousandths = if is_formattable { scaled as i64 } else { 0 };
        let sign = if thousandths < 0 { "-" } else { "" };
        let magnitude = thousandths.unsigned_abs();
        let integer_part = magnitude / FRACTION_SCALE;
        let fraction_part = magnitude % FRACTION_SCALE;
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
        let view_width = view.width();
        let view_height = view.height();
        let width = FixedDecimal(view_width);
        let height = FixedDecimal(view_height);
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
        let fill_property = CssIdentifier::from_static("fill");
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
