//! Resolve a reference to the one component path it is a dotted suffix of.

use svugy_model::path::ElementPath;

use super::ast::{ComponentItem, Reference};
use super::diagnostic::{Diagnostics, ProseList};

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

    /// Return the one path `reference` is a suffix of, or record it as unknown, with a suggestion, or as ambiguous, with its matches.
    pub(crate) fn resolve(
        &self,
        reference: &Reference,
        diagnostics: &mut Diagnostics,
    ) -> Option<ElementPath> {
        let segments = reference.segments.as_slice();
        let matches: Vec<&ElementPath> = self
            .paths
            .iter()
            .filter(|path| path.segments().ends_with(segments))
            .collect();
        let reference_text = reference.to_string();
        match matches.as_slice() {
            [path] => {
                let path = (*path).clone();
                Some(path)
            }
            [] => {
                let suffixes = self.suffixes_of_length(segments.len());
                diagnostics.record_unknown(
                    "no component",
                    &reference_text,
                    suffixes,
                    reference.span,
                );
                None
            }
            _ambiguous_matches => {
                let match_list = ProseList::join_quoted(&matches, "and");
                let message =
                    format!("`{reference_text}` matches {match_list}; add a parent to pick one");
                diagnostics.record(message, reference.span);
                None
            }
        }
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
