//! Split one item or heading line into tokens with byte spans, reporting unterminated strings and unknown escapes.

use std::fmt;

use logos::Logos;

use super::ast::Span;
use super::diagnostic::Diagnostics;
use super::skeleton::SourceText;

/// Tag one lexeme of an item line as a token; a bare value or label is any run of tokens the parser joins back by span.
#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(error = LexError)]
#[logos(skip r"[ \t]+")]
pub(crate) enum Token<'src> {
    #[token("->")]
    Arrow,
    #[token(".")]
    Dot,
    #[token(":")]
    Colon,
    #[token("=")]
    Equals,
    #[token(",")]
    Comma,
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[regex(r#""([^"\\]|\\.)*""#, LexError::reject_unknown_escape)]
    #[regex(r#""([^"\\]|\\.)*\\?"#, LexError::reject_unterminated_string)]
    Quoted(&'src str),
    #[regex(
        r"[A-Za-z_][A-Za-z0-9_]*(-+[A-Za-z0-9_]+)*",
        Token::absorb_trailing_dashes
    )]
    Name(&'src str),
    #[regex(r#"[^ \t,()\[\]"=:.A-Za-z_]+"#)]
    Other(&'src str),
}

/// Name the lex error that keeps a stretch of an item line from being a token.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum LexError {
    #[default]
    UnexpectedCharacter,
    UnterminatedString,
    UnknownEscape,
}

impl<'src> Token<'src> {
    /// Lex `line` into tokens spanned in the whole source, or record every invalid stretch and return `None`.
    pub(crate) fn lex_line(
        line: SourceText<'src>,
        diagnostics: &mut Diagnostics,
    ) -> Option<Vec<(Self, Span)>> {
        let lexer = Self::lexer(line.text).spanned();
        let mut tokens = Vec::new();
        let mut is_lexed = true;
        for (lexed, line_range) in lexer {
            let start = line.offset + line_range.start;
            let end = line.offset + line_range.end;
            let span = Span::from(start..end);
            match lexed {
                Ok(token) => tokens.push((token, span)),
                Err(error) => {
                    is_lexed = false;
                    let message = error.message();
                    diagnostics.record_error(message, span);
                }
            }
        }
        is_lexed.then_some(tokens)
    }

    /// Decode a quoted string, quotes included in `raw`, whose only escapes are `\"` and `\\`.
    pub(crate) fn decode_quoted(raw: &str) -> String {
        let closing_quote_index = raw.len() - 1;
        let inner = &raw[1..closing_quote_index];
        let mut decoded = String::with_capacity(inner.len());
        let mut characters = inner.chars();
        while let Some(character) = characters.next() {
            let is_plain_character = character != '\\';
            let decoded_character = if is_plain_character {
                Some(character)
            } else {
                characters.next()
            };
            decoded.extend(decoded_character);
        }
        decoded
    }

    /// Extend a name over the `-` run after it, except the `-` opening an arrow `->`, so `a->b` lexes as an edge.
    fn absorb_trailing_dashes(lexer: &mut logos::Lexer<'src, Token<'src>>) -> &'src str {
        let remainder = lexer.remainder();
        let after_dashes = remainder.trim_start_matches('-');
        let dash_count = remainder.len() - after_dashes.len();
        let is_arrow_after_dashes = dash_count > 0 && after_dashes.starts_with('>');
        let absorbed_dash_count = if is_arrow_after_dashes {
            dash_count - 1
        } else {
            dash_count
        };
        lexer.bump(absorbed_dash_count);
        lexer.slice()
    }
}

impl LexError {
    /// Reject a quoted string holding an escape other than `\"` and `\\`.
    fn reject_unknown_escape<'src>(
        lexer: &mut logos::Lexer<'src, Token<'src>>,
    ) -> Result<&'src str, Self> {
        let raw = lexer.slice();
        let mut characters = raw.chars();
        while let Some(character) = characters.next() {
            let is_plain_character = character != '\\';
            if is_plain_character {
                continue;
            }
            let escaped = characters.next();
            let is_unknown_escape = !matches!(escaped, Some('"' | '\\'));
            if is_unknown_escape {
                return Err(Self::UnknownEscape);
            }
        }
        Ok(raw)
    }

    /// Reject an unterminated string as a lex error, so logos reports it instead of a token.
    fn reject_unterminated_string<'src>(
        _lexer: &mut logos::Lexer<'src, Token<'src>>,
    ) -> Result<&'src str, Self> {
        Err(Self::UnterminatedString)
    }

    /// Spell the message of the Markdown error the lex error is reported as.
    fn message(&self) -> &'static str {
        match self {
            Self::UnexpectedCharacter => "unexpected character",
            Self::UnterminatedString => "unterminated string: add the closing quote",
            Self::UnknownEscape => "a quoted string escapes only `\\\"` and `\\\\`",
        }
    }
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arrow => formatter.write_str("`->`"),
            Self::Dot => formatter.write_str("`.`"),
            Self::Colon => formatter.write_str("`:`"),
            Self::Equals => formatter.write_str("`=`"),
            Self::Comma => formatter.write_str("`,`"),
            Self::LeftParen => formatter.write_str("`(`"),
            Self::RightParen => formatter.write_str("`)`"),
            Self::LeftBracket => formatter.write_str("`[`"),
            Self::RightBracket => formatter.write_str("`]`"),
            Self::Quoted(text) | Self::Name(text) | Self::Other(text) => {
                let holds_backtick = text.contains('`');
                if holds_backtick {
                    return write!(formatter, "`` {text} ``");
                }
                write!(formatter, "`{text}`")
            }
        }
    }
}
