//! Parse each verbatim item line of the skeleton, and each style of the frontmatter, into the document's items, recovering at every broken entry.

use chumsky::error::{RichPattern, RichReason};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use chumsky::span::Spanned;

use super::ast::{
    Assignment, ComponentItem, Document, EdgeItem, Entry, LegendItem, LegendKeyRow, LegendSection,
    Reference, Span, StyleDefinition, Value,
};
use super::diagnostic::{Diagnostics, ProseList};
use super::frontmatter::{Frontmatter, StyleSource};
use super::lex::Token;
use super::skeleton::{ComponentNode, LegendSkeleton, Skeleton, SourceText};

/// Type the parser extra every line parser shares: chumsky's rich errors over line tokens.
type ParserExtra<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, Span>>;

/// Name the end of an item line, where a parse error may find nothing more.
const END_OF_LINE: &str = "the end of the line";
/// Spell the opening of a legend key row, `(line=…): description`; any other legend line is free text.
const KEY_ROW_OPENING: char = '(';

/// Parse one verbatim line into the item it declares.
trait LineGrammar: Sized {
    /// Build the parser of the whole of `line`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>;

    /// Lex and parse `line`, recording every error; `None` when nothing of it parses.
    fn parse_line(line: SourceText<'_>, diagnostics: &mut Diagnostics) -> Option<Self> {
        let tokens = Token::lex_line(line, diagnostics)?;
        let line_end = line.span().end;
        let end_of_line = Span::from(line_end..line_end);
        let input = tokens
            .as_slice()
            .map(end_of_line, |(token, span)| (token, span));
        let parser = Self::line_parser(line);
        let parse_result = parser.parse(input);
        let (item, rich_errors) = parse_result.into_output_errors();
        for rich_error in rich_errors {
            let message = Token::describe_rich_error(&rich_error);
            let span = *rich_error.span();
            diagnostics.record(message, span);
        }
        item
    }
}

impl Document {
    /// Parse the skeleton's frontmatter and every item line into the document, recording every error.
    pub(crate) fn parse(skeleton: Skeleton<'_>, diagnostics: &mut Diagnostics) -> Self {
        let Skeleton {
            frontmatter,
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
            .filter_map(|style| StyleDefinition::parse(style, diagnostics))
            .collect();
        let components = ComponentItem::parse_nodes(components, diagnostics);
        let edges = edges
            .into_iter()
            .filter_map(|line| EdgeItem::parse_line(line, diagnostics))
            .collect();
        let legend = legend.map(|legend| LegendSection::parse(legend, diagnostics));
        Self {
            title,
            subtitle,
            styles,
            components,
            edges,
            legend,
        }
    }
}

impl StyleDefinition {
    /// Parse a style definition's properties list.
    fn parse(source: StyleSource, diagnostics: &mut Diagnostics) -> Option<Self> {
        let StyleSource {
            name,
            properties,
            properties_offset,
        } = source;
        let properties_text = SourceText {
            text: &properties,
            offset: properties_offset,
        };
        let entries = Vec::<Entry>::parse_line(properties_text, diagnostics)?;
        Some(Self { name, entries })
    }
}

impl ComponentItem {
    /// Parse each component node into its item, children first, recording each item a comment split off its parent's list.
    fn parse_nodes(nodes: Vec<ComponentNode<'_>>, diagnostics: &mut Diagnostics) -> Vec<Self> {
        let mut items = Vec::new();
        for node in nodes {
            let ComponentNode {
                line,
                is_indented_but_not_nested,
                children,
            } = node;
            let children = Self::parse_nodes(children, diagnostics);
            let Some(item) = Self::parse_line(line, diagnostics) else {
                continue;
            };
            if is_indented_but_not_nested {
                let id = &item.id.inner;
                let message = format!(
                    "`{id}` is indented but not nested: the comment above ends its parent's list; indent the comment like the item"
                );
                diagnostics.record(message, item.span);
            }
            let item = item.with_children(children);
            items.push(item);
        }
        items
    }

    /// Nest `children` under the item.
    fn with_children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }
}

impl LineGrammar for ComponentItem {
    /// Parse `ID[: "DISPLAY NAME"] [(PROPERTIES)]`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let id = Token::name_parser();
        let display_name = just(Token::Colon).ignore_then(Token::quoted_parser());
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
                    children: Vec::new(),
                    span: line.span(),
                }
            })
    }
}

impl LineGrammar for EdgeItem {
    /// Parse `REF -> REF ... [: "LABEL"] [(PROPERTIES)]`.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
    ) -> impl Parser<'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let hops = just(Token::Arrow)
            .ignore_then(Reference::parser())
            .repeated()
            .at_least(1)
            .collect::<Vec<Reference>>();
        let label = just(Token::Colon).ignore_then(Token::quoted_parser().spanned());
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
                }
            })
    }
}

impl LegendSection {
    /// Parse each legend line: a key row when it opens with `(`, free text otherwise.
    fn parse(legend: LegendSkeleton<'_>, diagnostics: &mut Diagnostics) -> Self {
        let LegendSkeleton { title, lines } = legend;
        let title = title.map(str::to_owned);
        let mut items = Vec::new();
        for line in lines {
            let is_key_row = line.text.starts_with(KEY_ROW_OPENING);
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

impl LineGrammar for LegendKeyRow {
    /// Parse `(PROPERTIES): DESCRIPTION`, the description read verbatim to the end of the line.
    fn line_parser<'tokens, 'src: 'tokens, I>(
        line: SourceText<'src>,
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
    /// Parse a quoted value, or bare pieces: the first one, then each one after a comma that cannot start an entry.
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
                Token::Comma | Token::LeftParen | Token::RightParen | Token::Quoted(_)
            );
            !is_delimiter
        });
        let bare_piece = bare_token
            .map_with(|_token, extra| -> Span { extra.span() })
            .repeated()
            .at_least(1)
            .collect::<Vec<Span>>()
            .validate(move |token_spans, extra, emitter| {
                let piece_span: Span = extra.span();
                let has_gap = token_spans
                    .windows(2)
                    .any(|adjacent| adjacent[0].end < adjacent[1].start);
                if has_gap {
                    let message = "a bare value holds no spaces: quote it";
                    let error = Rich::custom(piece_span, message);
                    emitter.emit(error);
                }
                line.slice(piece_span).to_owned()
            });
        let piece_after_comma_start = any()
            .filter(|token: &Token<'src>| !matches!(token, Token::Name(_)))
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
        select! { Token::Name(name) => name.to_owned() }
            .spanned()
            .labelled("a name")
    }

    /// Parse one quoted string, decoded.
    fn quoted_parser<'tokens, I>()
    -> impl Parser<'tokens, I, String, ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        select! { Token::Quoted(raw) => Token::decode_quoted(raw) }.labelled("a quoted string")
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
