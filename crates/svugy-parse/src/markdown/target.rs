//! Respell each `#` target of a description canonically: its references as whole paths, an edge written without `[n]` numbered as the description's scope decides.

use svugy_model::description::{DescriptionTarget, TargetLink};
use svugy_model::model::EdgeId;
use svugy_model::path::ElementPath;
use svugy_model::workflow::Workflow;

use super::resolve::{ReferenceMatch, SuffixIndex};

/// Spell the separator between an edge target's two references.
const EDGE_ARROW: &str = "->";
/// Spell the separator between a step target's workflow and step.
const STEP_SEPARATOR: char = '/';

/// Decide the ordinal an edge target written without `[n]` is stored with, by where its description is written.
pub(crate) trait EdgeNumbering {
    /// Return the ordinal Markdown stores the target `from -> to` with, `None` to store it unnumbered.
    fn number_unnumbered(&self, from: &ElementPath, to: &ElementPath) -> Option<u32>;
}

/// Store every edge target of a component description as written, since each workflow showing the component resolves it separately.
pub(crate) struct ComponentNumbering;

/// Respell description targets against every component, numbering edges written without `[n]` as `N` decides.
pub(crate) struct TargetRespeller<'a, N: ?Sized> {
    components: &'a SuffixIndex,
    numbering: &'a N,
}

impl<'a, N: EdgeNumbering + ?Sized> TargetRespeller<'a, N> {
    /// Create a new respeller resolving references against `components` and numbering edges as `numbering` decides.
    pub(crate) fn new(components: &'a SuffixIndex, numbering: &'a N) -> Self {
        Self {
            components,
            numbering,
        }
    }

    /// Return `description` with each `#` target respelled canonically, one the respeller cannot read kept as written for the model to report.
    pub(crate) fn respell(&self, description: &str) -> String {
        TargetLink::respell_all(description, |target_link| {
            self.respell_target(target_link.target)
        })
    }

    /// Respell `REF->REF[n]`, `REF->REF` or `REF` with whole paths; `None` for a step target, kept as written, or a target naming no single component.
    fn respell_target(&self, target: &str) -> Option<String> {
        let Some((from_text, to_and_ordinal)) = target.split_once(EDGE_ARROW) else {
            let is_step_target = target.contains(STEP_SEPARATOR);
            if is_step_target {
                return None;
            }
            let path = self.resolve(target)?;
            let element_target = DescriptionTarget::Element(path);
            let canonical_target = element_target.to_string();
            return Some(canonical_target);
        };
        let (to_text, written_ordinal) = Self::split_ordinal(to_and_ordinal)?;
        let from = self.resolve(from_text)?;
        let to = self.resolve(to_text)?;
        let ordinal = written_ordinal.or_else(|| self.numbering.number_unnumbered(&from, &to));
        let edge_target = match ordinal {
            Some(ordinal) => {
                let id = EdgeId::new(from, to, ordinal);
                DescriptionTarget::Edge(id)
            }
            None => DescriptionTarget::UnnumberedEdge { from, to },
        };
        let canonical_target = edge_target.to_string();
        Some(canonical_target)
    }

    /// Split `TO[n]` into `TO` and `n`, or `TO` alone into `TO` and `None`; `None` when the brackets hold no whole number.
    fn split_ordinal(to_and_ordinal: &str) -> Option<(&str, Option<u32>)> {
        let Some(to_and_bracketed) = to_and_ordinal.strip_suffix(']') else {
            return Some((to_and_ordinal, None));
        };
        let (to_text, ordinal_text) = to_and_bracketed.rsplit_once('[')?;
        let is_whole_number = ordinal_text.bytes().all(|byte| byte.is_ascii_digit());
        if !is_whole_number {
            return None;
        }
        let ordinal = ordinal_text.parse().ok()?;
        Some((to_text, Some(ordinal)))
    }

    /// Resolve a dotted reference as an edge's endpoint is resolved: the exact whole path, else the one path it is a suffix of.
    fn resolve(&self, reference_text: &str) -> Option<ElementPath> {
        let segments: Vec<String> = reference_text.split('.').map(str::to_owned).collect();
        let reference_match = self.components.find(&segments);
        let ReferenceMatch::Unique(path) = reference_match else {
            return None;
        };
        Some(path)
    }
}

impl EdgeNumbering for ComponentNumbering {
    fn number_unnumbered(&self, _from: &ElementPath, _to: &ElementPath) -> Option<u32> {
        None
    }
}

impl EdgeNumbering for Workflow {
    fn number_unnumbered(&self, from: &ElementPath, to: &ElementPath) -> Option<u32> {
        let only_edge = self.only_edge_between(from, to);
        only_edge.map(EdgeId::ordinal)
    }
}
