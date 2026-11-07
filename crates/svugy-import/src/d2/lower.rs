//! Lower a d2 document into the model, merging repeated keys, declaring every endpoint and warning on what the model cannot hold.

use std::collections::BTreeMap;
use std::iter;

use miette::Diagnostic;
use svugy_model::builder::{ElementRef, ModelBuilder};
use svugy_model::legend::{DashKey, Legend};
use svugy_model::model::{ByteRange, Edge, Element, Model, PlacementHint, Position, Shape};
use svugy_model::path::ElementPath;
use svugy_model::validate::{ModelAdmission, ModelReport};

use super::D2Parse;
use super::ast::{
    EdgeDeclaration, EdgeLink, EdgeOperator, Field, FieldAssignment, KeyPath, KeySegment, Scalar,
    ShapeDeclaration, Span, Statement, SvugyProperty, SvugyValue,
};
use super::diagnostic::{
    Construct, Diagnostics, LabelledDiagnostic, SyntaxError, UnsupportedConstruct,
};

/// Name the one d2 direction svugy's layered placement follows, its ranks advancing along plan x.
const LAYERED_DIRECTION: &str = "right";

/// Map each d2 `shape` value the model holds onto its primitive; `None` keeps the renderer's default.
const MAPPED_D2_SHAPES: [(&str, Option<Shape>); 5] = [
    ("rectangle", None),
    ("square", None),
    ("cylinder", Some(Shape::Cylinder)),
    ("circle", Some(Shape::Disc)),
    ("oval", Some(Shape::Disc)),
];

/// Name the d2 shapes that draw a diagram of their own instead of one node (d2target at v0.9.0).
const SPECIAL_D2_SHAPES: [&str; 4] = ["sql_table", "sequence_diagram", "class", "hierarchy"];

/// Map each primitive the carrier's `svugy.shape` names, spelled as in the model's JSON, onto its shape.
const SVUGY_PRIMITIVES: [(&str, Shape); 4] = [
    ("box", Shape::Box),
    ("slab", Shape::Slab),
    ("cylinder", Shape::Cylinder),
    ("disc", Shape::Disc),
];

/// Address the root carrier's one list attribute, the legend's lines.
const LEGEND_ENTRIES_PATH: [&str; 2] = ["legend", "entries"];

/// Collect what importing a d2 file yields: the model's admission, every syntax error, every construct the model does not hold.
#[derive(Debug)]
pub struct D2Import {
    admission: ModelAdmission,
    errors: Vec<SyntaxError>,
    warnings: Vec<UnsupportedConstruct>,
}

/// Lower each statement into drafts of the diagram's title, subtitle, legend, elements and edges, recording every diagnostic on the way.
struct Lowering {
    title: Option<String>,
    subtitle: Option<String>,
    legend: Option<LegendDraft>,
    elements: ElementDrafts,
    edges: Vec<EdgeDraft>,
    diagnostics: Diagnostics,
}

/// Draft every declared element under its case-folded key, declaring each path the way d2 does.
#[derive(Default)]
struct ElementDrafts {
    drafts: BTreeMap<ElementPath, ElementDraft>,
}

/// Merge every d2 declaration of one element, the last write winning as in d2.
struct ElementDraft {
    name: String,
    element: Element,
    d2_shape: Option<Shape>,
    svugy_shape: Option<Shape>,
    is_badge: bool,
}

/// Merge the root carrier's legend attributes, the last write winning as in d2; badge keys wait as written until every shape is declared.
#[derive(Default)]
struct LegendDraft {
    title: Option<String>,
    entries: Vec<String>,
    dashes: Vec<(DashKey, String)>,
    badges: Vec<BadgeDescriptionDraft>,
}

/// Describe the badges of the shape declared under `target_key`, which may be declared after the legend.
struct BadgeDescriptionDraft {
    target_key: ElementPath,
    description: String,
    span: Span,
}

/// Connect two element keys with an edge already carrying its label, style and span.
struct EdgeDraft {
    from_key: ElementPath,
    to_key: ElementPath,
    edge: Edge,
}

impl D2Parse {
    /// Lower the parsed document into a model, keeping its diagnostics and adding those of lowering and of the model's checks.
    pub fn lower(self) -> D2Import {
        let D2Parse {
            document,
            errors,
            warnings,
        } = self;
        let diagnostics = Diagnostics::from_parts(errors, warnings);
        let mut lowering = Lowering::new(diagnostics);
        let root_key = ElementPath::root();
        lowering.lower_statements(&root_key, document.statements);
        lowering.into_import()
    }
}

impl Lowering {
    /// Create a new lowering of no statement yet, carrying the parse's `diagnostics` on.
    fn new(diagnostics: Diagnostics) -> Self {
        let elements = ElementDrafts::default();
        let edges = Vec::new();
        Self {
            title: None,
            subtitle: None,
            legend: None,
            elements,
            edges,
            diagnostics,
        }
    }

    /// Lower each statement of `scope_key`'s body, in declaration order.
    fn lower_statements(&mut self, scope_key: &ElementPath, statements: Vec<Statement>) {
        for statement in statements {
            match statement {
                Statement::Shape(shape) => self.lower_shape(scope_key, shape),
                Statement::Edge(edge) => self.lower_edge(scope_key, edge),
                Statement::Field(assignment) => self.lower_field_assignment(scope_key, assignment),
            }
        }
    }

    /// Lower a shape declaration into its element's draft, labelled when it has a label, then lower its body inside it.
    fn lower_shape(&mut self, scope_key: &ElementPath, shape: ShapeDeclaration) {
        let ShapeDeclaration {
            path,
            label,
            body,
            span: _declaration_span,
        } = shape;
        let Some(element_key) = self
            .elements
            .declare_path(scope_key, &path, &mut self.diagnostics)
        else {
            return;
        };
        if let Some(label) = label {
            self.elements
                .edit_element(&element_key, |element| element.relabeled(label.text));
        }
        self.lower_statements(&element_key, body);
    }

    /// Add one edge per link of a chain, each spanning its two endpoints and sharing the chain's label and fields.
    fn lower_edge(&mut self, scope_key: &ElementPath, edge: EdgeDeclaration) {
        let EdgeDeclaration {
            source,
            links,
            label,
            fields,
            span: _declaration_span,
        } = edge;
        let shared_edge = self.build_shared_edge_or_warn(label, fields);
        let Some(mut tail_key) =
            self.elements
                .declare_path(scope_key, &source, &mut self.diagnostics)
        else {
            return;
        };
        let mut tail_start = source.span.start;
        for link in links {
            let EdgeLink {
                operator,
                operator_span,
                target,
            } = link;
            let Some(head_key) =
                self.elements
                    .declare_path(scope_key, &target, &mut self.diagnostics)
            else {
                return;
            };
            let link_head_key = head_key.clone();
            let (from_key, to_key) =
                self.orient_or_warn(operator, operator_span, tail_key, link_head_key);
            let link_span = Span::new(tail_start, target.span.end);
            let link_range = ByteRange::from(link_span);
            let edge = shared_edge.clone().with_span(link_range);
            let edge_draft = EdgeDraft {
                from_key,
                to_key,
                edge,
            };
            self.edges.push(edge_draft);
            tail_key = head_key;
            tail_start = target.span.start;
        }
    }

    /// Build the edge every link of a chain shares from its label and its map's fields, warning on fields edges cannot hold.
    fn build_shared_edge_or_warn(&mut self, label: Option<Scalar>, fields: Vec<Field>) -> Edge {
        let mut edge = Edge::new();
        if let Some(label) = label {
            edge = edge.with_label(label.text);
        }
        for field in fields {
            edge = self.apply_edge_field_or_warn(edge, field);
        }
        edge
    }

    /// Apply one field of an edge's map, warning on it unless it is a label or a style, all the model's edges hold.
    fn apply_edge_field_or_warn(&mut self, edge: Edge, field: Field) -> Edge {
        let keyword_name = field.keyword().name();
        match field {
            Field::Label(label) => edge.with_label(label.text),
            Field::Style(properties) => properties.into_iter().fold(edge, |edge, property| {
                edge.with_style(property.name.name, property.value.text)
            }),
            Field::Shape(value) | Field::Direction(value) | Field::Link(value) => {
                let construct = Construct::FieldOnEdge(keyword_name);
                self.diagnostics.record_warning(construct, value.span);
                edge
            }
            Field::Svugy(properties) => {
                for property in properties {
                    let construct = Construct::FieldOnEdge(keyword_name);
                    self.diagnostics.record_warning(construct, property.span);
                }
                edge
            }
        }
    }

    /// Orient a link's endpoints as the model's from and to, warning on the arrowheads of `<->` and `--` the model cannot draw.
    fn orient_or_warn(
        &mut self,
        operator: EdgeOperator,
        operator_span: Span,
        tail_key: ElementPath,
        head_key: ElementPath,
    ) -> (ElementPath, ElementPath) {
        match operator {
            EdgeOperator::Forward => (tail_key, head_key),
            EdgeOperator::Backward => (head_key, tail_key),
            EdgeOperator::Bidirectional | EdgeOperator::Undirected => {
                let construct = Construct::ArrowheadForm(operator);
                self.diagnostics.record_warning(construct, operator_span);
                (tail_key, head_key)
            }
        }
    }

    /// Apply a field to its target element, or to the diagram when the target is the root.
    fn lower_field_assignment(&mut self, scope_key: &ElementPath, assignment: FieldAssignment) {
        let FieldAssignment {
            target,
            field,
            span,
        } = assignment;
        let Some(target_key) =
            self.elements
                .declare_path(scope_key, &target, &mut self.diagnostics)
        else {
            return;
        };
        let is_root = target_key.is_root();
        if is_root {
            self.lower_root_field(field, span);
            return;
        }
        match field {
            Field::Direction(direction) => self.lower_direction_field(&target_key, direction),
            Field::Shape(shape) => self.lower_shape_field(&target_key, shape),
            Field::Label(label) => {
                self.elements
                    .edit_element(&target_key, |element| element.relabeled(label.text));
            }
            Field::Link(link) => {
                let link_range = ByteRange::from(link.span);
                self.elements.edit_element(&target_key, |element| {
                    element.with_link(link.text).with_link_span(link_range)
                });
            }
            Field::Style(properties) => self.elements.edit_element(&target_key, |element| {
                properties.into_iter().fold(element, |element, property| {
                    element.with_style(property.name.name, property.value.text)
                })
            }),
            Field::Svugy(properties) => self.lower_svugy_field(&target_key, properties, span),
        }
    }

    /// Title the diagram by the root's `label`, as d2 titles a board's PDF page, subtitle it by the root's carrier, keep its default direction, and warn on every other field.
    fn lower_root_field(&mut self, field: Field, span: Span) {
        let root_key = ElementPath::root();
        let keyword_name = field.keyword().name();
        match field {
            Field::Label(label) => self.title = Some(label.text),
            Field::Svugy(properties) => {
                for property in properties {
                    self.lower_root_svugy_property(property);
                }
            }
            Field::Direction(direction) => self.lower_direction_field(&root_key, direction),
            _element_only_field => {
                let construct = Construct::FieldOnRoot(keyword_name);
                self.diagnostics.record_warning(construct, span);
            }
        }
    }

    /// Apply one attribute of the root carrier, its subtitle or its legend, rejecting every other attribute: the title is the root's own `label`.
    fn lower_root_svugy_property(&mut self, property: SvugyProperty) {
        let SvugyProperty {
            attribute_path,
            value,
            span: property_span,
        } = property;
        let scalar = match value {
            SvugyValue::Scalar(scalar) => scalar,
            SvugyValue::List { items, span } => {
                self.lower_root_svugy_list(&attribute_path, items, span);
                return;
            }
        };
        let attribute_segments: Vec<&str> = attribute_path.iter().map(String::as_str).collect();
        match attribute_segments.as_slice() {
            ["subtitle"] => self.subtitle = Some(scalar.text),
            ["legend", "title"] => self.legend_mut().retitle(scalar.text),
            entries_path if entries_path == LEGEND_ENTRIES_PATH => {
                let entries_carrier_path = SvugyProperty::spell_carrier_path(&LEGEND_ENTRIES_PATH);
                let message = format!(
                    "`{entries_carrier_path}` is a list of lines, e.g. `[\"① …\"; \"② …\"]`"
                );
                self.diagnostics.record_error(message, scalar.span);
            }
            ["legend", "dashes"] | ["legend", "badges"] => {
                self.reject_root_svugy_attribute(&attribute_path, property_span);
            }
            ["legend", "dashes", dash_key_path @ ..] => {
                self.lower_legend_dash(dash_key_path, scalar, property_span);
            }
            ["legend", "badges", target_path @ ..] => {
                self.lower_legend_badge(target_path, scalar, property_span);
            }
            _unknown_attribute => self.reject_root_svugy_attribute(&attribute_path, property_span),
        }
    }

    /// Set the legend's entries from the one list attribute, the latest list replacing any earlier one as in d2; any other list is an error.
    fn lower_root_svugy_list(&mut self, attribute_path: &[String], items: Vec<Scalar>, span: Span) {
        let is_entries = attribute_path == LEGEND_ENTRIES_PATH;
        if is_entries {
            let entries = items.into_iter().map(|item| item.text).collect();
            self.legend_mut().replace_entries(entries);
            return;
        }
        self.reject_list(attribute_path, span);
    }

    /// Reject an attribute the root carrier does not hold.
    fn reject_root_svugy_attribute(&mut self, attribute_path: &[String], span: Span) {
        let carrier_path = SvugyProperty::spell_carrier_path(attribute_path);
        let message = format!(
            "`{carrier_path}` is not a svugy attribute of the root: expected `svugy.subtitle` or `svugy.legend` with `title`, `entries`, `dashes` or `badges`, and the title is the root's own `label`"
        );
        self.diagnostics.record_error(message, span);
    }

    /// Add the legend's description of the arcs dashed as long as `dash_key_path` spells, rejecting a key that is not a plain number.
    fn lower_legend_dash(&mut self, dash_key_path: &[&str], description: Scalar, span: Span) {
        let dash_key_segments = dash_key_path.iter().copied();
        let dash_key = ElementPath::from_segments(dash_key_segments).to_string();
        let Ok(parsed_dash_key) = dash_key.parse::<DashKey>() else {
            let message = format!(
                "`{dash_key}` is not a dash length: a `svugy.legend.dashes` key is a plain number like `5`, an arc's `stroke-dash`"
            );
            self.diagnostics.record_error(message, span);
            return;
        };
        self.legend_mut()
            .describe_dash(parsed_dash_key, description.text);
    }

    /// Add the legend's badge description of the shape `target_path` names, resolved once every shape is declared.
    fn lower_legend_badge(&mut self, target_path: &[&str], description: Scalar, span: Span) {
        let target_key = ElementDrafts::key_of(target_path);
        let badge_description = BadgeDescriptionDraft {
            target_key,
            description: description.text,
            span,
        };
        self.legend_mut().describe_badges(badge_description);
    }

    /// Reject a list given to an attribute that takes one value.
    fn reject_list(&mut self, attribute_path: &[String], span: Span) {
        let carrier_path = SvugyProperty::spell_carrier_path(attribute_path);
        let entries_carrier_path = SvugyProperty::spell_carrier_path(&LEGEND_ENTRIES_PATH);
        let message = format!(
            "`{carrier_path}` is not a svugy list attribute: a list carries only the root's `{entries_carrier_path}`"
        );
        self.diagnostics.record_error(message, span);
    }

    /// Map `direction: right` onto layered placement, which the root already has; svugy's placement follows no other direction.
    fn lower_direction_field(&mut self, element_key: &ElementPath, direction: Scalar) {
        let is_diff_layered_direction = direction.text != LAYERED_DIRECTION;
        if is_diff_layered_direction {
            let construct = Construct::UnmappedDirection(direction.text);
            self.diagnostics.record_warning(construct, direction.span);
            return;
        }
        let is_root = element_key.is_root();
        if is_root {
            return;
        }
        self.elements.edit_element(element_key, |element| {
            element.with_child_placement(PlacementHint::Layered)
        });
    }

    /// Map a d2 shape onto a primitive through `MAPPED_D2_SHAPES`, warning on special and unmapped shapes.
    fn lower_shape_field(&mut self, element_key: &ElementPath, shape: Scalar) {
        let lowercase_shape = shape.text.to_ascii_lowercase();
        let lowercase_shape_name = lowercase_shape.as_str();
        let is_special_shape = SPECIAL_D2_SHAPES.contains(&lowercase_shape_name);
        if is_special_shape {
            let construct = Construct::SpecialShape(shape.text);
            self.diagnostics.record_warning(construct, shape.span);
            return;
        }
        let mut mapped_shapes = MAPPED_D2_SHAPES.iter();
        let mapped_shape =
            mapped_shapes.find(|(d2_shape, _primitive)| *d2_shape == lowercase_shape);
        let Some((_d2_shape, primitive)) = mapped_shape else {
            let construct = Construct::UnmappedShape(shape.text);
            self.diagnostics.record_warning(construct, shape.span);
            return;
        };
        self.elements
            .draft_mut(element_key)
            .reshape_by_d2(*primitive);
    }

    /// Lower the `svugy` carrier field: apply its caption, primitive, height, badge mark and position, rejecting unknown attributes, lists and malformed values one by one.
    fn lower_svugy_field(
        &mut self,
        element_key: &ElementPath,
        properties: Vec<SvugyProperty>,
        span: Span,
    ) {
        let mut position_x = None;
        let mut position_y = None;
        for property in properties {
            let SvugyProperty {
                attribute_path,
                value,
                span: property_span,
            } = property;
            let value = match value {
                SvugyValue::Scalar(scalar) => scalar,
                SvugyValue::List {
                    items: _items,
                    span: list_span,
                } => {
                    self.reject_list(&attribute_path, list_span);
                    continue;
                }
            };
            let attribute_segments: Vec<&str> = attribute_path.iter().map(String::as_str).collect();
            match attribute_segments.as_slice() {
                ["caption"] => {
                    self.elements
                        .edit_element(element_key, |element| element.with_caption(value.text));
                }
                ["shape"] => self.lower_svugy_shape(element_key, value),
                ["height"] => self.lower_svugy_height(element_key, value),
                ["badge"] => self.lower_svugy_badge(element_key, value),
                ["position", "x"] => position_x = Some(value),
                ["position", "y"] => position_y = Some(value),
                _unknown_attribute => {
                    self.reject_element_svugy_attribute(&attribute_path, property_span);
                }
            }
        }
        self.lower_svugy_position(element_key, position_x, position_y, span);
    }

    /// Map the carrier's primitive name onto a shape through `SVUGY_PRIMITIVES`; it outranks d2's `shape`, which d2 itself still reads.
    fn lower_svugy_shape(&mut self, element_key: &ElementPath, primitive: Scalar) {
        let lowercase_primitive = primitive.text.to_ascii_lowercase();
        let mut known_primitives = SVUGY_PRIMITIVES.iter();
        let known_primitive = known_primitives
            .find(|(primitive_name, _shape)| *primitive_name == lowercase_primitive);
        let Some((_primitive_name, shape)) = known_primitive else {
            let message = format!(
                "`{}` is not a svugy primitive: expected `box`, `slab`, `cylinder` or `disc`",
                primitive.text
            );
            self.diagnostics.record_error(message, primitive.span);
            return;
        };
        self.elements
            .draft_mut(element_key)
            .reshape_by_carrier(*shape);
    }

    /// Stand the element as tall as the carrier's `height`; validation bounds the number.
    fn lower_svugy_height(&mut self, element_key: &ElementPath, height: Scalar) {
        let Some(height) = self.parse_decimal_or_reject(height, "`svugy.height`") else {
            return;
        };
        self.elements
            .edit_element(element_key, |element| element.with_height(height));
    }

    /// Mark or unmark the element a badge target, the latest mark winning as in d2; a value other than `true` or `false` is rejected.
    fn lower_svugy_badge(&mut self, element_key: &ElementPath, mark: Scalar) {
        let lowercase_mark = mark.text.to_ascii_lowercase();
        let is_badge = match lowercase_mark.as_str() {
            "true" => true,
            "false" => false,
            _not_a_boolean => {
                let message = format!(
                    "`{}` is not a boolean: `svugy.badge` is `true` or `false`",
                    mark.text
                );
                self.diagnostics.record_error(message, mark.span);
                return;
            }
        };
        self.elements
            .draft_mut(element_key)
            .mark_badge_target(is_badge);
    }

    /// Reject an attribute an element's carrier does not hold.
    fn reject_element_svugy_attribute(&mut self, attribute_path: &[String], span: Span) {
        let carrier_path = SvugyProperty::spell_carrier_path(attribute_path);
        let message = format!(
            "`{carrier_path}` is not a svugy attribute: expected `svugy.caption`, `svugy.shape`, `svugy.height`, `svugy.badge` or `svugy.position` with `x` and `y`"
        );
        self.diagnostics.record_error(message, span);
    }

    /// Pin the element at the carrier's `x` and `y`, written together in one `vars` map so a position is never half-set.
    fn lower_svugy_position(
        &mut self,
        element_key: &ElementPath,
        x: Option<Scalar>,
        y: Option<Scalar>,
        span: Span,
    ) {
        let is_x_absent = x.is_none();
        let is_y_absent = y.is_none();
        let is_position_absent = is_x_absent && is_y_absent;
        if is_position_absent {
            return;
        }
        let (Some(x), Some(y)) = (x, y) else {
            let message = "`svugy.position` needs both `x` and `y` in the same `vars` map";
            self.diagnostics.record_error(message, span);
            return;
        };
        let coordinate_attribute = "a `svugy.position` coordinate";
        let Some(x) = self.parse_decimal_or_reject(x, coordinate_attribute) else {
            return;
        };
        let Some(y) = self.parse_decimal_or_reject(y, coordinate_attribute) else {
            return;
        };
        let position = Position { x, y };
        self.elements
            .edit_element(element_key, |element| element.with_position(position));
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

    /// Finish into the import: build the model through its admission, its legend's badge keys resolved first, and sort the diagnostics by source offset.
    fn into_import(self) -> D2Import {
        let Self {
            title,
            subtitle,
            legend,
            elements,
            edges,
            mut diagnostics,
        } = self;
        let mut builder = ModelBuilder::new();
        if let Some(title) = title {
            builder = builder.with_title(title);
        }
        if let Some(subtitle) = subtitle {
            builder = builder.with_subtitle(subtitle);
        }
        if let Some(legend_draft) = legend {
            let legend = legend_draft.resolve(&elements, &mut diagnostics);
            builder = builder.with_legend(legend);
        }
        let admission = Self::build_model(builder, elements, edges);
        diagnostics.sort_by_source_offset();
        let (errors, warnings) = diagnostics.into_parts();
        D2Import {
            admission,
            errors,
            warnings,
        }
    }

    /// Hand the drafts to `builder`, parents before children, then the edges in declaration order, and return its admission.
    fn build_model(
        mut builder: ModelBuilder,
        elements: ElementDrafts,
        edges: Vec<EdgeDraft>,
    ) -> ModelAdmission {
        let element_refs = elements.add_to(&mut builder);
        for edge_draft in edges {
            let EdgeDraft {
                from_key,
                to_key,
                edge,
            } = edge_draft;
            let from_ref = &element_refs[&from_key];
            let to_ref = &element_refs[&to_key];
            builder.add_edge(from_ref, to_ref, edge);
        }
        builder.build()
    }

    /// Return the legend's draft, drafting an empty one on first use.
    fn legend_mut(&mut self) -> &mut LegendDraft {
        self.legend.get_or_insert_with(LegendDraft::default)
    }
}

impl ElementDrafts {
    /// Return the key of `path` from `scope_key`, one scope up per leading `_`, declaring each missing element on the way as d2 does.
    fn declare_path(
        &mut self,
        scope_key: &ElementPath,
        path: &KeyPath,
        diagnostics: &mut Diagnostics,
    ) -> Option<ElementPath> {
        let mut element_key = scope_key.clone();
        for _parent_reference in 0..path.parent_references {
            let Some(parent_key) = element_key.parent() else {
                let message = "`_` climbs above the root, which has no parent scope";
                diagnostics.record_error(message, path.span);
                return None;
            };
            element_key = parent_key;
        }
        for segment in &path.segments {
            let prefix_span = Span::new(path.span.start, segment.span.end);
            element_key = self.declare_child(&element_key, segment, prefix_span);
        }
        Some(element_key)
    }

    /// Return the key of `segment` under `parent_key`, declared at `span` when new.
    fn declare_child(
        &mut self,
        parent_key: &ElementPath,
        segment: &KeySegment,
        span: Span,
    ) -> ElementPath {
        let element_key = Self::child_key(parent_key, &segment.name);
        let drafted_key = element_key.clone();
        self.drafts.entry(drafted_key).or_insert_with(|| {
            let name = segment.name.clone();
            ElementDraft::new(name, span)
        });
        element_key
    }

    /// Key the element `name` under `parent_key` case-insensitively, as d2 matches keys.
    fn child_key(parent_key: &ElementPath, name: &str) -> ElementPath {
        let folded_name = name.to_lowercase();
        parent_key.child(folded_name)
    }

    /// Key the element at `path` from the root case-insensitively, as d2 matches keys.
    fn key_of(path: &[&str]) -> ElementPath {
        let root_key = ElementPath::root();
        path.iter().fold(root_key, |parent_key, name| {
            Self::child_key(&parent_key, name)
        })
    }

    /// Replace the drafted element by `edit` of it, as the model's builders consume the element they extend.
    fn edit_element(&mut self, element_key: &ElementPath, edit: impl FnOnce(Element) -> Element) {
        let draft = self.draft_mut(element_key);
        let element = draft.element.clone();
        draft.element = edit(element);
    }

    /// Return the model path of the shape declared under `element_key`, each segment spelled as its first declaration spelled it.
    fn declared_path_of(&self, element_key: &ElementPath) -> Option<ElementPath> {
        let ancestor_keys = element_key.ancestors();
        let own_key = element_key.clone();
        let own_key_once = iter::once(own_key);
        let element_and_ancestor_keys = ancestor_keys.chain(own_key_once);
        let declared_names: Option<Vec<String>> = element_and_ancestor_keys
            .map(|key| {
                let draft = self.drafts.get(&key)?;
                let declared_name = draft.name.clone();
                Some(declared_name)
            })
            .collect();
        let declared_names = declared_names?;
        let declared_path = ElementPath::from_segments(declared_names);
        Some(declared_path)
    }

    /// Add every drafted element to `builder`, parents before children, and return each one's ref by its key.
    fn add_to(self, builder: &mut ModelBuilder) -> BTreeMap<ElementPath, ElementRef> {
        let mut element_refs: BTreeMap<ElementPath, ElementRef> = BTreeMap::new();
        for (element_key, draft) in self.drafts {
            let parent_key = element_key.parent();
            let parent_ref = parent_key.and_then(|parent_key| element_refs.get(&parent_key));
            let (name, element) = draft.into_named_element();
            let element_ref = match parent_ref {
                Some(parent_ref) => builder.add_child(parent_ref, &name, element),
                None => builder.add_top_level(&name, element),
            };
            element_refs.insert(element_key, element_ref);
        }
        element_refs
    }

    /// Return the draft of the element `declare_path` declared under `element_key`.
    fn draft_mut(&mut self, element_key: &ElementPath) -> &mut ElementDraft {
        self.drafts
            .get_mut(element_key)
            .expect("declare_path declares every element it returns")
    }
}

impl LegendDraft {
    /// Retitle the legend, the latest title winning as in d2.
    fn retitle(&mut self, title: String) {
        self.title = Some(title);
    }

    /// Replace the legend's entries by `entries`, the latest list winning as in d2.
    fn replace_entries(&mut self, entries: Vec<String>) {
        self.entries = entries;
    }

    /// Describe the arcs dashed `dash_key`, after every earlier dash description.
    fn describe_dash(&mut self, dash_key: DashKey, description: String) {
        let dash_description = (dash_key, description);
        self.dashes.push(dash_description);
    }

    /// Describe the badges of a shape whose key waits as written until every shape is declared.
    fn describe_badges(&mut self, badge_description: BadgeDescriptionDraft) {
        self.badges.push(badge_description);
    }

    /// Resolve the draft into the model's legend, each badge key to the path its shape is declared under; a key naming no shape is an error dropping only its description.
    fn resolve(self, elements: &ElementDrafts, diagnostics: &mut Diagnostics) -> Legend {
        let Self {
            title,
            entries,
            dashes,
            badges,
        } = self;
        let mut legend = Legend::new();
        if let Some(title) = title {
            legend = legend.with_title(title);
        }
        legend = entries.into_iter().fold(legend, Legend::with_entry);
        for (dash_key, description) in dashes {
            legend = legend.with_dash_description(dash_key, description);
        }
        for badge_description in badges {
            let BadgeDescriptionDraft {
                target_key,
                description,
                span,
            } = badge_description;
            let Some(target) = elements.declared_path_of(&target_key) else {
                let message = format!(
                    "`{target_key}` names no shape: a `svugy.legend.badges` key is the path of a badge target, e.g. `pipeline.inter`"
                );
                diagnostics.record_error(message, span);
                continue;
            };
            legend = legend.with_badge_description(target, description);
        }
        legend
    }
}

impl ElementDraft {
    /// Draft an element labelled by its own name, declared at `span`.
    fn new(name: String, span: Span) -> Self {
        let source_range = ByteRange::from(span);
        let label = name.clone();
        let element = Element::new(label).with_span(source_range);
        Self {
            name,
            element,
            d2_shape: None,
            svugy_shape: None,
            is_badge: false,
        }
    }

    /// Reshape the element by d2's `shape`, `None` restoring the renderer's default as d2's `rectangle` does; the latest d2 shape wins.
    fn reshape_by_d2(&mut self, shape: Option<Shape>) {
        self.d2_shape = shape;
    }

    /// Reshape the element by the carrier's primitive, which outranks any d2 shape; the latest primitive wins.
    fn reshape_by_carrier(&mut self, shape: Shape) {
        self.svugy_shape = Some(shape);
    }

    /// Mark or unmark the element a badge target, the latest mark winning as in d2.
    fn mark_badge_target(&mut self, is_badge: bool) {
        self.is_badge = is_badge;
    }

    /// Split the draft into its key name and its element, marked badge by its last mark, shaped by the carrier first and d2 second, kept apart until now as d2's `rectangle` resets its shape.
    fn into_named_element(self) -> (String, Element) {
        let Self {
            name,
            element,
            d2_shape,
            svugy_shape,
            is_badge,
        } = self;
        let marked_element = if is_badge {
            element.with_badge_target()
        } else {
            element
        };
        let shape = svugy_shape.or(d2_shape);
        let Some(shape) = shape else {
            return (name, marked_element);
        };
        let shaped_element = marked_element.with_shape(shape);
        (name, shaped_element)
    }
}

impl D2Import {
    /// List every diagnostic in source order: errors, ignored constructs and model problems alike, unlabelled ones last.
    pub fn diagnostics(&self) -> Vec<&dyn Diagnostic> {
        let errors = self.errors.iter().map(|error| error as &dyn Diagnostic);
        let warnings = self
            .warnings
            .iter()
            .map(|warning| warning as &dyn Diagnostic);
        let problems = self
            .admission
            .report
            .problems()
            .iter()
            .map(|problem| problem as &dyn Diagnostic);
        let mut diagnostics: Vec<&dyn Diagnostic> =
            errors.chain(warnings).chain(problems).collect();
        diagnostics.sort_by_key(|diagnostic| {
            let first_label_offset = diagnostic.first_label_offset();
            first_label_offset.unwrap_or(usize::MAX)
        });
        diagnostics
    }

    /// Return the admitted model only when no diagnostic is an error, so a diagram the author has yet to fix is never drawn.
    pub fn admitted_model(&self) -> Option<&Model> {
        let has_errors = self.has_errors();
        if has_errors {
            return None;
        }
        self.admission.model.as_ref()
    }

    /// Split the import into its model, withheld as `admitted_model` withholds it, its syntax errors, its ignored constructs and the model's report.
    pub fn into_parts(
        self,
    ) -> (
        Option<Model>,
        Vec<SyntaxError>,
        Vec<UnsupportedConstruct>,
        ModelReport,
    ) {
        let has_errors = self.has_errors();
        let Self {
            admission,
            errors,
            warnings,
        } = self;
        let ModelAdmission { model, report } = admission;
        let admitted_model = if has_errors { None } else { model };
        (admitted_model, errors, warnings, report)
    }

    /// Tell whether any diagnostic is an error: a d2 syntax error or an error of the model's checks.
    fn has_errors(&self) -> bool {
        let has_syntax_errors = !self.errors.is_empty();
        let has_model_errors = self.admission.report.has_errors();
        has_syntax_errors || has_model_errors
    }

    /// Hand over the model's admission, its model and its full report, whatever the import's own errors.
    pub fn into_admission(self) -> ModelAdmission {
        self.admission
    }

    /// Return every syntax error of parsing and lowering, in source order.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    /// Return every warning: each ignored construct of parsing and lowering, in source order.
    pub fn warnings(&self) -> &[UnsupportedConstruct] {
        &self.warnings
    }
}
