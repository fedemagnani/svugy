//! Split d2 source into tokens with byte spans, reporting unterminated strings as syntax errors.

use std::fmt;

use logos::Logos;

use super::ast::{EdgeOperator, Span};
use super::diagnostic::Diagnostics;

/// Tag one lexeme of d2 as a token; unquoted text is split into pieces the parser joins back by span.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(error = LexError)]
#[logos(skip r"[ \t\f\r]+")]
#[logos(skip r"\\\r?\n")]
#[logos(skip(r"#[^\n]*", allow_greedy = true))]
#[logos(skip r#""""([^"]|"[^"]|""[^"])*""""#)]
pub(crate) enum Token<'src> {
    #[regex(r"\r?\n")]
    Newline,
    #[token(";")]
    Semicolon,
    #[token("{")]
    LeftBrace,
    #[token("}")]
    RightBrace,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token("...")]
    Spread,
    #[token("&")]
    Ampersand,
    #[token("!&")]
    NotAmpersand,
    #[token("*")]
    Star,
    #[token("@")]
    At,
    #[token("$")]
    Dollar,
    #[token("|")]
    Pipe,
    #[token("-")]
    Dash,
    #[token("<")]
    LessThan,
    #[token(">")]
    GreaterThan,
    #[regex(r"--+|-+>|<-+|<-+>", |lexer| {
        let arrow = lexer.slice();
        EdgeOperator::from_arrow(arrow)
    })]
    Edge(EdgeOperator),
    #[regex(r"\$\{[^}\n]*\}")]
    Substitution,
    #[regex(r#""([^"\\\n]|\\(.|\n))*""#)]
    DoubleQuoted(&'src str),
    #[regex(r#""([^"\\\n]|\\(.|\n))*"#, LexError::reject_unterminated_string)]
    UnterminatedDoubleQuoted,
    #[regex(r"'([^'\n]|'')*'")]
    SingleQuoted(&'src str),
    #[regex(r"'([^'\n]|'')*", LexError::reject_unterminated_string)]
    UnterminatedSingleQuoted,
    #[regex(r#"([^\s;{}\[\]():.<>&'"|*@$\\#-]|\\[^\n])([^\s;{}\[\]():.<>&|*@$\\#-]|\\[^\n])*"#)]
    Word,
    BlockString,
}

/// Name the lex error that keeps a stretch of source from being a token.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum LexError {
    #[default]
    UnexpectedCharacter,
    UnterminatedString,
    UnterminatedBlockString,
}

impl<'src> Token<'src> {
    /// Lex all of `source` into tokens, recording one error per invalid stretch and turning a `|` right after `:` into a whole block string, as d2 only opens one at a value's start.
    pub(crate) fn lex_all(source: &'src str, diagnostics: &mut Diagnostics) -> Vec<(Self, Span)> {
        let mut lexer = Self::lexer(source);
        let mut tokens: Vec<(Self, Span)> = Vec::new();
        while let Some(lexed) = lexer.next() {
            let start = lexer.span().start;
            let last_token = tokens.last();
            let is_value_start = matches!(last_token, Some((Token::Colon, _colon_span)));
            let lexed = match lexed {
                Ok(Self::Pipe) if is_value_start => Self::bump_block_string(&mut lexer),
                other_lexed => other_lexed,
            };
            let end = lexer.span().end;
            let span = Span::new(start, end);
            match lexed {
                Ok(token) => tokens.push((token, span)),
                Err(error) => {
                    let message = error.message();
                    diagnostics.record_error(message, span);
                }
            }
        }
        tokens
    }

    /// Consume a block string whose opening `|` was just lexed: the closer is the opening quote run followed by `|`.
    fn bump_block_string(lexer: &mut logos::Lexer<'src, Self>) -> Result<Self, LexError> {
        let after_pipe = lexer.remainder();
        let after_pipe_length = after_pipe.len();
        let quote_length = after_pipe
            .find(|character: char| {
                let is_whitespace = character.is_whitespace();
                let is_alphanumeric = character.is_alphanumeric();
                let is_underscore = character == '_';
                is_whitespace || is_alphanumeric || is_underscore
            })
            .unwrap_or(after_pipe_length);
        let (quote, after_quote) = after_pipe.split_at(quote_length);
        let after_quote_length = after_quote.len();
        let tag_length = after_quote
            .find(char::is_whitespace)
            .unwrap_or(after_quote_length);
        let body = &after_quote[tag_length..];
        let closer = format!("{quote}|");
        let Some(closer_offset) = body.find(&closer) else {
            lexer.bump(after_pipe_length);
            return Err(LexError::UnterminatedBlockString);
        };
        let block_length = quote_length + tag_length + closer_offset + closer.len();
        lexer.bump(block_length);
        Ok(Self::BlockString)
    }

    /// Decode a double-quoted string, quotes included in `raw`, through its backslash escapes.
    pub(crate) fn decode_double_quoted(raw: &str) -> String {
        let closing_quote_index = raw.len() - 1;
        let inner = &raw[1..closing_quote_index];
        Self::decode_escapes(inner)
    }

    /// Decode a single-quoted string, quotes included in `raw`, where only a doubled `''` escapes.
    pub(crate) fn decode_single_quoted(raw: &str) -> String {
        let closing_quote_index = raw.len() - 1;
        let inner = &raw[1..closing_quote_index];
        inner.replace("''", "'")
    }

    /// Decode d2's backslash escapes (`\n`, `\t`, …; any other escaped character stands for itself; `\` + newline vanishes).
    pub(crate) fn decode_escapes(raw: &str) -> String {
        let raw_length = raw.len();
        let mut decoded = String::with_capacity(raw_length);
        let mut characters = raw.chars();
        while let Some(character) = characters.next() {
            let is_plain_character = character != '\\';
            if is_plain_character {
                decoded.push(character);
                continue;
            }
            let Some(escaped) = characters.next() else {
                decoded.push('\\');
                break;
            };
            let decoded_escape = match escaped {
                'a' => Some('\u{7}'),
                'b' => Some('\u{8}'),
                'f' => Some('\u{c}'),
                'n' => Some('\n'),
                'r' => Some('\r'),
                't' => Some('\t'),
                'v' => Some('\u{b}'),
                '\n' => None,
                other => Some(other),
            };
            decoded.extend(decoded_escape);
        }
        decoded
    }
}

impl LexError {
    /// Reject an unterminated string as a lex error, so logos reports it instead of a token.
    fn reject_unterminated_string<'src>(
        _lexer: &mut logos::Lexer<'src, Token<'src>>,
    ) -> Result<(), Self> {
        Err(Self::UnterminatedString)
    }

    /// Spell the message of the syntax error the lex error is reported as.
    fn message(&self) -> &'static str {
        match self {
            Self::UnexpectedCharacter => "unexpected character",
            Self::UnterminatedString => "unterminated string: add the closing quote",
            Self::UnterminatedBlockString => "unterminated block string: add the closing `|`",
        }
    }
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Newline => "newline",
            Self::Semicolon => ";",
            Self::LeftBrace => "{",
            Self::RightBrace => "}",
            Self::LeftBracket => "[",
            Self::RightBracket => "]",
            Self::LeftParen => "(",
            Self::RightParen => ")",
            Self::Colon => ":",
            Self::Dot => ".",
            Self::Spread => "...",
            Self::Ampersand => "&",
            Self::NotAmpersand => "!&",
            Self::Star => "*",
            Self::At => "@",
            Self::Dollar => "$",
            Self::Pipe => "|",
            Self::Dash => "-",
            Self::LessThan => "<",
            Self::GreaterThan => ">",
            Self::Edge(_operator) => "edge operator",
            Self::Substitution => "substitution",
            Self::DoubleQuoted(_raw) | Self::SingleQuoted(_raw) => "quoted string",
            Self::UnterminatedDoubleQuoted | Self::UnterminatedSingleQuoted => {
                "unterminated string"
            }
            Self::Word => "text",
            Self::BlockString => "block string",
        };
        formatter.write_str(text)
    }
}
