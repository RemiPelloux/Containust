//! `.ctst` file parser built on `nom`.
//!
//! Transforms raw `.ctst` text into a validated AST through
//! lexing, parsing, and static analysis phases.

pub mod ast;
pub mod lexer;
pub mod validator;

use std::collections::BTreeMap;

use containust_common::error::{ContainustError, Result};

use self::ast::{ComponentDecl, CompositionFile, ConnectionDecl, HealthcheckDecl, ImportDecl};
use self::lexer::Token;

/// Cursor into a token stream for recursive-descent parsing.
struct TokenCursor<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> TokenCursor<'a> {
    const fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect_identifier(&mut self) -> Result<String> {
        match self.advance() {
            Some(Token::Identifier(s)) => Ok(s.clone()),
            other => Err(parse_err(format!("expected identifier, got {other:?}"))),
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<()> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            other => Err(parse_err(format!("expected {expected:?}, got {other:?}"))),
        }
    }

    fn expect_string(&mut self) -> Result<String> {
        match self.advance() {
            Some(Token::StringLiteral(s)) => Ok(s.clone()),
            other => Err(parse_err(format!("expected string literal, got {other:?}"))),
        }
    }

    fn expect_integer(&mut self) -> Result<i64> {
        match self.advance() {
            Some(Token::Integer(n)) => Ok(*n),
            other => Err(parse_err(format!("expected integer, got {other:?}"))),
        }
    }

    const fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }
}

const fn parse_err(message: String) -> ContainustError {
    ContainustError::Config { message }
}

fn skip_optional_comma(cursor: &mut TokenCursor<'_>) {
    if cursor.peek() == Some(&Token::Comma) {
        let _ = cursor.advance();
    }
}

/// Parses a `.ctst` file from its source text.
///
/// # Errors
///
/// Returns an error if the input contains syntax errors or fails validation.
pub fn parse_ctst(input: &str) -> Result<CompositionFile> {
    tracing::info!("parsing .ctst input");
    let tokens = lexer::tokenize(input)?;
    let mut cursor = TokenCursor::new(&tokens);
    let file = parse_file(&mut cursor)?;
    validator::validate(&file)?;
    Ok(file)
}

fn parse_file(cursor: &mut TokenCursor<'_>) -> Result<CompositionFile> {
    let mut file = CompositionFile::default();

    while let Some(tok) = cursor.peek() {
        match tok {
            Token::Import => file.imports.push(parse_import(cursor)?),
            Token::Component => file.components.push(parse_component(cursor)?),
            Token::Connect => file.connections.push(parse_connection(cursor)?),
            other => {
                return Err(parse_err(format!(
                    "expected IMPORT, COMPONENT, or CONNECT at top level, got {other:?}"
                )));
            }
        }
    }

    Ok(file)
}

fn parse_import(cursor: &mut TokenCursor<'_>) -> Result<ImportDecl> {
    cursor.expect_token(&Token::Import)?;
    let source = cursor.expect_string()?;
    let alias = if cursor.peek() == Some(&Token::As) {
        let _ = cursor.advance();
        Some(cursor.expect_identifier()?)
    } else {
        None
    };
    Ok(ImportDecl { source, alias })
}

fn parse_component(cursor: &mut TokenCursor<'_>) -> Result<ComponentDecl> {
    cursor.expect_token(&Token::Component)?;
    let name = cursor.expect_identifier()?;

    let from_template = if cursor.peek() == Some(&Token::From) {
        let _ = cursor.advance();
        Some(cursor.expect_identifier()?)
    } else {
        None
    };

    cursor.expect_token(&Token::BraceOpen)?;

    let mut comp = ComponentDecl {
        name,
        from_template,
        ..ComponentDecl::default()
    };

    while cursor.peek() != Some(&Token::BraceClose) {
        if cursor.at_end() {
            return Err(parse_err(
                "unexpected end of input inside COMPONENT block".into(),
            ));
        }
        parse_property(cursor, &mut comp)?;
    }

    cursor.expect_token(&Token::BraceClose)?;
    Ok(comp)
}

fn parse_property(cursor: &mut TokenCursor<'_>, comp: &mut ComponentDecl) -> Result<()> {
    let key = cursor.expect_identifier()?;
    cursor.expect_token(&Token::Equals)?;

    match key.as_str() {
        "image" => comp.image = Some(cursor.expect_string()?),
        "port" => {
            let val = cursor.expect_integer()?;
            comp.port = Some(
                u16::try_from(val)
                    .map_err(|_| parse_err(format!("port value out of range: {val}")))?,
            );
        }
        "ports" => comp.ports = parse_integer_list(cursor)?,
        "memory" => comp.memory = Some(cursor.expect_string()?),
        "cpu" => comp.cpu = Some(cursor.expect_string()?),
        "env" => comp.env = parse_env_map(cursor)?,
        "volume" => comp.volume = Some(cursor.expect_string()?),
        "volumes" => comp.volumes = parse_string_list(cursor)?,
        "command" => comp.command = parse_string_list(cursor)?,
        "readonly" => comp.readonly = Some(parse_bool(cursor)?),
        "workdir" => comp.workdir = Some(cursor.expect_string()?),
        "user" => comp.user = Some(cursor.expect_string()?),
        "hostname" => comp.hostname = Some(cursor.expect_string()?),
        "restart" => comp.restart = Some(cursor.expect_string()?),
        "network" => comp.network = Some(cursor.expect_string()?),
        "healthcheck" => comp.healthcheck = Some(parse_healthcheck(cursor)?),
        _ => {
            return Err(parse_err(format!("unknown component property: {key}")));
        }
    }

    Ok(())
}

fn parse_bool(cursor: &mut TokenCursor<'_>) -> Result<bool> {
    match cursor.advance() {
        Some(Token::True) => Ok(true),
        Some(Token::False) => Ok(false),
        other => Err(parse_err(format!("expected true or false, got {other:?}"))),
    }
}

fn parse_string_list(cursor: &mut TokenCursor<'_>) -> Result<Vec<String>> {
    cursor.expect_token(&Token::BracketOpen)?;
    let mut items = Vec::new();

    while cursor.peek() != Some(&Token::BracketClose) {
        if cursor.at_end() {
            return Err(parse_err("unexpected end of input inside list".into()));
        }
        items.push(cursor.expect_string()?);
        skip_optional_comma(cursor);
    }

    cursor.expect_token(&Token::BracketClose)?;
    Ok(items)
}

fn parse_integer_list(cursor: &mut TokenCursor<'_>) -> Result<Vec<u16>> {
    cursor.expect_token(&Token::BracketOpen)?;
    let mut items = Vec::new();

    while cursor.peek() != Some(&Token::BracketClose) {
        if cursor.at_end() {
            return Err(parse_err("unexpected end of input inside list".into()));
        }
        let val = cursor.expect_integer()?;
        items.push(
            u16::try_from(val).map_err(|_| parse_err(format!("port value out of range: {val}")))?,
        );
        skip_optional_comma(cursor);
    }

    cursor.expect_token(&Token::BracketClose)?;
    Ok(items)
}

fn parse_env_map(cursor: &mut TokenCursor<'_>) -> Result<BTreeMap<String, String>> {
    cursor.expect_token(&Token::BraceOpen)?;
    let mut map = BTreeMap::new();

    while cursor.peek() != Some(&Token::BraceClose) {
        if cursor.at_end() {
            return Err(parse_err("unexpected end of input inside env block".into()));
        }
        let key = cursor.expect_identifier()?;
        cursor.expect_token(&Token::Equals)?;
        let value = cursor.expect_string()?;
        let _ = map.insert(key, value);
        skip_optional_comma(cursor);
    }

    cursor.expect_token(&Token::BraceClose)?;
    Ok(map)
}

fn parse_healthcheck(cursor: &mut TokenCursor<'_>) -> Result<HealthcheckDecl> {
    cursor.expect_token(&Token::BraceOpen)?;

    let mut hc = HealthcheckDecl {
        command: Vec::new(),
        interval: None,
        timeout: None,
        retries: None,
        start_period: None,
    };

    while cursor.peek() != Some(&Token::BraceClose) {
        if cursor.at_end() {
            return Err(parse_err(
                "unexpected end of input inside healthcheck block".into(),
            ));
        }
        let key = cursor.expect_identifier()?;
        cursor.expect_token(&Token::Equals)?;
        match key.as_str() {
            "command" => hc.command = parse_string_list(cursor)?,
            "interval" => hc.interval = Some(cursor.expect_string()?),
            "timeout" => hc.timeout = Some(cursor.expect_string()?),
            "retries" => {
                let val = cursor.expect_integer()?;
                hc.retries = Some(
                    u32::try_from(val)
                        .map_err(|_| parse_err(format!("retries value out of range: {val}")))?,
                );
            }
            "start_period" => hc.start_period = Some(cursor.expect_string()?),
            _ => {
                return Err(parse_err(format!("unknown healthcheck property: {key}")));
            }
        }
        skip_optional_comma(cursor);
    }

    cursor.expect_token(&Token::BraceClose)?;
    Ok(hc)
}

fn parse_connection(cursor: &mut TokenCursor<'_>) -> Result<ConnectionDecl> {
    cursor.expect_token(&Token::Connect)?;
    let from = cursor.expect_identifier()?;
    cursor.expect_token(&Token::Arrow)?;
    let to = cursor.expect_identifier()?;
    Ok(ConnectionDecl { from, to })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_input() {
        let file = parse_ctst("").expect("should parse empty input");
        assert!(file.imports.is_empty());
        assert!(file.components.is_empty());
        assert!(file.connections.is_empty());
    }

    #[test]
    fn parse_import_without_alias() {
        let input = r#"IMPORT "base.ctst""#;
        let file = parse_ctst(input).expect("should parse");
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].source, "base.ctst");
        assert!(file.imports[0].alias.is_none());
    }

    #[test]
    fn parse_import_with_alias() {
        let input = r#"IMPORT "templates/postgres.ctst" AS pg"#;
        let file = parse_ctst(input).expect("should parse");
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].source, "templates/postgres.ctst");
        assert_eq!(file.imports[0].alias.as_deref(), Some("pg"));
    }

    #[test]
    fn parse_minimal_component() {
        let input = r#"COMPONENT api {
    image = "myapp:latest"
}"#;
        let file = parse_ctst(input).expect("should parse");
        assert_eq!(file.components.len(), 1);
        assert_eq!(file.components[0].name, "api");
        assert_eq!(file.components[0].image.as_deref(), Some("myapp:latest"));
    }

    #[test]
    fn parse_component_with_from() {
        let input = r#"IMPORT "pg.ctst" AS pg
COMPONENT db FROM pg {
    port = 5432
}"#;
        let file = parse_ctst(input).expect("should parse");
        assert_eq!(file.components[0].from_template.as_deref(), Some("pg"));
        assert_eq!(file.components[0].port, Some(5432));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn parse_component_all_properties() {
        let input = r#"COMPONENT web {
    image = "file:///opt/images/web"
    port = 8080
    ports = [8080, 8443]
    memory = "256MiB"
    cpu = "1024"
    env = {
        RUST_LOG = "info"
        DB_URL = "postgres://localhost/db"
    }
    volume = "/data:/app/data"
    volumes = ["/logs:/app/logs", "/tmp:/app/tmp"]
    command = ["./server", "--bind", "0.0.0.0:8080"]
    readonly = true
    workdir = "/app"
    user = "appuser"
    hostname = "web-server"
    restart = "always"
    network = "bridge"
    healthcheck = {
        command = ["curl", "-f", "http://localhost:8080/health"]
        interval = "30s"
        timeout = "5s"
        retries = 3
        start_period = "10s"
    }
}"#;
        let file = parse_ctst(input).expect("should parse");
        let c = &file.components[0];
        assert_eq!(c.name, "web");
        assert_eq!(c.image.as_deref(), Some("file:///opt/images/web"));
        assert_eq!(c.port, Some(8080));
        assert_eq!(c.ports, vec![8080, 8443]);
        assert_eq!(c.memory.as_deref(), Some("256MiB"));
        assert_eq!(c.cpu.as_deref(), Some("1024"));
        assert_eq!(c.env.len(), 2);
        assert_eq!(c.env.get("RUST_LOG").map(String::as_str), Some("info"));
        assert_eq!(c.volume.as_deref(), Some("/data:/app/data"));
        assert_eq!(c.volumes.len(), 2);
        assert_eq!(c.command, vec!["./server", "--bind", "0.0.0.0:8080"]);
        assert_eq!(c.readonly, Some(true));
        assert_eq!(c.workdir.as_deref(), Some("/app"));
        assert_eq!(c.user.as_deref(), Some("appuser"));
        assert_eq!(c.hostname.as_deref(), Some("web-server"));
        assert_eq!(c.restart.as_deref(), Some("always"));
        assert_eq!(c.network.as_deref(), Some("bridge"));
        let hc = c.healthcheck.as_ref().expect("healthcheck should exist");
        assert_eq!(
            hc.command,
            vec!["curl", "-f", "http://localhost:8080/health"]
        );
        assert_eq!(hc.interval.as_deref(), Some("30s"));
        assert_eq!(hc.timeout.as_deref(), Some("5s"));
        assert_eq!(hc.retries, Some(3));
        assert_eq!(hc.start_period.as_deref(), Some("10s"));
    }

    #[test]
    fn parse_connect() {
        let input = r#"COMPONENT api {
    image = "api:latest"
}
COMPONENT db {
    image = "postgres:15"
}
CONNECT api -> db"#;
        let file = parse_ctst(input).expect("should parse");
        assert_eq!(file.connections.len(), 1);
        assert_eq!(file.connections[0].from, "api");
        assert_eq!(file.connections[0].to, "db");
    }

    #[test]
    fn parse_full_ctst_file() {
        let input = r#"IMPORT "templates/postgres.ctst" AS pg

COMPONENT api {
    image = "file:///opt/images/myapp"
    port = 8080
    memory = "256MiB"
    cpu = "1024"
    env = {
        RUST_LOG = "info"
        DATABASE_URL = "postgres://${db.host}:${db.port}/app"
    }
    command = ["./server", "--bind", "0.0.0.0:8080"]
    readonly = true
}

COMPONENT db FROM pg {
    port = 5432
}

CONNECT api -> db"#;

        let file = parse_ctst(input).expect("should parse full file");
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.components.len(), 2);
        assert_eq!(file.connections.len(), 1);

        assert_eq!(file.imports[0].source, "templates/postgres.ctst");
        assert_eq!(file.imports[0].alias.as_deref(), Some("pg"));

        let api = &file.components[0];
        assert_eq!(api.name, "api");
        assert!(api.from_template.is_none());
        assert_eq!(api.image.as_deref(), Some("file:///opt/images/myapp"));
        assert_eq!(api.port, Some(8080));
        assert_eq!(api.readonly, Some(true));
        assert_eq!(api.command.len(), 3);

        let db = &file.components[1];
        assert_eq!(db.name, "db");
        assert_eq!(db.from_template.as_deref(), Some("pg"));
        assert_eq!(db.port, Some(5432));
    }

    #[test]
    fn parse_error_unknown_property() {
        let input = r#"COMPONENT x {
    image = "img"
    bogus = "val"
}"#;
        let result = parse_ctst(input);
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_missing_brace() {
        let input = r#"COMPONENT x {
    image = "img"
"#;
        let result = parse_ctst(input);
        assert!(result.is_err());
    }

    #[test]
    fn parse_comments_ignored() {
        let input = r#"// File header
COMPONENT api {
    // Image source
    image = "myapp"
    port = 8080 // HTTP port
}"#;
        let file = parse_ctst(input).expect("should parse with comments");
        assert_eq!(file.components[0].name, "api");
        assert_eq!(file.components[0].port, Some(8080));
    }

    #[test]
    fn parse_env_with_commas() {
        let input = r#"COMPONENT svc {
    image = "svc"
    env = {
        A = "1",
        B = "2",
    }
}"#;
        let file = parse_ctst(input).expect("should parse");
        assert_eq!(file.components[0].env.len(), 2);
    }

    #[test]
    fn parse_multiple_connections() {
        let input = r#"COMPONENT a { image = "a" }
COMPONENT b { image = "b" }
COMPONENT c { image = "c" }
CONNECT a -> b
CONNECT a -> c
CONNECT b -> c"#;
        let file = parse_ctst(input).expect("should parse");
        assert_eq!(file.connections.len(), 3);
    }
}
