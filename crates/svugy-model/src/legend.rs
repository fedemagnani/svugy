//! Describe the diagram's legend: authored lines, and the descriptions of the key rows the drawing generates from what it uses.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::path::ElementPath;
use crate::style::StyleValueKind;

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
        deserialize_with = "crate::json::deserialize_unique_keys"
    )]
    dashes: BTreeMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "crate::json::deserialize_unique_keys"
    )]
    badges: BTreeMap<ElementPath, String>,
}

impl Legend {
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Describe the key row of the arcs drawn with dash length `stroke_dash`, keyed by its shortest spelling.
    pub fn with_dash_description(
        mut self,
        stroke_dash: f64,
        description: impl Into<String>,
    ) -> Self {
        let dash_key = Self::dash_key(stroke_dash);
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
        let dash_key = Self::dash_key(stroke_dash);
        let description = self.dashes.get(&dash_key);
        description.map(String::as_str)
    }

    /// Return the description of the key row of `target`'s badges, if the legend has one.
    pub fn badge_description(&self, target: &ElementPath) -> Option<&str> {
        let description = self.badges.get(target);
        description.map(String::as_str)
    }

    /// Tell whether `dash_key` is a dash length a description can be keyed by: a plain number in its shortest spelling (`5`, not `5.0`).
    pub fn is_dash_key(dash_key: &str) -> bool {
        let stroke_dash = StyleValueKind::parse_number(dash_key);
        stroke_dash.is_some_and(|stroke_dash| Self::dash_key(stroke_dash) == dash_key)
    }

    /// Spell `stroke_dash` as the key its description is stored under, its shortest decimal form.
    fn dash_key(stroke_dash: f64) -> String {
        format!("{stroke_dash}")
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn dashes(&self) -> &BTreeMap<String, String> {
        &self.dashes
    }

    pub fn badges(&self) -> &BTreeMap<ElementPath, String> {
        &self.badges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dash_description_is_found_by_any_spelling_of_its_length() {
        let legend = Legend::new().with_dash_description(5.0, "HTTP");

        assert_eq!(legend.dash_description(5.0), Some("HTTP"));
        assert_eq!(legend.dash_description(2.0), None);
        assert_eq!(legend.dashes().keys().collect::<Vec<_>>(), ["5"]);
    }

    #[test]
    fn only_the_shortest_spelling_of_a_plain_number_keys_a_dash() {
        let cases = [
            ("5", true),
            ("1.5", true),
            ("0", true),
            ("5.0", false),
            ("05", false),
            ("-1", false),
            ("dotted", false),
            ("", false),
        ];
        for (dash_key, is_expected_key) in cases {
            assert_eq!(
                Legend::is_dash_key(dash_key),
                is_expected_key,
                "{dash_key:?}"
            );
        }
    }
}
