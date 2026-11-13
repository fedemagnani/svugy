//! Load a model from id-keyed JSON, rejecting duplicate keys and v3 keys and admitting it through every check, and print a model back as JSON.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use svugy_model::model::{ByteRange, Edge, EdgeId, Element, Model, Traveler, ViewAngle};
use svugy_model::path::ElementPath;
use svugy_model::unique_key_map::UniqueKeyMapVisitor;
use svugy_model::validate::{ModelAdmission, ModelReport, UncheckedModel};
use svugy_model::workflow::{ElementOverride, Step, StepId, StepItem, Workflow, WorkflowId};

use crate::diagnostic::{Import, NoWarning, SourceError, SourceLanguage};

/// Mark JSON as a source language: its one error is the text not parsing as a model document, and it raises no warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonLanguage;

/// Report JSON that does not parse as a model document, before any model check runs.
pub type JsonError = SourceError<JsonLanguage>;

/// Collect the import of a JSON model: the model's admission and the malformed-JSON error, if any.
pub type JsonImport = Import<JsonLanguage>;

/// Print a model as pretty-printed, deterministic JSON, which says every model.
pub struct JsonPrinter;

/// Shape a model as its JSON file: the view angle, elements keyed by path and workflows in written order; v3 root keys are rejected with their fix.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    view: Option<ViewAngle>,
    #[serde(deserialize_with = "UniqueKeyMapVisitor::deserialize")]
    elements: BTreeMap<ElementPath, Element>,
    workflows: Vec<WorkflowRecord>,
    #[serde(
        default,
        rename = "title",
        skip_serializing,
        deserialize_with = "V3RootKey::reject_title"
    )]
    _v3_title: (),
    #[serde(
        default,
        rename = "subtitle",
        skip_serializing,
        deserialize_with = "V3RootKey::reject_subtitle"
    )]
    _v3_subtitle: (),
    #[serde(
        default,
        rename = "description",
        skip_serializing,
        deserialize_with = "V3RootKey::reject_description"
    )]
    _v3_description: (),
    #[serde(
        default,
        rename = "legend",
        skip_serializing,
        deserialize_with = "V3RootKey::reject_legend"
    )]
    _v3_legend: (),
    #[serde(
        default,
        rename = "edges",
        skip_serializing,
        deserialize_with = "V3RootKey::reject_edges"
    )]
    _v3_edges: (),
}

/// Record one workflow as written in JSON.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowRecord {
    id: WorkflowId,
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    overrides: BTreeMap<ElementPath, ElementOverride>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "EdgeRecord::serialize_all",
        deserialize_with = "UniqueEdgeListVisitor::deserialize"
    )]
    edges: BTreeMap<EdgeId, Edge>,
    steps: Vec<StepRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
}

/// Record one step as written in JSON.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepRecord {
    id: StepId,
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    items: Vec<StepItemRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
}

/// Record one step item as written in JSON: `{"edge": {from, to, ordinal}}` or `{"element": PATH}`.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StepItemRecord {
    Edge(EdgeReference),
    Element(ElementPath),
}

/// Name an edge of the step's workflow by its endpoints and ordinal, 0 when omitted.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeReference {
    from: ElementPath,
    to: ElementPath,
    #[serde(default)]
    ordinal: u32,
}

/// Record one edge as written in JSON.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeRecord {
    from: ElementPath,
    to: ElementPath,
    #[serde(default)]
    ordinal: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    style: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    traveler: Option<Traveler>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    badge: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
}

/// Collect edge records, rejecting an id already seen.
struct UniqueEdgeListVisitor;

/// Reject each root key v3 accepted, whatever its value, with its v4 fix.
struct V3RootKey;

impl SourceLanguage for JsonLanguage {
    const ERROR_CODE: &'static str = "svugy::malformed_json";
    type Warning = NoWarning;

    fn import(source_text: &str) -> JsonImport {
        let read_document = ModelDocument::read(source_text);
        let document = match read_document {
            Ok(document) => document,
            Err(serde_error) => return Self::reject_malformed(source_text, &serde_error),
        };
        let unchecked_model = document.into_unchecked_model();
        let admission = unchecked_model.admit();
        JsonImport {
            admission,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

impl JsonLanguage {
    /// Withhold the model behind one error at where serde stopped reading `json`, its message serde's own, and no model problem.
    fn reject_malformed(json: &str, serde_error: &serde_json::Error) -> JsonImport {
        let withheld_admission = ModelAdmission {
            model: None,
            report: ModelReport::default(),
        };
        let line = serde_error.line();
        let column = serde_error.column();
        let offset = Self::byte_offset_at(json, line, column);
        let message = serde_error.to_string();
        let malformed_json_error = JsonError::new(message, offset..offset);
        let errors = vec![malformed_json_error];
        JsonImport {
            admission: withheld_admission,
            errors,
            warnings: Vec::new(),
        }
    }

    /// Turn serde's 1-based `line` and byte `column` into a byte offset into `json`, clamped to its end; line 0 means no position.
    fn byte_offset_at(json: &str, line: usize, column: usize) -> usize {
        let preceding_line_count = line.saturating_sub(1);
        let preceding_lines = json.split_inclusive('\n').take(preceding_line_count);
        let line_start: usize = preceding_lines.map(str::len).sum();
        let column_offset = column.saturating_sub(1);
        let offset = line_start + column_offset;
        offset.min(json.len())
    }
}

impl JsonPrinter {
    /// Print `model` as pretty-printed JSON, elements keyed by path, workflows in written order, edges in edge-id order.
    pub fn print(model: &Model) -> String {
        let document = ModelDocument::of(model);
        serde_json::to_string_pretty(&document)
            .expect("a model serializes: every map key is a string")
    }
}

impl ModelDocument {
    /// Copy `model` into the document its JSON file holds.
    fn of(model: &Model) -> Self {
        let view = model.view();
        let elements = model.elements().clone();
        let model_workflows = model.workflows().iter();
        let workflows = model_workflows.map(WorkflowRecord::of).collect();
        Self {
            view,
            elements,
            workflows,
            ..Self::default()
        }
    }

    /// Read `json` as one model document and nothing after it.
    fn read(json: &str) -> Result<Self, serde_json::Error> {
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let document = Self::deserialize(&mut deserializer)?;
        deserializer.end()?;
        Ok(document)
    }

    /// Hold the document's model unchecked, as written, ordinals included.
    fn into_unchecked_model(self) -> UncheckedModel {
        let records = self.workflows.into_iter();
        let workflows = records.map(WorkflowRecord::into_workflow).collect();
        let unchecked_model = UncheckedModel::from_parts(self.elements, workflows);
        let Some(view) = self.view else {
            return unchecked_model;
        };
        unchecked_model.with_view(view)
    }
}

impl WorkflowRecord {
    /// Record `workflow` as JSON writes it.
    fn of(workflow: &Workflow) -> Self {
        let id = workflow.id().clone();
        let title = workflow.title().to_owned();
        let subtitle = workflow.subtitle().map(str::to_owned);
        let description = workflow.description().map(str::to_owned);
        let overrides = workflow.overrides().clone();
        let edges = workflow.edges().clone();
        let workflow_steps = workflow.steps().iter();
        let steps = workflow_steps.map(StepRecord::of).collect();
        let span = workflow.span();
        Self {
            id,
            title,
            subtitle,
            description,
            overrides,
            edges,
            steps,
            span,
        }
    }

    /// Turn the record into the workflow it describes.
    fn into_workflow(self) -> Workflow {
        let mut workflow = Workflow::new(self.id, self.title)
            .with_overrides(self.overrides)
            .with_edges(self.edges);
        if let Some(subtitle) = self.subtitle {
            workflow = workflow.with_subtitle(subtitle);
        }
        if let Some(description) = self.description {
            workflow = workflow.with_description(description);
        }
        for step_record in self.steps {
            let step = step_record.into_step();
            workflow = workflow.with_step(step);
        }
        if let Some(span) = self.span {
            workflow = workflow.with_span(span);
        }
        workflow
    }
}

impl StepRecord {
    /// Record `step` as JSON writes it.
    fn of(step: &Step) -> Self {
        let id = step.id().clone();
        let title = step.title().to_owned();
        let description = step.description().map(str::to_owned);
        let step_items = step.items();
        let items = step_items.map(StepItemRecord::of).collect();
        let span = step.span();
        Self {
            id,
            title,
            description,
            items,
            span,
        }
    }

    /// Turn the record into the step it describes.
    fn into_step(self) -> Step {
        let mut step = Step::new(self.id, self.title);
        if let Some(description) = self.description {
            step = step.with_description(description);
        }
        for item_record in self.items {
            let item = item_record.into_item();
            step = step.with_item(item);
        }
        if let Some(span) = self.span {
            step = step.with_span(span);
        }
        step
    }
}

impl StepItemRecord {
    /// Record `item` as JSON writes it.
    fn of(item: &StepItem) -> Self {
        match item {
            StepItem::Edge(id) => {
                let from = id.from().clone();
                let to = id.to().clone();
                let ordinal = id.ordinal();
                let reference = EdgeReference { from, to, ordinal };
                Self::Edge(reference)
            }
            StepItem::Element(path) => Self::Element(path.clone()),
        }
    }

    /// Turn the record into the item it names.
    fn into_item(self) -> StepItem {
        match self {
            Self::Edge(reference) => {
                let id = EdgeId::new(reference.from, reference.to, reference.ordinal);
                StepItem::Edge(id)
            }
            Self::Element(path) => StepItem::Element(path),
        }
    }
}

impl EdgeRecord {
    /// Write the edge map as a list of records, in edge-id order.
    fn serialize_all<S: Serializer>(
        edges: &BTreeMap<EdgeId, Edge>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let records = edges
            .iter()
            .map(|(id, edge)| EdgeRecord::from_parts(id, edge));
        serializer.collect_seq(records)
    }

    /// Record the edge `id` names, as JSON writes it.
    fn from_parts(id: &EdgeId, edge: &Edge) -> Self {
        let from = id.from().clone();
        let to = id.to().clone();
        let ordinal = id.ordinal();
        let label = edge.label().map(str::to_owned);
        let description = edge.description().map(str::to_owned);
        let style = edge.style().clone();
        let traveler = edge.traveler();
        let badge = edge.is_badge();
        let span = edge.span();
        Self {
            from,
            to,
            ordinal,
            label,
            description,
            style,
            traveler,
            badge,
            span,
        }
    }

    /// Split the record into the id and the edge it describes.
    fn into_parts(self) -> (EdgeId, Edge) {
        let id = EdgeId::new(self.from, self.to, self.ordinal);
        let mut edge = Edge::new();
        if let Some(label) = self.label {
            edge = edge.with_label(label);
        }
        if let Some(description) = self.description {
            edge = edge.with_description(description);
        }
        for (key, value) in self.style {
            edge = edge.with_style(key, value);
        }
        if let Some(traveler) = self.traveler {
            edge = edge.with_traveler(traveler);
        }
        if self.badge {
            edge = edge.with_badge();
        }
        if let Some(span) = self.span {
            edge = edge.with_span(span);
        }
        (id, edge)
    }
}

impl UniqueEdgeListVisitor {
    /// Read the edge list into a map keyed by edge id, failing on a repeated id.
    fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<EdgeId, Edge>, D::Error> {
        deserializer.deserialize_seq(Self)
    }
}

impl<'de> Visitor<'de> for UniqueEdgeListVisitor {
    type Value = BTreeMap<EdgeId, Edge>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a list of edges with unique (from, to, ordinal)")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut records: A) -> Result<Self::Value, A::Error> {
        let mut edges = BTreeMap::new();
        while let Some(record) = records.next_element::<EdgeRecord>()? {
            let (id, edge) = record.into_parts();
            let is_duplicate = edges.contains_key(&id);
            if is_duplicate {
                let duplicate_edge_message = format!("duplicate edge `{id}`");
                let duplicate_edge_error = de::Error::custom(duplicate_edge_message);
                return Err(duplicate_edge_error);
            }
            edges.insert(id, edge);
        }
        Ok(edges)
    }
}

impl V3RootKey {
    /// Reject a root `title`, which moved into workflows.
    fn reject_title<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        Self::reject(
            "`title` moved from the root into workflows: set it as the first workflow's `title`",
        )
    }

    /// Reject a root `subtitle`, which moved into workflows.
    fn reject_subtitle<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        Self::reject(
            "`subtitle` moved from the root into workflows: set it as the first workflow's `subtitle`",
        )
    }

    /// Reject a root `description`, which moved into workflows.
    fn reject_description<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        Self::reject(
            "`description` moved from the root into workflows: set it as the first workflow's `description`",
        )
    }

    /// Reject a root `legend`, which v4 removed.
    fn reject_legend<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        Self::reject(
            "`legend` was removed: say what each entry meant in a step's or an edge's `description`",
        )
    }

    /// Reject root `edges`, which moved into workflows.
    fn reject_edges<'de, D: Deserializer<'de>>(_value: D) -> Result<(), D::Error> {
        Self::reject(
            "`edges` moved from the root into workflows: list each edge in a workflow's `edges` and name it in a step's `items`",
        )
    }

    /// Fail reading the key with `message`.
    fn reject<E: de::Error>(message: &str) -> Result<(), E> {
        let v3_key_error = E::custom(message);
        Err(v3_key_error)
    }
}
