//! Parse each verbatim item line of the skeleton, and each style of the frontmatter, into the document's items, recovering at every broken entry.

use chumsky::error::{RichPattern, RichReason};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use chumsky::span::Spanned;
use logos::Logos;

use super::ast::{
    Assignment, ComponentItem, Document, EdgeItem, Entry, LegendItem, LegendKeyRow, LegendSection,
    Reference, Span, StyleDefinition, Value,
};
use super::diagnostic::Diagnostics;
use super::frontmatter::{Frontmatter, StyleSource};
use super::lex::Token;
use super::skeleton::{ComponentNode, DescribedLine, LegendSkeleton, Skeleton, SourceText};
use crate::prose::ProseList;

/// Type the parser extra every line parser shares: chumsky's rich errors over line tokens.
type ParserExtra<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, Span>>;

/// Name the end of an item line, where a parse error may find nothing more.
const END_OF_LINE: &str = "end of line";
/// Spell the error of a name ending with `-` or holding `--`, which the lexer keeps whole so the item still parses.
const DASHED_NAME: &str = "an ID cannot end with `-` or hold `--`";
/// Spell the error of a bare label holding a quoted string.
const PARTLY_QUOTED_LABEL: &str = "a label is either quoted whole or holds no `\"`";
/// Spell the hint added to an error inside the trailing group after a label, which a label ending in parentheses runs into.
const PARENTHESISED_LABEL_HINT: &str = "a label ending in parentheses needs quotes";

/// Read an item's label: one quoted string, or the bare text up to the item's trailing property group.
pub(crate) struct ItemLabel;

/// Parse one verbatim line into the item it declares.
trait LineGrammar: Sized {
    /// Build the parser of the whole of `line`, whose trailing property group, if any, spans `trailing_group`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<Span>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>;

    /// Lex and parse `line`, recording every error, hinted at quotes inside a trailing group after a `:`; `None` when nothing of it parses.
    fn parse_line(line: SourceText<'_>, diagnostics: &mut Diagnostics) -> Option<Self> {
        let tokens = Token::lex_line(line, diagnostics)?;
        let trailing_group = Token::trailing_group_span(&tokens);
        let label_hinted_group = trailing_group.filter(|group| {
            let mut tokens_before_group = tokens
                .iter()
                .take_while(|(_token, span)| span.end <= group.start);
            tokens_before_group.any(|(token, _span)| *token == Token::Colon)
        });
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
            let is_in_label_hinted_group = label_hinted_group
                .is_some_and(|group| group.start <= span.start && span.end <= group.end);
            let message = if is_in_label_hinted_group {
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
    /// Parse the skeleton's frontmatter and every item line into the document, recording every error.
    pub(crate) fn parse(skeleton: Skeleton<'_>, diagnostics: &mut Diagnostics) -> Self {
        let Skeleton {
            frontmatter,
            description,
            components,
            edges,
            legend,
        } = skeleton;
        let frontmatter = frontmatter.map(|body| Frontmatter::read(body, diagnostics));
        let Frontmatter {
            title,
            subtitle,
            styles,
        } = frontmatter.unwrap_or_default();
        let styles = styles
            .into_iter()
            .map(|style| StyleDefinition::parse(style, diagnostics))
            .collect();
        let components = ComponentItem::parse_nodes(components, diagnostics);
        let edges = edges
            .into_iter()
            .filter_map(|edge| EdgeItem::parse_described_line(edge, diagnostics))
            .collect();
        let legend = legend.map(|legend| LegendSection::parse(legend, diagnostics));
        Self {
            title,
            subtitle,
            description,
            styles,
            components,
            edges,
            legend,
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
            let description = description.map(|description| description.inner);
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
        let mut tokens = Token::lexer(line.text).spanned();
        let (first_token, line_range) = tokens.next()?;
        let (Ok(Token::Name(id)) | Ok(Token::DashedName(id))) = first_token else {
            return None;
        };
        let start = line.offset + line_range.start;
        let end = line.offset + line_range.end;
        let id_span = Span::from(start..end);
        let id_text = id.to_owned();
        let id = Spanned {
            inner: id_text,
            span: id_span,
        };
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
        trailing_group: Option<Span>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let id = Token::name_parser();
        let display_name = ItemLabel::parser(line, trailing_group).map(|label| label.inner);
        let properties = Entry::properties_parser(line);
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

impl EdgeItem {
    /// Parse an edge's line, then give it the description under it; `None` when nothing of the line parses.
    fn parse_described_line(edge: DescribedLine<'_>, diagnostics: &mut Diagnostics) -> Option<Self> {
        let DescribedLine { line, description } = edge;
        let edge_item = Self::parse_line(line, diagnostics)?;
        let described_edge_item = Self {
            description,
            ..edge_item
        };
        Some(described_edge_item)
    }
}

impl LineGrammar for EdgeItem {
    /// Parse `REF -> REF ... [: LABEL] [(PROPERTIES)]`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        trailing_group: Option<Span>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let hops = just(Token::Arrow)
            .ignore_then(Reference::parser())
            .repeated()
            .at_least(1)
            .collect::<Vec<Reference>>();
        let label = ItemLabel::parser(line, trailing_group);
        let properties = Entry::properties_parser(line);
        Reference::parser()
            .then(hops)
            .then(label.or_not())
            .then(properties.or_not())
            .then_ignore(end())
            .map(|(((source, targets), label), properties)| {
                let mut references = vec![source];
                references.extend(targets);
                Self {
                    references,
                    label,
                    properties,
                    description: None,
                }
            })
    }
}

impl LegendSection {
    /// Parse each legend line: a key row when it opens with `(KEY=`, free text otherwise.
    fn parse(legend: LegendSkeleton<'_>, diagnostics: &mut Diagnostics) -> Self {
        let LegendSkeleton { title, lines } = legend;
        let title = title.map(str::to_owned);
        let mut items = Vec::new();
        for line in lines {
            let is_key_row = LegendKeyRow::opens_with_assignment(line);
            if is_key_row {
                let key_row = LegendKeyRow::parse_line(line, diagnostics);
                items.extend(key_row.map(LegendItem::KeyRow));
                continue;
            }
            let free_line = line.text.to_owned();
            items.push(LegendItem::Line(free_line));
        }
        Self { title, items }
    }
}

impl LegendKeyRow {
    /// Tell whether `line` opens with `(` then a key and `=`, which only a key row does.
    fn opens_with_assignment(line: SourceText<'_>) -> bool {
        let mut tokens = Token::lexer(line.text);
        let first_token = tokens.next();
        let Some(Ok(Token::LeftParen)) = first_token else {
            return false;
        };
        let mut ends_with_key_segment = false;
        for token in tokens {
            match (token, ends_with_key_segment) {
                (Ok(Token::Name(_segment)), false) => ends_with_key_segment = true,
                (Ok(Token::Dot), true) => ends_with_key_segment = false,
                (Ok(Token::Equals), true) => return true,
                _other_token => return false,
            }
        }
        false
    }
}

impl LineGrammar for LegendKeyRow {
    /// Parse `(PROPERTIES): DESCRIPTION`, the description read verbatim to the end of the line.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        _trailing_group: Option<Span>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let colon_end = just(Token::Colon).map_with(|_colon, extra| {
            let colon_span: Span = extra.span();
            colon_span.end
        });
        let description = any().repeated();
        Entry::properties_parser(line)
            .then(colon_end)
            .then_ignore(description)
            .then_ignore(end())
            .map(move |(properties, colon_end)| {
                let description_span = Span::from(colon_end..line.span().end);
                let description = line.slice(description_span).trim().to_owned();
                Self {
                    entries: properties.inner,
                    description,
                    span: line.span(),
                }
            })
    }
}

impl LineGrammar for Vec<Entry> {
    /// Parse a style's whole value, one properties list.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
        _trailing_group: Option<Span>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        Entry::properties_parser(line)
            .then_ignore(end())
            .map(|properties| properties.inner)
    }
}

impl Entry {
    /// Parse `(ENTRY, ENTRY, …)`, skipping each broken entry to the next `,` or `)` after recording it.
    fn properties_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
    ) -> impl Parser<'tokens, I, Spanned<Vec<Self>>, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
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
        let style = Token::name_parser().map(Self::Style);
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
        trailing_group: Option<Span>,
    ) -> impl Parser<'tokens, I, Spanned<String>, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let label_token = any()
            .map_with(|token: Token<'src>, extra| -> (Token<'src>, Span) { (token, extra.span()) })
            .filter(move |(_token, span)| {
                trailing_group.is_none_or(|group| span.end <= group.start)
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
        let trailing_group = Token::trailing_group_span(&tokens);
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
    /// Parse one name: an ID, a key segment or a style name.
    fn name_parser<'tokens, I>()
    -> impl Parser<'tokens, I, Spanned<String>, ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let name = select! { Token::Name(name) => name.to_owned() };
        let dashed_name = select! { Token::DashedName(name) => name.to_owned() }.validate(
            |name, extra, emitter| {
                let name_span: Span = extra.span();
                let error = Rich::custom(name_span, DASHED_NAME);
                emitter.emit(error);
                name
            },
        );
        choice((name, dashed_name)).spanned().labelled("a name")
    }

    /// Find the span of the trailing property group of a line's `tokens`: the final `(…)`, balanced, whose `)` is the last token.
    fn trailing_group_span(tokens: &[(Self, Span)]) -> Option<Span> {
        let (last_token, last_span) = tokens.last()?;
        let is_unclosed_at_end = *last_token != Token::RightParen;
        if is_unclosed_at_end {
            return None;
        }
        let mut depth = 0_usize;
        for (token, span) in tokens.iter().rev() {
            match token {
                Token::RightParen => depth += 1,
                Token::LeftParen => depth -= 1,
                _other_token => continue,
            }
            let is_balanced = depth == 0;
            if is_balanced {
                let group_span = Span::from(span.start..last_span.end);
                return Some(group_span);
            }
        }
        None
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
