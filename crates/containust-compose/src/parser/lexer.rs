//! Tokenization of `.ctst` source text using `nom`.
//!
//! Produces a stream of tokens from raw input for the parser to consume.

/// A token in the `.ctst` language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// `IMPORT` keyword.
    Import,
    /// `COMPONENT` keyword.
    Component,
    /// `CONNECT` keyword.
    Connect,
    /// An identifier (component name, parameter name).
    Identifier(String),
    /// A string literal.
    StringLiteral(String),
    /// `{` opening brace.
    BraceOpen,
    /// `}` closing brace.
    BraceClose,
    /// `->` arrow for connections.
    Arrow,
    /// `=` assignment.
    Equals,
    /// `,` comma separator.
    Comma,
}
