//! Abstract Syntax Tree for `.ctst` composition files.

/// Root node of a parsed `.ctst` file.
#[derive(Debug, Clone)]
pub struct CompositionFile {
    /// Import declarations.
    pub imports: Vec<ImportDecl>,
    /// Component definitions.
    pub components: Vec<ComponentDecl>,
    /// Connection declarations.
    pub connections: Vec<ConnectionDecl>,
}

/// An `IMPORT` declaration.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    /// Source path or URL.
    pub source: String,
    /// Optional alias.
    pub alias: Option<String>,
}

/// A `COMPONENT` block definition.
#[derive(Debug, Clone)]
pub struct ComponentDecl {
    /// Component name.
    pub name: String,
    /// Image source URI.
    pub image: String,
    /// Key-value parameters.
    pub params: Vec<(String, String)>,
    /// Exposed ports.
    pub ports: Vec<u16>,
    /// Volume mounts.
    pub volumes: Vec<String>,
}

/// A `CONNECT` declaration linking two components.
#[derive(Debug, Clone)]
pub struct ConnectionDecl {
    /// Source component name.
    pub from: String,
    /// Target component name.
    pub to: String,
}
