//! Print a model back as a Markdown diagram, the text form any imported model can be saved as.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use thiserror::Error;

use svugy_model::link::Link;
use svugy_model::model::{Edge, EdgeId, Element, Model, PlacementHint};
use svugy_model::path::ElementPath;
use svugy_model::style::{ColorKey, NumberKey, NumberValue, StyleValue};

use super::ast::Value;
use super::parse::ItemLabel;
use super::property::LineStyle;
use super::skeleton::Skeleton;

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
/// List the characters that end a one-line list item.
const LINE_BREAKS: [char; 2] = ['\n', '\r'];
/// Spell the opening of an HTML comment, which the language drops wherever it appears.
const COMMENT_OPENING: &str = "<!--";
/// List the first characters CommonMark may read, at the start of a list item, as a heading, quote, list, rule, code fence, HTML block or link definition.
const BLOCK_MARKERS: [char; 10] = ['#', '>', '-', '+', '*', '_', '`', '~', '<', '['];
/// List the characters that turn leading digits into an ordered list marker.
const ORDERED_LIST_DELIMITERS: [char; 2] = ['.', ')'];

/// Report every part of a model Markdown cannot say, so one round fixes them all.
#[derive(Clone, Debug, PartialEq, Error)]
#[error(
    "Markdown cannot say {} part(s) of this model:{}",
    .unprintables.len(),
    Self::bulleted(.unprintables)
)]
pub struct UnprintableModel {
    pub unprintables: Vec<Unprintable>,
}

impl UnprintableModel {
    /// Spell each unprintable part on a line of its own, opened by `- `.
    fn bulleted(unprintables: &[Unprintable]) -> String {
        let lines = unprintables
            .iter()
            .map(|unprintable| format!("\n- {unprintable}"));
        lines.collect()
    }
}

/// Name one part of a model Markdown cannot say, with the element, edge or legend it belongs to.
#[derive(Clone, Debug, PartialEq, Error)]
#[error("{owner}: {reason}")]
pub struct Unprintable {
    pub owner: UnprintableOwner,
    pub reason: UnprintableReason,
}

/// Name the diagram, element, edge or legend an unprintable part belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum UnprintableOwner {
    Diagram,
    Element(ElementPath),
    Edge(EdgeId),
    Legend,
}

impl fmt::Display for UnprintableOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagram => formatter.write_str("the diagram"),
            Self::Element(path) => write!(formatter, "element `{path}`"),
            Self::Edge(id) => write!(formatter, "edge `{id}`"),
            Self::Legend => formatter.write_str("the legend"),
        }
    }
}

/// Explain why Markdown cannot say one part of a model.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum UnprintableReason {
    #[error(
        "path segment `{0}` is not an ID: ASCII letters, digits, `_` and single inner `-`, starting with a letter or `_`"
    )]
    InvalidId(String),
    #[error("metadata name `{0}` is not an ID, so no `meta.NAME` key spells it")]
    InvalidMetadataName(String),
    #[error("its {field} {text:?} holds a line break, which no one-line item can")]
    LineBreak { field: TextField, text: String },
    #[error("its {field} {text:?} starts or ends with whitespace, which the language trims")]
    PaddedText { field: TextField, text: String },
    #[error("its {field} {text:?} holds `<!--`, which the language reads as a comment and drops")]
    CommentOpening { field: TextField, text: String },
    #[error(
        "its description does not read back verbatim it must hold only paragraphs, numbered lists, quotes and closed code blocks, and start and end with one of them"
    )]
    UnreadableDescription,
    #[error("its title ends with `#`, which Markdown reads as the heading's closing `#`s")]
    TitleEndingWithHash,
    #[error("it has an empty entry, which no list item can hold")]
    EmptyEntry,
    #[error(
        "entry `{0}` opens like a Markdown heading, quote, list, rule, code fence, HTML block or link definition, so it would not read back as text"
    )]
    BlockOpeningEntry(String),
    #[error("entry `{0}` starts with `(` followed by `key=`, so it would read as a key row")]
    KeyRowShapedEntry(String),
    #[error("stroke-dash `{0}` is neither 5 (`line=dashed`) nor 2 (`line=dotted`)")]
    UnsayableStrokeDash(String),
    #[error("style `{0}` has no property in the language")]
    UnmappedStyleKey(String),
    #[error("a badge edge already says `line=badge`, so it cannot also have a stroke-dash")]
    BadgeWithStrokeDash,
    #[error(
        "link `{0}` holds no `:`, `/`, `?` or `#`, so the language would read it as a component reference; write it as `./{0}`"
    )]
    UrlWithoutUrlCharacter(String),
    #[error(
        "link `{link}` spells element `{target}` another way, and the language keeps only the path; write it as `{target}`"
    )]
    RespelledElementLink { link: String, target: ElementPath },
    #[error("it joins an element to itself, which the language rejects")]
    SelfLoop,
    #[error("it joins an element to its own ancestor or descendant, which the language rejects")]
    AncestorDescendantEdge,
    #[error(
        "its ordinal is {ordinal}, but the language numbers repeated edges in the order written, so it would read back as {reading_ordinal}"
    )]
    RenumberedEdge { ordinal: u32, reading_ordinal: u32 },
    #[error("`{0}` names no element, so no reference points at it")]
    UnknownReference(ElementPath),
}

/// Name the text of an element, edge or legend an unprintable reason is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::Display)]
pub enum TextField {
    #[strum(to_string = "display name")]
    DisplayName,
    #[strum(to_string = "caption")]
    Caption,
    #[strum(to_string = "link")]
    Link,
    #[strum(to_string = "metadata value")]
    MetadataValue,
    #[strum(to_string = "label")]
    Label,
    #[strum(to_string = "title")]
    LegendTitle,
    #[strum(to_string = "entry")]
    LegendEntry,
    #[strum(to_string = "dash description")]
    DashDescription,
    #[strum(to_string = "badge description")]
    BadgeDescription,
}

/// Print a model as a Markdown diagram, collecting every part Markdown cannot say.
pub struct MarkdownPrinter<'a> {
    model: &'a Model,
    references: ReferenceIndex<'a>,
    markdown: String,
    unprintables: Vec<Unprintable>,
}

impl<'a> MarkdownPrinter<'a> {
    /// Create a new printer of `model` that has written nothing yet.
    fn new(model: &'a Model) -> Self {
        let references = ReferenceIndex::new(model);
        Self {
            model,
            references,
            markdown: String::new(),
            unprintables: Vec::new(),
        }
    }

    /// Print `model` as a Markdown diagram in canonical form, or report every part of it Markdown cannot say.
    pub fn print(model: &'a Model) -> Result<String, UnprintableModel> {
        let mut printer = Self::new(model);
        printer.write_frontmatter();
        printer.write_diagram_description();
        printer.write_components();
        printer.write_edges();
        printer.write_legend();
        printer.finish()
    }

    /// Write the frontmatter holding the title and subtitle, only when the model has either, each value double-quoted.
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
            let yaml_value = Self::spell_yaml_quoted(value);
            let entry = format!("{key}: {yaml_value}\n");
            self.markdown.push_str(&entry);
        }
        self.markdown.push_str("---\n\n");
    }

    /// Write the diagram's description before `## Components`, or report one that would not read back verbatim.
    fn write_diagram_description(&mut self) {
        let Some(description) = self.model.description() else {
            return;
        };
        let description_spelling = Self::spell_description(description, "");
        let mut reasons = Vec::new();
        let spelled_description = Self::spelling_or_push_reason(description_spelling, &mut reasons);
        self.markdown.push_str(&spelled_description);
        self.markdown.push('\n');
        self.report(reasons, || UnprintableOwner::Diagram);
    }

    /// Write one component item per element, in path order, nested two spaces per level, a blank line after each description.
    fn write_components(&mut self) {
        self.markdown.push_str("## Components\n");
        let model = self.model;
        let mut is_after_description = false;
        for (path, element) in model.elements() {
            if is_after_description {
                self.markdown.push('\n');
            }
            is_after_description = element.description().is_some();
            let mut reasons = Vec::new();
            let item = self.component_item(path, element, &mut reasons);
            self.markdown.push_str(&item);
            self.report(reasons, || {
                let element_path = path.clone();
                UnprintableOwner::Element(element_path)
            });
        }
    }

    /// Spell `- ID[: DISPLAY NAME] [(PROPERTIES)]` at the element's depth, the display name only when the label differs from the ID, then its description, pushing onto `reasons` what the language cannot say.
    fn component_item(
        &self,
        path: &ElementPath,
        element: &Element,
        reasons: &mut Vec<UnprintableReason>,
    ) -> String {
        let id = path.name().unwrap_or_default();
        let is_invalid_id = !Self::is_id(id);
        if is_invalid_id {
            let invalid_segment = id.to_owned();
            let invalid_id = UnprintableReason::InvalidId(invalid_segment);
            reasons.push(invalid_id);
        }
        let depth = path.depth();
        let nesting_depth = depth.saturating_sub(1);
        let indent = "  ".repeat(nesting_depth);
        let label = element.label();
        let is_label_diff_from_id = label != id;
        let display_name = if is_label_diff_from_id {
            let label_spelling = Self::spell_label(label, TextField::DisplayName);
            let display_name = Self::spelling_or_push_reason(label_spelling, reasons);
            format!(": {display_name}")
        } else {
            String::new()
        };
        let properties = self.component_properties(element, reasons);
        let properties_list = Self::spell_properties_list(&properties);
        let item_line = format!("{indent}- {id}{display_name}{properties_list}\n");
        let Some(description) = element.description() else {
            return item_line;
        };
        let description_indent = format!("{indent}  ");
        let description_spelling = Self::spell_description(description, &description_indent);
        let spelled_description = Self::spelling_or_push_reason(description_spelling, reasons);
        format!("{item_line}\n{spelled_description}")
    }

    /// Spell the element's properties in the language's table order, from `caption` to `meta.NAME`, pushing onto `reasons` what the language cannot say.
    fn component_properties(
        &self,
        element: &Element,
        reasons: &mut Vec<UnprintableReason>,
    ) -> Vec<String> {
        let mut properties = Vec::new();
        let style = element.style();
        if let Some(caption) = element.caption() {
            let caption_spelling = Self::spell_value(caption, TextField::Caption);
            let caption_value = Self::spelling_or_push_reason(caption_spelling, reasons);
            let property = format!("caption={caption_value}");
            properties.push(property);
        }
        if let Some(fill) = style.get(FILL) {
            let property = format!("color={fill}");
            properties.push(property);
        }
        if let Some(stroke) = style.get(STROKE) {
            let property = format!("stroke={stroke}");
            properties.push(property);
        }
        if let Some(stroke_dash) = style.get(STROKE_DASH) {
            let line_spelling = Self::spell_line(stroke_dash);
            let line = Self::spelling_or_push_reason(line_spelling, reasons);
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
            let link_spelling = self.spell_link(raw_link);
            let link_target = Self::spelling_or_push_reason(link_spelling, reasons);
            let property = format!("link={link_target}");
            properties.push(property);
        }
        for (name, value) in element.metadata() {
            let is_invalid_name = !Self::is_id(name);
            if is_invalid_name {
                let invalid_metadata_name = name.clone();
                let invalid_name = UnprintableReason::InvalidMetadataName(invalid_metadata_name);
                reasons.push(invalid_name);
            }
            let metadata_spelling = Self::spell_value(value, TextField::MetadataValue);
            let metadata_value = Self::spelling_or_push_reason(metadata_spelling, reasons);
            let property = format!("meta.{name}={metadata_value}");
            properties.push(property);
        }
        let unmapped_style_keys =
            Self::find_unmapped_style_keys(style, &SAYABLE_ELEMENT_STYLE_KEYS);
        reasons.extend(unmapped_style_keys);
        properties
    }

    /// Spell a `link` value the way the language reads it back: an element by its shortest reference, a URL only when it holds a URL character.
    fn spell_link(&self, raw_link: &str) -> Result<String, UnprintableReason> {
        let link = Link::resolve(raw_link, self.model);
        match link {
            Link::Element(target) => {
                let canonical_link = target.to_string();
                let is_canonical = canonical_link == raw_link;
                if is_canonical {
                    return self.references.spell(&target);
                }
                let link = raw_link.to_owned();
                let respelled_link = UnprintableReason::RespelledElementLink { link, target };
                Err(respelled_link)
            }
            Link::Url(url) => {
                let is_url_shaped = Link::is_url_shaped(url);
                if is_url_shaped {
                    return Self::spell_value(url, TextField::Link);
                }
                let unshaped_url = url.to_owned();
                let url_without_url_character =
                    UnprintableReason::UrlWithoutUrlCharacter(unshaped_url);
                Err(url_without_url_character)
            }
        }
    }

    /// Write one edge item per edge, in edge order, under `## Edges` when the model has any, a blank line after each description.
    fn write_edges(&mut self) {
        let model = self.model;
        let edges = model.edges();
        let is_edgeless = edges.is_empty();
        if is_edgeless {
            return;
        }
        self.markdown.push_str("\n## Edges\n");
        let mut previous_pair = None;
        let mut reading_ordinal = 0;
        let mut is_after_description = false;
        for (id, edge) in edges {
            if is_after_description {
                self.markdown.push('\n');
            }
            is_after_description = edge.description().is_some();
            let from = id.from();
            let to = id.to();
            let pair = (from, to);
            let is_repeated_pair = previous_pair == Some(pair);
            reading_ordinal = if is_repeated_pair {
                reading_ordinal + 1
            } else {
                0
            };
            previous_pair = Some(pair);
            let mut reasons = Vec::new();
            let item = self.edge_item(id, edge, reading_ordinal, &mut reasons);
            self.markdown.push_str(&item);
            self.report(reasons, || {
                let edge_id = id.clone();
                UnprintableOwner::Edge(edge_id)
            });
        }
    }

    /// Spell `- FROM -> TO[: LABEL] [(PROPERTIES)]`, each endpoint by its shortest unique reference, then its description, pushing onto `reasons` what the language cannot say, an edge it would number other than `reading_ordinal` included.
    fn edge_item(
        &self,
        id: &EdgeId,
        edge: &Edge,
        reading_ordinal: u32,
        reasons: &mut Vec<UnprintableReason>,
    ) -> String {
        let from = id.from();
        let to = id.to();
        let is_self_loop = from == to;
        if is_self_loop {
            reasons.push(UnprintableReason::SelfLoop);
        }
        let joins_ancestor_and_descendant = id.joins_ancestor_and_descendant();
        if joins_ancestor_and_descendant {
            reasons.push(UnprintableReason::AncestorDescendantEdge);
        }
        let ordinal = id.ordinal();
        let is_renumbered = ordinal != reading_ordinal;
        if is_renumbered {
            let renumbered_edge = UnprintableReason::RenumberedEdge {
                ordinal,
                reading_ordinal,
            };
            reasons.push(renumbered_edge);
        }
        let from_spelling = self.references.spell(from);
        let from_reference = Self::spelling_or_push_reason(from_spelling, reasons);
        let to_spelling = self.references.spell(to);
        let to_reference = Self::spelling_or_push_reason(to_spelling, reasons);
        let label = match edge.label() {
            Some(label) => {
                let label_spelling = Self::spell_label(label, TextField::Label);
                let label = Self::spelling_or_push_reason(label_spelling, reasons);
                format!(": {label}")
            }
            None => String::new(),
        };
        let properties = Self::edge_properties(edge, reasons);
        let properties_list = Self::spell_properties_list(&properties);
        let item_line = format!("- {from_reference} -> {to_reference}{label}{properties_list}\n");
        let Some(description) = edge.description() else {
            return item_line;
        };
        let description_spelling = Self::spell_description(description, "  ");
        let spelled_description = Self::spelling_or_push_reason(description_spelling, reasons);
        format!("{item_line}\n{spelled_description}")
    }

    /// Spell the edge's properties in the language's table order, `line`, `color`, `traveler`, pushing onto `reasons` what the language cannot say.
    fn edge_properties(edge: &Edge, reasons: &mut Vec<UnprintableReason>) -> Vec<String> {
        let mut properties = Vec::new();
        let style = edge.style();
        let is_badge = edge.is_badge();
        let stroke_dash = style.get(STROKE_DASH);
        match (is_badge, stroke_dash) {
            (true, Some(_stroke_dash)) => reasons.push(UnprintableReason::BadgeWithStrokeDash),
            (true, None) => {
                let badge: &str = LineStyle::Badge.into();
                let property = format!("line={badge}");
                properties.push(property);
            }
            (false, Some(stroke_dash)) => {
                let line_spelling = Self::spell_line(stroke_dash);
                let line = Self::spelling_or_push_reason(line_spelling, reasons);
                let property = format!("line={line}");
                properties.push(property);
            }
            (false, None) => {}
        }
        if let Some(stroke) = style.get(STROKE) {
            let property = format!("color={stroke}");
            properties.push(property);
        }
        if let Some(traveler) = edge.traveler() {
            let traveler_keyword: &str = traveler.into();
            let property = format!("traveler={traveler_keyword}");
            properties.push(property);
        }
        let unmapped_style_keys = Self::find_unmapped_style_keys(style, &SAYABLE_EDGE_STYLE_KEYS);
        reasons.extend(unmapped_style_keys);
        properties
    }

    /// Write the legend section: its free-text entries, then a key row per dash and per badge description, each written verbatim or reported.
    fn write_legend(&mut self) {
        let model = self.model;
        let Some(legend) = model.legend() else {
            return;
        };
        let mut reasons = Vec::new();
        let heading = match legend.title() {
            Some(title) => {
                let title_problem = Self::check_legend_title(title);
                reasons.extend(title_problem);
                let spaced_title = Self::spaced(title);
                format!("\n## Legend:{spaced_title}\n")
            }
            None => "\n## Legend\n".to_owned(),
        };
        self.markdown.push_str(&heading);
        for entry in legend.entries() {
            let entry_problem = Self::check_legend_entry(entry);
            reasons.extend(entry_problem);
            let item = format!("- {entry}\n");
            self.markdown.push_str(&item);
        }
        for (dash_key, description) in legend.dashes() {
            let dash_spelling = dash_key.to_string();
            let line_spelling = Self::spell_line(&dash_spelling);
            let line = Self::spelling_or_push_reason(line_spelling, &mut reasons);
            let description_problem =
                Self::check_verbatim_text(description, TextField::DashDescription);
            reasons.extend(description_problem);
            let spaced_description = Self::spaced(description);
            let item = format!("- (line={line}):{spaced_description}\n");
            self.markdown.push_str(&item);
        }
        for (target, description) in legend.badges() {
            let target_spelling = self.references.spell(target);
            let target_reference = Self::spelling_or_push_reason(target_spelling, &mut reasons);
            let description_problem =
                Self::check_verbatim_text(description, TextField::BadgeDescription);
            reasons.extend(description_problem);
            let spaced_description = Self::spaced(description);
            let item = format!("- (line=badge, target={target_reference}):{spaced_description}\n");
            self.markdown.push_str(&item);
        }
        self.report(reasons, || UnprintableOwner::Legend);
    }

    /// Check that a legend title reads back from its heading as itself: verbatim text not ending in a closing `#`.
    fn check_legend_title(title: &str) -> Option<UnprintableReason> {
        let verbatim_problem = Self::check_verbatim_text(title, TextField::LegendTitle);
        let has_verbatim_problem = verbatim_problem.is_some();
        if has_verbatim_problem {
            return verbatim_problem;
        }
        let is_ending_with_hash = title.ends_with('#');
        if is_ending_with_hash {
            return Some(UnprintableReason::TitleEndingWithHash);
        }
        None
    }

    /// Check that a free-text legend entry reads back as itself: non-empty verbatim text that opens no Markdown block and no key row.
    fn check_legend_entry(entry: &str) -> Option<UnprintableReason> {
        let is_empty = entry.is_empty();
        if is_empty {
            return Some(UnprintableReason::EmptyEntry);
        }
        let verbatim_problem = Self::check_verbatim_text(entry, TextField::LegendEntry);
        let has_verbatim_problem = verbatim_problem.is_some();
        if has_verbatim_problem {
            return verbatim_problem;
        }
        let opens_markdown_block = Self::opens_markdown_block(entry);
        if opens_markdown_block {
            let block_opening_text = entry.to_owned();
            let block_opening_entry = UnprintableReason::BlockOpeningEntry(block_opening_text);
            return Some(block_opening_entry);
        }
        let is_key_row_shaped = Self::is_key_row_shaped(entry);
        if is_key_row_shaped {
            let key_row_shaped_text = entry.to_owned();
            let key_row_shaped_entry = UnprintableReason::KeyRowShapedEntry(key_row_shaped_text);
            return Some(key_row_shaped_entry);
        }
        None
    }

    /// Check that `text` reads back verbatim from the rest of a line, which the parser trims: one-line text with no surrounding whitespace.
    fn check_verbatim_text(text: &str, field: TextField) -> Option<UnprintableReason> {
        let one_line_problem = Self::check_one_line_text(text, field);
        let has_one_line_problem = one_line_problem.is_some();
        if has_one_line_problem {
            return one_line_problem;
        }
        let trimmed_text = text.trim();
        let is_padded = trimmed_text != text;
        if is_padded {
            let text = text.to_owned();
            let padded_text = UnprintableReason::PaddedText { field, text };
            return Some(padded_text);
        }
        None
    }

    /// Check that `text` survives inside one item line: no line break ends it and no comment opening drops it.
    fn check_one_line_text(text: &str, field: TextField) -> Option<UnprintableReason> {
        let has_line_break = text.contains(LINE_BREAKS);
        if has_line_break {
            let text = text.to_owned();
            let line_break = UnprintableReason::LineBreak { field, text };
            return Some(line_break);
        }
        let has_comment_opening = text.contains(COMMENT_OPENING);
        if has_comment_opening {
            let text = text.to_owned();
            let comment_opening = UnprintableReason::CommentOpening { field, text };
            return Some(comment_opening);
        }
        None
    }

    /// Tell whether CommonMark may read `text`, at the start of a list item, as a block other than a paragraph.
    fn opens_markdown_block(text: &str) -> bool {
        let starts_with_block_marker = text.starts_with(BLOCK_MARKERS);
        let after_leading_digits =
            text.trim_start_matches(|character: char| character.is_ascii_digit());
        let has_leading_digits = after_leading_digits.len() < text.len();
        let has_list_delimiter_after_digits =
            after_leading_digits.starts_with(ORDERED_LIST_DELIMITERS);
        let opens_ordered_list = has_leading_digits && has_list_delimiter_after_digits;
        starts_with_block_marker || opens_ordered_list
    }

    /// Tell whether `text` opens like a key row, `(` followed by `key=`, spaces allowed, which the parser reads as one.
    fn is_key_row_shaped(text: &str) -> bool {
        let Some(after_parenthesis) = text.strip_prefix('(') else {
            return false;
        };
        let key_onwards = after_parenthesis.trim_start();
        let after_key = key_onwards.trim_start_matches(|character: char| {
            let is_alphanumeric = character.is_ascii_alphanumeric();
            let is_key_punctuation = matches!(character, '_' | '-' | '.');
            is_alphanumeric || is_key_punctuation
        });
        let has_key = after_key.len() < key_onwards.len();
        let equals_onwards = after_key.trim_start();
        let has_equals_after_key = equals_onwards.starts_with('=');
        has_key && has_equals_after_key
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

    /// Return the spelling, or push the reason there is none onto `reasons` and return an empty spelling, which an unprintable model never shows.
    fn spelling_or_push_reason<T: Default>(
        spelling: Result<T, UnprintableReason>,
        reasons: &mut Vec<UnprintableReason>,
    ) -> T {
        spelling.unwrap_or_else(|reason| {
            reasons.push(reason);
            T::default()
        })
    }

    /// Find every style key of `style` outside `sayable_keys`, the keys no language property lowers to.
    fn find_unmapped_style_keys(
        style: &BTreeMap<String, String>,
        sayable_keys: &[&str],
    ) -> impl Iterator<Item = UnprintableReason> {
        let style_keys = style.keys();
        let unmapped_keys = style_keys.filter(|key| {
            let is_sayable = sayable_keys.contains(&key.as_str());
            !is_sayable
        });
        unmapped_keys
            .cloned()
            .map(UnprintableReason::UnmappedStyleKey)
    }

    /// Spell a stroke-dash as the `line` keyword drawing it, or report a length no keyword draws.
    fn spell_line(stroke_dash: &str) -> Result<&'static str, UnprintableReason> {
        let dash_length = NumberValue::parse(stroke_dash);
        let line_style = dash_length.and_then(LineStyle::drawing_dash);
        let Some(line_style) = line_style else {
            let unsayable_dash = stroke_dash.to_owned();
            let unsayable_stroke_dash = UnprintableReason::UnsayableStrokeDash(unsayable_dash);
            return Err(unsayable_stroke_dash);
        };
        let line_keyword: &'static str = line_style.into();
        Ok(line_keyword)
    }

    /// Spell `text` as a value: bare when it reads back as itself, otherwise quoted.
    fn spell_value(text: &str, field: TextField) -> Result<String, UnprintableReason> {
        let reads_back_bare = Value::reads_back_bare(text);
        Self::spell_bare_or_quoted(text, reads_back_bare, field)
    }

    /// Spell `text` as a label or display name: bare when it reads back as itself, otherwise quoted.
    fn spell_label(text: &str, field: TextField) -> Result<String, UnprintableReason> {
        let reads_back_bare = ItemLabel::reads_back_bare(text);
        Self::spell_bare_or_quoted(text, reads_back_bare, field)
    }

    /// Spell `text` bare when `reads_back_bare`, otherwise quoted, or report text no item line keeps.
    fn spell_bare_or_quoted(
        text: &str,
        reads_back_bare: bool,
        field: TextField,
    ) -> Result<String, UnprintableReason> {
        let one_line_problem = Self::check_one_line_text(text, field);
        if let Some(one_line_problem) = one_line_problem {
            return Err(one_line_problem);
        }
        if reads_back_bare {
            let bare_text = text.to_owned();
            return Ok(bare_text);
        }
        Self::spell_quoted(text, field)
    }

    /// Spell `text` as a quoted string, escaping `"` and `\`, its only escapes, or report text no item line keeps.
    fn spell_quoted(text: &str, field: TextField) -> Result<String, UnprintableReason> {
        let one_line_problem = Self::check_one_line_text(text, field);
        if let Some(one_line_problem) = one_line_problem {
            return Err(one_line_problem);
        }
        let backslash_escaped = text.replace('\\', "\\\\");
        let escaped = backslash_escaped.replace('"', "\\\"");
        let quoted = format!("\"{escaped}\"");
        Ok(quoted)
    }

    /// Spell each line of `description` after `indent`, or report a description that would not read back verbatim under an item.
    fn spell_description(description: &str, indent: &str) -> Result<String, UnprintableReason> {
        let item_spelling = Self::indented_lines(description, "  ");
        let reads_back = Skeleton::reads_back_as_description(description, &item_spelling);
        if !reads_back {
            return Err(UnprintableReason::UnreadableDescription);
        }
        let spelled_description = Self::indented_lines(description, indent);
        Ok(spelled_description)
    }

    /// Spell each line of `text` after `indent`, an empty line left empty, each ending with a line break.
    fn indented_lines(text: &str, indent: &str) -> String {
        let lines = text.split('\n').map(|line| {
            let is_empty_line = line.is_empty();
            if is_empty_line {
                return "\n".to_owned();
            }
            format!("{indent}{line}\n")
        });
        lines.collect()
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

    /// Spell `text` after the space that separates it from what precedes, or as nothing when it is empty.
    fn spaced(text: &str) -> String {
        let is_empty = text.is_empty();
        if is_empty {
            return String::new();
        }
        format!(" {text}")
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

    /// Spell `text` as a YAML double-quoted scalar, escaping quotes, backslashes and every character YAML forbids unescaped.
    fn spell_yaml_quoted(text: &str) -> String {
        let mut quoted = String::from('"');
        for character in text.chars() {
            let is_unprintable_in_yaml =
                character.is_control() || matches!(character, '\u{FFFE}' | '\u{FFFF}');
            match character {
                '"' => quoted.push_str("\\\""),
                '\\' => quoted.push_str("\\\\"),
                unprintable if is_unprintable_in_yaml => {
                    let code_point = u32::from(unprintable);
                    let escape = format!("\\u{code_point:04X}");
                    quoted.push_str(&escape);
                }
                printable => quoted.push(printable),
            }
        }
        quoted.push('"');
        quoted
    }
}

/// Count the elements each dotted suffix of a path matches, to spell every element by its shortest unique reference.
struct ReferenceIndex<'a> {
    model: &'a Model,
    element_counts_by_suffix: HashMap<&'a [String], usize>,
}

impl<'a> ReferenceIndex<'a> {
    /// Index every dotted suffix of every element path of `model`.
    fn new(model: &'a Model) -> Self {
        let mut element_counts_by_suffix = HashMap::new();
        let elements = model.elements();
        for path in elements.keys() {
            let segments = path.segments();
            let depth = segments.len();
            let suffix_starts = 0..depth;
            for suffix_start in suffix_starts {
                let suffix = &segments[suffix_start..];
                let element_count = element_counts_by_suffix.entry(suffix).or_default();
                *element_count += 1;
            }
        }
        Self {
            model,
            element_counts_by_suffix,
        }
    }

    /// Spell the shortest dotted suffix of `path` that ends no other element's path, else the whole path, which always names it exactly.
    fn spell(&self, path: &ElementPath) -> Result<String, UnprintableReason> {
        let element = self.model.element(path);
        let is_unknown_element = element.is_none();
        if is_unknown_element {
            let unknown_path = path.clone();
            let unknown_reference = UnprintableReason::UnknownReference(unknown_path);
            return Err(unknown_reference);
        }
        let segments = path.segments();
        let depth = segments.len();
        let suffix_starts = 0..depth;
        let shortest_first_starts = suffix_starts.rev();
        let suffixes = shortest_first_starts.map(|suffix_start| &segments[suffix_start..]);
        let mut unique_suffixes = suffixes.filter(|suffix| {
            let element_count = self.element_counts_by_suffix.get(suffix);
            element_count == Some(&1)
        });
        let Some(unique_suffix) = unique_suffixes.next() else {
            let whole_path = segments.join(".");
            return Ok(whole_path);
        };
        let reference = unique_suffix.join(".");
        Ok(reference)
    }
}
