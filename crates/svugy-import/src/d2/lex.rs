//! Split d2 source into tokens with byte spans, reporting unterminated strings as syntax errors.

use std::fmt;

use logos::Logos;

use super::ast::{EdgeOperator, Span};
use super::diagnostic::SyntaxError;

/// Tokenize d2 source, keeping the lexically valid tokens and one error per invalid stretch.
pub(crate) struct Tokens<'src> {
    pub(crate) tokens: Vec<(Token<'src>, Span)>,
    pub(crate) errors: Vec<SyntaxError>,
}

/// One lexeme of d2; unquoted text is split into pieces the parser joins back by span.
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
    #[regex(r"--+|-+>|<-+|<-+>", |lexer| EdgeOperator::from_arrow(lexer.slice()))]
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

/// Why a stretch of source is not a token.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum LexError {
    #[default]
    UnexpectedCharacter,
    UnterminatedString,
    UnterminatedBlockString,
}

impl<'src> Tokens<'src> {
    /// Lex `source`, turning a `|` right after `:` into a whole block string, as d2 only opens one at a value's start.
    pub(crate) fn lex(source: &'src str) -> Self {
        let mut lexer = Token::lexer(source);
        let mut tokens: Vec<(Token<'src>, Span)> = Vec::new();
        let mut errors = Vec::new();
        while let Some(lexed) = lexer.next() {
            let start = lexer.span().start;
            let is_value_start = matches!(tokens.last(), Some((Token::Colon, _colon_span)));
            let lexed = match lexed {
                Ok(Token::Pipe) if is_value_start => Self::bump_block_string(&mut lexer),
                other => other,
            };
            let span = Span::new(start, lexer.span().end);
            match lexed {
                Ok(token) => tokens.push((token, span)),
                Err(error) => {
                    let syntax_error = error.into_syntax_error(span);
                    errors.push(syntax_error);
                }
            }
        }
        Self { tokens, errors }
    }

    /// Consume a block string whose opening `|` was just lexed: the closer is the opening quote run followed by `|`.
    fn bump_block_string(
        lexer: &mut logos::Lexer<'src, Token<'src>>,
    ) -> Result<Token<'src>, LexError> {
        let after_pipe = lexer.remainder();
        let quote_length = after_pipe
            .find(|character: char| {
                character.is_whitespace() || character.is_alphanumeric() || character == '_'
            })
            .unwrap_or(after_pipe.len());
        let (quote, after_quote) = after_pipe.split_at(quote_length);
        let tag_length = after_quote
            .find(char::is_whitespace)
            .unwrap_or(after_quote.len());
        let body = &after_quote[tag_length..];
        let closer = format!("{quote}|");
        let Some(closer_offset) = body.find(&closer) else {
            lexer.bump(after_pipe.len());
            return Err(LexError::UnterminatedBlockString);
        };
        let block_length = quote_length + tag_length + closer_offset + closer.len();
        lexer.bump(block_length);
        Ok(Token::BlockString)
    }
}

impl Token<'_> {
    /// Decode a double-quoted string, quotes included in `raw`, through its backslash escapes.
    pub(crate) fn decode_double_quoted(raw: &str) -> String {
        let inner = &raw[1..raw.len() - 1];
        Self::decode_escapes(inner)
    }

    /// Decode a single-quoted string, quotes included in `raw`, where only a doubled `''` escapes.
    pub(crate) fn decode_single_quoted(raw: &str) -> String {
        let inner = &raw[1..raw.len() - 1];
        inner.replace("''", "'")
    }

    /// Decode d2's backslash escapes (`\n`, `\t`, …; any other escaped character stands for itself; `\` + newline vanishes).
    pub(crate) fn decode_escapes(raw: &str) -> String {
        let mut decoded = String::with_capacity(raw.len());
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
    fn reject_unterminated_string<'src>(
        _lexer: &mut logos::Lexer<'src, Token<'src>>,
    ) -> Result<(), Self> {
        Err(Self::UnterminatedString)
    }

    fn into_syntax_error(self, span: Span) -> SyntaxError {
        let message = match self {
            Self::UnexpectedCharacter => "unexpected character",
            Self::UnterminatedString => "unterminated string: add the closing quote",
            Self::UnterminatedBlockString => "unterminated block string: add the closing `|`",
        };
        SyntaxError::new(message.to_owned(), span)
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
