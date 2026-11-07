//! Read the YAML frontmatter into the diagram's title, subtitle and style sources, reporting every unknown key.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use chumsky::span::Spanned;
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_saphyr::{DefaultMessageFormatter, Location, MessageFormatter, Spanned as YamlSpanned};
use strum::{EnumString, VariantNames};

use super::ast::Span;
use super::diagnostic::Diagnostics;
use super::skeleton::SourceText;
use crate::prose::ProseList;

/// List the quotes a YAML flow scalar may open and close with.
const QUOTES: [char; 2] = ['"', '\''];

/// Hold what the frontmatter sets: the title, the subtitle, and each style's verbatim properties list.
#[derive(Debug, Default)]
pub(crate) struct Frontmatter {
    pub(crate) title: Option<String>,
    pub(crate) subtitle: Option<String>,
    pub(crate) styles: Vec<StyleSource>,
}

/// Name a style and keep its decoded properties list for the line grammar, with its value's span and, when the source spells it byte for byte, the offset it starts at.
#[derive(Debug)]
pub(crate) struct StyleSource {
    pub(crate) name: Spanned<String>,
    pub(crate) properties: String,
    pub(crate) properties_span: Span,
    pub(crate) verbatim_offset: Option<usize>,
}

/// Name a key the frontmatter may set, in the spec's table order.
#[derive(Debug, Clone, Copy, EnumString, VariantNames)]
#[strum(serialize_all = "lowercase")]
enum FrontmatterKey {
    Title,
    Subtitle,
    Styles,
}

/// Collect the frontmatter's fields as YAML spells them, each value read as its key decides, unknown keys kept for reporting.
#[derive(Default)]
struct FrontmatterFields {
    title: Option<String>,
    subtitle: Option<String>,
    styles: HashMap<YamlSpanned<String>, YamlSpanned<String>>,
    unknown_keys: Vec<YamlSpanned<String>>,
}

impl Frontmatter {
    /// Read the frontmatter YAML of `body`, recording malformed YAML and every unknown key.
    pub(crate) fn read(body: SourceText<'_>, diagnostics: &mut Diagnostics) -> Self {
        let is_blank = body.text.trim().is_empty();
        if is_blank {
            return Self::default();
        }
        let options = serde_saphyr::options! { with_snippet: false };
        let parsed = serde_saphyr::from_str_with_options::<FrontmatterFields>(body.text, options);
        let fields = match parsed {
            Ok(fields) => fields,
            Err(malformed) => {
                let location = malformed.location();
                let error_span = location
                    .map_or_else(|| body.span(), |location| Self::source_span(body, location));
                let malformed_message = DefaultMessageFormatter.format_message(&malformed);
                let message = format!("malformed frontmatter: {malformed_message}");
                diagnostics.record_error(message, error_span);
                return Self::default();
            }
        };
        let FrontmatterFields {
            title,
            subtitle,
            styles,
            unknown_keys,
        } = fields;
        for unknown_key in unknown_keys {
            let key_span = Self::source_span(body, unknown_key.referenced);
            let known_spellings = FrontmatterKey::VARIANTS;
            let prefix = "unknown frontmatter key";
            diagnostics.record_unknown(prefix, &unknown_key.value, known_spellings, key_span);
        }
        let mut styles: Vec<StyleSource> = styles
            .into_iter()
            .map(|(name, properties)| Self::style_source(body, name, properties))
            .collect();
        styles.sort_by_key(|style| style.name.span.start);
        Self {
            title,
            subtitle,
            styles,
        }
    }

    /// Pair a style's spanned name with its properties list as YAML decodes it, verbatim when the scalar, inside its quotes if any, is those very bytes.
    fn style_source(
        body: SourceText<'_>,
        name: YamlSpanned<String>,
        properties: YamlSpanned<String>,
    ) -> StyleSource {
        let name_span = Self::source_span(body, name.referenced);
        let name = Spanned {
            inner: name.value,
            span: name_span,
        };
        let scalar_span = Self::source_span(body, properties.referenced);
        let scalar = body.slice(scalar_span).trim_end();
        let scalar_end = scalar_span.start + scalar.len();
        let properties_span = Span::from(scalar_span.start..scalar_end);
        let unquoted_scalar = scalar
            .strip_prefix(QUOTES)
            .and_then(|after_opening_quote| after_opening_quote.strip_suffix(QUOTES));
        let is_quoted = unquoted_scalar.is_some();
        let quote_length = usize::from(is_quoted);
        let scalar_text = unquoted_scalar.unwrap_or(scalar);
        let is_verbatim = scalar_text == properties.value;
        let verbatim_start = properties_span.start + quote_length;
        let verbatim_offset = is_verbatim.then_some(verbatim_start);
        StyleSource {
            name,
            properties: properties.value,
            properties_span,
            verbatim_offset,
        }
    }

    /// Return the byte range a YAML location covers in the whole source, the YAML starting at `body`'s offset.
    fn source_span(body: SourceText<'_>, location: Location) -> Span {
        let yaml_span = location.span();
        let yaml_start = yaml_span.byte_offset().unwrap_or_default();
        let yaml_length = yaml_span.byte_len().unwrap_or_default();
        let body_start = usize::try_from(yaml_start).unwrap_or_default();
        let length = usize::try_from(yaml_length).unwrap_or_default();
        let body_length = body.text.len();
        let clamped_start = body_start.min(body_length);
        let clamped_end = (body_start + length).min(body_length);
        let start = body.offset + clamped_start;
        let end = body.offset + clamped_end;
        Span::from(start..end)
    }
}

impl<'de> Deserialize<'de> for FrontmatterFields {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(FrontmatterFieldsVisitor)
    }
}

/// Visit the frontmatter mapping, the one place a frontmatter key decides how its value is read.
struct FrontmatterFieldsVisitor;

impl<'de> Visitor<'de> for FrontmatterFieldsVisitor {
    type Value = FrontmatterFields;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key_list = ProseList::join_quoted(FrontmatterKey::VARIANTS, "and");
        write!(formatter, "a mapping of {key_list}")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Self::Value, A::Error> {
        let mut fields = FrontmatterFields::default();
        while let Some(key) = entries.next_key::<YamlSpanned<String>>()? {
            let frontmatter_key = FrontmatterKey::from_str(&key.value);
            match frontmatter_key {
                Ok(FrontmatterKey::Title) => fields.title = Some(entries.next_value()?),
                Ok(FrontmatterKey::Subtitle) => fields.subtitle = Some(entries.next_value()?),
                Ok(FrontmatterKey::Styles) => fields.styles = entries.next_value()?,
                Err(_unknown_key) => {
                    let _ignored_value: IgnoredAny = entries.next_value()?;
                    fields.unknown_keys.push(key);
                }
            }
        }
        Ok(fields)
    }
}
