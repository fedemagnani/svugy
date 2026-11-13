//! Expand a properties list into typed properties: its styles first, left to right, then its `key=value` entries over them.

use strum::IntoEnumIterator;

use super::ast::{Assignment, Entry, Span, StyleDefinition};
use super::diagnostic::Diagnostics;
use super::parse::PARENTHESISED_LABEL_HINT;
use super::property::{Property, PropertyKey, PropertyValue};

/// Keep each frontmatter style's typed properties by name, read once where the style is defined.
#[derive(Debug, Default)]
pub(crate) struct StyleBook {
    styles: Vec<(String, Vec<Property>)>,
}

impl StyleBook {
    /// Read every style definition's entries, recording each error once, at the definition.
    pub(crate) fn read(definitions: Vec<StyleDefinition>, diagnostics: &mut Diagnostics) -> Self {
        let mut styles = Vec::new();
        for definition in definitions {
            let StyleDefinition { name, entries } = definition;
            for entry in &entries {
                let Entry::Style {
                    name: nested_style,
                    may_end_label: _may_end_label,
                } = entry
                else {
                    continue;
                };
                let nested_name = &nested_style.inner;
                let message = format!(
                    "a style holds `key=value` entries only, not the style `{nested_name}`"
                );
                diagnostics.record_error(message, nested_style.span);
            }
            let properties = Self::read_assignments(&entries, diagnostics);
            styles.push((name.inner, properties));
        }
        Self { styles }
    }

    /// Expand `entries` into typed properties, each key once: every style's, then every `key=value` entry's over them; an unknown style that may end a label is hinted as its unquoted end.
    pub(crate) fn expand(&self, entries: &[Entry], diagnostics: &mut Diagnostics) -> Vec<Property> {
        let mut properties: Vec<Property> = Vec::new();
        for entry in entries {
            let Entry::Style {
                name: style_name,
                may_end_label,
            } = entry
            else {
                continue;
            };
            let Some(style_properties) = self.find(&style_name.inner) else {
                let unknown_name = &style_name.inner;
                if *may_end_label {
                    let message = format!("no style `{unknown_name}`; {PARENTHESISED_LABEL_HINT}");
                    diagnostics.record_error(message, style_name.span);
                    continue;
                }
                let style_names = self.styles.iter().map(|(name, _properties)| name);
                diagnostics.record_unknown("no style", unknown_name, style_names, style_name.span);
                continue;
            };
            for style_property in style_properties {
                let property = Property {
                    span: style_name.span,
                    ..style_property.clone()
                };
                Self::push_overriding(&mut properties, property);
            }
        }
        let assigned_properties = Self::read_assignments(entries, diagnostics);
        for property in assigned_properties {
            Self::push_overriding(&mut properties, property);
        }
        properties
    }

    /// Push `property` after dropping the earlier property of its key, so the later one wins.
    fn push_overriding(properties: &mut Vec<Property>, property: Property) {
        let key = property.value.key();
        properties.retain(|earlier| earlier.value.key() != key);
        properties.push(property);
    }

    /// Read each `key=value` entry into a typed property, recording unknown keys, repeated keys and values of the wrong kind, the last kept as unreadable.
    fn read_assignments(entries: &[Entry], diagnostics: &mut Diagnostics) -> Vec<Property> {
        let mut properties = Vec::new();
        let mut assigned_keys: Vec<PropertyKey> = Vec::new();
        for entry in entries {
            let Entry::Assignment(assignment) = entry else {
                continue;
            };
            let Assignment { key, value } = assignment;
            let Some(property_key) = PropertyKey::parse(&key.inner) else {
                let known_keys = PropertyKey::iter();
                diagnostics.record_unknown("unknown key", &key.inner, known_keys, key.span);
                continue;
            };
            let is_repeated_key = assigned_keys.contains(&property_key);
            if is_repeated_key {
                let message = format!("`{property_key}` appears twice");
                diagnostics.record_error(message, key.span);
                continue;
            }
            assigned_keys.push(property_key.clone());
            let read_value = property_key.read(value);
            let property_value = match read_value {
                Ok(property_value) => property_value,
                Err(message) => {
                    diagnostics.record_error(message, value.span);
                    PropertyValue::Unreadable(property_key)
                }
            };
            let span = Span::from(key.span.start..value.span.end);
            let property = Property {
                value: property_value,
                span,
            };
            properties.push(property);
        }
        properties
    }

    /// Return the typed properties of the style named `name`, if the frontmatter defines one.
    fn find(&self, name: &str) -> Option<&[Property]> {
        let mut styles = self.styles.iter();
        let style = styles.find(|(style_name, _properties)| style_name == name);
        style.map(|(_name, properties)| properties.as_slice())
    }
}
