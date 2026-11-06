//! Load and save a model as id-keyed JSON, rejecting duplicate keys and admitting the loaded model through every check.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use miette::Diagnostic;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::legend::Legend;
use crate::model::{ByteRange, Edge, EdgeId, Element, Model};
use crate::path::ElementPath;
use crate::validate::ModelAdmission;

impl Model {
    /// Parse a JSON model, then admit it unless a check finds an error, with the report of every problem found.
    pub fn load_json(json: &str) -> Result<ModelAdmission, MalformedJson> {
        let document: ModelDocument = serde_json::from_str(json).map_err(MalformedJson)?;
        let ModelDocument {
            title,
            subtitle,
            legend,
            elements,
            edges,
        } = document;
        let mut unchecked_model = Model::new_unchecked(elements, edges);
        if let Some(title) = title {
            unchecked_model = unchecked_model.with_title(title);
        }
        if let Some(subtitle) = subtitle {
            unchecked_model = unchecked_model.with_subtitle(subtitle);
        }
        if let Some(legend) = legend {
            unchecked_model = unchecked_model.with_legend(legend);
        }
        let admission = unchecked_model.admit(Vec::new());
        Ok(admission)
    }

    /// Serialize the model as pretty-printed, deterministic JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a model serializes: every map key is a string")
    }
}

/// Reject JSON that does not parse as a model document, before any model check runs.
#[derive(Debug, Error, Diagnostic)]
#[error("malformed model JSON: {0}")]
#[diagnostic(code(svugy::malformed_json))]
pub struct MalformedJson(#[source] pub serde_json::Error);

/// Shape of the JSON file: the diagram's title, subtitle and legend, elements keyed by path, edges listed with their endpoints and ordinal.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelDocument {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    legend: Option<Legend>,
    #[serde(default, deserialize_with = "deserialize_unique_keys")]
    elements: BTreeMap<ElementPath, Element>,
    #[serde(default, deserialize_with = "deserialize_unique_edges")]
    edges: BTreeMap<EdgeId, Edge>,
}

/// One edge as written in JSON.
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
        deserialize_with = "deserialize_unique_keys"
    )]
    style: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<ByteRange>,
}

impl EdgeRecord {
    fn from_parts(id: &EdgeId, edge: &Edge) -> Self {
        let from = id.from().clone();
        let to = id.to().clone();
        let label = edge.label().map(str::to_owned);
        let style = edge.style().clone();
        Self {
            from,
            to,
            ordinal: id.ordinal(),
            label,
            style,
            span: edge.span(),
        }
    }

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

/// Write the edge map as a list of records, in edge-id order.
pub(crate) fn serialize_edges<S: Serializer>(
    edges: &BTreeMap<EdgeId, Edge>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let records = edges
        .iter()
        .map(|(id, edge)| EdgeRecord::from_parts(id, edge));
    serializer.collect_seq(records)
}

/// Read a JSON object into an ordered map, failing on a repeated key instead of keeping the last value.
pub(crate) fn deserialize_unique_keys<'de, D, K, V>(
    deserializer: D,
) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord + fmt::Display,
    V: Deserialize<'de>,
{
    let visitor = UniqueKeyMapVisitor {
        entry_types: PhantomData,
    };
    deserializer.deserialize_map(visitor)
}

/// Collect map entries, rejecting a key already seen.
struct UniqueKeyMapVisitor<K, V> {
    entry_types: PhantomData<(K, V)>,
}

impl<'de, K, V> Visitor<'de> for UniqueKeyMapVisitor<K, V>
where
    K: Deserialize<'de> + Ord + fmt::Display,
    V: Deserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with unique keys")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Self::Value, A::Error> {
        let mut map = BTreeMap::new();
        while let Some(key) = entries.next_key::<K>()? {
            let is_duplicate = map.contains_key(&key);
            if is_duplicate {
                let duplicate_key_error = de::Error::custom(format!("duplicate key `{key}`"));
                return Err(duplicate_key_error);
            }
            let value = entries.next_value()?;
            map.insert(key, value);
        }
        Ok(map)
    }
}

/// Read the edge list into a map keyed by edge id, failing on a repeated id.
fn deserialize_unique_edges<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<EdgeId, Edge>, D::Error> {
    deserializer.deserialize_seq(UniqueEdgeListVisitor)
}

/// Collect edge records, rejecting an id already seen.
struct UniqueEdgeListVisitor;

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
                let duplicate_edge_error = de::Error::custom(format!("duplicate edge `{id}`"));
                return Err(duplicate_edge_error);
            }
            edges.insert(id, edge);
        }
        Ok(edges)
    }
}
