//! Load and save a model as id-keyed JSON, rejecting duplicate keys and admitting the loaded model through every check.

use std::collections::BTreeMap;
use std::fmt;

use miette::Diagnostic;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::legend::Legend;
use crate::model::{ByteRange, Edge, EdgeId, Element, Model, ModelContent, UncheckedModel};
use crate::path::ElementPath;
use crate::unique_key_map::UniqueKeyMapVisitor;
use crate::validate::ModelAdmission;

impl Model {
    /// Parse a JSON model, then admit it unless a check finds an error, with the report of every problem found.
    pub fn load_json(json: &str) -> Result<ModelAdmission, MalformedJson> {
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let content = ModelDocument::deserialize(&mut deserializer).map_err(MalformedJson)?;
        deserializer.end().map_err(MalformedJson)?;
        let unchecked_model = UncheckedModel::new(content);
        let admission = unchecked_model.admit();
        Ok(admission)
    }

    /// Serialize the model as pretty-printed, deterministic JSON.
    pub fn to_json(&self) -> String {
        let mut json = Vec::new();
        let mut serializer = serde_json::Serializer::pretty(&mut json);
        ModelDocument::serialize(&self.content, &mut serializer)
            .expect("a model serializes: every map key is a string");
        String::from_utf8(json).expect("serde_json writes UTF-8")
    }
}

/// Reject JSON that does not parse as a model document, before any model check runs.
#[derive(Debug, Error, Diagnostic)]
#[error("malformed model JSON: {0}")]
#[diagnostic(code(svugy::malformed_json))]
pub struct MalformedJson(#[source] pub serde_json::Error);

/// Shape a model's content as its JSON file: the diagram's title, subtitle and legend, elements keyed by path, edges listed with their endpoints and ordinal.
#[derive(Serialize, Deserialize)]
#[serde(remote = "ModelContent", deny_unknown_fields)]
struct ModelDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
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
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    style: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
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
        let style = edge.style().clone();
        let span = edge.span();
        Self {
            from,
            to,
            ordinal,
            label,
            style,
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
        for (key, value) in self.style {
            edge = edge.with_style(key, value);
        }
        if let Some(span) = self.span {
            edge = edge.with_span(span);
        }
        (id, edge)
    }
}

/// Collect edge records, rejecting an id already seen.
struct UniqueEdgeListVisitor;

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
