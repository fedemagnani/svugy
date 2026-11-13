//! Parse each verbatim item line and heading of the skeleton, and each style of the frontmatter, into the document's items, recovering at every broken entry.

use std::num::ParseIntError;

use chumsky::error::{RichPattern, RichReason};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use chumsky::span::Spanned;
use logos::Logos;

use super::ast::{
    Assignment, ChainItem, ComponentItem, Description, Document, EdgeItem, Entry, ListedEdge,
    OverrideItem, Reference, Span, StepItem, StepSection, StyleDefinition, Value, WorkflowSection,
};
use super::diagnostic::Diagnostics;
use super::frontmatter::{Frontmatter, StyleSource};
use super::lex::Token;
use super::skeleton::{
    ComponentNode, DescribedLine, HeadedPart, Skeleton, SourceText, WorkflowSkeleton,
};
use crate::prose::ProseList;

/// Type the parser extra every line parser shares: chumsky's rich errors over line tokens.
type ParserExtra<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, Span>>;

/// Name the end of an item line, where a parse error may find nothing more.
const END_OF_LINE: &str = "end of line";
/// Spell the error of a name ending with `-` or holding `--`, which the parser keeps whole so the item still parses.
const DASHED_NAME: &str = "an ID cannot end with `-` or hold `--`";
/// Spell the error of a bare label holding a quoted string.
const PARTLY_QUOTED_LABEL: &str = "a label is either quoted whole or holds no `\"`";
/// Spell the hint added to an error inside a trailing group that may end a label, which a label ending in parentheses runs into.
pub(crate) const PARENTHESISED_LABEL_HINT: &str = "a label ending in parentheses needs quotes";
/// Spell the error of a step heading ending in a parenthesised group.
const STEP_HEADING_PROPERTIES: &str = "a step heading takes no properties";
/// Spell the error of an edge written among a workflow's overrides, before its first step.
const EDGE_AMONG_OVERRIDES: &str = "an edge goes under a step heading `#### ID: TITLE`: before the first step, an item overrides a component, `REF (PROPERTIES)`";
/// Spell the one property a workflow heading takes.
const SUBTITLE_KEY: &str = "subtitle";
/// Name a bare step item, `REF`, which takes nothing after its reference.
const BARE_ITEM: ItemShape = ItemShape {
    noun: "a bare item",
    hint: "write it on an edge declaration `REF -> REF`",
    description_hint: "describe the component under `## Components`",
};
/// Name a step item listing an edge again, `REF -> REF[n]`, which takes nothing after its ordinal.
const LISTED_EDGE: ItemShape = ItemShape {
    noun: "an `[n]` item",
    hint: "write it where the edge is declared",
    description_hint: "write it where the edge is declared",
};
/// Name a chain of edges, `REF -> REF -> REF`, which takes nothing after its last reference.
const CHAIN: ItemShape = ItemShape {
    noun: "a chain",
    hint: "write its hops on separate lines",
    description_hint: "write its hops on separate lines",
};

/// Read an item's label: one quoted string, or the bare text up to the item's trailing property group.
pub(crate) struct ItemLabel;

/// Hold a workflow heading as written: `ID: TITLE [(PROPERTIES)]`.
struct WorkflowHeading {
    id: Spanned<String>,
    title: String,
    entries: Vec<Entry>,
}

/// Hold a step heading as written: `ID: TITLE`.
struct StepHeading {
    id: Spanned<String>,
    title: String,
}

/// Locate a line's trailing property group, the final balanced `(…)` its `)` ends, and tell whether it may be a label's unquoted end: it follows a `:` and holds no `=`.
#[derive(Debug, Clone, Copy)]
struct TrailingGroup {
    span: Span,
    may_end_label: bool,
}

/// Hold a step item line as written, `REF (-> REF)* [[n]] [: LABEL] [(PROPERTIES)]`, before its shape decides which parts it may hold.
struct RawStepItem {
    first: Reference,
    hops: Vec<Reference>,
    ordinal: Option<Spanned<u32>>,
    label: Option<Spanned<String>>,
    properties: Option<Spanned<Vec<Entry>>>,
}

/// Hold what only an edge declaration may write after its references: a label, a properties list and a description.
struct ItemExtras {
    label: Option<Spanned<String>>,
    properties: Option<Spanned<Vec<Entry>>>,
    description: Option<Description>,
}

/// Name a step item shape that takes no extras, with the hints telling where a label or properties list and a description go instead.
struct ItemShape {
    noun: &'static str,
    hint: &'static str,
    description_hint: &'static str,
}

/// Parse one verbatim line into the item it declares.
trait LineGrammar: Sized {
    /// Build the parser of the whole of `line`, ending in `trailing_group` when it has one.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>;

    /// Lex and parse `line`, recording every error, hinted at quotes inside a trailing group that may end a label; `None` when nothing of it parses.
    fn parse_line(line: SourceText<'_>, diagnostics: &mut Diagnostics) -> Option<Self> {
        let tokens = Token::lex_line(line, diagnostics)?;
        let trailing_group = TrailingGroup::find(&tokens);
        let line_end = line.span().end;
        let end_of_line = Span::from(line_end..line_end);
        let input = tokens
            .as_slice()
            .map(end_of_line, |(token, span)| (token, span));
        let parser = Self::line_parser(line, trailing_group);
        let parse_result = parser.parse(input);
        let (item, rich_errors) = parse_result.into_output_errors();
        for rich_error in rich_errors {
            let description = Token::describe_rich_error(&rich_error);
            let span = *rich_error.span();
            let is_in_possible_label_end =
                trailing_group.is_some_and(|group| group.may_end_label_around(span));
            let message = if is_in_possible_label_end {
                format!("{description}; {PARENTHESISED_LABEL_HINT}")
            } else {
                description
            };
            diagnostics.record_error(message, span);
        }
        item
    }
}

impl Document {
    /// Parse the skeleton's frontmatter, every item line and every heading into the document, recording every error.
    pub(crate) fn parse(skeleton: Skeleton<'_>, diagnostics: &mut Diagnostics) -> Self {
        let Skeleton {
            frontmatter,
            components,
            workflows,
        } = skeleton;
        let frontmatter = frontmatter.map(|body| Frontmatter::read(body, diagnostics));
        let Frontmatter { view, styles } = frontmatter.unwrap_or_default();
        let styles = styles
            .into_iter()
            .map(|style| StyleDefinition::parse(style, diagnostics))
            .collect();
        let components = ComponentItem::parse_nodes(components, diagnostics);
        let workflows = workflows
            .into_iter()
            .filter_map(|workflow| WorkflowSection::parse(workflow, diagnostics))
            .collect();
        Self {
            view,
            styles,
            components,
            workflows,
        }
    }
}

impl StyleDefinition {
    /// Parse a style definition's properties list, kept with no entries when nothing of it parses so its uses add no error.
    fn parse(source: StyleSource, diagnostics: &mut Diagnostics) -> Self {
        let StyleSource {
            name,
            properties,
            properties_span,
            verbatim_offset,
        } = source;
        let Some(verbatim_offset) = verbatim_offset else {
            let entries = Self::parse_labelled_whole(&properties, properties_span, diagnostics);
            return Self { name, entries };
        };
        let properties_text = SourceText {
            text: &properties,
            offset: verbatim_offset,
        };
        let entries = Vec::<Entry>::parse_line(properties_text, diagnostics);
        let entries = entries.unwrap_or_default();
        Self { name, entries }
    }

    /// Parse a style whose decoded text differs from its source, labelling each error and each entry at the whole value, `properties_span`.
    fn parse_labelled_whole(
        properties: &str,
        properties_span: Span,
        diagnostics: &mut Diagnostics,
    ) -> Vec<Entry> {
        let properties_text = SourceText {
            text: properties,
            offset: properties_span.start,
        };
        let mut decoded_diagnostics = Diagnostics::default();
        let entries = Vec::<Entry>::parse_line(properties_text, &mut decoded_diagnostics);
        let (decoded_errors, _no_warnings) = decoded_diagnostics.into_sorted_parts();
        for decoded_error in decoded_errors {
            diagnostics.record_error(decoded_error.message, properties_span);
        }
        let entries = entries.unwrap_or_default();
        entries
            .into_iter()
            .map(|entry| entry.relabelled(properties_span))
            .collect()
    }
}

impl ComponentItem {
    /// Parse each component node into its item with its description, children first, recording each item a comment split off its parent's list; a broken item stays by its ID when that parses.
    fn parse_nodes(nodes: Vec<ComponentNode<'_>>, diagnostics: &mut Diagnostics) -> Vec<Self> {
        let mut items = Vec::new();
        for node in nodes {
            let ComponentNode {
                item: DescribedLine { line, description },
                is_indented_but_not_nested,
                children,
            } = node;
            let children = Self::parse_nodes(children, diagnostics);
            let parsed_item = Self::parse_line(line, diagnostics);
            let item = parsed_item.or_else(|| Self::from_leading_id(line));
            let Some(item) = item else {
                continue;
            };
            if is_indented_but_not_nested {
                let id = &item.id.inner;
                let message = format!(
                    "`{id}` is indented but not nested: the comment above ends its parent's list; indent the comment like the item"
                );
                diagnostics.record_error(message, item.span);
            }
            let described_item = Self {
                description,
                children,
                ..item
            };
            items.push(described_item);
        }
        items
    }

    /// Keep a broken item by the ID its line opens with, so references to it and to its children still resolve; `None` when no ID opens it.
    fn from_leading_id(line: SourceText<'_>) -> Option<Self> {
        let id = Token::leading_name(line)?;
        let line_span = line.span();
        let item = Self {
            id,
            display_name: None,
            entries: Vec::new(),
            description: None,
            children: Vec::new(),
            span: line_span,
        };
        Some(item)
    }
}

impl LineGrammar for ComponentItem {
    /// Parse `ID[: DISPLAY NAME] [(PROPERTIES)]`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let id = Token::name_parser();
        let display_name = ItemLabel::parser(line, trailing_group).map(|label| label.inner);
        let properties = Entry::properties_parser(line, trailing_group);
        id.then(display_name.or_not())
            .then(properties.or_not())
            .then_ignore(end())
            .map(move |((id, display_name), properties)| {
                let entries = properties.map(|properties| properties.inner);
                Self {
                    id,
                    display_name,
                    entries: entries.unwrap_or_default(),
                    description: None,
                    children: Vec::new(),
                    span: line.span(),
                }
            })
    }
}

impl WorkflowSection {
    /// Parse a workflow's heading, overrides and steps; a broken heading keeps the workflow by the text before its `:`, titled by it, and `None` drops a heading with no such text after its body is parsed.
    fn parse(skeleton: WorkflowSkeleton<'_>, diagnostics: &mut Diagnostics) -> Option<Self> {
        let WorkflowSkeleton { part, steps } = skeleton;
        let HeadedPart {
            heading,
            prose,
            items,
        } = part;
        let parsed_heading = WorkflowHeading::parse_line(heading, diagnostics);
        let overrides = items
            .into_iter()
            .filter_map(|item| OverrideItem::parse_described_line(item, diagnostics))
            .collect();
        let steps = steps
            .into_iter()
            .filter_map(|step| StepSection::parse(step, diagnostics))
            .collect();
        let recovered_heading = parsed_heading.or_else(|| WorkflowHeading::from_raw_id(heading));
        let WorkflowHeading { id, title, entries } = recovered_heading?;
        let subtitle = WorkflowHeading::read_subtitle(entries, diagnostics);
        let workflow = Self {
            id,
            title,
            subtitle,
            prose,
            overrides,
            steps,
        };
        Some(workflow)
    }
}

impl WorkflowHeading {
    /// Keep a broken heading by the text before its `:`, titled by it; `None` when that text is blank.
    fn from_raw_id(heading: SourceText<'_>) -> Option<Self> {
        let id = Token::raw_heading_id(heading)?;
        let title = id.inner.clone();
        let entries = Vec::new();
        let heading = Self { id, title, entries };
        Some(heading)
    }

    /// Read the subtitle out of the heading's entries, recording a style, hinted as a label's unquoted end when it may be one, any other key and a repeated `subtitle`.
    fn read_subtitle(entries: Vec<Entry>, diagnostics: &mut Diagnostics) -> Option<String> {
        let mut subtitle = None;
        for entry in entries {
            let assignment = match entry {
                Entry::Assignment(assignment) => assignment,
                Entry::Style {
                    name,
                    may_end_label,
                } => {
                    let no_style_message =
                        "a workflow heading takes no style: its only property is `subtitle`";
                    let message = if may_end_label {
                        format!("{no_style_message}; {PARENTHESISED_LABEL_HINT}")
                    } else {
                        no_style_message.to_owned()
                    };
                    diagnostics.record_error(message, name.span);
                    continue;
                }
            };
            let Assignment { key, value } = assignment;
            let is_other_key = key.inner != SUBTITLE_KEY;
            if is_other_key {
                let prefix = "a workflow heading takes only `subtitle`, not";
                diagnostics.record_unknown(prefix, &key.inner, [SUBTITLE_KEY], key.span);
                continue;
            }
            let is_repeated = subtitle.is_some();
            if is_repeated {
                let message = format!("`{SUBTITLE_KEY}` appears twice");
                diagnostics.record_error(message, key.span);
                continue;
            }
            match value.inner.read_text() {
                Ok(text) => subtitle = Some(text),
                Err(message) => diagnostics.record_error(message, value.span),
            }
        }
        subtitle
    }
}

impl LineGrammar for WorkflowHeading {
    /// Parse `ID: TITLE [(PROPERTIES)]`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let title = ItemLabel::parser(line, trailing_group).map(|title| title.inner);
        let properties = Entry::properties_parser(line, trailing_group);
        Token::heading_id_parser(line)
            .then(title)
            .then(properties.or_not())
            .then_ignore(end())
            .map(|((id, title), properties)| {
                let entries = properties.map(|properties| properties.inner);
                Self {
                    id,
                    title,
                    entries: entries.unwrap_or_default(),
                }
            })
    }
}

impl StepSection {
    /// Parse a step's heading and items; a broken heading keeps the step by the text before its `:`, titled by it, and `None` drops a heading with no such text after its items are parsed.
    fn parse(part: HeadedPart<'_>, diagnostics: &mut Diagnostics) -> Option<Self> {
        let HeadedPart {
            heading,
            prose,
            items,
        } = part;
        let parsed_heading = StepHeading::parse_line(heading, diagnostics);
        let items = items
            .into_iter()
            .filter_map(|item| StepItem::parse_described_line(item, diagnostics))
            .collect();
        let recovered_heading = parsed_heading.or_else(|| StepHeading::from_raw_id(heading));
        let StepHeading { id, title } = recovered_heading?;
        let step = Self {
            id,
            title,
            prose,
            items,
        };
        Some(step)
    }
}

impl StepHeading {
    /// Keep a broken heading by the text before its `:`, titled by it; `None` when that text is blank.
    fn from_raw_id(heading: SourceText<'_>) -> Option<Self> {
        let id = Token::raw_heading_id(heading)?;
        let title = id.inner.clone();
        let heading = Self { id, title };
        Some(heading)
    }
}

impl LineGrammar for StepHeading {
    /// Parse `ID: TITLE`, recording the trailing group the title runs up to.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let title = ItemLabel::parser(line, trailing_group).map(|title| title.inner);
        let rejected_group =
            any()
                .repeated()
                .at_least(1)
                .validate(|_group_tokens, extra, emitter| {
                    let group_span: Span = extra.span();
                    let error = Rich::custom(group_span, STEP_HEADING_PROPERTIES);
                    emitter.emit(error);
                });
        Token::heading_id_parser(line)
            .then(title)
            .then_ignore(rejected_group.or_not())
            .then_ignore(end())
            .map(|(id, title)| Self { id, title })
    }
}

impl OverrideItem {
    /// Parse an override's line, recording a description under it; `None` when nothing of the line parses.
    fn parse_described_line(
        described_line: DescribedLine<'_>,
        diagnostics: &mut Diagnostics,
    ) -> Option<Self> {
        let DescribedLine { line, description } = described_line;
        if let Some(description) = description {
            let message =
                "an override takes no description: describe the component under `## Components`";
            diagnostics.record_error(message, description.span);
        }
        Self::parse_line(line, diagnostics)
    }
}

impl LineGrammar for OverrideItem {
    /// Parse `REF (PROPERTIES)`, rejecting an edge `REF -> …`, which goes under a step heading.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let properties = Entry::properties_parser(line, trailing_group);
        let unexpected_arrow = any().filter(|token: &Token<'src>| *token == Token::Arrow);
        let rejected_edge = unexpected_arrow.try_map(|_arrow, arrow_span: Span| {
            let error = Rich::custom(arrow_span, EDGE_AMONG_OVERRIDES);
            Err::<Spanned<Vec<Entry>>, _>(error)
        });
        Reference::parser()
            .then(choice((properties, rejected_edge)))
            .then_ignore(end())
            .map(move |(reference, properties)| Self {
                reference,
                entries: properties.inner,
                span: line.span(),
            })
    }
}

impl StepItem {
    /// Parse a step item's line, then shape it with the description under it; `None` when nothing of the line parses.
    fn parse_described_line(
        described_line: DescribedLine<'_>,
        diagnostics: &mut Diagnostics,
    ) -> Option<Self> {
        let DescribedLine { line, description } = described_line;
        let line_span = line.span();
        let raw_item = RawStepItem::parse_line(line, diagnostics)?;
        let step_item = Self::shape(raw_item, line_span, description, diagnostics);
        Some(step_item)
    }

    /// Shape a raw item written at `line_span` as a bare component, a chain, a listed edge or an edge declaration, recording each part its shape takes no part of.
    fn shape(
        raw_item: RawStepItem,
        line_span: Span,
        description: Option<Description>,
        diagnostics: &mut Diagnostics,
    ) -> Self {
        let RawStepItem {
            first,
            hops,
            ordinal,
            label,
            properties,
        } = raw_item;
        let extras = ItemExtras {
            label,
            properties,
            description,
        };
        let mut hops = hops.into_iter();
        let Some(first_hop) = hops.next() else {
            if let Some(ordinal) = ordinal {
                let message = "`[n]` lists an edge: `REF -> REF[n]`";
                diagnostics.record_error(message, ordinal.span);
            }
            extras.reject_on(&BARE_ITEM, diagnostics);
            return Self::Element(first);
        };
        if let Some(second_hop) = hops.next() {
            if let Some(ordinal) = ordinal {
                let message = "a chain takes no `[n]`; write its hops on separate lines";
                diagnostics.record_error(message, ordinal.span);
            }
            extras.reject_on(&CHAIN, diagnostics);
            let later_hops = hops.collect();
            let chain_item = ChainItem {
                start: first,
                first_hop,
                second_hop,
                later_hops,
            };
            return Self::Chain(chain_item);
        }
        if let Some(ordinal) = ordinal {
            extras.reject_on(&LISTED_EDGE, diagnostics);
            let listed_edge = ListedEdge {
                from: first,
                to: first_hop,
                ordinal: ordinal.inner,
                span: line_span,
            };
            return Self::ListedEdge(listed_edge);
        }
        let ItemExtras {
            label,
            properties,
            description,
        } = extras;
        let entries = properties.map(|properties| properties.inner);
        let edge_item = EdgeItem {
            from: first,
            to: first_hop,
            label,
            entries: entries.unwrap_or_default(),
            description,
        };
        Self::Edge(edge_item)
    }
}

impl ItemExtras {
    /// Record each extra written on an item of `shape`, which takes none.
    fn reject_on(self, shape: &ItemShape, diagnostics: &mut Diagnostics) {
        let Self {
            label,
            properties,
            description,
        } = self;
        let ItemShape {
            noun,
            hint,
            description_hint,
        } = shape;
        if let Some(label) = label {
            let message = format!("{noun} takes no label; {hint}");
            diagnostics.record_error(message, label.span);
        }
        if let Some(properties) = properties {
            let message = format!("{noun} takes no properties list; {hint}");
            diagnostics.record_error(message, properties.span);
        }
        if let Some(description) = description {
            let message = format!("{noun} takes no description; {description_hint}");
            diagnostics.record_error(message, description.span);
        }
    }
}

impl LineGrammar for RawStepItem {
    /// Parse `REF (-> REF)* [[n]] [: LABEL] [(PROPERTIES)]`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let hops = just(Token::Arrow)
            .ignore_then(Reference::parser())
            .repeated()
            .collect::<Vec<Reference>>();
        let ordinal = Token::ordinal_parser();
        let label = ItemLabel::parser(line, trailing_group);
        let properties = Entry::properties_parser(line, trailing_group);
        Reference::parser()
            .then(hops)
            .then(ordinal.or_not())
            .then(label.or_not())
            .then(properties.or_not())
            .then_ignore(end())
            .map(|((((first, hops), ordinal), label), properties)| Self {
                first,
                hops,
                ordinal,
                label,
                properties,
            })
    }
}

impl LineGrammar for Vec<Entry> {
    /// Parse a style's whole value, one properties list.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        Entry::properties_parser(line, trailing_group)
            .then_ignore(end())
            .map(|properties| properties.inner)
    }
}

impl Entry {
    /// Parse `(ENTRY, ENTRY, …)`, the line's `trailing_group`, skipping each broken entry to the next `,` or `)` after recording it.
    fn properties_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Spanned<Vec<Self>>, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let may_end_label = trailing_group.is_some_and(|group| group.may_end_label);
        let key = Token::name_parser()
            .map(|name| name.inner)
            .separated_by(just(Token::Dot))
            .at_least(1)
            .collect::<Vec<String>>()
            .map(|segments| segments.join("."))
            .spanned();
        let assignment = key
            .then_ignore(just(Token::Equals))
            .then(Value::parser(line))
            .map(|(key, value)| Self::Assignment(Assignment { key, value }));
        let style = Token::name_parser().map(move |name| Self::Style {
            name,
            may_end_label,
        });
        let skipped_entry = none_of([Token::Comma, Token::RightParen])
            .repeated()
            .at_least(1)
            .to(None);
        let entry = choice((assignment, style))
            .map(Some)
            .recover_with(via_parser(skipped_entry));
        entry
            .separated_by(just(Token::Comma))
            .collect::<Vec<Option<Self>>>()
            .map(|entries| entries.into_iter().flatten().collect())
            .delimited_by(just(Token::LeftParen), just(Token::RightParen))
            .spanned()
    }
}

impl Value {
    /// Parse a quoted value, or bare pieces, each running to the next `,` or `)`: the first one, then each one after a comma that cannot start an entry.
    fn parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
    ) -> impl Parser<'tokens, I, Spanned<Self>, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let quoted = select! { Token::Quoted(raw) => Self::Quoted(Token::decode_quoted(raw)) };
        let bare_token = any().filter(|token: &Token<'src>| {
            let is_delimiter = matches!(
                token,
                Token::Comma | Token::LeftParen | Token::RightParen | Token::Quoted(..)
            );
            !is_delimiter
        });
        let bare_piece = bare_token
            .repeated()
            .at_least(1)
            .map_with(move |_tokens, extra| {
                let piece_span: Span = extra.span();
                line.slice(piece_span).to_owned()
            });
        let piece_after_comma_start = any()
            .filter(|token: &Token<'src>| !matches!(token, Token::Name(_name)))
            .rewind();
        let piece_after_comma = just(Token::Comma)
            .ignore_then(piece_after_comma_start)
            .ignore_then(bare_piece);
        let bare = bare_piece
            .then(piece_after_comma.repeated().collect::<Vec<String>>())
            .map(|(first_piece, later_pieces)| {
                let mut pieces = vec![first_piece];
                pieces.extend(later_pieces);
                Self::Bare(pieces)
            });
        choice((quoted, bare)).spanned().labelled("a value")
    }

    /// Tell whether `text`, written bare after a key's `=`, reads back as itself: it lexes, holds no `,`, `(`, `)` or `"`, and no space pads it.
    pub(crate) fn reads_back_bare(text: &str) -> bool {
        let Some(tokens) = Token::lex_unpadded(text) else {
            return false;
        };
        tokens.iter().all(|(token, _span)| {
            let is_delimiter = matches!(
                token,
                Token::Comma | Token::LeftParen | Token::RightParen | Token::Quoted(..)
            );
            !is_delimiter
        })
    }
}

impl ItemLabel {
    /// Parse `: LABEL`: one quoted string, decoded, or the bare text from the `:` to `trailing_group`, else to the end of the line.
    fn parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<TrailingGroup>,
    ) -> impl Parser<'tokens, I, Spanned<String>, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let label_token = any()
            .map_with(|token: Token<'src>, extra| -> (Token<'src>, Span) { (token, extra.span()) })
            .filter(move |(_token, span)| {
                trailing_group.is_none_or(|group| span.end <= group.span.start)
            });
        let label = label_token
            .repeated()
            .at_least(1)
            .collect::<Vec<(Token<'src>, Span)>>()
            .validate(move |label_tokens, extra, emitter| {
                let label_span: Span = extra.span();
                let label_text = match label_tokens.as_slice() {
                    [(Token::Quoted(raw), _quoted_span)] => Token::decode_quoted(raw),
                    _bare_tokens => line.slice(label_span).to_owned(),
                };
                let holds_quoted = label_tokens.len() > 1
                    && label_tokens
                        .iter()
                        .any(|(token, _span)| matches!(token, Token::Quoted(..)));
                if holds_quoted {
                    let error = Rich::custom(label_span, PARTLY_QUOTED_LABEL);
                    emitter.emit(error);
                }
                label_text
            })
            .spanned()
            .labelled("a label");
        just(Token::Colon).ignore_then(label)
    }

    /// Tell whether `text`, written bare after an item's `:`, reads back as itself: it lexes, holds no `"`, no space pads it and no parenthesised group ends it.
    pub(crate) fn reads_back_bare(text: &str) -> bool {
        let Some(tokens) = Token::lex_unpadded(text) else {
            return false;
        };
        let is_quote_free = tokens
            .iter()
            .all(|(token, _span)| !matches!(token, Token::Quoted(..)));
        let trailing_group = TrailingGroup::find(&tokens);
        let is_group_free_at_end = trailing_group.is_none();
        is_quote_free && is_group_free_at_end
    }
}

impl Reference {
    /// Parse `ID(.ID)*`.
    fn parser<'tokens, 'src: 'tokens, I>()
    -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        Token::name_parser()
            .map(|segment| segment.inner)
            .separated_by(just(Token::Dot))
            .at_least(1)
            .collect::<Vec<String>>()
            .map_with(|segments, extra| {
                let span = extra.span();
                Self { segments, span }
            })
    }
}

impl<'src> Token<'src> {
    /// Return the name `line` opens with, the ID a broken component item is kept by; `None` when no name opens it.
    fn leading_name(line: SourceText<'_>) -> Option<Spanned<String>> {
        let mut tokens = Token::lexer(line.text).spanned();
        let (first_token, line_range) = tokens.next()?;
        let Ok(Token::Name(name)) = first_token else {
            return None;
        };
        let start = line.offset + line_range.start;
        let end = line.offset + line_range.end;
        let name_span = Span::from(start..end);
        let name_text = name.to_owned();
        let leading_name = Spanned {
            inner: name_text,
            span: name_span,
        };
        Some(leading_name)
    }

    /// Return the text of a heading before its first `:`, trimmed, its shape left to the model's ID check; `None` when blank.
    fn raw_heading_id(heading: SourceText<'_>) -> Option<Spanned<String>> {
        let colon_index = heading.text.find(':').unwrap_or(heading.text.len());
        let before_colon = &heading.text[..colon_index];
        let id_text = before_colon.trim();
        let is_blank = id_text.is_empty();
        if is_blank {
            return None;
        }
        let leading_space_length = before_colon.len() - before_colon.trim_start().len();
        let start = heading.offset + leading_space_length;
        let end = start + id_text.len();
        let id_span = Span::from(start..end);
        let raw_id = Spanned {
            inner: id_text.to_owned(),
            span: id_span,
        };
        Some(raw_id)
    }

    /// Parse a heading's ID, the tokens before its `:` verbatim, its shape left to the model's ID check.
    fn heading_id_parser<'tokens, I>(
        line: SourceText<'src>,
    ) -> impl Parser<'tokens, I, Spanned<String>, ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        none_of([Token::Colon])
            .repeated()
            .at_least(1)
            .map_with(move |_id_tokens, extra| {
                let id_span: Span = extra.span();
                let id_text = line.slice(id_span).to_owned();
                Spanned {
                    inner: id_text,
                    span: id_span,
                }
            })
            .labelled("an ID")
    }

    /// Parse `[n]`, n ASCII digits whose number fits a `u32`, the ordinal of an edge listed again.
    fn ordinal_parser<'tokens, I>()
    -> impl Parser<'tokens, I, Spanned<u32>, ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let digits = select! { Token::Other(text) => text }.labelled("a whole number");
        let ordinal = digits.try_map(|digits: &str, digits_span: Span| {
            let has_non_digit = digits.bytes().any(|byte| !byte.is_ascii_digit());
            if has_non_digit {
                let not_digits_error = Rich::custom(digits_span, "`[n]` takes a whole number");
                return Err(not_digits_error);
            }
            let parsed_ordinal: Result<u32, ParseIntError> = digits.parse();
            parsed_ordinal.map_err(|_overflow| {
                let message = format!("`[n]` is at most {}", u32::MAX);
                Rich::custom(digits_span, message)
            })
        });
        ordinal
            .delimited_by(just(Token::LeftBracket), just(Token::RightBracket))
            .spanned()
            .labelled("`[n]`")
    }

    /// Parse one name: an ID, a key segment or a style name.
    fn name_parser<'tokens, I>()
    -> impl Parser<'tokens, I, Spanned<String>, ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let name = select! { Token::Name(name) => name.to_owned() };
        let checked_name = name.validate(|name, extra, emitter| {
            let ends_with_dash = name.ends_with('-');
            let holds_double_dash = name.contains("--");
            let is_dashed = ends_with_dash || holds_double_dash;
            if is_dashed {
                let name_span: Span = extra.span();
                let error = Rich::custom(name_span, DASHED_NAME);
                emitter.emit(error);
            }
            name
        });
        checked_name.spanned().labelled("a name")
    }

    /// Lex `text` on its own into tokens covering it end to end, `None` when it is empty, padded with spaces or holds a stretch that is not a token.
    fn lex_unpadded(text: &'src str) -> Option<Vec<(Self, Span)>> {
        let lexer = Self::lexer(text).spanned();
        let lexed_tokens = lexer.map(|(lexed, range)| {
            let span = Span::from(range);
            lexed.ok().map(|token| (token, span))
        });
        let tokens: Vec<(Self, Span)> = lexed_tokens.collect::<Option<_>>()?;
        let (_first_token, first_span) = tokens.first()?;
        let (_last_token, last_span) = tokens.last()?;
        let is_unpadded = first_span.start == 0 && last_span.end == text.len();
        is_unpadded.then_some(tokens)
    }

    /// Describe a parse error as the author reads it: what was found, and what the line could hold there.
    fn describe_rich_error(rich_error: &Rich<'_, Token<'src>, Span>) -> String {
        let reason = rich_error.reason();
        if let RichReason::Custom(message) = reason {
            return message.clone();
        }
        let found = rich_error.found();
        let found_phrase = found.map_or_else(|| END_OF_LINE.to_owned(), Token::to_string);
        let expected_phrases: Vec<String> = rich_error
            .expected()
            .filter_map(|pattern| match pattern {
                RichPattern::Token(token) => Some(token.to_string()),
                RichPattern::Label(label) => Some(label.to_string()),
                RichPattern::EndOfInput => Some(END_OF_LINE.to_owned()),
                _unnamed_pattern => None,
            })
            .collect();
        let expected_list = ProseList::join(&expected_phrases, "or");
        format!("unexpected {found_phrase}: expected {expected_list}")
    }
}

impl TrailingGroup {
    /// Find the trailing group of a line's `tokens`, the final balanced `(…)` whose `)` is the last token, telling whether it may end a label.
    fn find(tokens: &[(Token<'_>, Span)]) -> Option<Self> {
        let (last_token, last_span) = tokens.last()?;
        let is_unclosed_at_end = *last_token != Token::RightParen;
        if is_unclosed_at_end {
            return None;
        }
        let mut depth = 0_usize;
        for (group_start_index, (token, span)) in tokens.iter().enumerate().rev() {
            match token {
                Token::RightParen => depth += 1,
                Token::LeftParen => depth -= 1,
                _other_token => continue,
            }
            let is_unbalanced = depth != 0;
            if is_unbalanced {
                continue;
            }
            let (tokens_before_group, group_tokens) = tokens.split_at(group_start_index);
            let is_after_label = tokens_before_group
                .iter()
                .any(|(token, _span)| *token == Token::Colon);
            let holds_no_assignment = group_tokens
                .iter()
                .all(|(token, _span)| *token != Token::Equals);
            let span = Span::from(span.start..last_span.end);
            let may_end_label = is_after_label && holds_no_assignment;
            let trailing_group = Self {
                span,
                may_end_label,
            };
            return Some(trailing_group);
        }
        None
    }

    /// Tell whether an error at `span` falls inside this group while it may end a label.
    fn may_end_label_around(&self, span: Span) -> bool {
        let is_inside = self.span.start <= span.start && span.end <= self.span.end;
        self.may_end_label && is_inside
    }
}
