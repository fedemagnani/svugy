//! Prove each svugy-parse dependency covers its intended use.

use chumsky::input::{Stream, ValueInput};
use chumsky::prelude::*;
use logos::Logos;
use miette::SourceSpan;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
enum Token<'src> {
    Error,
    #[regex(r"[A-Za-z_][A-Za-z0-9_.]*")]
    Ident(&'src str),
    #[token("->")]
    Arrow,
    #[token("\n")]
    Newline,
}

#[derive(Debug, Clone, PartialEq)]
enum Statement<'src> {
    Edge { from: &'src str, to: &'src str },
    Invalid,
}

fn lex_spanned(source: &str) -> Vec<(Token<'_>, SimpleSpan)> {
    let spanned_results = Token::lexer(source).spanned();
    spanned_results
        .map(|(lexed, byte_range)| {
            let token = lexed.unwrap_or(Token::Error);
            let span = SimpleSpan::from(byte_range);
            (token, span)
        })
        .collect()
}

fn statements_parser<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Statement<'src>>, extra::Err<Rich<'tokens, Token<'src>>>>
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>,
{
    let ident = select! { Token::Ident(name) => name };
    let edge = ident
        .then_ignore(just(Token::Arrow))
        .then(ident)
        .map(|(from, to)| Statement::Edge { from, to });
    let rest_of_line = none_of(Token::Newline)
        .repeated()
        .at_least(1)
        .to(Statement::Invalid);
    let statement = edge.recover_with(via_parser(rest_of_line));
    statement
        .separated_by(just(Token::Newline))
        .allow_trailing()
        .collect()
}

#[test]
fn logos_lexes_tokens_with_byte_spans() {
    let tokens = lex_spanned("api -> db.primary");

    let lexed_tokens: Vec<Token> = tokens.iter().map(|(token, _span)| token.clone()).collect();
    let db_span = tokens[2].1;
    assert_eq!(
        lexed_tokens,
        [
            Token::Ident("api"),
            Token::Arrow,
            Token::Ident("db.primary")
        ]
    );
    assert_eq!(db_span.into_range(), 7..17);
}

#[test]
fn chumsky_reports_every_error_with_recovery() {
    let source = "a -> b\nc ->\nd e\nf -> g";
    let tokens = lex_spanned(source);
    let end_of_input = SimpleSpan::from(source.len()..source.len());
    let token_stream = Stream::from_iter(tokens).map(end_of_input, |(token, span)| (token, span));

    let parse_result = statements_parser().parse(token_stream);

    let (partial_output, errors) = parse_result.into_output_errors();
    let statements = partial_output.expect("recovery yields a partial output");
    assert_eq!(errors.len(), 2);
    assert_eq!(
        statements,
        [
            Statement::Edge { from: "a", to: "b" },
            Statement::Invalid,
            Statement::Invalid,
            Statement::Edge { from: "f", to: "g" },
        ]
    );
}

#[test]
fn miette_labels_a_chumsky_span() {
    let chumsky_span = SimpleSpan::from(7..17);

    let byte_range = chumsky_span.into_range();
    let miette_span = SourceSpan::from(byte_range);

    assert_eq!(miette_span.offset(), 7);
    assert_eq!(miette_span.len(), 10);
}
