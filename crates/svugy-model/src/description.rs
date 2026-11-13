//! Name the diagram object a description link targets, and find those links in a description as the runtime's Markdown subset reads them.

use std::fmt;
use std::ops::Range;

use crate::model::EdgeId;
use crate::path::ElementPath;
use crate::workflow::{StepId, WorkflowId};

/// Spell the separator between an edge target's two endpoints.
const EDGE_ARROW: &str = "->";
/// Spell the line opening or closing a fenced code block, inside which no link is read.
const FENCE: &str = "```";
/// Spell the first character of a link destination that targets a diagram object.
const TARGET_MARKER: char = '#';
/// Spell the separator between a step target's workflow and step.
const STEP_SEPARATOR: char = '/';

/// Name whose description a text is: an element's, a workflow's, a step's or a workflow edge's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescriptionOwner {
    Element(ElementPath),
    Workflow(WorkflowId),
    Step(WorkflowId, StepId),
    Edge(WorkflowId, EdgeId),
}

/// Name the diagram object a description link targets, canonically spelled after its `#` as `PATH`, `FROM->TO[n]`, `FROM->TO` or `WORKFLOW/STEP`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescriptionTarget {
    Element(ElementPath),
    Edge(EdgeId),
    UnnumberedEdge { from: ElementPath, to: ElementPath },
    Step { workflow: WorkflowId, step: StepId },
}

/// Locate a description link `[TEXT](#TARGET)`: the target spelled after its `#`, and that spelling's byte range in the description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetLink<'a> {
    pub target: &'a str,
    pub range: Range<usize>,
}

/// Match one alternative of the runtime's inline pattern at the start of a text, each range relative to that start.
enum InlineMatch {
    CodeSpan {
        length: usize,
    },
    Emphasis {
        content: Range<usize>,
        length: usize,
    },
    Link {
        destination: Range<usize>,
        length: usize,
    },
}

impl DescriptionTarget {
    /// Read the canonical spelling after a link's `#`, trying in order `FROM->TO[n]`, `WORKFLOW/STEP`, `FROM->TO`, then `PATH`; `None` when it spells none.
    pub fn parse(target: &str) -> Option<Self> {
        let numbered_edge = Self::parse_numbered_edge(target);
        let step = || Self::parse_step(target);
        let unnumbered_edge = || Self::parse_unnumbered_edge(target);
        let element = || Self::parse_element(target);
        numbered_edge
            .or_else(step)
            .or_else(unnumbered_edge)
            .or_else(element)
    }

    /// Read `FROM->TO[n]`, splitting at the first `->` that leaves a path on both sides.
    fn parse_numbered_edge(target: &str) -> Option<Self> {
        let endpoints_and_ordinal = target.strip_suffix(']')?;
        let (endpoints, ordinal_text) = endpoints_and_ordinal.rsplit_once('[')?;
        let has_non_digit = ordinal_text.bytes().any(|byte| !byte.is_ascii_digit());
        let is_unnumbered = ordinal_text.is_empty() || has_non_digit;
        if is_unnumbered {
            return None;
        }
        let ordinal = ordinal_text.parse().ok()?;
        let (from, to) = Self::split_endpoints(endpoints)?;
        let id = EdgeId::new(from, to, ordinal);
        let edge_target = Self::Edge(id);
        Some(edge_target)
    }

    /// Read `WORKFLOW/STEP`, both well-formed ids.
    fn parse_step(target: &str) -> Option<Self> {
        let (workflow_text, step_text) = target.split_once(STEP_SEPARATOR)?;
        let workflow = WorkflowId::new(workflow_text);
        let step = StepId::new(step_text);
        let is_either_id_malformed = !workflow.is_well_formed() || !step.is_well_formed();
        if is_either_id_malformed {
            return None;
        }
        let step_target = Self::Step { workflow, step };
        Some(step_target)
    }

    /// Read `FROM->TO`, splitting at the first `->` that leaves a path on both sides.
    fn parse_unnumbered_edge(target: &str) -> Option<Self> {
        let (from, to) = Self::split_endpoints(target)?;
        let unnumbered_edge_target = Self::UnnumberedEdge { from, to };
        Some(unnumbered_edge_target)
    }

    /// Read `PATH`.
    fn parse_element(target: &str) -> Option<Self> {
        let path = target.parse().ok()?;
        let element_target = Self::Element(path);
        Some(element_target)
    }

    /// Split `FROM->TO` at the first `->` that leaves a path on both sides.
    fn split_endpoints(endpoints: &str) -> Option<(ElementPath, ElementPath)> {
        for (arrow_start, _arrow) in endpoints.match_indices(EDGE_ARROW) {
            let from_text = &endpoints[..arrow_start];
            let to_start = arrow_start + EDGE_ARROW.len();
            let to_text = &endpoints[to_start..];
            let (Ok(from), Ok(to)) = (from_text.parse(), to_text.parse()) else {
                continue;
            };
            return Some((from, to));
        }
        None
    }
}

impl fmt::Display for DescriptionTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(path) => write!(formatter, "{path}"),
            Self::Edge(id) => {
                let from = id.from();
                let to = id.to();
                let ordinal = id.ordinal();
                write!(formatter, "{from}{EDGE_ARROW}{to}[{ordinal}]")
            }
            Self::UnnumberedEdge { from, to } => write!(formatter, "{from}{EDGE_ARROW}{to}"),
            Self::Step { workflow, step } => write!(formatter, "{workflow}{STEP_SEPARATOR}{step}"),
        }
    }
}

impl fmt::Display for DescriptionOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(path) => write!(formatter, "element `{path}`"),
            Self::Workflow(workflow) => write!(formatter, "workflow `{workflow}`"),
            Self::Step(workflow, step) => write!(formatter, "step `{workflow}/{step}`"),
            Self::Edge(workflow, id) => write!(formatter, "edge `{id}` of workflow `{workflow}`"),
        }
    }
}

impl<'a> TargetLink<'a> {
    /// Find every link `[TEXT](#TARGET)` of `description` the runtime reads as one: outside fenced code and code spans, `TEXT` holding no `]`, `TARGET` no whitespace or parenthesis.
    pub fn find_all(description: &'a str) -> Vec<Self> {
        let mut target_links = Vec::new();
        let mut is_in_fenced_code = false;
        let mut line_start = 0;
        for line in description.split('\n') {
            let line_end = line_start + line.len();
            let is_fence = line.trim().starts_with(FENCE);
            if is_fence {
                is_in_fenced_code = !is_in_fenced_code;
            }
            let is_inline_text = !is_fence && !is_in_fenced_code;
            if is_inline_text {
                let line_range = line_start..line_end;
                Self::find_in_inline_text(description, line_range, &mut target_links);
            }
            line_start = line_end + 1;
        }
        target_links
    }

    /// Rewrite each target link's target to what `respell` returns for it, keeping one it returns `None` for as written.
    pub fn respell_all(
        description: &str,
        mut respell: impl FnMut(&TargetLink<'_>) -> Option<String>,
    ) -> String {
        let mut respelled = String::with_capacity(description.len());
        let mut copied_end = 0;
        for target_link in TargetLink::find_all(description) {
            let Some(respelled_target) = respell(&target_link) else {
                continue;
            };
            let target_range = target_link.range;
            let text_before_target = &description[copied_end..target_range.start];
            respelled.push_str(text_before_target);
            respelled.push_str(&respelled_target);
            copied_end = target_range.end;
        }
        let text_after_last_target = &description[copied_end..];
        respelled.push_str(text_after_last_target);
        respelled
    }

    /// Find the target links of the inline text `description[range]`, matching left to right as the runtime does: code spans skipped, bold and emphasis read inside.
    fn find_in_inline_text(
        description: &'a str,
        range: Range<usize>,
        target_links: &mut Vec<Self>,
    ) {
        let mut index = range.start;
        while index < range.end {
            let rest = &description[index..range.end];
            let inline_match = InlineMatch::at_start_of(rest);
            match inline_match {
                Some(InlineMatch::CodeSpan { length }) => index += length,
                Some(InlineMatch::Emphasis { content, length }) => {
                    let content_range = index + content.start..index + content.end;
                    Self::find_in_inline_text(description, content_range, target_links);
                    index += length;
                }
                Some(InlineMatch::Link {
                    destination,
                    length,
                }) => {
                    let destination_text = &rest[destination.clone()];
                    let is_target = destination_text.starts_with(TARGET_MARKER);
                    if is_target {
                        let target_start = index + destination.start + TARGET_MARKER.len_utf8();
                        let target_end = index + destination.end;
                        let target = &description[target_start..target_end];
                        let range = target_start..target_end;
                        let target_link = Self { target, range };
                        target_links.push(target_link);
                    }
                    index += length;
                }
                None => {
                    let first_character = rest.chars().next();
                    index += first_character.map_or(1, char::len_utf8);
                }
            }
        }
    }
}

impl InlineMatch {
    /// Match at the start of `text` the first alternative of the runtime's inline pattern that fits: a code span, bold, emphasis, then a link.
    fn at_start_of(text: &str) -> Option<Self> {
        let code_span = Self::code_span(text);
        let bold = || Self::bold(text);
        let emphasis = || Self::emphasis(text);
        let link = || Self::link(text);
        code_span.or_else(bold).or_else(emphasis).or_else(link)
    }

    /// Match `` `CODE` ``, CODE one or more characters other than a backtick.
    fn code_span(text: &str) -> Option<Self> {
        let after_opening = text.strip_prefix('`')?;
        let code_length = after_opening.find('`')?;
        let is_empty_code = code_length == 0;
        if is_empty_code {
            return None;
        }
        let length = code_length + 2;
        Some(Self::CodeSpan { length })
    }

    /// Match `**TEXT**`, TEXT the fewest characters, at least one, before a closing `**`.
    fn bold(text: &str) -> Option<Self> {
        let after_opening = text.strip_prefix("**")?;
        let first_character = after_opening.chars().next()?;
        let first_length = first_character.len_utf8();
        let after_first_character = &after_opening[first_length..];
        let closing_offset = after_first_character.find("**")?;
        let content_end = 2 + first_length + closing_offset;
        let content = 2..content_end;
        let length = content_end + 2;
        Some(Self::Emphasis { content, length })
    }

    /// Match `*TEXT*`, TEXT opening with neither `*` nor whitespace and holding no `*`.
    fn emphasis(text: &str) -> Option<Self> {
        let after_opening = text.strip_prefix('*')?;
        let first_character = after_opening.chars().next()?;
        let is_invalid_start = first_character == '*' || first_character.is_whitespace();
        if is_invalid_start {
            return None;
        }
        let closing_offset = after_opening.find('*')?;
        let content_end = 1 + closing_offset;
        let content = 1..content_end;
        let length = content_end + 1;
        Some(Self::Emphasis { content, length })
    }

    /// Match `[TEXT](DESTINATION)`, TEXT one or more characters other than `]`, DESTINATION one or more other than whitespace and parentheses.
    fn link(text: &str) -> Option<Self> {
        let after_opening = text.strip_prefix('[')?;
        let text_length = after_opening.find(']')?;
        let is_empty_text = text_length == 0;
        if is_empty_text {
            return None;
        }
        let parenthesis_start = 1 + text_length + 1;
        let after_text = &text[parenthesis_start..];
        let after_parenthesis = after_text.strip_prefix('(')?;
        let destination_length = after_parenthesis.find(|character: char| {
            let is_parenthesis = matches!(character, '(' | ')');
            is_parenthesis || character.is_whitespace()
        })?;
        let after_destination = &after_parenthesis[destination_length..];
        let is_unclosed = !after_destination.starts_with(')');
        let is_empty_destination = destination_length == 0;
        if is_unclosed || is_empty_destination {
            return None;
        }
        let destination_start = parenthesis_start + 1;
        let destination_end = destination_start + destination_length;
        let destination = destination_start..destination_end;
        let length = destination_end + 1;
        Some(Self::Link {
            destination,
            length,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_links_the_runtime_reads_are_target_links() {
        let description = "See [the API](#backend.api), **[the job](#api->queue[1])** and [docs](https://x.y).\n`[code](#no)` stays code.\n```\n[fenced](#no)\n```\n[empty]() [spaced](#a b)";
        let target_links = TargetLink::find_all(description);
        let targets: Vec<&str> = target_links.iter().map(|link| link.target).collect();
        assert_eq!(
            targets,
            ["backend.api", "api->queue[1]"],
            "a target link is a `[TEXT](#TARGET)` outside code, as the runtime renders it"
        );
        let respelled = TargetLink::respell_all(description, |link| {
            let respelled_target = link.target.to_uppercase();
            Some(respelled_target)
        });
        let is_rest_unchanged =
            respelled.ends_with("[spaced](#a b)") && respelled.contains("[code](#no)");
        assert!(
            respelled.starts_with("See [the API](#BACKEND.API), **[the job](#API->QUEUE[1])**")
        );
        assert!(
            is_rest_unchanged,
            "respelling touches targets only: {respelled}"
        );
    }

    #[test]
    fn a_canonical_target_reads_back_as_itself() {
        let canonical_targets = [
            "backend.api",
            "backend.api->backend.queue[0]",
            "a->b[12]",
            "a->b",
            "checkout/charge",
        ];
        for canonical_target in canonical_targets {
            let target = DescriptionTarget::parse(canonical_target)
                .unwrap_or_else(|| panic!("{canonical_target} is canonical"));
            let respelled = target.to_string();
            assert_eq!(
                respelled, canonical_target,
                "parse then display is the identity on canonical targets"
            );
        }
        let element_named_like_a_url = DescriptionTarget::parse("docs/guide.html");
        let expected_element = ElementPath::from_segments(["docs/guide", "html"]);
        assert_eq!(
            element_named_like_a_url,
            Some(DescriptionTarget::Element(expected_element)),
            "a step target joins two well-formed ids; anything else with a `/` is a path"
        );
    }
}
