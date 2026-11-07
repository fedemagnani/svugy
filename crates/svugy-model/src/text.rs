//! Write a model as a diagram in the svugy language, the Markdown text form any imported model can be saved as.

use std::collections::BTreeMap;
use std::fmt;

use strum::{EnumString, IntoStaticStr, VariantNames};
use thiserror::Error;

use crate::link::Link;
use crate::model::{Edge, EdgeId, Element, Model, PlacementHint};
use crate::path::ElementPath;
use crate::style::{ColorKey, NumberKey, NumberValue, StyleValue};

/// Name the style key an element's `color` lowers to.
const FILL: &str = ColorKey::FILL.name();
/// Name the style key an element's `stroke` and an edge's `color` lower to.
const STROKE: &str = ColorKey::STROKE.name();
/// Name the style key `line=dashed` and `line=dotted` lower to.
const STROKE_DASH: &str = NumberKey::STROKE_DASH.name();
/// List the element style keys a language property says: `color`, `stroke` and `line`.
const SAYABLE_ELEMENT_STYLE_KEYS: [&str; 3] = [FILL, STROKE, STROKE_DASH];
/// List the edge style keys a language property says: `color` and `line`.
const SAYABLE_EDGE_STYLE_KEYS: [&str; 2] = [STROKE, STROKE_DASH];
/// Dash length `line=dashed` lowers to.
const DASHED_LENGTH: f64 = 5.0;
/// Dash length `line=dotted` lowers to.
const DOTTED_LENGTH: f64 = 2.0;
/// List the characters besides whitespace that end a bare value, so a value holding one is quoted.
const VALUE_DELIMITERS: [char; 5] = [',', '(', ')', '"', '\''];
/// List the characters that end a one-line list item.
const LINE_BREAKS: [char; 2] = ['\n', '\r'];
/// List the first characters that stop a YAML plain scalar from reading as text.
const YAML_INDICATORS: [char; 19] = [
    '-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
];
/// List the lowercase plain YAML scalars that read as a boolean, a null or a special number instead of text.
const YAML_RESERVED_WORDS: [&str; 12] = [
    "true", "false", "yes", "no", "on", "off", "y", "n", "null", "~", ".inf", ".nan",
];

impl Model {
    /// Write the model as a svugy-language diagram in canonical form, or report every part of it the language cannot say.
    pub fn to_svugy_markdown(&self) -> Result<String, UnprintableModel> {
        let mut printer = SvugyPrinter::new(self);
        printer.write_frontmatter();
        printer.write_components();
        printer.write_edges();
        printer.write_legend();
        printer.finish()
    }
}

/// Report every part of a model the svugy language cannot say, so one round fixes them all.
#[derive(Clone, Debug, PartialEq)]
pub struct UnprintableModel {
    pub unprintables: Vec<Unprintable>,
}

impl fmt::Display for UnprintableModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unprintable_count = self.unprintables.len();
        write!(
            formatter,
            "the svugy language cannot say {unprintable_count} part(s) of this model:"
        )?;
        for unprintable in &self.unprintables {
            write!(formatter, "\n- {unprintable}")?;
        }
        Ok(())
    }
}

impl std::error::Error for UnprintableModel {}

/// Name one part of a model the svugy language cannot say, with the element, edge or legend it belongs to.
#[derive(Clone, Debug, PartialEq, Error)]
#[error("{owner}: {reason}")]
pub struct Unprintable {
    pub owner: UnprintableOwner,
    pub reason: UnprintableReason,
}

/// Name the element, edge or legend an unprintable part belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum UnprintableOwner {
    Element(ElementPath),
    Edge(EdgeId),
    Legend,
}

impl fmt::Display for UnprintableOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(path) => write!(formatter, "element `{path}`"),
            Self::Edge(id) => write!(formatter, "edge `{id}`"),
            Self::Legend => formatter.write_str("the legend"),
        }
    }
}

/// Explain why the svugy language cannot say one part of a model.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum UnprintableReason {
    #[error(
        "path segment `{0}` is not an ID: ASCII letters, digits, `_` and single inner `-`, starting with a letter or `_`"
    )]
    InvalidId(String),
    #[error("metadata name `{0}` is not an ID, so no `meta.NAME` key spells it")]
    InvalidMetadataName(String),
    #[error("its {0} holds a line break, which no one-line item can")]
    LineBreak(&'static str),
    #[error("stroke-dash `{0}` is neither 5 (`line=dashed`) nor 2 (`line=dotted`)")]
    UnsayableStrokeDash(String),
    #[error("style `{0}` has no property in the language")]
    UnmappedStyleKey(String),
    #[error("a badge edge already says `line=badge`, so it cannot also have a stroke-dash")]
    BadgeWithStrokeDash,
    #[error("`{0}` names no element, so no reference points at it")]
    UnknownReference(ElementPath),
    #[error(
        "every dotted suffix of `{0}` also ends another element's path, so no reference names it alone"
    )]
    AmbiguousReference(ElementPath),
    #[error("entry `{0}` starts with `(`, so it would read as a key row")]
    KeyRowShapedEntry(String),
}

/// Write a model's sections as svugy-language Markdown, collecting every part the language cannot say.
struct SvugyPrinter<'a> {
    model: &'a Model,
    markdown: String,
    unprintables: Vec<Unprintable>,
}

impl<'a> SvugyPrinter<'a> {
    /// Create a new printer of `model` that has written nothing yet.
    fn new(model: &'a Model) -> Self {
        Self {
            model,
            markdown: String::new(),
            unprintables: Vec::new(),
        }
    }

    /// Write the frontmatter holding the title and subtitle, only when the model has either.
    fn write_frontmatter(&mut self) {
        let title = self.model.title();
        let subtitle = self.model.subtitle();
        let is_untitled = title.is_none();
        let is_unsubtitled = subtitle.is_none();
        let has_no_heading = is_untitled && is_unsubtitled;
        if has_no_heading {
            return;
        }
        self.markdown.push_str("---\n");
        let frontmatter_entries = [("title", title), ("subtitle", subtitle)];
        for (key, value) in frontmatter_entries {
            let Some(value) = value else {
                continue;
            };
            let yaml_value = Self::spell_yaml_scalar(value);
            let entry = format!("{key}: {yaml_value}\n");
            self.markdown.push_str(&entry);
        }
        self.markdown.push_str("---\n\n");
    }

    /// Write one component item per element, in path order, nested two spaces per level.
    fn write_components(&mut self) {
        self.markdown.push_str("## Components\n");
        let model = self.model;
        for (path, element) in model.elements() {
            let mut reasons = Vec::new();
            let item = self.component_item(path, element, &mut reasons);
            self.markdown.push_str(&item);
            self.report(reasons, || UnprintableOwner::Element(path.clone()));
        }
    }

    /// Spell `- ID[: "DISPLAY NAME"] [(PROPERTIES)]` at the element's depth, the display name only when the label differs from the ID.
    fn component_item(
        &self,
        path: &ElementPath,
        element: &Element,
        reasons: &mut Vec<UnprintableReason>,
    ) -> String {
        let id = path.name().unwrap_or_default();
        let is_invalid_id = !Self::is_id(id);
        if is_invalid_id {
            let invalid_id = UnprintableReason::InvalidId(id.to_owned());
            reasons.push(invalid_id);
        }
        let depth = path.depth();
        let nesting_depth = depth.saturating_sub(1);
        let indent = "  ".repeat(nesting_depth);
        let label = element.label();
        let is_label_diff_from_id = label != id;
        let display_name = if is_label_diff_from_id {
            let quoted_label = Self::spell_quoted(label, "display name", reasons);
            format!(": {quoted_label}")
        } else {
            String::new()
        };
        let properties = self.component_properties(element, reasons);
        let properties_list = Self::spell_properties_list(&properties);
        format!("{indent}- {id}{display_name}{properties_list}\n")
    }

    /// Spell the element's properties in the language's table order, from `caption` to `meta.NAME`.
    fn component_properties(
        &self,
        element: &Element,
        reasons: &mut Vec<UnprintableReason>,
    ) -> Vec<String> {
        let mut properties = Vec::new();
        let style = element.style();
        if let Some(caption) = element.caption() {
            let quoted_caption = Self::spell_quoted(caption, "caption", reasons);
            let property = format!("caption={quoted_caption}");
            properties.push(property);
        }
        if let Some(fill) = style.get(FILL) {
            let color = Self::spell_value(fill, "color", reasons);
            let property = format!("color={color}");
            properties.push(property);
        }
        if let Some(stroke) = style.get(STROKE) {
            let stroke_color = Self::spell_value(stroke, "stroke", reasons);
            let property = format!("stroke={stroke_color}");
            properties.push(property);
        }
        if let Some(stroke_dash) = style.get(STROKE_DASH) {
            let line = Self::spell_line(stroke_dash, reasons);
            let property = format!("line={line}");
            properties.push(property);
        }
        if let Some(shape) = element.shape() {
            let shape_keyword: &str = shape.into();
            let property = format!("shape={shape_keyword}");
            properties.push(property);
        }
        if let Some(height) = element.height() {
            let property = format!("height={height}");
            properties.push(property);
        }
        if let Some(position) = element.position() {
            let x = position.x;
            let y = position.y;
            let property = format!("position={x},{y}");
            properties.push(property);
        }
        if let Some(child_placement) = element.child_placement() {
            let layout_keyword: &str = child_placement.into();
            let property = format!("layout={layout_keyword}");
            properties.push(property);
        }
        if let Some(PlacementHint::Grid { columns }) = element.child_placement() {
            let property = format!("columns={columns}");
            properties.push(property);
        }
        if let Some(raw_link) = element.link() {
            let link = Link::resolve(raw_link, self.model);
            let link_target = match link {
                Link::Element(target) => self.spell_reference(&target, reasons),
                Link::Url(url) => Self::spell_value(url, "link", reasons),
            };
            let property = format!("link={link_target}");
            properties.push(property);
        }
        for (name, value) in element.metadata() {
            let is_invalid_name = !Self::is_id(name);
            if is_invalid_name {
                let invalid_name = UnprintableReason::InvalidMetadataName(name.clone());
                reasons.push(invalid_name);
            }
            let metadata_value = Self::spell_value(value, "metadata value", reasons);
            let property = format!("meta.{name}={metadata_value}");
            properties.push(property);
        }
        Self::report_unmapped_style_keys(style, &SAYABLE_ELEMENT_STYLE_KEYS, reasons);
        properties
    }

    /// Write one edge item per edge, in edge order, under `## Edges` when the model has any.
    fn write_edges(&mut self) {
        let model = self.model;
        let edges = model.edges();
        let is_edgeless = edges.is_empty();
        if is_edgeless {
            return;
        }
        self.markdown.push_str("\n## Edges\n");
        for (id, edge) in edges {
            let mut reasons = Vec::new();
            let item = self.edge_item(id, edge, &mut reasons);
            self.markdown.push_str(&item);
            self.report(reasons, || UnprintableOwner::Edge(id.clone()));
        }
    }

    /// Spell `- FROM -> TO[: "LABEL"] [(PROPERTIES)]`, each endpoint by its shortest unique reference.
    fn edge_item(&self, id: &EdgeId, edge: &Edge, reasons: &mut Vec<UnprintableReason>) -> String {
        let from_reference = self.spell_reference(id.from(), reasons);
        let to_reference = self.spell_reference(id.to(), reasons);
        let label = match edge.label() {
            Some(label) => {
                let quoted_label = Self::spell_quoted(label, "label", reasons);
                format!(": {quoted_label}")
            }
            None => String::new(),
        };
        let properties = Self::edge_properties(edge, reasons);
        let properties_list = Self::spell_properties_list(&properties);
        format!("- {from_reference} -> {to_reference}{label}{properties_list}\n")
    }

    /// Spell the edge's properties in the language's table order: `line`, `color`, `traveler`.
    fn edge_properties(edge: &Edge, reasons: &mut Vec<UnprintableReason>) -> Vec<String> {
        let mut properties = Vec::new();
        let style = edge.style();
        let is_badge = edge.is_badge();
        let stroke_dash = style.get(STROKE_DASH);
        match (is_badge, stroke_dash) {
            (true, Some(_stroke_dash)) => reasons.push(UnprintableReason::BadgeWithStrokeDash),
            (true, None) => properties.push("line=badge".to_owned()),
            (false, Some(stroke_dash)) => {
                let line = Self::spell_line(stroke_dash, reasons);
                let property = format!("line={line}");
                properties.push(property);
            }
            (false, None) => {}
        }
        if let Some(stroke) = style.get(STROKE) {
            let color = Self::spell_value(stroke, "color", reasons);
            let property = format!("color={color}");
            properties.push(property);
        }
        if let Some(traveler) = edge.traveler() {
            let traveler_keyword: &str = traveler.into();
            let property = format!("traveler={traveler_keyword}");
            properties.push(property);
        }
        Self::report_unmapped_style_keys(style, &SAYABLE_EDGE_STYLE_KEYS, reasons);
        properties
    }

    /// Write the legend section: its free-text entries, then a key row per dash and per badge description.
    fn write_legend(&mut self) {
        let model = self.model;
        let Some(legend) = model.legend() else {
            return;
        };
        let mut reasons = Vec::new();
        let heading = match legend.title() {
            Some(title) => {
                Self::report_line_break(title, "title", &mut reasons);
                format!("\n## Legend: {title}\n")
            }
            None => "\n## Legend\n".to_owned(),
        };
        self.markdown.push_str(&heading);
        for entry in legend.entries() {
            let is_key_row_shaped = entry.starts_with('(');
            if is_key_row_shaped {
                let key_row_shaped = UnprintableReason::KeyRowShapedEntry(entry.clone());
                reasons.push(key_row_shaped);
            }
            Self::report_line_break(entry, "entry", &mut reasons);
            let item = format!("- {entry}\n");
            self.markdown.push_str(&item);
        }
        for (dash_key, description) in legend.dashes() {
            let dash_spelling = dash_key.to_string();
            let line = Self::spell_line(&dash_spelling, &mut reasons);
            Self::report_line_break(description, "dash description", &mut reasons);
            let item = format!("- (line={line}): {description}\n");
            self.markdown.push_str(&item);
        }
        for (target, description) in legend.badges() {
            let target_reference = self.spell_reference(target, &mut reasons);
            Self::report_line_break(description, "badge description", &mut reasons);
            let item = format!("- (line=badge, target={target_reference}): {description}\n");
            self.markdown.push_str(&item);
        }
        self.report(reasons, || UnprintableOwner::Legend);
    }

    /// Hand out the written Markdown, or every unprintable part found while writing it.
    fn finish(self) -> Result<String, UnprintableModel> {
        let is_printable = self.unprintables.is_empty();
        if is_printable {
            return Ok(self.markdown);
        }
        let unprintables = self.unprintables;
        let unprintable_model = UnprintableModel { unprintables };
        Err(unprintable_model)
    }

    /// Record each reason as an unprintable part of `owner`, building the owner only when there is a reason.
    fn report(&mut self, reasons: Vec<UnprintableReason>, owner: impl Fn() -> UnprintableOwner) {
        let unprintables = reasons.into_iter().map(|reason| {
            let owner = owner();
            Unprintable { owner, reason }
        });
        self.unprintables.extend(unprintables);
    }

    /// Report every style key of `style` outside `sayable_keys`, the keys no language property lowers to.
    fn report_unmapped_style_keys(
        style: &BTreeMap<String, String>,
        sayable_keys: &[&str],
        reasons: &mut Vec<UnprintableReason>,
    ) {
        let style_keys = style.keys();
        let unmapped_keys = style_keys.filter(|key| {
            let is_sayable = sayable_keys.contains(&key.as_str());
            !is_sayable
        });
        let unmapped_reasons = unmapped_keys
            .cloned()
            .map(UnprintableReason::UnmappedStyleKey);
        reasons.extend(unmapped_reasons);
    }

    /// Spell the shortest dotted suffix of `path` that ends no other element's path, the language's reference to it.
    fn spell_reference(&self, path: &ElementPath, reasons: &mut Vec<UnprintableReason>) -> String {
        let element = self.model.element(path);
        let is_unknown_element = element.is_none();
        if is_unknown_element {
            let unknown_reference = UnprintableReason::UnknownReference(path.clone());
            reasons.push(unknown_reference);
            return path.to_string();
        }
        let segments = path.segments();
        let depth = segments.len();
        let suffix_lengths = 1..=depth;
        let suffixes = suffix_lengths.map(|suffix_length| &segments[depth - suffix_length..]);
        let mut unique_suffixes = suffixes.filter(|suffix| {
            let matching_count = self.count_elements_ending_with(suffix);
            matching_count == 1
        });
        let Some(unique_suffix) = unique_suffixes.next() else {
            let ambiguous_reference = UnprintableReason::AmbiguousReference(path.clone());
            reasons.push(ambiguous_reference);
            return path.to_string();
        };
        unique_suffix.join(".")
    }

    /// Count the elements whose path ends with `suffix`: the elements a reference spelled `suffix` matches.
    fn count_elements_ending_with(&self, suffix: &[String]) -> usize {
        let elements = self.model.elements();
        let element_paths = elements.keys();
        let matching_paths = element_paths.filter(|path| {
            let segments = path.segments();
            segments.ends_with(suffix)
        });
        matching_paths.count()
    }

    /// Spell a stroke-dash as the `line` keyword drawing it, reporting a length no keyword draws.
    fn spell_line(stroke_dash: &str, reasons: &mut Vec<UnprintableReason>) -> &'static str {
        let dash_length = NumberValue::parse(stroke_dash);
        let line_style = dash_length.and_then(LineStyle::drawing_dash);
        let Some(line_style) = line_style else {
            let unsayable_dash = UnprintableReason::UnsayableStrokeDash(stroke_dash.to_owned());
            reasons.push(unsayable_dash);
            return LineStyle::Solid.into();
        };
        line_style.into()
    }

    /// Spell `text` as a bare token when it holds no whitespace, comma, parenthesis or quote, otherwise quoted.
    fn spell_value(
        text: &str,
        field: &'static str,
        reasons: &mut Vec<UnprintableReason>,
    ) -> String {
        let is_empty = text.is_empty();
        let mut characters = text.chars();
        let has_delimiter = characters.any(|character| {
            let is_whitespace = character.is_whitespace();
            let is_value_delimiter = VALUE_DELIMITERS.contains(&character);
            is_whitespace || is_value_delimiter
        });
        let needs_quotes = is_empty || has_delimiter;
        if needs_quotes {
            return Self::spell_quoted(text, field, reasons);
        }
        text.to_owned()
    }

    /// Spell `text` as a quoted string, escaping `"` and `\`, its only escapes.
    fn spell_quoted(
        text: &str,
        field: &'static str,
        reasons: &mut Vec<UnprintableReason>,
    ) -> String {
        Self::report_line_break(text, field, reasons);
        let backslash_escaped = text.replace('\\', "\\\\");
        let escaped = backslash_escaped.replace('"', "\\\"");
        format!("\"{escaped}\"")
    }

    /// Report `field` when `text` holds a line break, which would end its one-line item.
    fn report_line_break(text: &str, field: &'static str, reasons: &mut Vec<UnprintableReason>) {
        let has_line_break = text.contains(LINE_BREAKS);
        if has_line_break {
            let line_break = UnprintableReason::LineBreak(field);
            reasons.push(line_break);
        }
    }

    /// Spell a properties list as ` (a, b)`, or as nothing when there is no property.
    fn spell_properties_list(properties: &[String]) -> String {
        let is_empty = properties.is_empty();
        if is_empty {
            return String::new();
        }
        let entries = properties.join(", ");
        format!(" ({entries})")
    }

    /// Tell whether `text` is an ID: ASCII letters, digits, `_` and `-`, starting with a letter or `_`, each `-` followed by a non-`-`.
    fn is_id(text: &str) -> bool {
        let mut characters = text.chars();
        let Some(first_character) = characters.next() else {
            return false;
        };
        let is_valid_start = matches!(first_character, 'a'..='z' | 'A'..='Z' | '_');
        let is_valid_rest = characters
            .all(|character| matches!(character, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'));
        let has_double_hyphen = text.contains("--");
        let has_trailing_hyphen = text.ends_with('-');
        let has_dangling_hyphen = has_double_hyphen || has_trailing_hyphen;
        is_valid_start && is_valid_rest && !has_dangling_hyphen
    }

    /// Spell `text` as a YAML plain scalar when it reads back as the same text, otherwise double-quoted.
    fn spell_yaml_scalar(text: &str) -> String {
        let is_empty = text.is_empty();
        let trimmed_text = text.trim();
        let is_padded = trimmed_text != text;
        let starts_with_indicator = text.starts_with(YAML_INDICATORS);
        let has_mapping_mark = text.contains(": ");
        let has_comment_mark = text.contains(" #");
        let ends_with_colon = text.ends_with(':');
        let has_control_character = text.contains(char::is_control);
        let lowercase_text = text.to_ascii_lowercase();
        let is_reserved_word = YAML_RESERVED_WORDS.contains(&lowercase_text.as_str());
        let parsed_number = text.parse::<f64>();
        let is_number = parsed_number.is_ok();
        let needs_quotes = is_empty
            || is_padded
            || starts_with_indicator
            || has_mapping_mark
            || has_comment_mark
            || ends_with_colon
            || has_control_character
            || is_reserved_word
            || is_number;
        if needs_quotes {
            return Self::spell_yaml_double_quoted(text);
        }
        text.to_owned()
    }

    /// Spell `text` as a YAML double-quoted scalar, escaping quotes, backslashes and control characters.
    fn spell_yaml_double_quoted(text: &str) -> String {
        let mut quoted = String::from('"');
        for character in text.chars() {
            match character {
                '"' => quoted.push_str("\\\""),
                '\\' => quoted.push_str("\\\\"),
                '\n' => quoted.push_str("\\n"),
                '\r' => quoted.push_str("\\r"),
                '\t' => quoted.push_str("\\t"),
                control if control.is_control() => {
                    let code_point = u32::from(control);
                    let escape = format!("\\u{code_point:04X}");
                    quoted.push_str(&escape);
                }
                plain => quoted.push(plain),
            }
        }
        quoted.push('"');
        quoted
    }
}

/// Name how a `line` keyword draws: an outline or edge dash, or an edge drawn as a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    Badge,
}

impl LineStyle {
    /// Find the dashed line style drawing `dash_length`, `None` for a length no keyword draws.
    pub fn drawing_dash(dash_length: f64) -> Option<Self> {
        let is_dashed = dash_length == DASHED_LENGTH;
        if is_dashed {
            return Some(Self::Dashed);
        }
        let is_dotted = dash_length == DOTTED_LENGTH;
        if is_dotted {
            return Some(Self::Dotted);
        }
        None
    }

    /// Return the dash length this line style draws, `None` for a solid line or a badge.
    pub fn stroke_dash(self) -> Option<f64> {
        match self {
            Self::Dashed => Some(DASHED_LENGTH),
            Self::Dotted => Some(DOTTED_LENGTH),
            Self::Solid | Self::Badge => None,
        }
    }
}
