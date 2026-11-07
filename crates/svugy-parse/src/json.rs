//! Load a model from id-keyed JSON, rejecting duplicate keys and admitting it through every check, and print a model back as JSON.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use svugy_model::builder::ModelBuilder;
use svugy_model::legend::Legend;
use svugy_model::model::{ByteRange, Edge, EdgeId, Element, Model, Traveler};
use svugy_model::path::ElementPath;
use svugy_model::unique_key_map::UniqueKeyMapVisitor;
use svugy_model::validate::UncheckedModel;

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

/// Shape a model as its JSON file: the diagram's title, subtitle, description and legend, elements keyed by path, edges listed with their endpoints and ordinal.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    legend: Option<Legend>,
    #[serde(default, deserialize_with = "UniqueKeyMapVisitor::deserialize")]
    elements: BTreeMap<ElementPath, Element>,
    #[serde(
        default,
        serialize_with = "EdgeRecord::serialize_all",
        deserialize_with = "UniqueEdgeListVisitor::deserialize"
    )]
    edges: BTreeMap<EdgeId, Edge>,
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
    /// Withhold the model behind one error at where serde stopped reading `json`, its message serde's own.
    fn reject_malformed(json: &str, serde_error: &serde_json::Error) -> JsonImport {
        let empty_builder = ModelBuilder::new();
        let empty_admission = empty_builder.build();
        let line = serde_error.line();
        let column = serde_error.column();
        let offset = Self::byte_offset_at(json, line, column);
        let message = serde_error.to_string();
        let malformed_json_error = JsonError::new(message, offset..offset);
        let errors = vec![malformed_json_error];
        JsonImport {
            admission: empty_admission,
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
    /// Print `model` as pretty-printed JSON, elements keyed by path and edges listed in edge-id order.
    pub fn print(model: &Model) -> String {
        let document = ModelDocument::of(model);
        serde_json::to_string_pretty(&document)
            .expect("a model serializes: every map key is a string")
    }
}

impl ModelDocument {
    /// Copy `model` into the document its JSON file holds.
    fn of(model: &Model) -> Self {
        let title = model.title().map(str::to_owned);
        let subtitle = model.subtitle().map(str::to_owned);
        let description = model.description().map(str::to_owned);
        let legend = model.legend().cloned();
        let elements = model.elements().clone();
        let edges = model.edges().clone();
        Self {
            title,
            subtitle,
            description,
            legend,
            elements,
            edges,
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
        let Self {
            title,
            subtitle,
            description,
            legend,
            elements,
            edges,
        } = self;
        let unchecked_model = UncheckedModel::from_parts(title, subtitle, legend, elements, edges);
        match description {
            Some(description) => unchecked_model.with_description(description),
            None => unchecked_model,
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
