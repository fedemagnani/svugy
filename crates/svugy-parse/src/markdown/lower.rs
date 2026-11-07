//! Lower a parsed diagram file into the model: styles expanded, references resolved, each property applied where it suits.

use std::collections::{BTreeMap, BTreeSet};

use chumsky::span::Spanned;
use svugy_model::builder::{ElementRef, ModelBuilder};
use svugy_model::legend::Legend;
use svugy_model::link::Link;
use svugy_model::model::{ByteRange, Edge, Element, PlacementHint};
use svugy_model::path::ElementPath;
use svugy_model::style::{ColorKey, NumberKey};

use super::ast::{
    ComponentItem, Document, EdgeItem, LegendItem, LegendKeyRow, LegendSection, Reference, Span,
};
use super::diagnostic::Diagnostics;
use super::property::{LineStyle, Property, PropertyKey, PropertyValue};
use super::resolve::SuffixIndex;
use super::style::StyleBook;
use super::{MarkdownImport, MarkdownParse};

/// Lower each item into the builder through the style book and the suffix index, the lowering recording every error on the way.
struct Lowering {
    styles: StyleBook,
    references: SuffixIndex,
    element_refs: BTreeMap<ElementPath, ElementRef>,
    container_paths: BTreeSet<ElementPath>,
    builder: ModelBuilder,
    diagnostics: Diagnostics,
}

/// Draft a component's element with its `layout` and `columns`, which only together make its placement hint, unless either was unreadable.
struct ComponentDraft {
    element: Element,
    layout: Option<Spanned<PlacementHint>>,
    columns: Option<Spanned<u32>>,
    has_unreadable_placement: bool,
}

/// Draft the edge every hop of an edge item shares, with where it asked for `line=badge`.
struct EdgeDraft {
    edge: Edge,
    badge_span: Option<Span>,
}

impl MarkdownParse {
    /// Lower the parsed document into a model, keeping the parse's errors and adding those of lowering.
    pub fn lower(self) -> MarkdownImport {
        let Self { document, errors } = self;
        let Document {
            title,
            subtitle,
            styles,
            components,
            edges,
            legend,
        } = document;
        let mut diagnostics = Diagnostics::from_parts(errors, Vec::new());
        let style_book = StyleBook::read(styles, &mut diagnostics);
        let suffix_index = SuffixIndex::index(&components);
        let mut builder = ModelBuilder::new();
        if let Some(title) = title {
            builder = builder.with_title(title);
        }
        if let Some(subtitle) = subtitle {
            builder = builder.with_subtitle(subtitle);
        }
        let mut lowering = Lowering::new(style_book, suffix_index, builder, diagnostics);
        lowering.lower_components(None, components);
        for edge in edges {
            lowering.lower_edge(edge);
        }
        let legend = legend.map(|legend| lowering.lower_legend(legend));
        lowering.into_import(legend)
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
            container_paths: BTreeSet::new(),
            builder,
            diagnostics,
        }
    }

    /// Add each component under `parent`, the top level when `None`, then its children under it.
    fn lower_components(&mut self, parent: Option<&ElementRef>, components: Vec<ComponentItem>) {
        for component in components {
            let ComponentItem {
                id,
                display_name,
                entries,
                children,
                span,
            } = component;
            let label = display_name.unwrap_or_else(|| id.inner.clone());
            let source_range = ByteRange::new(span.start, span.end);
            let element = Element::new(label).with_span(source_range);
            let properties = self.styles.expand(&entries, &mut self.diagnostics);
            let element = self.build_component(element, properties);
            let element_ref = self.add_component(parent, &id, element);
            let has_children = !children.is_empty();
            if has_children {
                let container_path = element_ref.path().clone();
                self.container_paths.insert(container_path);
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

    /// Apply each property to the component's element, then its placement hint from `layout` and `columns` together, checked only when both read.
    fn build_component(&mut self, element: Element, properties: Vec<Property>) -> Element {
        let mut draft = ComponentDraft {
            element,
            layout: None,
            columns: None,
            has_unreadable_placement: false,
        };
        for property in properties {
            draft = self.apply_component_property(draft, property);
        }
        let ComponentDraft {
            element,
            layout,
            columns,
            has_unreadable_placement,
        } = draft;
        if has_unreadable_placement {
            return element;
        }
        let layout = layout.map(|layout| (layout.inner, layout.span));
        let columns = columns.map(|columns| (columns.inner, columns.span));
        let placement = match (layout, columns) {
            (None, None) => return element,
            (Some((PlacementHint::Grid { .. }, _layout_span)), Some((columns, _columns_span))) => {
                PlacementHint::Grid { columns }
            }
            (Some((PlacementHint::Grid { .. }, layout_span)), None) => {
                self.diagnostics
                    .record_error("`layout=grid` needs `columns`", layout_span);
                return element;
            }
            (_layout, Some((_columns, columns_span))) => {
                self.diagnostics
                    .record_error("`columns` needs `layout=grid`", columns_span);
                return element;
            }
            (Some((placement, _layout_span)), None) => placement,
        };
        element.with_child_placement(placement)
    }

    /// Apply one property to a component, recording one that suits only edges or legend key rows; an unreadable one is already reported.
    fn apply_component_property(
        &mut self,
        mut draft: ComponentDraft,
        property: Property,
    ) -> ComponentDraft {
        let Property { value, span } = property;
        let element = draft.element;
        draft.element = match value {
            PropertyValue::Caption(caption) => element.with_caption(caption),
            PropertyValue::Color(color) => element.with_style(ColorKey::FILL.name(), color),
            PropertyValue::Stroke(stroke) => element.with_style(ColorKey::STROKE.name(), stroke),
            PropertyValue::Line(LineStyle::Badge) => {
                let message = "`badge` is an edge's line: a component's `line` is `solid`, `dashed` or `dotted`";
                self.diagnostics.record_error(message, span);
                element
            }
            PropertyValue::Line(line) => match line.stroke_dash() {
                Some(stroke_dash) => {
                    element.with_style(NumberKey::STROKE_DASH.name(), stroke_dash.to_string())
                }
                None => element,
            },
            PropertyValue::Shape(shape) => element.with_shape(shape),
            PropertyValue::Height(height) => element.with_height(height),
            PropertyValue::Position(position) => element.with_position(position),
            PropertyValue::Layout(layout) => {
                draft.layout = Some(Spanned {
                    inner: layout,
                    span,
                });
                element
            }
            PropertyValue::Columns(columns) => {
                draft.columns = Some(Spanned {
                    inner: columns,
                    span,
                });
                element
            }
            PropertyValue::Link(link) => {
                let link_range = ByteRange::new(link.span.start, link.span.end);
                match self.resolve_link(link) {
                    Some(target) => element.with_link(target).with_link_span(link_range),
                    None => element,
                }
            }
            PropertyValue::Meta { name, value } => element.with_metadata(name, value),
            PropertyValue::Unreadable(PropertyKey::Layout | PropertyKey::Columns) => {
                draft.has_unreadable_placement = true;
                element
            }
            PropertyValue::Unreadable(_other_unreadable_key) => element,
            component_inapplicable @ (PropertyValue::Traveler(..) | PropertyValue::Target(..)) => {
                let key = component_inapplicable.key();
                let message = format!("`{key}` does not apply to a component");
                self.diagnostics.record_error(message, span);
                element
            }
        };
        draft
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

    /// Add one edge per hop of the item whose two references resolve, after checking all of them; a chain drops its label and properties after recording them.
    fn lower_edge(&mut self, edge: EdgeItem) {
        let EdgeItem {
            references,
            label,
            properties,
        } = edge;
        let endpoints: Vec<Option<(ElementPath, Span)>> = references
            .iter()
            .map(|reference| {
                let path = self.references.resolve(reference, &mut self.diagnostics)?;
                Some((path, reference.span))
            })
            .collect();
        let is_chain = references.len() > 2;
        let label = match label {
            Some(label) if is_chain => {
                let message = "a chain takes no label; write its hops on separate lines";
                self.diagnostics.record_error(message, label.span);
                None
            }
            label => label,
        };
        let properties = match properties {
            Some(properties) if is_chain => {
                let message = "a chain takes no properties; write its hops on separate lines";
                self.diagnostics.record_error(message, properties.span);
                None
            }
            properties => properties,
        };
        let entries = properties
            .map(|properties| properties.inner)
            .unwrap_or_default();
        let expanded_properties = self.styles.expand(&entries, &mut self.diagnostics);
        let edge = match label {
            Some(label) => Edge::new().with_label(label.inner),
            None => Edge::new(),
        };
        let draft = self.build_edge(edge, expanded_properties);
        for hop in endpoints.windows(2) {
            let [Some((from, from_span)), Some((to, to_span))] = hop else {
                continue;
            };
            let hop_span = Span::from(from_span.start..to_span.end);
            self.add_hop(&draft, from, to, hop_span);
        }
    }

    /// Apply each property to the edge every hop shares, recording one that suits only components or legend key rows; an unreadable one is already reported.
    fn build_edge(&mut self, edge: Edge, properties: Vec<Property>) -> EdgeDraft {
        let mut draft = EdgeDraft {
            edge,
            badge_span: None,
        };
        for property in properties {
            let Property { value, span } = property;
            let edge = draft.edge;
            draft.edge = match value {
                PropertyValue::Line(LineStyle::Badge) => {
                    draft.badge_span = Some(span);
                    edge
                }
                PropertyValue::Line(line) => match line.stroke_dash() {
                    Some(stroke_dash) => {
                        edge.with_style(NumberKey::STROKE_DASH.name(), stroke_dash.to_string())
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
                | PropertyValue::Target(..)
                | PropertyValue::Meta { .. }) => {
                    let key = edge_inapplicable.key();
                    let message = format!("`{key}` does not apply to an edge");
                    self.diagnostics.record_error(message, span);
                    edge
                }
            };
        }
        draft
    }

    /// Add the hop from `from` to `to`, recording one from a component to itself or between it and its own ancestor or descendant, badged only between two leaves.
    fn add_hop(&mut self, draft: &EdgeDraft, from: &ElementPath, to: &ElementPath, hop_span: Span) {
        let is_self_edge = from == to;
        if is_self_edge {
            let message = format!("an edge cannot join `{from}` to itself");
            self.diagnostics.record_error(message, hop_span);
            return;
        }
        let is_from_ancestor = from.is_ancestor_of(to);
        if is_from_ancestor {
            let message = format!("an edge cannot join `{from}` and its descendant `{to}`");
            self.diagnostics.record_error(message, hop_span);
            return;
        }
        let is_to_ancestor = to.is_ancestor_of(from);
        if is_to_ancestor {
            let message = format!("an edge cannot join `{from}` and its ancestor `{to}`");
            self.diagnostics.record_error(message, hop_span);
            return;
        }
        let hop_range = ByteRange::new(hop_span.start, hop_span.end);
        let edge = draft.edge.clone().with_span(hop_range);
        let edge = match draft.badge_span {
            Some(badge_span) => self.badge_between_leaves(edge, from, to, badge_span),
            None => edge,
        };
        let from_ref = &self.element_refs[from];
        let to_ref = &self.element_refs[to];
        self.builder.add_edge(from_ref, to_ref, edge);
    }

    /// Draw `edge` as a badge when both its ends are leaves, recording the first end that has children otherwise.
    fn badge_between_leaves(
        &mut self,
        edge: Edge,
        from: &ElementPath,
        to: &ElementPath,
        badge_span: Span,
    ) -> Edge {
        let mut ends = [from, to].into_iter();
        let container_end = ends.find(|end| self.container_paths.contains(*end));
        let Some(container_end) = container_end else {
            return edge.with_badge();
        };
        let message =
            format!("a badge edge needs a leaf at both ends; `{container_end}` has children");
        self.diagnostics.record_error(message, badge_span);
        edge
    }

    /// Lower the legend's title, free lines and key rows into the model's legend.
    fn lower_legend(&mut self, section: LegendSection) -> Legend {
        let LegendSection { title, items } = section;
        let mut legend = Legend::new();
        if let Some(title) = title {
            legend = legend.with_title(title);
        }
        for item in items {
            legend = match item {
                LegendItem::Line(line) => legend.with_entry(line),
                LegendItem::KeyRow(key_row) => self.lower_key_row(legend, key_row),
            };
        }
        legend
    }

    /// Describe the edges a key row's `line` draws: a dash length, or the badges of its `target`; checked only when every property read.
    fn lower_key_row(&mut self, legend: Legend, key_row: LegendKeyRow) -> Legend {
        let LegendKeyRow {
            entries,
            description,
            span,
        } = key_row;
        let properties = self.styles.expand(&entries, &mut self.diagnostics);
        let mut line = None;
        let mut target = None;
        let mut has_unreadable_property = false;
        for property in properties {
            let Property { value, span } = property;
            match value {
                PropertyValue::Line(line_style) => line = Some(line_style),
                PropertyValue::Target(reference) => target = Some(reference),
                PropertyValue::Unreadable(_unreadable_key) => has_unreadable_property = true,
                key_row_inapplicable @ (PropertyValue::Caption(..)
                | PropertyValue::Color(..)
                | PropertyValue::Stroke(..)
                | PropertyValue::Shape(..)
                | PropertyValue::Height(..)
                | PropertyValue::Position(..)
                | PropertyValue::Layout(..)
                | PropertyValue::Columns(..)
                | PropertyValue::Link(..)
                | PropertyValue::Traveler(..)
                | PropertyValue::Meta { .. }) => {
                    let key = key_row_inapplicable.key();
                    let message = format!("`{key}` does not apply to a legend key row");
                    self.diagnostics.record_error(message, span);
                }
            }
        }
        if has_unreadable_property {
            return legend;
        }
        let is_badge = line == Some(LineStyle::Badge);
        let stroke_dash = line.and_then(LineStyle::stroke_dash);
        match (is_badge, stroke_dash, target) {
            (true, _stroke_dash, Some(target)) => {
                let Some(target_path) = self.references.resolve(&target, &mut self.diagnostics)
                else {
                    return legend;
                };
                legend.with_badge_description(target_path, description)
            }
            (true, _stroke_dash, None) => {
                let message = "a badge key row names its badges' target: `target=COMPONENT`";
                self.diagnostics.record_error(message, span);
                legend
            }
            (false, _stroke_dash, Some(target)) => {
                self.diagnostics
                    .record_error("`target` needs `line=badge`", target.span);
                legend
            }
            (false, Some(stroke_dash), None) => {
                legend.with_dash_description(stroke_dash, description)
            }
            (false, None, None) => {
                let message = "a key row describes `line=dashed`, `line=dotted` or `line=badge`";
                self.diagnostics.record_error(message, span);
                legend
            }
        }
    }

    /// Finish into the import: build the model through its admission, the legend given, and sort the errors by source offset.
    fn into_import(self, legend: Option<Legend>) -> MarkdownImport {
        let Self {
            builder,
            diagnostics,
            ..
        } = self;
        let builder = match legend {
            Some(legend) => builder.with_legend(legend),
            None => builder,
        };
        let admission = builder.build();
        let (errors, warnings) = diagnostics.into_sorted_parts();
        MarkdownImport {
            admission,
            errors,
            warnings,
        }
    }
}
