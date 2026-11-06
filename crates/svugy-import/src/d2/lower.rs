//! Lower a d2 document into the model, merging repeated keys, declaring every endpoint and warning on what the model cannot hold.

use std::collections::BTreeMap;
use std::iter;

use miette::Diagnostic;
use svugy_model::builder::{ElementRef, ModelBuilder};
use svugy_model::legend::Legend;
use svugy_model::model::{ByteRange, Edge, Element, PlacementHint, Position, Shape};
use svugy_model::path::ElementPath;
use svugy_model::style::StyleValueKind;
use svugy_model::validate::ModelAdmission;

use super::D2Parse;
use super::ast::{
    EdgeDeclaration, EdgeLink, EdgeOperator, Field, FieldAssignment, KeyPath, KeySegment, Scalar,
    ShapeDeclaration, Span, Statement, SvugyProperty, SvugyValue,
};
use super::diagnostic::{Construct, SyntaxError, UnsupportedConstruct};

/// The one d2 direction svugy's layered placement follows, its ranks advancing along plan x.
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

/// The root carrier's one list attribute, the legend's lines.
const LEGEND_ENTRIES: &str = "svugy.legend.entries";

/// Prefix of a root carrier attribute describing the arcs of the dash length that follows.
const LEGEND_DASHES_PREFIX: &str = "svugy.legend.dashes.";

/// Prefix of a root carrier attribute describing the badges of the shape whose path follows.
const LEGEND_BADGES_PREFIX: &str = "svugy.legend.badges.";

/// Collect what importing a d2 file yields: the model's admission, every syntax error, every construct the model does not hold.
#[derive(Debug)]
pub struct D2Import {
    pub admission: ModelAdmission,
    pub errors: Vec<SyntaxError>,
    pub warnings: Vec<UnsupportedConstruct>,
}

/// Accumulate the diagram's title, subtitle and legend, element drafts keyed by case-folded path and edges in declaration order, with every diagnostic found on the way.
struct Lowering {
    title: Option<String>,
    subtitle: Option<String>,
    legend: Option<LegendDraft>,
    elements: BTreeMap<ElementPath, ElementDraft>,
    edges: Vec<EdgeDraft>,
    errors: Vec<SyntaxError>,
    warnings: Vec<UnsupportedConstruct>,
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
    dashes: Vec<(f64, String)>,
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
        let mut lowering = Lowering {
            title: None,
            subtitle: None,
            legend: None,
            elements: BTreeMap::new(),
            edges: Vec::new(),
            errors,
            warnings,
        };
        let root_key = ElementPath::root();
        lowering.lower_statements(&root_key, document.statements);
        lowering.into_import()
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
        diagnostics.sort_by_key(|diagnostic| Self::source_offset(*diagnostic));
        diagnostics
    }

    /// Return where a diagnostic's first label starts, or `usize::MAX` so an unlabelled one sorts last.
    fn source_offset(diagnostic: &dyn Diagnostic) -> usize {
        let mut labels = diagnostic.labels().into_iter().flatten();
        let first_label = labels.next();
        first_label.map_or(usize::MAX, |label| label.offset())
    }
}

impl Lowering {
    fn lower_statements(&mut self, scope_key: &ElementPath, statements: Vec<Statement>) {
        for statement in statements {
            match statement {
                Statement::Shape(shape) => self.lower_shape(scope_key, shape),
                Statement::Edge(edge) => self.lower_edge(scope_key, edge),
                Statement::Field(assignment) => self.lower_field_assignment(scope_key, assignment),
            }
        }
    }

    fn lower_shape(&mut self, scope_key: &ElementPath, shape: ShapeDeclaration) {
        let ShapeDeclaration {
            path,
            label,
            body,
            span: _declaration_span,
        } = shape;
        let Some(element_key) = self.declare_path(scope_key, &path) else {
            return;
        };
        if let Some(label) = label {
            self.edit_element(&element_key, |element| element.with_label(label.text));
        }
        self.lower_statements(&element_key, body);
    }

    /// Return the key of `path` from `scope_key`, one scope up per leading `_`, declaring each missing element on the way as d2 does.
    fn declare_path(&mut self, scope_key: &ElementPath, path: &KeyPath) -> Option<ElementPath> {
        let mut element_key = scope_key.clone();
        for _parent_reference in 0..path.parent_references {
            let Some(parent_key) = element_key.parent() else {
                let message = "`_` climbs above the root, which has no parent scope";
                return self.reject(message, path.span);
            };
            element_key = parent_key;
        }
        for segment in &path.segments {
            let prefix_span = Span::new(path.span.start, segment.span.end);
            element_key = self.declare_child(&element_key, segment, prefix_span);
        }
        Some(element_key)
    }

    /// Return the key of `segment` under `parent_key`, matched case-insensitively as d2 does, declared at `span` when new.
    fn declare_child(
        &mut self,
        parent_key: &ElementPath,
        segment: &KeySegment,
        span: Span,
    ) -> ElementPath {
        let folded_name = segment.name.to_lowercase();
        let element_key = parent_key.child(folded_name);
        let is_declared = self.elements.contains_key(&element_key);
        if is_declared {
            return element_key;
        }
        let name = segment.name.clone();
        let draft = ElementDraft::new(name, span);
        self.elements.insert(element_key.clone(), draft);
        element_key
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
        let Some(mut tail_key) = self.declare_path(scope_key, &source) else {
            return;
        };
        let mut tail_start = source.span.start;
        for link in links {
            let EdgeLink {
                operator,
                operator_span,
                target,
            } = link;
            let Some(head_key) = self.declare_path(scope_key, &target) else {
                return;
            };
            let (from_key, to_key) =
                self.orient_or_warn(operator, operator_span, tail_key, head_key.clone());
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
                let construct = Construct::FieldOnEdge(keyword_name.to_owned());
                self.warn(construct, value.span);
                edge
            }
            Field::Svugy(properties) => {
                for property in properties {
                    let construct = Construct::FieldOnEdge(keyword_name.to_owned());
                    self.warn(construct, property.span);
                }
                edge
            }
        }
    }

    /// Order a link's endpoints as the model's from and to, warning on the arrowheads of `<->` and `--` the model cannot draw.
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
                self.warn(construct, operator_span);
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
        let Some(target_key) = self.declare_path(scope_key, &target) else {
            return;
        };
        let is_root = target_key.is_root();
        if is_root {
            return self.lower_root_field(field, span);
        }
        match field {
            Field::Direction(direction) => self.lower_direction_field(&target_key, direction),
            Field::Shape(shape) => self.lower_shape_field(&target_key, shape),
            Field::Label(label) => {
                self.edit_element(&target_key, |element| element.with_label(label.text));
            }
            Field::Link(link) => {
                let link_range = ByteRange::from(link.span);
                self.edit_element(&target_key, |element| {
                    element.with_link(link.text).with_link_span(link_range)
                });
            }
            Field::Style(properties) => self.edit_element(&target_key, |element| {
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
            Field::Svugy(properties) => self.lower_root_svugy_field(properties),
            Field::Direction(direction) => self.lower_direction_field(&root_key, direction),
            _element_only_field => {
                let construct = Construct::FieldOnRoot(keyword_name.to_owned());
                self.warn(construct, span);
            }
        }
    }

    /// Apply the root carrier's subtitle and legend, rejecting every other attribute: the title is the root's own `label`.
    fn lower_root_svugy_field(&mut self, properties: Vec<SvugyProperty>) {
        for property in properties {
            let SvugyProperty {
                name,
                value,
                span: property_span,
            } = property;
            let scalar = match value {
                SvugyValue::Scalar(scalar) => scalar,
                SvugyValue::List { items, span } => {
                    self.lower_root_svugy_list(&name, items, span);
                    continue;
                }
            };
            if let Some(dash_key) = name.strip_prefix(LEGEND_DASHES_PREFIX) {
                self.lower_legend_dash(dash_key, scalar, property_span);
                continue;
            }
            if let Some(badge_key) = name.strip_prefix(LEGEND_BADGES_PREFIX) {
                self.lower_legend_badge(badge_key, scalar, property_span);
                continue;
            }
            match name.as_str() {
                "svugy.subtitle" => self.subtitle = Some(scalar.text),
                "svugy.legend.title" => self.legend_mut().title = Some(scalar.text),
                LEGEND_ENTRIES => {
                    let message =
                        format!("`{LEGEND_ENTRIES}` is a list of lines, e.g. `[\"① …\"; \"② …\"]`");
                    self.error(&message, scalar.span);
                }
                unknown_name => {
                    let message = format!(
                        "`{unknown_name}` is not a svugy attribute of the root: expected `svugy.subtitle` or `svugy.legend` with `title`, `entries`, `dashes` or `badges`, and the title is the root's own `label`"
                    );
                    self.error(&message, property_span);
                }
            }
        }
    }

    /// Set the legend's entries from the one list attribute, the latest list replacing any earlier one as in d2; any other list is an error.
    fn lower_root_svugy_list(&mut self, name: &str, items: Vec<Scalar>, span: Span) {
        let is_entries = name == LEGEND_ENTRIES;
        if is_entries {
            let entries = items.into_iter().map(|item| item.text).collect();
            self.legend_mut().entries = entries;
            return;
        }
        self.reject_list(name, span);
    }

    /// Describe the arcs dashed `dash_key` long, rejecting a key that is not a plain number.
    fn lower_legend_dash(&mut self, dash_key: &str, description: Scalar, span: Span) {
        let Some(stroke_dash) = StyleValueKind::parse_number(dash_key) else {
            let message = format!(
                "`{dash_key}` is not a dash length: a `svugy.legend.dashes` key is a plain number like `5`, an arc's `stroke-dash`"
            );
            return self.error(&message, span);
        };
        let legend = self.legend_mut();
        legend.dashes.push((stroke_dash, description.text));
    }

    /// Describe the badges of the shape `badge_key` names, resolved once every shape is declared; a key that is no path is rejected now.
    fn lower_legend_badge(&mut self, badge_key: &str, description: Scalar, span: Span) {
        let Ok(target) = badge_key.parse::<ElementPath>() else {
            let message = format!(
                "`{badge_key}` is not a shape path: a `svugy.legend.badges` key is the path of a badge target, e.g. `pipeline.inter`"
            );
            return self.error(&message, span);
        };
        let folded_segments = target
            .segments()
            .iter()
            .map(|segment| segment.to_lowercase());
        let target_key = ElementPath::from_segments(folded_segments);
        let badge_description = BadgeDescriptionDraft {
            target_key,
            description: description.text,
            span,
        };
        self.legend_mut().badges.push(badge_description);
    }

    /// Reject a list given to an attribute that takes one value.
    fn reject_list(&mut self, name: &str, span: Span) {
        let message = format!(
            "`{name}` is not a svugy list attribute: a list carries only the root's `{LEGEND_ENTRIES}`"
        );
        self.error(&message, span);
    }

    fn legend_mut(&mut self) -> &mut LegendDraft {
        self.legend.get_or_insert_with(LegendDraft::default)
    }

    /// Map `direction: right` onto layered placement, which the root already has; svugy's placement follows no other direction.
    fn lower_direction_field(&mut self, element_key: &ElementPath, direction: Scalar) {
        let is_diff_layered_direction = direction.text != LAYERED_DIRECTION;
        if is_diff_layered_direction {
            let construct = Construct::UnmappedDirection(direction.text);
            return self.warn(construct, direction.span);
        }
        let is_root = element_key.is_root();
        if is_root {
            return;
        }
        self.edit_element(element_key, |element| {
            element.with_child_placement(PlacementHint::Layered)
        });
    }

    /// Map a d2 shape onto a primitive through `MAPPED_D2_SHAPES`, warning on special and unmapped shapes.
    fn lower_shape_field(&mut self, element_key: &ElementPath, shape: Scalar) {
        let lowercase_shape = shape.text.to_ascii_lowercase();
        let is_special_shape = SPECIAL_D2_SHAPES.contains(&lowercase_shape.as_str());
        if is_special_shape {
            let construct = Construct::SpecialShape(shape.text);
            return self.warn(construct, shape.span);
        }
        let mut mapped_shapes = MAPPED_D2_SHAPES.iter();
        let mapped_shape =
            mapped_shapes.find(|(d2_shape, _primitive)| *d2_shape == lowercase_shape);
        let Some((_d2_shape, primitive)) = mapped_shape else {
            let construct = Construct::UnmappedShape(shape.text);
            return self.warn(construct, shape.span);
        };
        self.draft_mut(element_key).d2_shape = *primitive;
    }

    /// Apply the carrier's caption, primitive, height, badge mark and position, rejecting unknown attributes, lists and malformed values one by one.
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
                name,
                value,
                span: property_span,
            } = property;
            let value = match value {
                SvugyValue::Scalar(scalar) => scalar,
                SvugyValue::List {
                    items: _items,
                    span: list_span,
                } => {
                    self.reject_list(&name, list_span);
                    continue;
                }
            };
            match name.as_str() {
                "svugy.caption" => {
                    self.edit_element(element_key, |element| element.with_caption(value.text));
                }
                "svugy.shape" => self.lower_svugy_shape(element_key, value),
                "svugy.height" => self.lower_svugy_height(element_key, value),
                "svugy.badge" => self.lower_svugy_badge(element_key, value),
                "svugy.position.x" => position_x = Some(value),
                "svugy.position.y" => position_y = Some(value),
                unknown_name => {
                    let message = format!(
                        "`{unknown_name}` is not a svugy attribute: expected `svugy.caption`, `svugy.shape`, `svugy.height`, `svugy.badge` or `svugy.position` with `x` and `y`"
                    );
                    self.error(&message, property_span);
                }
            }
        }
        self.lower_svugy_position(element_key, position_x, position_y, span);
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
                return self.error(&message, mark.span);
            }
        };
        self.draft_mut(element_key).is_badge = is_badge;
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
            return self.error(&message, primitive.span);
        };
        self.draft_mut(element_key).svugy_shape = Some(*shape);
    }

    /// Stand the element as tall as the carrier's `height`; validation bounds the number.
    fn lower_svugy_height(&mut self, element_key: &ElementPath, height: Scalar) {
        let Some(height) = self.parse_decimal_or_reject(height, "`svugy.height`") else {
            return;
        };
        self.edit_element(element_key, |element| element.with_height(height));
    }

    /// Pin the element at the carrier's `x` and `y`, written together in one `vars` map so a position is never half-set.
    fn lower_svugy_position(
        &mut self,
        element_key: &ElementPath,
        x: Option<Scalar>,
        y: Option<Scalar>,
        span: Span,
    ) {
        let is_position_absent = x.is_none() && y.is_none();
        if is_position_absent {
            return;
        }
        let (Some(x), Some(y)) = (x, y) else {
            let message = "`svugy.position` needs both `x` and `y` in the same `vars` map";
            return self.error(message, span);
        };
        let coordinate_attribute = "a `svugy.position` coordinate";
        let Some(x) = self.parse_decimal_or_reject(x, coordinate_attribute) else {
            return;
        };
        let Some(y) = self.parse_decimal_or_reject(y, coordinate_attribute) else {
            return;
        };
        let position = Position { x, y };
        self.edit_element(element_key, |element| element.with_position(position));
    }

    /// Parse `number` as the value of `attribute`, rejecting it at its span unless it is a decimal.
    fn parse_decimal_or_reject(&mut self, number: Scalar, attribute: &str) -> Option<f64> {
        let Ok(value) = number.text.parse() else {
            let message = format!(
                "`{}` is not a number: {attribute} is a plain decimal like `91.5`",
                number.text
            );
            return self.reject(&message, number.span);
        };
        Some(value)
    }

    /// Replace the drafted element by `edit` of it, as the model's builders consume the element they extend.
    fn edit_element(&mut self, element_key: &ElementPath, edit: impl FnOnce(Element) -> Element) {
        let draft = self.draft_mut(element_key);
        let element = draft.element.clone();
        draft.element = edit(element);
    }

    fn warn(&mut self, construct: Construct, span: Span) {
        let warning = UnsupportedConstruct::new(construct, span);
        self.warnings.push(warning);
    }

    /// Record an error and drop the statement it sits in.
    fn reject<T>(&mut self, message: &str, span: Span) -> Option<T> {
        self.error(message, span);
        None
    }

    fn error(&mut self, message: &str, span: Span) {
        let error = SyntaxError::new(message.to_owned(), span);
        self.errors.push(error);
    }

    fn draft_mut(&mut self, element_key: &ElementPath) -> &mut ElementDraft {
        self.elements
            .get_mut(element_key)
            .expect("declare_path declares every element it returns")
    }

    /// Build the model through its admission, its legend's badge keys resolved first, and sort the diagnostics by source offset.
    fn into_import(mut self) -> D2Import {
        let legend = self.resolve_legend();
        let Self {
            title,
            subtitle,
            legend: _legend_draft,
            elements,
            edges,
            mut errors,
            mut warnings,
        } = self;
        let mut builder = ModelBuilder::new();
        if let Some(title) = title {
            builder = builder.with_title(title);
        }
        if let Some(subtitle) = subtitle {
            builder = builder.with_subtitle(subtitle);
        }
        if let Some(legend) = legend {
            builder = builder.with_legend(legend);
        }
        let admission = Self::build_model(builder, elements, edges);
        errors.sort_by_key(|error| error.span.offset());
        warnings.sort_by_key(|warning| warning.span.offset());
        D2Import {
            admission,
            errors,
            warnings,
        }
    }

    /// Turn the legend draft into the model's legend, each badge key resolved to the path its shape is declared under; a key naming no shape is an error dropping only its description.
    fn resolve_legend(&mut self) -> Option<Legend> {
        let legend_draft = self.legend.take()?;
        let LegendDraft {
            title,
            entries,
            dashes,
            badges,
        } = legend_draft;
        let mut legend = Legend::new();
        if let Some(title) = title {
            legend = legend.with_title(title);
        }
        legend = entries.into_iter().fold(legend, Legend::with_entry);
        for (stroke_dash, description) in dashes {
            legend = legend.with_dash_description(stroke_dash, description);
        }
        for badge_description in badges {
            let BadgeDescriptionDraft {
                target_key,
                description,
                span,
            } = badge_description;
            let Some(target) = self.declared_path_of(&target_key) else {
                let message = format!(
                    "`{target_key}` names no shape: a `svugy.legend.badges` key is the path of a badge target, e.g. `pipeline.inter`"
                );
                self.error(&message, span);
                continue;
            };
            legend = legend.with_badge_description(target, description);
        }
        Some(legend)
    }

    /// Return the model path of the shape declared under `element_key`, each segment spelled as its first declaration spelled it.
    fn declared_path_of(&self, element_key: &ElementPath) -> Option<ElementPath> {
        let ancestor_keys = element_key.ancestors();
        let element_and_ancestor_keys = ancestor_keys.chain(iter::once(element_key.clone()));
        let declared_names: Option<Vec<String>> = element_and_ancestor_keys
            .map(|key| {
                let draft = self.elements.get(&key)?;
                Some(draft.name.clone())
            })
            .collect();
        let declared_names = declared_names?;
        let declared_path = ElementPath::from_segments(declared_names);
        Some(declared_path)
    }

    /// Hand the drafts to `builder`, parents before children, then the edges in declaration order, and return its admission.
    fn build_model(
        mut builder: ModelBuilder,
        elements: BTreeMap<ElementPath, ElementDraft>,
        edges: Vec<EdgeDraft>,
    ) -> ModelAdmission {
        let mut element_refs: BTreeMap<ElementPath, ElementRef> = BTreeMap::new();
        for (element_key, draft) in elements {
            let parent_key = element_key.parent();
            let parent_ref = parent_key.and_then(|parent_key| element_refs.get(&parent_key));
            let (name, element) = draft.into_named_element();
            let element_ref = match parent_ref {
                Some(parent_ref) => builder.add_child(parent_ref, &name, element),
                None => builder.add_top_level(&name, element),
            };
            element_refs.insert(element_key, element_ref);
        }
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
}

impl ElementDraft {
    /// Draft an element labelled by its own name, declared at `span`.
    fn new(name: String, span: Span) -> Self {
        let source_range = ByteRange::from(span);
        let element = Element::new(name.clone()).with_span(source_range);
        Self {
            name,
            element,
            d2_shape: None,
            svugy_shape: None,
            is_badge: false,
        }
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
            element.with_badge()
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
