//! Resolve each `#` target of a description to its canonical spelling, warning on one that names no component or edge, or several.

use std::collections::BTreeMap;

use svugy_model::description::{DescriptionTarget, TargetLink};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;
use svugy_model::problem::Suggestion;

use super::ast::DescriptionBlock;
use super::diagnostic::Diagnostics;
use super::resolve::{ReferenceMatch, SuffixIndex};

/// Spell the separator between an edge target's two references.
const EDGE_ARROW: &str = "->";

/// Count the edges added between each ordered pair of components, the ordinals an edge target may pick from.
pub(crate) type ParallelEdgeCounts = BTreeMap<(ElementPath, ElementPath), u32>;

/// Resolve description targets against every component and every edge, so a target may name one declared after it.
pub(crate) struct TargetResolver<'a> {
    components: &'a SuffixIndex,
    parallel_edge_counts: &'a ParallelEdgeCounts,
}

impl<'a> TargetResolver<'a> {
    /// Create a resolver over the indexed components and the edges added between each pair.
    pub(crate) fn new(
        components: &'a SuffixIndex,
        parallel_edge_counts: &'a ParallelEdgeCounts,
    ) -> Self {
        Self {
            components,
            parallel_edge_counts,
        }
    }

    /// Return the description's text with each `#` target respelled canonically, recording a warning at each one left as written.
    pub(crate) fn canonicalize(
        &self,
        description: &DescriptionBlock,
        diagnostics: &mut Diagnostics,
    ) -> String {
        TargetLink::respell_all(&description.text, |target_link| {
            let target = target_link.target;
            let resolution = self.resolve(target);
            let unresolved_reason = match resolution {
                Ok(description_target) => {
                    let canonical_target = description_target.to_string();
                    return Some(canonical_target);
                }
                Err(unresolved_reason) => unresolved_reason,
            };
            let message = format!("`#{target}` stays plain text: {unresolved_reason}");
            let target_range = target_link.range.clone();
            let target_span = description.source_span(target_range);
            diagnostics.record_unresolved_target(message, target_span);
            None
        })
    }

    /// Resolve `COMPONENT` or `FROM->TO[n]`, else say why it names no single object.
    fn resolve(&self, target: &str) -> Result<DescriptionTarget, String> {
        let Some((from_text, to_and_ordinal)) = target.split_once(EDGE_ARROW) else {
            let path = self.resolve_component(target)?;
            let element_target = DescriptionTarget::Element(path);
            return Ok(element_target);
        };
        let (to_text, ordinal) = Self::split_ordinal(to_and_ordinal)?;
        let is_missing_reference = from_text.is_empty() || to_text.is_empty();
        if is_missing_reference {
            return Err(Self::malformed());
        }
        let from = self.resolve_component(from_text)?;
        let to = self.resolve_component(to_text)?;
        let id = self.pick_parallel_edge(from, to, ordinal)?;
        let edge_target = DescriptionTarget::Edge(id);
        Ok(edge_target)
    }

    /// Split `TO[n]` into `TO` and `n`, or `TO` alone into `TO` and `None`.
    fn split_ordinal(to_and_ordinal: &str) -> Result<(&str, Option<u32>), String> {
        let Some(to_and_bracketed) = to_and_ordinal.strip_suffix(']') else {
            return Ok((to_and_ordinal, None));
        };
        let (to_text, ordinal_text) = to_and_bracketed
            .rsplit_once('[')
            .ok_or_else(Self::malformed)?;
        let has_non_digit = ordinal_text.bytes().any(|byte| !byte.is_ascii_digit());
        if has_non_digit {
            return Err(Self::malformed());
        }
        let ordinal = ordinal_text.parse().map_err(|_error| Self::malformed())?;
        Ok((to_text, Some(ordinal)))
    }

    /// Resolve a dotted reference as an edge's endpoint is resolved: the exact whole path, else the one path it is a suffix of.
    fn resolve_component(&self, reference_text: &str) -> Result<ElementPath, String> {
        let segments: Vec<String> = reference_text.split('.').map(str::to_owned).collect();
        let reference_match = self.components.find(&segments);
        match reference_match {
            ReferenceMatch::Unique(path) => Ok(path),
            ReferenceMatch::Unknown => {
                let suffixes = self.components.suffixes_of_length(segments.len());
                let suggestion = Suggestion::closest_to(reference_text, suffixes);
                let reason = match suggestion {
                    Some(suggestion) => format!("no component `{reference_text}`; {suggestion}"),
                    None => format!("no component `{reference_text}`"),
                };
                Err(reason)
            }
            ReferenceMatch::Ambiguous(matches) => {
                let reason = SuffixIndex::ambiguity(reference_text, &matches);
                Err(reason)
            }
        }
    }

    /// Pick the edge from `from` to `to` numbered `ordinal`, or the only one when no ordinal is written.
    fn pick_parallel_edge(
        &self,
        from: ElementPath,
        to: ElementPath,
        ordinal: Option<u32>,
    ) -> Result<EdgeId, String> {
        let pair = (from, to);
        let edge_count = self.parallel_edge_counts.get(&pair).copied().unwrap_or(0);
        let (from, to) = pair;
        let is_edgeless_pair = edge_count == 0;
        if is_edgeless_pair {
            let reason = format!("no edge goes from `{from}` to `{to}`");
            return Err(reason);
        }
        let last_ordinal = edge_count - 1;
        let picked_ordinal = match ordinal {
            Some(ordinal) if ordinal <= last_ordinal => ordinal,
            Some(_out_of_range_ordinal) => {
                let reason = format!("the last edge from `{from}` to `{to}` is `[{last_ordinal}]`");
                return Err(reason);
            }
            None if last_ordinal == 0 => 0,
            None => {
                let reason = format!(
                    "{edge_count} edges go from `{from}` to `{to}`; add `[0]` to `[{last_ordinal}]` to pick one"
                );
                return Err(reason);
            }
        };
        let id = EdgeId::new(from, to, picked_ordinal);
        Ok(id)
    }

    /// Spell the reason of a target shaped like neither `COMPONENT` nor `FROM->TO[n]`.
    fn malformed() -> String {
        "write `#COMPONENT` or `#FROM->TO[n]`".to_owned()
    }
}
