//! Parse d2 tokens into a syntax tree of every d2 statement, recovering from each broken one.

use chumsky::input::ValueInput;
use chumsky::prelude::*;

use super::ast::{EdgeOperator, KeySegment, Scalar, Span};
use super::diagnostic::Diagnostics;
use super::lex::Token;

/// Type the parser extra every d2 parser shares: chumsky's rich errors over d2 tokens.
type ParserExtra<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, Span>>;

/// Represent one d2 statement as written, before the v1 subset is applied.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SyntaxStatement {
    Declaration(SyntaxDeclaration),
    EdgeReference(Span),
    Spread { value: SyntaxValue, span: Span },
}

/// Hold the declaration of a key or an edge chain, with whatever is assigned to it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SyntaxDeclaration {
    pub(crate) head: SyntaxHead,
    pub(crate) assignment: SyntaxAssignment,
    pub(crate) span: Span,
}

/// Head a declaration with what it names: one key, or an edge chain between keys.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SyntaxHead {
    Key(SyntaxKey),
    Edges {
        source: SyntaxKey,
        links: Vec<SyntaxLink>,
    },
}

/// Hold the assignment following a declaration's head: its `: value` and its `{ map }`, either one optional.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SyntaxAssignment {
    pub(crate) value: Option<SyntaxValue>,
    pub(crate) map: Option<Vec<SyntaxStatement>>,
}

/// Link one hop of an edge chain to its target key.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SyntaxLink {
    pub(crate) operator: EdgeOperator,
    pub(crate) operator_span: Span,
    pub(crate) target: SyntaxKey,
}

/// Spell a dotted key, flagged when it carries a glob or a `&`/`!&` filter.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SyntaxKey {
    pub(crate) is_filtered: bool,
    pub(crate) segments: Vec<SyntaxSegment>,
    pub(crate) span: Span,
}

/// Spell one key segment, flagged when it contains a `*` glob or is an unquoted `_` naming the parent scope.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SyntaxSegment {
    pub(crate) segment: KeySegment,
    pub(crate) is_glob: bool,
    pub(crate) is_parent_reference: bool,
}

/// Represent a value as written; only `Scalar` survives the v1 subset, and an `Array` of scalars only in the `svugy` carrier.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SyntaxValue {
    Scalar(Scalar),
    Null(Span),
    BlockString(Span),
    Import(Span),
    Substitution(Span),
    Array(SyntaxArray),
    /// Mark a `{ … }` map, which only an array holds as a value.
    Map(Span),
}

/// List the items of a `[ … ]` array as written, in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SyntaxArray {
    pub(crate) items: Vec<SyntaxValue>,
    pub(crate) span: Span,
}

impl SyntaxStatement {
    /// Parse every statement of a token stream, keeping the valid ones and recording one error per broken one.
    pub(crate) fn parse_all<'src>(
        tokens: &[(Token<'src>, Span)],
        source: &'src str,
        diagnostics: &mut Diagnostics,
    ) -> Vec<Self> {
        let source_length = source.len();
        let end_of_input = Span::new(source_length, source_length);
        let input = tokens.map(end_of_input, |(token, span)| (token, span));
        let document_parser = Self::document_parser(source);
        let parse_result = document_parser.parse(input);
        let (statements, rich_errors) = parse_result.into_output_errors();
        for rich_error in rich_errors {
            let message = rich_error.reason().to_string();
            let span = *rich_error.span();
            diagnostics.record_error(message, span);
        }
        statements.unwrap_or_default()
    }

    /// Parse the whole document's statement list, reporting each unmatched `}` and carrying on after it.
    fn document_parser<'tokens, 'src: 'tokens, I>(
        source: &'src str,
    ) -> impl Parser<'tokens, I, Vec<Self>, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let unmatched_closer = just(Token::RightBrace).validate(|_closer, extra, emitter| {
            let closer_span = extra.span();
            let error = Rich::custom(closer_span, "unmatched `}`");
            emitter.emit(error);
        });
        let end_of_input = end();
        Self::list_parser(source)
            .separated_by(unmatched_closer)
            .collect::<Vec<Vec<Self>>>()
            .map(|lists| lists.into_iter().flatten().collect())
            .then_ignore(end_of_input)
    }

    /// Parse statements up to the next `}` or the end, skipping each broken statement to its terminator.
    fn list_parser<'tokens, 'src: 'tokens, I>(
        source: &'src str,
    ) -> Boxed<'tokens, 'tokens, I, Vec<Self>, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        recursive(|statement_list| {
            let left_brace = just(Token::LeftBrace);
            let right_brace = just(Token::RightBrace);
            let map = statement_list.delimited_by(left_brace, right_brace);
            let declaration = SyntaxDeclaration::parser(source, map);
            let reference_trees = Token::balanced_tree_parser().repeated();
            let edge_reference = just(Token::LeftParen)
                .then(reference_trees)
                .to_span()
                .map(Self::EdgeReference);
            let spread_value = SyntaxValue::parser(source);
            let spread = just(Token::Spread)
                .ignore_then(spread_value)
                .map_with(|value, extra| {
                    let span = extra.span();
                    Self::Spread { value, span }
                });
            let skipped_statement = Self::skipped_statement_parser();
            let skip_broken_statement = via_parser(skipped_statement);
            let terminator = Token::terminator_parser();
            let statement = choice((spread, edge_reference, declaration))
                .then_ignore(terminator)
                .map(Some)
                .recover_with(skip_broken_statement);
            let separator = Token::separator_parser().to(None);
            choice((separator, statement))
                .repeated()
                .collect::<Vec<Option<Self>>>()
                .map(|statements| statements.into_iter().flatten().collect())
        })
        .boxed()
    }

    /// Skip a broken statement: up to its terminator when its brackets balance, otherwise to the end of its line.
    fn skipped_statement_parser<'tokens, 'src: 'tokens, I>()
    -> impl Parser<'tokens, I, Option<Self>, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let terminator = Token::terminator_parser();
        let balanced_statement = Token::balanced_tree_parser()
            .repeated()
            .at_least(1)
            .then_ignore(terminator);
        let rest_of_line = none_of([Token::Newline, Token::Semicolon, Token::RightBrace])
            .repeated()
            .at_least(1);
        choice((balanced_statement, rest_of_line)).to(None)
    }
}

impl SyntaxDeclaration {
    /// Parse `head [: value] [{ map }]`, where the head is a key or an edge chain `key (op key)+`.
    fn parser<'tokens, 'src: 'tokens, I, M>(
        source: &'src str,
        map: M,
    ) -> impl Parser<'tokens, I, SyntaxStatement, ParserExtra<'tokens, 'src>> + Clone
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
        M: Parser<'tokens, I, Vec<SyntaxStatement>, ParserExtra<'tokens, 'src>> + Clone,
    {
        let key = SyntaxKey::parser(source);
        let operator = select! { Token::Edge(operator) => operator }
            .labelled("edge operator")
            .map_with(|operator, extra| {
                let operator_span: Span = extra.span();
                (operator, operator_span)
            });
        let target_key = key.clone();
        let link = operator
            .then(target_key)
            .map(|((operator, operator_span), target)| SyntaxLink {
                operator,
                operator_span,
                target,
            });
        let links = link.repeated().collect::<Vec<SyntaxLink>>();
        let head = key
            .then(links)
            .map(|(source, links)| SyntaxHead::from_chain(source, links));
        let optional_value = SyntaxValue::parser(source).or_not();
        let value = just(Token::Colon).ignore_then(optional_value);
        let optional_map = map.or_not();
        let assignment = value.or_not().then(optional_map).map(|(value, map)| {
            let value = value.flatten();
            SyntaxAssignment { value, map }
        });
        head.then(assignment).map_with(|(head, assignment), extra| {
            let span = extra.span();
            let declaration = Self {
                head,
                assignment,
                span,
            };
            SyntaxStatement::Declaration(declaration)
        })
    }
}

impl SyntaxHead {
    /// Head a declaration from its parsed chain: a lone key without links, else the edge chain.
    fn from_chain(source: SyntaxKey, links: Vec<SyntaxLink>) -> Self {
        let is_single_key = links.is_empty();
        if is_single_key {
            return Self::Key(source);
        }
        Self::Edges { source, links }
    }
}

impl SyntaxKey {
    /// Parse `[&|!&] segment (. segment)*`, a segment being one quoted string or a run of unquoted pieces.
    fn parser<'tokens, 'src: 'tokens, I>(
        source: &'src str,
    ) -> Boxed<'tokens, 'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let quoted_segment = select! {
            Token::DoubleQuoted(raw) => Token::decode_double_quoted(raw),
            Token::SingleQuoted(raw) => Token::decode_single_quoted(raw),
        }
        .map_with(|name, extra| {
            let span = extra.span();
            let segment = KeySegment { name, span };
            SyntaxSegment {
                segment,
                is_glob: false,
                is_parent_reference: false,
            }
        });
        let unquoted_piece_is_glob = select! {
            Token::Word => false,
            Token::Dash => false,
            Token::Dollar => false,
            Token::Pipe => false,
            Token::Star => true,
        };
        let unquoted_segment = unquoted_piece_is_glob
            .repeated()
            .at_least(1)
            .collect::<Vec<bool>>()
            .map_with(move |piece_is_glob, extra| {
                let span: Span = extra.span();
                let raw_range = span.range();
                let raw = &source[raw_range];
                let is_parent_reference = raw == "_";
                let name = Token::decode_escapes(raw);
                let segment = KeySegment { name, span };
                let is_glob = piece_is_glob.contains(&true);
                SyntaxSegment {
                    segment,
                    is_glob,
                    is_parent_reference,
                }
            });
        let segment = choice((quoted_segment, unquoted_segment));
        let ampersand = just(Token::Ampersand);
        let not_ampersand = just(Token::NotAmpersand);
        let filter = choice((ampersand, not_ampersand));
        let dot = just(Token::Dot);
        let segments = segment
            .separated_by(dot)
            .at_least(1)
            .collect::<Vec<SyntaxSegment>>();
        filter
            .or_not()
            .then(segments)
            .map_with(|(filter, segments), extra| {
                let is_filtered = filter.is_some();
                let span = extra.span();
                Self {
                    is_filtered,
                    segments,
                    span,
                }
            })
            .labelled("key")
            .boxed()
    }
}

impl SyntaxValue {
    /// Parse a value up to the next separator, brace or bracket, or one `[ … ]` array of values and maps split by separators.
    fn parser<'tokens, 'src: 'tokens, I>(
        source: &'src str,
    ) -> Boxed<'tokens, 'tokens, I, Self, ParserExtra<'tokens, 'src>>
    where
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        recursive(|value| {
            let separator = Token::separator_parser().to(None);
            let braced_tree = Token::balanced_tree_parser();
            let map = just(Token::LeftBrace)
                .rewind()
                .ignore_then(braced_tree)
                .to_span()
                .map(Self::Map);
            let item = choice((map, value)).map(Some);
            let left_bracket = just(Token::LeftBracket);
            let right_bracket = just(Token::RightBracket);
            let array = choice((item, separator))
                .repeated()
                .collect::<Vec<Option<Self>>>()
                .delimited_by(left_bracket, right_bracket)
                .map_with(|items, extra| {
                    let items = items.into_iter().flatten().collect();
                    let span = extra.span();
                    let array = SyntaxArray { items, span };
                    Self::Array(array)
                });
            let run = none_of(Token::SEPARATORS_AND_BRACKETS)
                .repeated()
                .at_least(1)
                .collect::<Vec<Token<'src>>>()
                .map_with(move |pieces, extra| {
                    let span = extra.span();
                    Self::from_run(&pieces, span, source)
                });
            choice((array, run)).labelled("value")
        })
        .boxed()
    }

    /// Read a run of value tokens: a lone quoted string or block string, an `@` import, a `${…}` substitution, else unquoted text.
    fn from_run(pieces: &[Token<'_>], span: Span, source: &str) -> Self {
        match pieces {
            [Token::DoubleQuoted(raw)] => {
                let text = Token::decode_double_quoted(raw);
                let scalar = Scalar { text, span };
                Self::Scalar(scalar)
            }
            [Token::SingleQuoted(raw)] => {
                let text = Token::decode_single_quoted(raw);
                let scalar = Scalar { text, span };
                Self::Scalar(scalar)
            }
            [Token::BlockString] => Self::BlockString(span),
            [Token::At, ..] => Self::Import(span),
            substituted_pieces if substituted_pieces.contains(&Token::Substitution) => {
                Self::Substitution(span)
            }
            _unquoted_pieces => {
                let raw_range = span.range();
                let raw = &source[raw_range];
                if raw == "null" {
                    return Self::Null(span);
                }
                let text = Token::decode_escapes(raw);
                let scalar = Scalar { text, span };
                Self::Scalar(scalar)
            }
        }
    }
}

impl<'src> Token<'src> {
    /// List the tokens that end a value: the two separators and every bracket.
    const SEPARATORS_AND_BRACKETS: [Self; 6] = [
        Self::Newline,
        Self::Semicolon,
        Self::LeftBrace,
        Self::RightBrace,
        Self::LeftBracket,
        Self::RightBracket,
    ];

    /// Match one statement separator: a newline or a `;`.
    fn separator_parser<'tokens, I>()
    -> impl Parser<'tokens, I, Token<'src>, ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let newline = just(Token::Newline);
        let semicolon = just(Token::Semicolon);
        choice((newline, semicolon))
    }

    /// Match one balanced tree: a token outside brackets, or a whole `{ … }` / `[ … ]` group, separators inside it included.
    fn balanced_tree_parser<'tokens, I>()
    -> impl Parser<'tokens, I, (), ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        recursive(|tree| {
            let separator = Self::separator_parser().ignored();
            let contents = choice((tree, separator)).repeated();
            let left_brace = just(Token::LeftBrace);
            let right_brace = just(Token::RightBrace);
            let braced = contents.clone().delimited_by(left_brace, right_brace);
            let left_bracket = just(Token::LeftBracket);
            let right_bracket = just(Token::RightBracket);
            let bracketed = contents.delimited_by(left_bracket, right_bracket);
            let plain = none_of(Self::SEPARATORS_AND_BRACKETS).ignored();
            choice((braced, bracketed, plain))
        })
    }

    /// Look ahead, without consuming, for a statement terminator: a separator, a `}` or the end.
    fn terminator_parser<'tokens, I>()
    -> impl Parser<'tokens, I, (), ParserExtra<'tokens, 'src>> + Clone
    where
        'src: 'tokens,
        I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
    {
        let separator = Self::separator_parser();
        let right_brace = just(Token::RightBrace);
        let closing_token = choice((separator, right_brace)).ignored();
        let end_of_input = end();
        choice((closing_token, end_of_input))
            .labelled("end of statement")
            .rewind()
    }
}

impl SyntaxValue {
    /// Return the span of the value as written.
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Scalar(scalar) => scalar.span,
            Self::Array(array) => array.span,
            Self::Null(span)
            | Self::BlockString(span)
            | Self::Import(span)
            | Self::Substitution(span)
            | Self::Map(span) => *span,
        }
    }
}

impl SyntaxStatement {
    /// Return the span of the statement as written.
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Declaration(declaration) => declaration.span,
            Self::EdgeReference(span) => *span,
            Self::Spread {
                value: _value,
                span,
            } => *span,
        }
    }
}
