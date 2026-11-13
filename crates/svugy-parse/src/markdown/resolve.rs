//! Resolve a reference to the component path it spells exactly, else to the one path it is a dotted suffix of.

use svugy_model::path::ElementPath;

use super::ast::{ComponentItem, Reference};
use super::diagnostic::Diagnostics;
use crate::prose::ProseList;

/// Tell what a reference's segments match: one component path, none, or several.
pub(crate) enum ReferenceMatch<'a> {
    Unique(ElementPath),
    Unknown,
    Ambiguous(Vec<&'a ElementPath>),
}

/// Index every component path in declaration order, so a reference resolves by unique dotted suffix.
#[derive(Debug)]
pub(crate) struct SuffixIndex {
    paths: Vec<ElementPath>,
}

impl SuffixIndex {
    /// Index the path of every component of the tree, parents before children, each path once.
    pub(crate) fn index(components: &[ComponentItem]) -> Self {
        let mut paths = Vec::new();
        let root = ElementPath::root();
        Self::index_children(&mut paths, &root, components);
        Self { paths }
    }

    /// Return the path `reference` spells exactly, else the one path it is a suffix of, or record it as unknown, with a suggestion, or as ambiguous, with its matches.
    pub(crate) fn resolve(
        &self,
        reference: &Reference,
        diagnostics: &mut Diagnostics,
    ) -> Option<ElementPath> {
        let segments = reference.segments.as_slice();
        let reference_match = self.find(segments);
        let reference_text = reference.to_string();
        match reference_match {
            ReferenceMatch::Unique(path) => Some(path),
            ReferenceMatch::Unknown => {
                let suffixes = self.suffixes_of_length(segments.len());
                diagnostics.record_unknown(
                    "no component",
                    &reference_text,
                    suffixes,
                    reference.span,
                );
                None
            }
            ReferenceMatch::Ambiguous(matches) => {
                let message = Self::ambiguity(&reference_text, &matches);
                diagnostics.record_error(message, reference.span);
                None
            }
        }
    }

    /// Match `segments` to the path they spell exactly, else to every path they are a dotted suffix of.
    pub(crate) fn find(&self, segments: &[String]) -> ReferenceMatch<'_> {
        let exact_path = self.paths.iter().find(|path| path.segments() == segments);
        if let Some(exact_path) = exact_path {
            let exact_path = exact_path.clone();
            return ReferenceMatch::Unique(exact_path);
        }
        let mut matches: Vec<&ElementPath> = self
            .paths
            .iter()
            .filter(|path| path.segments().ends_with(segments))
            .collect();
        let is_unique = matches.len() == 1;
        if is_unique {
            let unique_path = matches.remove(0).clone();
            return ReferenceMatch::Unique(unique_path);
        }
        let is_unknown = matches.is_empty();
        if is_unknown {
            return ReferenceMatch::Unknown;
        }
        ReferenceMatch::Ambiguous(matches)
    }

    /// Spell why `reference_text` names no single component: it matches each of `matches`.
    fn ambiguity(reference_text: &str, matches: &[&ElementPath]) -> String {
        let match_list = ProseList::join_quoted(matches, "and");
        format!("`{reference_text}` matches {match_list}; add a parent to pick one")
    }

    /// Append the path of each of `children` under `parent`, then their own children's.
    fn index_children(
        paths: &mut Vec<ElementPath>,
        parent: &ElementPath,
        children: &[ComponentItem],
    ) {
        for child in children {
            let path = parent.child(child.id.inner.clone());
            let is_new_path = !paths.contains(&path);
            if is_new_path {
                paths.push(path.clone());
            }
            Self::index_children(paths, &path, &child.children);
        }
    }

    /// List every distinct suffix of `length` segments, the candidates a misspelled reference of that length is suggested from.
    fn suffixes_of_length(&self, length: usize) -> Vec<String> {
        let mut suffixes: Vec<String> = Vec::new();
        for path in &self.paths {
            let segments = path.segments();
            let Some(suffix_start) = segments.len().checked_sub(length) else {
                continue;
            };
            let suffix = segments[suffix_start..].join(".");
            let is_new_suffix = !suffixes.contains(&suffix);
            if is_new_suffix {
                suffixes.push(suffix);
            }
        }
        suffixes
    }
}
