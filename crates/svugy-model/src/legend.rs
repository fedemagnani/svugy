//! Describe the diagram's legend: authored lines, and the descriptions of the key rows the drawing generates from what it uses.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::path::ElementPath;
use crate::style::{NumberValue, StyleValue};
use crate::unique_key_map::UniqueKeyMapVisitor;

/// Describe the legend the top-level view shows top-right: its title, its entries, and a description per edge dash and per badge target its key may draw.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Legend {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entries: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    dashes: BTreeMap<DashKey, String>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "UniqueKeyMapVisitor::deserialize"
    )]
    badges: BTreeMap<ElementPath, String>,
}

/// Key a legend dash row by its dash length's shortest spelling (`5`, not `5.0`), so every spelling of one length finds the same row.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DashKey {
    spelling: String,
}

impl Legend {
    /// Create a new legend with no title, entries or key-row descriptions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the title drawn atop the legend.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.title = Some(title);
        self
    }

    /// Append one authored line, drawn below the title in the order appended.
    pub fn with_entry(mut self, entry: impl Into<String>) -> Self {
        let entry = entry.into();
        self.entries.push(entry);
        self
    }

    /// Describe the key row of the arcs drawn with dash length `stroke_dash`.
    pub fn with_dash_description(
        mut self,
        stroke_dash: impl Into<DashKey>,
        description: impl Into<String>,
    ) -> Self {
        let dash_key = stroke_dash.into();
        let description = description.into();
        self.dashes.insert(dash_key, description);
        self
    }

    /// Describe the key row of the badges of `target`, the element every badged edge points to.
    pub fn with_badge_description(
        mut self,
        target: ElementPath,
        description: impl Into<String>,
    ) -> Self {
        let description = description.into();
        self.badges.insert(target, description);
        self
    }

    /// Return the description of the key row of arcs dashed `stroke_dash` long, if the legend has one.
    pub fn dash_description(&self, stroke_dash: f64) -> Option<&str> {
        let dash_key = DashKey::from(stroke_dash);
        let description = self.dashes.get(&dash_key);
        description.map(String::as_str)
    }

    /// Return the description of the key row of `target`'s badges, if the legend has one.
    pub fn badge_description(&self, target: &ElementPath) -> Option<&str> {
        let description = self.badges.get(target);
        description.map(String::as_str)
    }

    /// Return the title drawn atop the legend, if any.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Return the authored entries, in the order appended.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Return each dash key row's description, keyed by its dash key.
    pub fn dashes(&self) -> &BTreeMap<DashKey, String> {
        &self.dashes
    }

    /// Return each badge key row's description, keyed by its badge target.
    pub fn badges(&self) -> &BTreeMap<ElementPath, String> {
        &self.badges
    }
}

impl DashKey {
    /// Return the dash length the key spells in any plain-number spelling, or `None` when it spells none.
    pub fn stroke_dash(&self) -> Option<f64> {
        NumberValue::parse(&self.spelling)
    }

    /// Tell whether the key misspells a dash length: no plain number, or not its shortest spelling, so no arc's dash finds it.
    pub fn is_malformed(&self) -> bool {
        let Some(stroke_dash) = self.stroke_dash() else {
            return true;
        };
        let shortest_key = Self::from(stroke_dash);
        shortest_key != *self
    }
}

impl From<f64> for DashKey {
    /// Spell `stroke_dash` in its shortest decimal form.
    fn from(stroke_dash: f64) -> Self {
        let spelling = format!("{stroke_dash}");
        Self { spelling }
    }
}

impl FromStr for DashKey {
    type Err = MalformedDashKey;

    /// Parse any plain-number spelling of a dash length into its shortest spelling.
    fn from_str(text: &str) -> Result<Self, MalformedDashKey> {
        let Some(stroke_dash) = NumberValue::parse(text) else {
            return Err(MalformedDashKey);
        };
        let dash_key = Self::from(stroke_dash);
        Ok(dash_key)
    }
}

impl fmt::Display for DashKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.spelling)
    }
}

/// Reject a dash key that is not a plain number.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("a dash key is a plain number like `5`, an arc's `stroke-dash`")]
pub struct MalformedDashKey;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dash_description_is_found_by_any_spelling_of_its_length() {
        let legend = Legend::new().with_dash_description(5.0, "HTTP");

        let dash_keys: Vec<String> = legend.dashes().keys().map(DashKey::to_string).collect();
        assert_eq!(legend.dash_description(5.0), Some("HTTP"));
        assert_eq!(legend.dash_description(2.0), None);
        assert_eq!(dash_keys, ["5"]);
    }

    #[test]
    fn only_the_shortest_spelling_of_a_plain_number_keys_a_dash() {
        let cases = [
            ("5", false),
            ("1.5", false),
            ("0", false),
            ("5.0", true),
            ("05", true),
            ("-1", true),
            ("dotted", true),
            ("", true),
        ];
        for (spelling, is_expected_malformed) in cases {
            let owned_spelling = spelling.to_owned();
            let dash_key = DashKey {
                spelling: owned_spelling,
            };
            let is_malformed = dash_key.is_malformed();
            assert_eq!(is_malformed, is_expected_malformed, "{spelling:?}");
        }
    }

    #[test]
    fn any_plain_number_parses_into_its_shortest_spelling() {
        let parsed: Result<DashKey, MalformedDashKey> = "5.0".parse();
        let rejected: Result<DashKey, MalformedDashKey> = "dotted".parse();

        assert_eq!(parsed, Ok(DashKey::from(5.0)));
        assert_eq!(rejected, Err(MalformedDashKey));
    }
}
