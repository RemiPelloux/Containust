//! Tokenization of `.ctst` source text using `nom`.
//!
//! Produces a stream of [`Token`]s from raw input for the parser to consume.
//! Whitespace and `//` line comments are discarded between tokens.

use containust_common::error::{ContainustError, Result};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, digit1, multispace1, not_line_ending},
    combinator::value,
    multi::many0,
    sequence::preceded,
};

/// A token in the `.ctst` language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// `IMPORT` keyword.
    Import,
    /// `AS` keyword.
    As,
    /// `COMPONENT` keyword.
    Component,
    /// `FROM` keyword.
    From,
    /// `CONNECT` keyword.
    Connect,
    /// Boolean literal `true`.
    True,
    /// Boolean literal `false`.
    False,
    /// An identifier (component name, property name).
    Identifier(String),
    /// A double-quoted string literal.
    StringLiteral(String),
    /// An integer literal.
    Integer(i64),
    /// `{` opening brace.
    BraceOpen,
    /// `}` closing brace.
    BraceClose,
    /// `[` opening bracket.
    BracketOpen,
    /// `]` closing bracket.
    BracketClose,
    /// `->` arrow for connections.
    Arrow,
    /// `=` assignment.
    Equals,
    /// `,` comma separator.
    Comma,
}

/// Skippable items: whitespace or line comments.
fn skip_trivia(input: &str) -> IResult<&str, ()> {
    let comment = value((), preceded(tag("//"), not_line_ending));
    let ws = value((), multispace1);
    let (input, _) = many0(alt((ws, comment))).parse(input)?;
    Ok((input, ()))
}

/// Parses a double-quoted string literal with basic escape support.
fn string_literal(input: &str) -> IResult<&str, Token> {
    let (input, _) = char('"')(input)?;
    let mut result = String::new();
    let mut chars = input.char_indices();
    loop {
        match chars.next() {
            Some((idx, '"')) => {
                let remaining = &input[idx + 1..];
                return Ok((remaining, Token::StringLiteral(result)));
            }
            Some((_, '\\')) => match chars.next() {
                Some((_, 'n')) => result.push('\n'),
                Some((_, 't')) => result.push('\t'),
                Some((_, '\\')) => result.push('\\'),
                Some((_, '"')) => result.push('"'),
                Some((_, c)) => {
                    result.push('\\');
                    result.push(c);
                }
                None => {
                    return Err(nom::Err::Failure(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::Char,
                    )));
                }
            },
            Some((_, c)) => result.push(c),
            None => {
                return Err(nom::Err::Failure(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Char,
                )));
            }
        }
    }
}

/// Parses an integer literal (sequence of digits).
fn integer_literal(input: &str) -> IResult<&str, Token> {
    let (input, digits) = digit1(input)?;
    let val: i64 = digits.parse().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;
    Ok((input, Token::Integer(val)))
}

const fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

const fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Parses an identifier or keyword.
fn identifier_or_keyword(input: &str) -> IResult<&str, Token> {
    let (input, first) = take_while1(is_ident_start)(input)?;
    let (input, rest) = take_while(is_ident_continue)(input)?;
    let word = format!("{first}{rest}");
    let token = match word.as_str() {
        "IMPORT" => Token::Import,
        "AS" => Token::As,
        "COMPONENT" => Token::Component,
        "FROM" => Token::From,
        "CONNECT" => Token::Connect,
        "true" => Token::True,
        "false" => Token::False,
        _ => Token::Identifier(word),
    };
    Ok((input, token))
}

/// Parses a symbol token.
fn symbol(input: &str) -> IResult<&str, Token> {
    alt((
        value(Token::Arrow, tag("->")),
        value(Token::BraceOpen, char('{')),
        value(Token::BraceClose, char('}')),
        value(Token::BracketOpen, char('[')),
        value(Token::BracketClose, char(']')),
        value(Token::Equals, char('=')),
        value(Token::Comma, char(',')),
    ))
    .parse(input)
}

/// Parses a single token (after trivia has been skipped).
fn single_token(input: &str) -> IResult<&str, Token> {
    alt((
        string_literal,
        symbol,
        integer_literal,
        identifier_or_keyword,
    ))
    .parse(input)
}

/// Tokenizes a `.ctst` source string into a vector of tokens.
///
/// Whitespace and `//` line comments are discarded.
///
/// # Errors
///
/// Returns an error if the input contains characters that cannot be tokenized.
pub fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut remaining = input;

    loop {
        let (rest, ()) = skip_trivia(remaining).map_err(|e| ContainustError::Config {
            message: format!("lexer error skipping whitespace: {e}"),
        })?;
        remaining = rest;

        if remaining.is_empty() {
            break;
        }

        let (rest, token) = single_token(remaining).map_err(|e| ContainustError::Config {
            message: format!(
                "unexpected character at: \"{}\" ({e})",
                &remaining[..remaining.len().min(20)]
            ),
        })?;
        tokens.push(token);
        remaining = rest;
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_keywords() {
        let tokens =
            tokenize("IMPORT AS COMPONENT FROM CONNECT true false").expect("should tokenize");
        assert_eq!(
            tokens,
            vec![
                Token::Import,
                Token::As,
                Token::Component,
                Token::From,
                Token::Connect,
                Token::True,
                Token::False,
            ]
        );
    }

    #[test]
    fn tokenize_symbols() {
        let tokens = tokenize("{ } [ ] -> = ,").expect("should tokenize");
        assert_eq!(
            tokens,
            vec![
                Token::BraceOpen,
                Token::BraceClose,
                Token::BracketOpen,
                Token::BracketClose,
                Token::Arrow,
                Token::Equals,
                Token::Comma,
            ]
        );
    }

    #[test]
    fn tokenize_string_literal() {
        let tokens = tokenize(r#""hello world""#).expect("should tokenize");
        assert_eq!(tokens, vec![Token::StringLiteral("hello world".into())]);
    }

    #[test]
    fn tokenize_string_with_escapes() {
        let tokens = tokenize(r#""line\nnew\ttab\\slash\"quote""#).expect("should tokenize");
        assert_eq!(
            tokens,
            vec![Token::StringLiteral("line\nnew\ttab\\slash\"quote".into())]
        );
    }

    #[test]
    fn tokenize_integer() {
        let tokens = tokenize("8080 5432").expect("should tokenize");
        assert_eq!(tokens, vec![Token::Integer(8080), Token::Integer(5432)]);
    }

    #[test]
    fn tokenize_identifier() {
        let tokens = tokenize("my_app db-service").expect("should tokenize");
        assert_eq!(
            tokens,
            vec![
                Token::Identifier("my_app".into()),
                Token::Identifier("db-service".into()),
            ]
        );
    }

    #[test]
    fn tokenize_skips_comments() {
        let input = "COMPONENT api // this is a comment\n{ }";
        let tokens = tokenize(input).expect("should tokenize");
        assert_eq!(
            tokens,
            vec![
                Token::Component,
                Token::Identifier("api".into()),
                Token::BraceOpen,
                Token::BraceClose,
            ]
        );
    }

    #[test]
    fn tokenize_empty_input() {
        let tokens = tokenize("").expect("should tokenize");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_only_comments() {
        let tokens = tokenize("// just a comment\n// another one").expect("should tokenize");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_full_component() {
        let input = r#"COMPONENT api {
    image = "myapp"
    port = 8080
}"#;
        let tokens = tokenize(input).expect("should tokenize");
        assert_eq!(
            tokens,
            vec![
                Token::Component,
                Token::Identifier("api".into()),
                Token::BraceOpen,
                Token::Identifier("image".into()),
                Token::Equals,
                Token::StringLiteral("myapp".into()),
                Token::Identifier("port".into()),
                Token::Equals,
                Token::Integer(8080),
                Token::BraceClose,
            ]
        );
    }

    #[test]
    fn tokenize_connect() {
        let tokens = tokenize("CONNECT api -> db").expect("should tokenize");
        assert_eq!(
            tokens,
            vec![
                Token::Connect,
                Token::Identifier("api".into()),
                Token::Arrow,
                Token::Identifier("db".into()),
            ]
        );
    }

    #[test]
    fn tokenize_import_with_alias() {
        let input = r#"IMPORT "templates/pg.ctst" AS pg"#;
        let tokens = tokenize(input).expect("should tokenize");
        assert_eq!(
            tokens,
            vec![
                Token::Import,
                Token::StringLiteral("templates/pg.ctst".into()),
                Token::As,
                Token::Identifier("pg".into()),
            ]
        );
    }

    #[test]
    fn tokenize_error_on_invalid_char() {
        let result = tokenize("COMPONENT @invalid");
        assert!(result.is_err());
    }
}
