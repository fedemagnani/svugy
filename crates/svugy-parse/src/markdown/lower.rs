//! Lower a parsed diagram file into the model: styles expanded, references resolved, each property applied where it suits, each workflow's edges declared by its steps.

use std::collections::BTreeMap;

use chumsky::span::Spanned;
use svugy_model::builder::{DescribedRef, EdgeRef, ElementRef, ModelBuilder, StepRef, WorkflowRef};
use svugy_model::link::Link;
use svugy_model::model::{ByteRange, Edge, EdgeId, Element, PlacementHint, Position};
use svugy_model::path::ElementPath;
use svugy_model::properties::ComponentProperties;
use svugy_model::style::{ColorKey, NumberKey};
use svugy_model::workflow::{ElementOverride, Step, StepId, Workflow, WorkflowId};

use super::ast::{
    ChainItem, ComponentItem, Description, Document, EdgeItem, ListedEdge, OverrideItem, Reference,
    Span, StepItem, StepSection, WorkflowSection,
};
use super::diagnostic::Diagnostics;
use super::property::{LineStyle, Property, PropertyKey, PropertyValue};
use super::resolve::SuffixIndex;
use super::style::StyleBook;
use super::target::{ComponentNumbering, TargetRespeller};
use super::{MarkdownImport, MarkdownParse};

/// Lower each item into the builder through the style book and the suffix index, recording every error on the way.
struct Lowering {
    styles: StyleBook,
    references: SuffixIndex,
    element_refs: BTreeMap<ElementPath, ElementRef>,
    builder: ModelBuilder,
    diagnostics: Diagnostics,
}

/// Track a workflow while its steps are lowered: its handle and the descriptions held until every edge they may name is declared.
struct WorkflowDraft {
    handle: WorkflowRef,
    pending_descriptions: Vec<(DescribedRef, Description)>,
}

/// Collect where a component is pinned and how it places its children, `layout` and `columns` making the placement hint only together.
#[derive(Default)]
struct PlacementDraft {
    position: Option<Spanned<Position>>,
    layout: Option<Spanned<PlacementHint>>,
    columns: Option<Spanned<u32>>,
    has_unreadable_placement: bool,
}

impl MarkdownParse {
    /// Lower the parsed document into a model, keeping the parse's errors and adding those of lowering.
    pub fn lower(self) -> MarkdownImport {
        let Self { document, errors } = self;
        let Document {
            view,
            styles,
            components,
            workflows,
        } = document;
        let mut diagnostics = Diagnostics::from_parts(errors, Vec::new());
        let style_book = StyleBook::read(styles, &mut diagnostics);
        let suffix_index = SuffixIndex::index(&components);
        let builder = ModelBuilder::new();
        let builder = match view {
            Some(view) => builder.with_view(view),
            None => builder,
        };
        let mut lowering = Lowering::new(style_book, suffix_index, builder, diagnostics);
        lowering.lower_components(None, components);
        for workflow in workflows {
            lowering.lower_workflow(workflow);
        }
        lowering.into_import()
    }
}

impl Lowering {
    /// Create a lowering that adds to `builder`, nothing added yet.
    fn new(
        styles: StyleBook,
        references: SuffixIndex,
        builder: ModelBuilder,
        diagnostics: Diagnostics,
    ) -> Self {
        Self {
            styles,
            references,
            element_refs: BTreeMap::new(),
            builder,
            diagnostics,
        }
    }

    /// Add each component under `parent`, the top level when `None`, described with its targets respelled, then its children under it.
    fn lower_components(&mut self, parent: Option<&ElementRef>, components: Vec<ComponentItem>) {
        for component in components {
            let ComponentItem {
                id,
                display_name,
                entries,
                description,
                children,
                span,
            } = component;
            let label = display_name.unwrap_or_else(|| id.inner.clone());
            let source_range = ByteRange::new(span.start, span.end);
            let properties = self.styles.expand(&entries, &mut self.diagnostics);
            let (component_properties, placement) = self.lower_component_properties(properties);
            placement.reject_on_component(&mut self.diagnostics);
            let element = Element::new(label)
                .with_span(source_range)
                .with_properties(component_properties);
            let element_ref = self.add_component(parent, &id, element);
            if let Some(description) = description {
                let respeller = TargetRespeller::new(&self.references, &ComponentNumbering);
                let canonical_text = respeller.respell(&description.text);
                let described_element = element_ref.clone();
                let owner = DescribedRef::Element(described_element);
                self.builder.describe(&owner, canonical_text);
            }
            self.lower_components(Some(&element_ref), children);
        }
    }

    /// Add `element` as the component `id` under `parent`, or record `id` declared twice among its siblings and return its first declaration.
    fn add_component(
        &mut self,
        parent: Option<&ElementRef>,
        id: &Spanned<String>,
        element: Element,
    ) -> ElementRef {
        let id_text = &id.inner;
        let parent_path = parent.map_or_else(ElementPath::root, |parent| parent.path().clone());
        let path = parent_path.child(id_text);
        let first_declaration = self.element_refs.get(&path).cloned();
        let Some(first_declaration) = first_declaration else {
            let element_ref = match parent {
                Some(parent) => self.builder.add_child(parent, id_text, element),
                None => self.builder.add_top_level(id_text, element),
            };
            let declared_ref = element_ref.clone();
            self.element_refs.insert(path, declared_ref);
            return element_ref;
        };
        let is_top_level = parent_path.is_root();
        let message = if is_top_level {
            format!("`{id_text}` is declared twice at the top level")
        } else {
            format!("`{id_text}` is declared twice under `{parent_path}`")
        };
        self.diagnostics.record_error(message, id.span);
        first_declaration
    }

    /// Add the workflow with its overrides and steps, then describe its prose, its steps' prose and its edges once every edge is declared.
    fn lower_workflow(&mut self, section: WorkflowSection) {
        let WorkflowSection {
            id,
            title,
            subtitle,
            prose,
            overrides,
            steps,
        } = section;
        let workflow_id = WorkflowId::new(id.inner);
        let id_range = ByteRange::new(id.span.start, id.span.end);
        let workflow = Workflow::new(workflow_id, title).with_span(id_range);
        let workflow = match subtitle {
            Some(subtitle) => workflow.with_subtitle(subtitle),
            None => workflow,
        };
        let handle = self.builder.add_workflow(workflow);
        let mut draft = WorkflowDraft {
            handle,
            pending_descriptions: Vec::new(),
        };
        if let Some(prose) = prose {
            let described_workflow = draft.handle.clone();
            let owner = DescribedRef::Workflow(described_workflow);
            draft.pending_descriptions.push((owner, prose));
        }
        for override_item in overrides {
            self.lower_override(&draft, override_item);
        }
        for step in steps {
            self.lower_step(&mut draft, step);
        }
        let WorkflowDraft {
            handle,
            pending_descriptions,
        } = draft;
        let declared_workflow = self.builder.workflow(&handle);
        let respeller = TargetRespeller::new(&self.references, declared_workflow);
        let canonical_descriptions: Vec<(DescribedRef, String)> = pending_descriptions
            .into_iter()
            .map(|(owner, description)| {
                let canonical_text = respeller.respell(&description.text);
                (owner, canonical_text)
            })
            .collect();
        for (owner, canonical_text) in canonical_descriptions {
            self.builder.describe(&owner, canonical_text);
        }
    }

    /// Override how the workflow draws the component the item names, applying its styles then its `key=value` entries.
    fn lower_override(&mut self, draft: &WorkflowDraft, override_item: OverrideItem) {
        let OverrideItem {
            reference,
            entries,
            span,
        } = override_item;
        let path = self.references.resolve(&reference, &mut self.diagnostics);
        let properties = self.styles.expand(&entries, &mut self.diagnostics);
        let override_range = ByteRange::new(span.start, span.end);
        let (component_properties, placement) = self.lower_component_properties(properties);
        let child_placement = placement.child_placement(&mut self.diagnostics);
        let mut element_override = ElementOverride::new()
            .with_span(override_range)
            .with_properties(component_properties);
        if let Some(position) = placement.position {
            element_override = element_override.with_position(position.inner);
        }
        if let Some(child_placement) = child_placement {
            element_override = element_override.with_child_placement(child_placement);
        }
        let Some(path) = path else {
            return;
        };
        let element_ref = &self.element_refs[&path];
        self.builder
            .override_element(&draft.handle, element_ref, element_override);
    }

    /// Add the step, then each of its items in written order, holding its prose until every edge of the workflow is declared.
    fn lower_step(&mut self, draft: &mut WorkflowDraft, step: StepSection) {
        let StepSection {
            id,
            title,
            prose,
            items,
        } = step;
        let step_id = StepId::new(id.inner);
        let id_range = ByteRange::new(id.span.start, id.span.end);
        let step = Step::new(step_id, title).with_span(id_range);
        let step_ref = self.builder.add_step(&draft.handle, step);
        if let Some(prose) = prose {
            let described_step = step_ref.clone();
            let owner = DescribedRef::Step(described_step);
            draft.pending_descriptions.push((owner, prose));
        }
        for item in items {
            match item {
                StepItem::Element(reference) => self.list_element(&step_ref, &reference),
                StepItem::ListedEdge(listed_edge) => self.list_edge(&step_ref, listed_edge),
                StepItem::Edge(edge_item) => self.declare_edge(draft, &step_ref, edge_item),
                StepItem::Chain(chain_item) => self.declare_chain(&step_ref, chain_item),
            }
        }
    }

    /// List the component `reference` names in the step, shown without an edge, written where the reference is.
    fn list_element(&mut self, step_ref: &StepRef, reference: &Reference) {
        let Some(path) = self.references.resolve(reference, &mut self.diagnostics) else {
            return;
        };
        let element_ref = &self.element_refs[&path];
        let reference_range = ByteRange::new(reference.span.start, reference.span.end);
        self.builder
            .list_element_written_at(step_ref, element_ref, Some(reference_range));
    }

    /// List again in the step the edge `FROM -> TO[n]`, which validation checks some step of the workflow declares.
    fn list_edge(&mut self, step_ref: &StepRef, listed_edge: ListedEdge) {
        let ListedEdge {
            from,
            to,
            ordinal,
            span,
        } = listed_edge;
        let from_path = self.references.resolve(&from, &mut self.diagnostics);
        let to_path = self.references.resolve(&to, &mut self.diagnostics);
        let (Some(from_path), Some(to_path)) = (from_path, to_path) else {
            return;
        };
        let id = EdgeId::new(from_path, to_path, ordinal);
        let listing_range = ByteRange::new(span.start, span.end);
        self.builder
            .list_edge_written_at(step_ref, id, Some(listing_range));
    }

    /// Declare the item's edge in the step, holding its description until every edge of the workflow is declared.
    fn declare_edge(&mut self, draft: &mut WorkflowDraft, step_ref: &StepRef, edge_item: EdgeItem) {
        let EdgeItem {
            from,
            to,
            label,
            entries,
            description,
        } = edge_item;
        let from_path = self.references.resolve(&from, &mut self.diagnostics);
        let to_path = self.references.resolve(&to, &mut self.diagnostics);
        let properties = self.styles.expand(&entries, &mut self.diagnostics);
        let edge = match label {
            Some(label) => Edge::new().with_label(label.inner),
            None => Edge::new(),
        };
        let edge = self.apply_edge_properties(edge, properties);
        let (Some(from_path), Some(to_path)) = (from_path, to_path) else {
            return;
        };
        let hop_span = Span::from(from.span.start..to.span.end);
        let edge_ref = self.add_hop(step_ref, from_path, to_path, edge, hop_span);
        let Some(description) = description else {
            return;
        };
        let owner = DescribedRef::Edge(edge_ref);
        draft.pending_descriptions.push((owner, description));
    }

    /// Declare in the step one plain edge per hop of the chain whose two references resolve, after resolving all of them.
    fn declare_chain(&mut self, step_ref: &StepRef, chain: ChainItem) {
        let ChainItem {
            start,
            first_hop,
            second_hop,
            later_hops,
        } = chain;
        let leading_references = [start, first_hop, second_hop];
        let references = leading_references.into_iter().chain(later_hops);
        let endpoints: Vec<Option<(ElementPath, Span)>> = references
            .map(|reference| {
                let path = self.references.resolve(&reference, &mut self.diagnostics)?;
                Some((path, reference.span))
            })
            .collect();
        for hop in endpoints.windows(2) {
            let [Some((from, from_span)), Some((to, to_span))] = hop else {
                continue;
            };
            let hop_span = Span::from(from_span.start..to_span.end);
            let from_path = from.clone();
            let to_path = to.clone();
            let edge = Edge::new();
            self.add_hop(step_ref, from_path, to_path, edge, hop_span);
        }
    }

    /// Declare `edge` from `from` to `to` in the step, written at `hop_span`.
    fn add_hop(
        &mut self,
        step_ref: &StepRef,
        from: ElementPath,
        to: ElementPath,
        edge: Edge,
        hop_span: Span,
    ) -> EdgeRef {
        let hop_range = ByteRange::new(hop_span.start, hop_span.end);
        let spanned_edge = edge.with_span(hop_range);
        let from_ref = &self.element_refs[&from];
        let to_ref = &self.element_refs[&to];
        self.builder
            .add_edge(step_ref, from_ref, to_ref, spanned_edge)
    }

    /// Lower each property into the component properties, collecting placements apart for the site to take or reject, recording one that suits only edges; an unreadable one is already reported.
    fn lower_component_properties(
        &mut self,
        properties: Vec<Property>,
    ) -> (ComponentProperties, PlacementDraft) {
        let mut component_properties = ComponentProperties::new();
        let mut placement = PlacementDraft::default();
        for property in properties {
            let Property { value, span } = property;
            let lowered = component_properties;
            component_properties = match value {
                PropertyValue::Caption(caption) => lowered.with_caption(caption),
                PropertyValue::Color(color) => lowered.with_style(ColorKey::FILL.name(), color),
                PropertyValue::Stroke(stroke) => {
                    lowered.with_style(ColorKey::STROKE.name(), stroke)
                }
                PropertyValue::Line(LineStyle::Badge) => {
                    let message = "`badge` is an edge's line: a component's `line` is `solid`, `dashed` or `dotted`";
                    self.diagnostics.record_error(message, span);
                    lowered
                }
                PropertyValue::Line(line) => match line.stroke_dash() {
                    Some(stroke_dash) => {
                        let stroke_dash_text = stroke_dash.to_string();
                        lowered.with_style(NumberKey::STROKE_DASH.name(), stroke_dash_text)
                    }
                    None => lowered,
                },
                PropertyValue::Shape(shape) => lowered.with_shape(shape),
                PropertyValue::Height(height) => lowered.with_height(height),
                PropertyValue::Position(position) => {
                    let spanned_position = Spanned {
                        inner: position,
                        span,
                    };
                    placement.position = Some(spanned_position);
                    lowered
                }
                PropertyValue::Layout(layout) => {
                    let spanned_layout = Spanned {
                        inner: layout,
                        span,
                    };
                    placement.layout = Some(spanned_layout);
                    lowered
                }
                PropertyValue::Columns(columns) => {
                    let spanned_columns = Spanned {
                        inner: columns,
                        span,
                    };
                    placement.columns = Some(spanned_columns);
                    lowered
                }
                PropertyValue::Link(link) => {
                    let link_range = ByteRange::new(link.span.start, link.span.end);
                    match self.resolve_link(link) {
                        Some(target) => lowered.with_link_written_at(target, Some(link_range)),
                        None => lowered,
                    }
                }
                PropertyValue::Code(code) => {
                    let code_range = ByteRange::new(code.span.start, code.span.end);
                    lowered.with_code_written_at(code.inner, Some(code_range))
                }
                PropertyValue::Meta { name, value } => lowered.with_metadata(name, value),
                PropertyValue::Unreadable(PropertyKey::Layout | PropertyKey::Columns) => {
                    placement.has_unreadable_placement = true;
                    lowered
                }
                PropertyValue::Unreadable(_other_unreadable_key) => lowered,
                PropertyValue::Traveler(..) => {
                    let message = "`traveler` does not apply to a component";
                    self.diagnostics.record_error(message, span);
                    lowered
                }
            };
        }
        (component_properties, placement)
    }

    /// Return a URL as written, or the path of the component a reference names; `None` once an unresolved reference is recorded.
    fn resolve_link(&mut self, link: Spanned<String>) -> Option<String> {
        let is_url = Link::is_url_shaped(&link.inner);
        if is_url {
            return Some(link.inner);
        }
        let reference = Reference::from_dotted(&link.inner, link.span);
        let path = self.references.resolve(&reference, &mut self.diagnostics)?;
        let path_text = path.to_string();
        Some(path_text)
    }

    /// Apply each property to an edge, recording one that suits only components; an unreadable one is already reported.
    fn apply_edge_properties(&mut self, edge: Edge, properties: Vec<Property>) -> Edge {
        let mut edge = edge;
        for property in properties {
            let Property { value, span } = property;
            edge = match value {
                PropertyValue::Line(LineStyle::Badge) => edge.with_badge(),
                PropertyValue::Line(line) => match line.stroke_dash() {
                    Some(stroke_dash) => {
                        let stroke_dash_text = stroke_dash.to_string();
                        edge.with_style(NumberKey::STROKE_DASH.name(), stroke_dash_text)
                    }
                    None => edge,
                },
                PropertyValue::Color(color) => edge.with_style(ColorKey::STROKE.name(), color),
                PropertyValue::Traveler(traveler) => edge.with_traveler(traveler),
                PropertyValue::Unreadable(_unreadable_key) => edge,
                edge_inapplicable @ (PropertyValue::Caption(..)
                | PropertyValue::Stroke(..)
                | PropertyValue::Shape(..)
                | PropertyValue::Height(..)
                | PropertyValue::Position(..)
                | PropertyValue::Layout(..)
                | PropertyValue::Columns(..)
                | PropertyValue::Link(..)
                | PropertyValue::Code(..)
                | PropertyValue::Meta { .. }) => {
                    let key = edge_inapplicable.key();
                    let message = format!("`{key}` does not apply to an edge");
                    self.diagnostics.record_error(message, span);
                    edge
                }
            };
        }
        edge
    }

    /// Finish into the import: build the model through its admission and sort the errors by source offset.
    fn into_import(self) -> MarkdownImport {
        let Self {
            builder,
            diagnostics,
            ..
        } = self;
        let admission = builder.build();
        let (errors, warnings) = diagnostics.into_sorted_parts();
        MarkdownImport {
            admission,
            errors,
            warnings,
        }
    }
}

impl PlacementDraft {
    /// Return the placement hint `layout` and `columns` make together, recording a grid without `columns` and `columns` without a grid; `None` when either was unreadable.
    fn child_placement(&self, diagnostics: &mut Diagnostics) -> Option<PlacementHint> {
        if self.has_unreadable_placement {
            return None;
        }
        let layout = self
            .layout
            .as_ref()
            .map(|layout| (layout.inner, layout.span));
        let columns = self
            .columns
            .as_ref()
            .map(|columns| (columns.inner, columns.span));
        match (layout, columns) {
            (None, None) => None,
            (Some((PlacementHint::Grid { .. }, _layout_span)), Some((columns, _columns_span))) => {
                let grid = PlacementHint::Grid { columns };
                Some(grid)
            }
            (Some((PlacementHint::Grid { .. }, layout_span)), None) => {
                diagnostics.record_error("`layout=grid` needs `columns`", layout_span);
                None
            }
            (_layout, Some((_columns, columns_span))) => {
                diagnostics.record_error("`columns` needs `layout=grid`", columns_span);
                None
            }
            (Some((placement, _layout_span)), None) => Some(placement),
        }
    }

    /// Record each placement key set, which only a workflow's override may set.
    fn reject_on_component(&self, diagnostics: &mut Diagnostics) {
        let position_span = self.position.as_ref().map(|position| position.span);
        let layout_span = self.layout.as_ref().map(|layout| layout.span);
        let columns_span = self.columns.as_ref().map(|columns| columns.span);
        let placement_spans = [
            (PropertyKey::Position, position_span),
            (PropertyKey::Layout, layout_span),
            (PropertyKey::Columns, columns_span),
        ];
        for (key, span) in placement_spans {
            let Some(span) = span else {
                continue;
            };
            let message =
                format!("`{key}` on a component is v3: set it in each workflow's overrides");
            diagnostics.record_error(message, span);
        }
    }
}
