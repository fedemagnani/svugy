//! Print a model back as a Markdown diagram, the text form any imported model can be saved as.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use thiserror::Error;

use svugy_model::description::{DescriptionTarget, TargetLink};
use svugy_model::link::Link;
use svugy_model::model::{Edge, EdgeId, Element, Model, PlacementHint, Position};
use svugy_model::path::ElementPath;
use svugy_model::properties::ComponentProperties;
use svugy_model::style::{ColorKey, NumberKey, NumberValue, StyleValue};
use svugy_model::workflow::{ElementOverride, Step, StepId, StepItem, Workflow, WorkflowId};

use super::ast::Value;
use super::parse::ItemLabel;
use super::property::LineStyle;
use super::skeleton::{DESCRIPTION_INFO, Skeleton};
use super::target::{ComponentNumbering, EdgeNumbering};

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
/// Count the backticks of the shortest code fence CommonMark reads.
const MIN_FENCE_LENGTH: usize = 3;

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

/// Name one part of a model Markdown cannot say, with the element, workflow, override, step or edge it belongs to.
#[derive(Clone, Debug, PartialEq, Error)]
#[error("{owner}: {reason}")]
pub struct Unprintable {
    pub owner: UnprintableOwner,
    pub reason: UnprintableReason,
}

/// Name the element, workflow, override, step or workflow edge an unprintable part belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum UnprintableOwner {
    Element(ElementPath),
    Workflow(WorkflowId),
    Override(WorkflowId, ElementPath),
    Step(WorkflowId, StepId),
    Edge(WorkflowId, EdgeId),
}

impl fmt::Display for UnprintableOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(path) => write!(formatter, "element `{path}`"),
            Self::Workflow(workflow) => write!(formatter, "workflow `{workflow}`"),
            Self::Override(workflow, path) => {
                write!(formatter, "override of `{path}` in workflow `{workflow}`")
            }
            Self::Step(workflow, step) => write!(formatter, "step `{workflow}/{step}`"),
            Self::Edge(workflow, id) => write!(formatter, "edge `{id}` of workflow `{workflow}`"),
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
    #[error("its {field} {text:?} holds `<!--`, which the language reads as a comment and drops")]
    CommentOpening { field: TextField, text: String },
    #[error(
        "its description does not read back verbatim from a ```description block: Markdown reads a carriage return as a line break"
    )]
    UnreadableDescription,
    #[error(
        "its prose does not read back verbatim under a heading: outside a code fence it holds no `- ` bullet list, `#` heading or comment, and no blank line opens or closes it"
    )]
    UnreadableProse,
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
    #[error(
        "its ordinal is {ordinal}, but the language numbers repeated edges in the order their steps declare them, so it would read back as {reading_ordinal}"
    )]
    RenumberedEdge { ordinal: u32, reading_ordinal: u32 },
}

/// Name the text of an element, workflow, step or edge an unprintable reason is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::Display)]
pub enum TextField {
    #[strum(to_string = "display name")]
    DisplayName,
    #[strum(to_string = "caption")]
    Caption,
    #[strum(to_string = "link")]
    Link,
    #[strum(to_string = "code")]
    Code,
    #[strum(to_string = "metadata value")]
    MetadataValue,
    #[strum(to_string = "label")]
    Label,
    #[strum(to_string = "title")]
    Title,
    #[strum(to_string = "subtitle")]
    Subtitle,
}

/// Print a model as a Markdown diagram, collecting every part Markdown cannot say.
pub struct MarkdownPrinter<'a> {
    model: &'a Model,
    references: ReferenceIndex<'a>,
    markdown: String,
    unprintables: Vec<Unprintable>,
}

/// Track which edges the printed steps of one workflow have declared, the ordinal Markdown reads each declaration back with, and how many listings of each edge are still to print.
struct EdgeDeclarations<'a> {
    declared_edges: BTreeSet<&'a EdgeId>,
    declaration_counts: BTreeMap<(&'a ElementPath, &'a ElementPath), u32>,
    listings_left: BTreeMap<&'a EdgeId, usize>,
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
        printer.write_components();
        printer.write_workflows();
        printer.finish()
    }

    /// Write the frontmatter holding the view angle, only when the model sets one.
    fn write_frontmatter(&mut self) {
        let Some(view) = self.model.view() else {
            return;
        };
        let view_spelling: &'static str = view.into();
        let frontmatter = format!("---\nview: {view_spelling}\n---\n\n");
        self.markdown.push_str(&frontmatter);
    }

    /// Write one component item per element, in path order, nested two spaces per level.
    fn write_components(&mut self) {
        self.markdown.push_str("## Components\n");
        let model = self.model;
        for (path, element) in model.elements() {
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
        let is_invalid_id = !ElementPath::is_component_id(id);
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
        let element_properties = element.properties();
        let properties = self.component_properties(element_properties, None, None, reasons);
        let properties_list = Self::spell_properties_list(&properties);
        let item_line = format!("{indent}- {id}{display_name}{properties_list}\n");
        let Some(description) = element.description() else {
            return item_line;
        };
        let description_indent = format!("{indent}  ");
        let description_spelling =
            self.spell_description(description, &description_indent, &ComponentNumbering);
        let spelled_description = Self::spelling_or_push_reason(description_spelling, reasons);
        format!("{item_line}{spelled_description}")
    }

    /// Spell a component's properties, with the `position` and `child_placement` only an override sets, in the language's table order from `caption` to `meta.NAME`, pushing onto `reasons` what the language cannot say.
    fn component_properties(
        &self,
        component: &ComponentProperties,
        position: Option<Position>,
        child_placement: Option<PlacementHint>,
        reasons: &mut Vec<UnprintableReason>,
    ) -> Vec<String> {
        let mut properties = Vec::new();
        let style = component.style();
        if let Some(caption) = component.caption() {
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
        if let Some(shape) = component.shape() {
            let shape_keyword: &str = shape.into();
            let property = format!("shape={shape_keyword}");
            properties.push(property);
        }
        if let Some(height) = component.height() {
            let property = format!("height={height}");
            properties.push(property);
        }
        if let Some(position) = position {
            let x = position.x;
            let y = position.y;
            let property = format!("position={x},{y}");
            properties.push(property);
        }
        if let Some(child_placement) = child_placement {
            let layout_keyword: &str = child_placement.into();
            let property = format!("layout={layout_keyword}");
            properties.push(property);
        }
        if let Some(PlacementHint::Grid { columns }) = child_placement {
            let property = format!("columns={columns}");
            properties.push(property);
        }
        if let Some(raw_link) = component.link() {
            let link_spelling = self.spell_link(raw_link);
            let link_target = Self::spelling_or_push_reason(link_spelling, reasons);
            let property = format!("link={link_target}");
            properties.push(property);
        }
        if let Some(code) = component.code() {
            let code_spelling = Self::spell_value(code, TextField::Code);
            let code_value = Self::spelling_or_push_reason(code_spelling, reasons);
            let property = format!("code={code_value}");
            properties.push(property);
        }
        for (name, value) in component.metadata() {
            let is_invalid_name = !ElementPath::is_component_id(name);
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
        let elements = self.model.elements();
        let link = Link::resolve(raw_link, elements);
        match link {
            Link::Element(target) => {
                let canonical_link = target.to_string();
                let is_canonical = canonical_link == raw_link;
                if is_canonical {
                    let reference = self.references.spell(&target);
                    return Ok(reference);
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

    /// Write `## Workflows`, then each workflow in written order.
    fn write_workflows(&mut self) {
        self.markdown.push_str("\n## Workflows\n");
        let model = self.model;
        for workflow in model.workflows() {
            self.write_workflow(workflow);
        }
    }

    /// Write the workflow's heading with its prose and one override item per overridden component, then each of its steps.
    fn write_workflow(&mut self, workflow: &'a Workflow) {
        let workflow_id = workflow.id();
        let mut reasons = Vec::new();
        let heading = Self::workflow_heading(workflow, &mut reasons);
        let prose = match workflow.description() {
            Some(prose) => {
                let prose_spelling = self.spell_prose(prose, workflow);
                let spelled_prose = Self::spelling_or_push_reason(prose_spelling, &mut reasons);
                Some(spelled_prose)
            }
            None => None,
        };
        self.report(reasons, || {
            let owner_id = workflow_id.clone();
            UnprintableOwner::Workflow(owner_id)
        });
        let mut override_items = String::new();
        for (path, element_override) in workflow.overrides() {
            let mut reasons = Vec::new();
            let item = self.override_item(path, element_override, &mut reasons);
            override_items.push_str(&item);
            self.report(reasons, || {
                let owner_id = workflow_id.clone();
                let overridden_path = path.clone();
                UnprintableOwner::Override(owner_id, overridden_path)
            });
        }
        self.write_headed_part(&heading, prose.as_deref(), &override_items);
        let mut declarations = EdgeDeclarations::of(workflow);
        for step in workflow.steps() {
            self.write_step(workflow, step, &mut declarations);
        }
    }

    /// Spell `### ID: TITLE [(subtitle=TEXT)]`, pushing onto `reasons` what the language cannot say.
    fn workflow_heading(workflow: &Workflow, reasons: &mut Vec<UnprintableReason>) -> String {
        let workflow_id = workflow.id();
        let workflow_title = workflow.title();
        let title_spelling = Self::spell_title(workflow_title);
        let title = Self::spelling_or_push_reason(title_spelling, reasons);
        let Some(subtitle) = workflow.subtitle() else {
            return format!("### {workflow_id}: {title}");
        };
        let subtitle_spelling = Self::spell_value(subtitle, TextField::Subtitle);
        let subtitle_value = Self::spelling_or_push_reason(subtitle_spelling, reasons);
        format!("### {workflow_id}: {title} (subtitle={subtitle_value})")
    }

    /// Spell `- REF (PROPERTIES)`, the parentheses kept for an override that sets nothing, pushing onto `reasons` what the language cannot say.
    fn override_item(
        &self,
        path: &ElementPath,
        element_override: &ElementOverride,
        reasons: &mut Vec<UnprintableReason>,
    ) -> String {
        let reference = self.references.spell(path);
        let overridden_properties = element_override.properties();
        let position = element_override.position();
        let child_placement = element_override.child_placement();
        let properties =
            self.component_properties(overridden_properties, position, child_placement, reasons);
        let entries = properties.join(", ");
        format!("- {reference} ({entries})\n")
    }

    /// Write the step's heading with its prose, then one item per item it lists, an edge declared at its first listing that Markdown reads back with its ordinal.
    fn write_step(
        &mut self,
        workflow: &'a Workflow,
        step: &'a Step,
        declarations: &mut EdgeDeclarations<'a>,
    ) {
        let workflow_id = workflow.id();
        let step_id = step.id();
        let mut reasons = Vec::new();
        let step_title = step.title();
        let title_spelling = Self::spell_title(step_title);
        let title = Self::spelling_or_push_reason(title_spelling, &mut reasons);
        let heading = format!("#### {step_id}: {title}");
        let prose = match step.description() {
            Some(prose) => {
                let prose_spelling = self.spell_prose(prose, workflow);
                let spelled_prose = Self::spelling_or_push_reason(prose_spelling, &mut reasons);
                Some(spelled_prose)
            }
            None => None,
        };
        self.report(reasons, || {
            let owner_workflow_id = workflow_id.clone();
            let owner_step_id = step_id.clone();
            UnprintableOwner::Step(owner_workflow_id, owner_step_id)
        });
        let mut items = String::new();
        for item in step.items() {
            let item_line = match item {
                StepItem::Element(path) => {
                    let reference = self.references.spell(path);
                    format!("- {reference}\n")
                }
                StepItem::Edge(id) => self.edge_item(workflow, id, declarations),
            };
            items.push_str(&item_line);
        }
        self.write_headed_part(&heading, prose.as_deref(), &items);
    }

    /// Spell the edge `id` as a step item: a declaration with its label, properties and description where `EdgeDeclarations` declares it, `- FROM -> TO[n]` elsewhere, reporting what the language cannot say.
    fn edge_item(
        &mut self,
        workflow: &'a Workflow,
        id: &'a EdgeId,
        declarations: &mut EdgeDeclarations<'a>,
    ) -> String {
        let from_path = id.from();
        let to_path = id.to();
        let from_reference = self.references.spell(from_path);
        let to_reference = self.references.spell(to_path);
        let edge = workflow.edge(id);
        let reading_ordinal = declarations.declare(id);
        let (Some(edge), Some(reading_ordinal)) = (edge, reading_ordinal) else {
            let ordinal = id.ordinal();
            return format!("- {from_reference} -> {to_reference}[{ordinal}]\n");
        };
        let mut reasons = Vec::new();
        let ordinal = id.ordinal();
        let is_renumbered = ordinal != reading_ordinal;
        if is_renumbered {
            let renumbered_edge = UnprintableReason::RenumberedEdge {
                ordinal,
                reading_ordinal,
            };
            reasons.push(renumbered_edge);
        }
        let label = match edge.label() {
            Some(label) => {
                let label_spelling = Self::spell_label(label, TextField::Label);
                let label = Self::spelling_or_push_reason(label_spelling, &mut reasons);
                format!(": {label}")
            }
            None => String::new(),
        };
        let properties = Self::edge_properties(edge, &mut reasons);
        let properties_list = Self::spell_properties_list(&properties);
        let item_line = format!("- {from_reference} -> {to_reference}{label}{properties_list}\n");
        let spelled_item = match edge.description() {
            Some(description) => {
                let description_spelling = self.spell_description(description, "  ", workflow);
                let spelled_description =
                    Self::spelling_or_push_reason(description_spelling, &mut reasons);
                format!("{item_line}{spelled_description}")
            }
            None => item_line,
        };
        self.report(reasons, || {
            let owner_workflow_id = workflow.id().clone();
            let edge_id = id.clone();
            UnprintableOwner::Edge(owner_workflow_id, edge_id)
        });
        spelled_item
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

    /// Write a heading after a blank line, then its prose, then its items after a blank line when prose precedes them.
    fn write_headed_part(&mut self, heading: &str, prose: Option<&str>, items: &str) {
        let heading_line = format!("\n{heading}\n");
        self.markdown.push_str(&heading_line);
        if let Some(prose) = prose {
            let prose_lines = format!("{prose}\n");
            self.markdown.push_str(&prose_lines);
        }
        let has_items = !items.is_empty();
        let has_prose_before_items = prose.is_some() && has_items;
        if has_prose_before_items {
            self.markdown.push('\n');
        }
        self.markdown.push_str(items);
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

    /// Spell `text` as a heading's title: bare when it reads back as a label and ends with no `#`, which Markdown reads as the heading's closing `#`s, otherwise quoted.
    fn spell_title(text: &str) -> Result<String, UnprintableReason> {
        let reads_back_as_label = ItemLabel::reads_back_bare(text);
        let is_ending_with_hash = text.ends_with('#');
        let reads_back_bare = reads_back_as_label && !is_ending_with_hash;
        Self::spell_bare_or_quoted(text, reads_back_bare, TextField::Title)
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
        let backslash_escaped = text.replace('\\', "\\\\");
        let escaped = backslash_escaped.replace('"', "\\\"");
        let quoted = format!("\"{escaped}\"");
        Ok(quoted)
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

    /// Spell `description` as a description block after `indent`, its targets respelled as references, or report a description that would not read back verbatim under an item.
    fn spell_description(
        &self,
        description: &str,
        indent: &str,
        numbering: &(impl EdgeNumbering + ?Sized),
    ) -> Result<String, UnprintableReason> {
        let respelled_description = self.respell_targets(description, numbering);
        let description = respelled_description.as_str();
        let item_spelling = Self::fenced_description(description, "  ");
        let reads_back = Skeleton::reads_back_as_description(description, &item_spelling);
        let is_lossy = !reads_back;
        if is_lossy {
            return Err(UnprintableReason::UnreadableDescription);
        }
        let spelled_description = Self::fenced_description(description, indent);
        Ok(spelled_description)
    }

    /// Spell `prose` to be written verbatim under a heading of `workflow`, its targets respelled as references, or report prose that would not read back verbatim there.
    fn spell_prose(&self, prose: &str, workflow: &Workflow) -> Result<String, UnprintableReason> {
        let respelled_prose = self.respell_targets(prose, workflow);
        let reads_back = Skeleton::reads_back_as_prose(&respelled_prose);
        let is_lossy = !reads_back;
        if is_lossy {
            return Err(UnprintableReason::UnreadableProse);
        }
        Ok(respelled_prose)
    }

    /// Respell each canonical `#` target of `description` by its shortest unique references, `[n]` dropped where `numbering` adds it back on reading, so an unnumbered target it numbers reads back numbered.
    fn respell_targets(
        &self,
        description: &str,
        numbering: &(impl EdgeNumbering + ?Sized),
    ) -> String {
        TargetLink::respell_all(description, |target_link| {
            let description_target = DescriptionTarget::parse(target_link.target)?;
            match description_target {
                DescriptionTarget::Element(path) => {
                    let reference = self.references.spell(&path);
                    Some(reference)
                }
                DescriptionTarget::Edge(id) => {
                    let from_reference = self.references.spell(id.from());
                    let to_reference = self.references.spell(id.to());
                    let ordinal = id.ordinal();
                    let reading_ordinal = numbering.number_unnumbered(id.from(), id.to());
                    let is_numbered_on_reading = reading_ordinal == Some(ordinal);
                    if is_numbered_on_reading {
                        let unnumbered_target = format!("{from_reference}->{to_reference}");
                        return Some(unnumbered_target);
                    }
                    let numbered_target = format!("{from_reference}->{to_reference}[{ordinal}]");
                    Some(numbered_target)
                }
                DescriptionTarget::UnnumberedEdge { from, to } => {
                    let from_reference = self.references.spell(&from);
                    let to_reference = self.references.spell(&to);
                    let unnumbered_target = format!("{from_reference}->{to_reference}");
                    Some(unnumbered_target)
                }
                DescriptionTarget::Step { .. } => None,
            }
        })
    }

    /// Fence `description` as a description block after `indent`; fence length = max(3, 1 + longest backtick run opening a line).
    fn fenced_description(description: &str, indent: &str) -> String {
        let opening_backtick_runs = description.split('\n').map(|line| {
            let after_spaces = line.trim_start_matches(' ');
            let after_backticks = after_spaces.trim_start_matches('`');
            after_spaces.len() - after_backticks.len()
        });
        let longest_opening_run = opening_backtick_runs.max().unwrap_or_default();
        let fence_length = MIN_FENCE_LENGTH.max(longest_opening_run + 1);
        let fence = "`".repeat(fence_length);
        let content = Self::indented_lines(description, indent);
        format!("{indent}{fence}{DESCRIPTION_INFO}\n{content}{indent}{fence}\n")
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
}

impl<'a> EdgeDeclarations<'a> {
    /// Create new declarations of `workflow`, none printed yet, counting every listing of each of its edges.
    fn of(workflow: &'a Workflow) -> Self {
        let mut listings_left: BTreeMap<&'a EdgeId, usize> = BTreeMap::new();
        let items = workflow.steps().iter().flat_map(Step::items);
        for item in items {
            let StepItem::Edge(id) = item else {
                continue;
            };
            let listing_count = listings_left.entry(id).or_default();
            *listing_count += 1;
        }
        Self {
            declared_edges: BTreeSet::new(),
            declaration_counts: BTreeMap::new(),
            listings_left,
        }
    }

    /// Declare the edge `id` at the first listing reached once every lower ordinal of its pair is declared, or at its last listing, returning the ordinal Markdown reads that declaration back with; `None` while it stays a reference.
    fn declare(&mut self, id: &'a EdgeId) -> Option<u32> {
        let listing_count = self.listings_left.entry(id).or_default();
        *listing_count = listing_count.saturating_sub(1);
        let has_later_listing = *listing_count > 0;
        let is_already_declared = self.declared_edges.contains(id);
        if is_already_declared {
            return None;
        }
        let pair = (id.from(), id.to());
        let declaration_count = self.declaration_counts.entry(pair).or_default();
        let reading_ordinal = *declaration_count;
        let is_declared_out_of_order = id.ordinal() != reading_ordinal;
        let is_declaration_deferred = is_declared_out_of_order && has_later_listing;
        if is_declaration_deferred {
            return None;
        }
        *declaration_count += 1;
        self.declared_edges.insert(id);
        Some(reading_ordinal)
    }
}

/// Count the elements each dotted suffix of a path matches, to spell every element by its shortest unique reference.
struct ReferenceIndex<'a> {
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
            element_counts_by_suffix,
        }
    }

    /// Spell the shortest dotted suffix of `path` that ends no other element's path, else the whole path, which always names it exactly.
    fn spell(&self, path: &ElementPath) -> String {
        let segments = path.segments();
        let depth = segments.len();
        let suffix_starts = 0..depth;
        let shortest_first_starts = suffix_starts.rev();
        let suffixes = shortest_first_starts.map(|suffix_start| &segments[suffix_start..]);
        let mut unique_suffixes = suffixes.filter(|suffix| {
            let element_count = self.element_counts_by_suffix.get(suffix);
            element_count == Some(&1)
        });
        let unique_suffix = unique_suffixes.next();
        let reference_segments = unique_suffix.unwrap_or(segments);
        reference_segments.join(".")
    }
}
