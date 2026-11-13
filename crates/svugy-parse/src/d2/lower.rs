//! Lower a d2 document into a model of one workflow, merging repeated keys, declaring every endpoint and warning on what the model cannot hold.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use svugy_model::builder::{ElementRef, ModelBuilder, WorkflowRef};
use svugy_model::model::{ByteRange, Edge, Element, PlacementHint, Shape};
use svugy_model::path::ElementPath;
use svugy_model::validate::{ModelAdmission, ModelReport};
use svugy_model::workflow::{ElementOverride, Step, StepId, Workflow, WorkflowId};

use super::D2Import;
use super::D2Parse;
use super::ast::{
    EdgeDeclaration, EdgeLink, EdgeOperator, Field, FieldAssignment, KeyPath, KeySegment, Scalar,
    ShapeDeclaration, Span, Statement,
};
use super::carrier::{CarrierReader, ElementAttribute};
use super::diagnostic::{Construct, Diagnostics};

/// Name the one workflow a d2 import yields, also its title when the root has no label.
const MAIN_WORKFLOW_ID: &str = "main";

/// Name the main workflow's one step, which lists every edge.
const EVERY_EDGE_STEP_ID: &str = "main";

/// Title the main workflow's one step.
const EVERY_EDGE_STEP_TITLE: &str = "Every edge";

/// Spell the error of a d2 file declaring no shape, which leaves the main step nothing to list.
const NO_SHAPE: &str = "the diagram declares no shape, so there is nothing to draw";

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

/// Lower each statement into drafts of the main workflow's title and subtitle, the elements and the edges, recording every diagnostic on the way.
struct Lowering {
    title: Option<String>,
    subtitle: Option<String>,
    elements: ElementDrafts,
    edges: Vec<EdgeDraft>,
    diagnostics: Diagnostics,
}

/// Draft every declared element under its case-folded key, declaring each path the way d2 does.
#[derive(Default)]
struct ElementDrafts {
    drafts: BTreeMap<ElementPath, ElementDraft>,
}

/// Merge every d2 declaration of one element and of its override in the main workflow, the last write winning as in d2.
struct ElementDraft {
    declaration_index: usize,
    name: String,
    element: Element,
    main_override: Option<ElementOverride>,
    d2_shape: Option<Shape>,
    svugy_shape: Option<Shape>,
    badge_mark_span: Option<Span>,
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
            self.push_edge_unless_lineage_or_warn(edge_draft, link_span);
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

    /// Apply one field of an edge's map, warning on it unless it is a label, a tooltip or a style, all the model's edges hold; the subset filter rejects a `vars` carrier on an edge.
    fn apply_edge_field_or_warn(&mut self, edge: Edge, field: Field) -> Edge {
        let field_keyword = field.keyword();
        let keyword_name: &str = field_keyword.into();
        match field {
            Field::Label(label) => edge.with_label(label.text),
            Field::Tooltip(tooltip) => edge.with_description(tooltip.text),
            Field::Style(properties) => properties.into_iter().fold(edge, |edge, property| {
                edge.with_style(property.name.name, property.value.text)
            }),
            Field::Shape(value) | Field::Direction(value) | Field::Link(value) => {
                let construct = Construct::FieldOnEdge(keyword_name);
                self.diagnostics.record_warning(construct, value.span);
                edge
            }
            Field::Svugy(_rejected_carrier) => edge,
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

    /// Keep `edge_draft` unless it joins a component to itself, its ancestor or its descendant, which svugy can't draw: drop that one with a warning at `link_span`.
    fn push_edge_unless_lineage_or_warn(&mut self, edge_draft: EdgeDraft, link_span: Span) {
        let is_lineage_edge = edge_draft.from_key.shares_lineage_with(&edge_draft.to_key);
        if is_lineage_edge {
            self.diagnostics
                .record_warning(Construct::LineageEdge, link_span);
            return;
        }
        self.edges.push(edge_draft);
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
            Field::Tooltip(tooltip) => {
                self.elements.edit_element(&target_key, |element| {
                    element.with_description(tooltip.text)
                });
            }
            Field::Link(link) => {
                let link_range = ByteRange::from(link.span);
                self.elements.edit_element(&target_key, |element| {
                    element.with_link_written_at(link.text, Some(link_range))
                });
            }
            Field::Style(properties) => self.elements.edit_element(&target_key, |element| {
                properties.into_iter().fold(element, |element, property| {
                    element.with_style(property.name.name, property.value.text)
                })
            }),
            Field::Svugy(properties) => {
                let mut carrier_reader = CarrierReader::new(&mut self.diagnostics);
                let element_attributes = carrier_reader.read_element(properties, span);
                self.merge_element_carrier(&target_key, element_attributes, span);
            }
        }
    }

    /// Title the main workflow by the root's `label`, as d2 titles a board's PDF page, subtitle it by the root's carrier, keep its default direction, and warn on every other field.
    fn lower_root_field(&mut self, field: Field, span: Span) {
        let root_key = ElementPath::root();
        let field_keyword = field.keyword();
        let keyword_name: &str = field_keyword.into();
        match field {
            Field::Label(label) => self.title = Some(label.text),
            Field::Svugy(properties) => {
                let mut carrier_reader = CarrierReader::new(&mut self.diagnostics);
                let subtitle = carrier_reader.read_root_subtitle(properties);
                if let Some(subtitle) = subtitle {
                    self.subtitle = Some(subtitle);
                }
            }
            Field::Direction(direction) => self.lower_direction_field(&root_key, direction),
            _element_only_field => {
                let construct = Construct::FieldOnRoot(keyword_name);
                self.diagnostics.record_warning(construct, span);
            }
        }
    }

    /// Map `direction: right` onto layered placement in the main workflow, which the root already has; svugy's placement follows no other direction.
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
        self.elements
            .edit_main_override(element_key, direction.span, |element_override| {
                element_override.with_child_placement(PlacementHint::Layered)
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
        self.elements.draft_mut(element_key).d2_shape = *primitive;
    }

    /// Merge an element carrier's attributes, written at `carrier_span`, into its draft in write order, the latest write winning as in d2; the carrier's primitive outranks d2's `shape`.
    fn merge_element_carrier(
        &mut self,
        element_key: &ElementPath,
        attributes: Vec<ElementAttribute>,
        carrier_span: Span,
    ) {
        for attribute in attributes {
            match attribute {
                ElementAttribute::Caption(caption) => {
                    self.elements
                        .edit_element(element_key, |element| element.with_caption(caption));
                }
                ElementAttribute::Primitive(shape) => {
                    self.elements.draft_mut(element_key).svugy_shape = Some(shape);
                }
                ElementAttribute::Height(height) => {
                    self.elements
                        .edit_element(element_key, |element| element.with_height(height));
                }
                ElementAttribute::BadgeMark { is_marked, span } => {
                    let badge_mark_span = is_marked.then_some(span);
                    self.elements.draft_mut(element_key).badge_mark_span = badge_mark_span;
                }
                ElementAttribute::Position(position) => {
                    self.elements.edit_main_override(
                        element_key,
                        carrier_span,
                        |element_override| element_override.with_position(position),
                    );
                }
            }
        }
    }

    /// Finish into the import: withhold the model of a diagram of no shape behind one error at the file start, else warn at container badge marks and build the model through its admission; sort the diagnostics by source offset.
    fn into_import(self) -> D2Import {
        let Self {
            title,
            subtitle,
            elements,
            edges,
            mut diagnostics,
        } = self;
        let declares_no_shape = elements.drafts.is_empty();
        if declares_no_shape {
            let file_start = Span::new(0, 0);
            diagnostics.record_error(NO_SHAPE, file_start);
            let withheld_admission = ModelAdmission {
                model: None,
                report: ModelReport::default(),
            };
            let (errors, warnings) = diagnostics.into_sorted_parts();
            return D2Import {
                admission: withheld_admission,
                errors,
                warnings,
            };
        }
        elements.warn_container_badge_marks(&mut diagnostics);
        let main_workflow_id = WorkflowId::new(MAIN_WORKFLOW_ID);
        let main_title = title.unwrap_or_else(|| MAIN_WORKFLOW_ID.to_owned());
        let mut main_workflow = Workflow::new(main_workflow_id, main_title);
        if let Some(subtitle) = subtitle {
            main_workflow = main_workflow.with_subtitle(subtitle);
        }
        let admission = Self::build_model(main_workflow, elements, edges);
        let (errors, warnings) = diagnostics.into_sorted_parts();
        D2Import {
            admission,
            errors,
            warnings,
        }
    }

    /// Build the model of `main_workflow`: every element with its override, parents before children, then one step listing every edge in declaration order and every leaf no edge touches, so it draws what d2 draws.
    fn build_model(
        main_workflow: Workflow,
        elements: ElementDrafts,
        edges: Vec<EdgeDraft>,
    ) -> ModelAdmission {
        let badged_edges: Vec<EdgeDraft> = edges
            .into_iter()
            .map(|edge_draft| edge_draft.badged_if_from_leaf_into_badge_leaf(&elements))
            .collect();
        let untouched_leaf_keys = elements.leaf_keys_no_edge_touches(&badged_edges);
        let mut builder = ModelBuilder::new();
        let main_workflow_ref = builder.add_workflow(main_workflow);
        let element_refs = elements.add_to(&mut builder, &main_workflow_ref);
        let every_edge_step_id = StepId::new(EVERY_EDGE_STEP_ID);
        let every_edge_step = Step::new(every_edge_step_id, EVERY_EDGE_STEP_TITLE);
        let every_edge_step_ref = builder.add_step(&main_workflow_ref, every_edge_step);
        for edge_draft in badged_edges {
            let EdgeDraft {
                from_key,
                to_key,
                edge,
            } = edge_draft;
            let from_ref = &element_refs[&from_key];
            let to_ref = &element_refs[&to_key];
            builder.add_edge(&every_edge_step_ref, from_ref, to_ref, edge);
        }
        for leaf_key in untouched_leaf_keys {
            let leaf_ref = &element_refs[&leaf_key];
            builder.list_element(&every_edge_step_ref, leaf_ref);
        }
        builder.build()
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
        let is_declared = self.drafts.contains_key(&element_key);
        if is_declared {
            return element_key;
        }
        let declaration_index = self.drafts.len();
        let name = segment.name.clone();
        let draft = ElementDraft::new(declaration_index, name, span);
        let drafted_key = element_key.clone();
        self.drafts.insert(drafted_key, draft);
        element_key
    }

    /// Key the element `name` under `parent_key` case-insensitively, as d2 matches keys.
    fn child_key(parent_key: &ElementPath, name: &str) -> ElementPath {
        let folded_name = name.to_lowercase();
        parent_key.child(folded_name)
    }

    /// Replace the drafted element by `edit` of it, as the model's builders consume the element they extend.
    fn edit_element(&mut self, element_key: &ElementPath, edit: impl FnOnce(Element) -> Element) {
        let draft = self.draft_mut(element_key);
        let element = draft.element.clone();
        draft.element = edit(element);
    }

    /// Replace the element's override of the main workflow by `edit` of it, spanned at `span`, the field that wrote it last.
    fn edit_main_override(
        &mut self,
        element_key: &ElementPath,
        span: Span,
        edit: impl FnOnce(ElementOverride) -> ElementOverride,
    ) {
        let draft = self.draft_mut(element_key);
        let main_override = draft.main_override.take().unwrap_or_default();
        let field_range = ByteRange::from(span);
        let edited_override = edit(main_override).with_span(field_range);
        draft.main_override = Some(edited_override);
    }

    /// Return the key of every leaf `edges` leave untouched, in declaration order: an element with no drafted child, an empty container included.
    fn leaf_keys_no_edge_touches(&self, edges: &[EdgeDraft]) -> Vec<ElementPath> {
        let touched_keys: BTreeSet<&ElementPath> = edges
            .iter()
            .flat_map(|edge_draft| [&edge_draft.from_key, &edge_draft.to_key])
            .collect();
        let mut untouched_leaf_drafts: Vec<(&ElementPath, &ElementDraft)> = self
            .drafts
            .iter()
            .filter(|(element_key, _draft)| {
                let is_leaf = !self.is_container(element_key);
                let is_untouched = !touched_keys.contains(element_key);
                is_leaf && is_untouched
            })
            .collect();
        untouched_leaf_drafts.sort_by_key(|(_element_key, draft)| draft.declaration_index);
        untouched_leaf_drafts
            .into_iter()
            .map(|(element_key, _draft)| element_key.clone())
            .collect()
    }

    /// Add every drafted element to `builder`, parents before children, with its override of `workflow`, and return each one's ref by its key.
    fn add_to(
        self,
        builder: &mut ModelBuilder,
        workflow: &WorkflowRef,
    ) -> BTreeMap<ElementPath, ElementRef> {
        let mut element_refs: BTreeMap<ElementPath, ElementRef> = BTreeMap::new();
        for (element_key, draft) in self.drafts {
            let parent_key = element_key.parent();
            let parent_ref = parent_key.and_then(|parent_key| element_refs.get(&parent_key));
            let (name, element, main_override) = draft.into_parts();
            let element_ref = match parent_ref {
                Some(parent_ref) => builder.add_child(parent_ref, &name, element),
                None => builder.add_top_level(&name, element),
            };
            if let Some(main_override) = main_override {
                builder.override_element(workflow, &element_ref, main_override);
            }
            element_refs.insert(element_key, element_ref);
        }
        element_refs
    }

    /// Warn at each container's badge mark, ignored since a container is entered rather than badged.
    fn warn_container_badge_marks(&self, diagnostics: &mut Diagnostics) {
        for (element_key, draft) in &self.drafts {
            let Some(badge_mark_span) = draft.badge_mark_span else {
                continue;
            };
            let is_leaf = !self.is_container(element_key);
            if is_leaf {
                continue;
            }
            diagnostics.record_warning(Construct::ContainerBadge, badge_mark_span);
        }
    }

    /// Tell whether the element drafted under `element_key` was last marked a badge target by `vars.svugy.badge`.
    fn is_badge_target(&self, element_key: &ElementPath) -> bool {
        let draft = self.drafts.get(element_key);
        draft.is_some_and(|draft| draft.badge_mark_span.is_some())
    }

    /// Tell whether the element drafted under `element_key` has a drafted child, which is what makes it a container.
    fn is_container(&self, element_key: &ElementPath) -> bool {
        let after_element_key = (Bound::Excluded(element_key), Bound::Unbounded);
        let mut later_drafts = self.drafts.range::<ElementPath, _>(after_element_key);
        let next_draft = later_drafts.next();
        next_draft.is_some_and(|(next_key, _draft)| element_key.is_ancestor_of(next_key))
    }

    /// Return the draft of the element `declare_path` declared under `element_key`.
    fn draft_mut(&mut self, element_key: &ElementPath) -> &mut ElementDraft {
        self.drafts
            .get_mut(element_key)
            .expect("declare_path declares every element it returns")
    }
}

impl ElementDraft {
    /// Draft an element labelled by its own name, declared `declaration_index`-th at `span`.
    fn new(declaration_index: usize, name: String, span: Span) -> Self {
        let source_range = ByteRange::from(span);
        let label = name.clone();
        let element = Element::new(label).with_span(source_range);
        Self {
            declaration_index,
            name,
            element,
            main_override: None,
            d2_shape: None,
            svugy_shape: None,
            badge_mark_span: None,
        }
    }

    /// Split the draft into its key name, its element and its override of the main workflow; the element is shaped by the carrier first and d2 second, kept apart until now as d2's `rectangle` resets its shape.
    fn into_parts(self) -> (String, Element, Option<ElementOverride>) {
        let Self {
            declaration_index: _declaration_index,
            name,
            element,
            main_override,
            d2_shape,
            svugy_shape,
            badge_mark_span: _badge_mark_span,
        } = self;
        let shape = svugy_shape.or(d2_shape);
        let Some(shape) = shape else {
            return (name, element, main_override);
        };
        let shaped_element = element.with_shape(shape);
        (name, shaped_element, main_override)
    }
}

impl EdgeDraft {
    /// Badge the edge when it runs from a leaf into a leaf marked a badge target, the edges `vars.svugy.badge` stands for.
    fn badged_if_from_leaf_into_badge_leaf(self, elements: &ElementDrafts) -> Self {
        let is_from_container = elements.is_container(&self.from_key);
        if is_from_container {
            return self;
        }
        let is_into_container = elements.is_container(&self.to_key);
        if is_into_container {
            return self;
        }
        let is_into_unmarked_element = !elements.is_badge_target(&self.to_key);
        if is_into_unmarked_element {
            return self;
        }
        let badged_edge = self.edge.with_badge();
        Self {
            edge: badged_edge,
            ..self
        }
    }
}
