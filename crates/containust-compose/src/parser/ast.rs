//! Abstract Syntax Tree for `.ctst` composition files.

use std::collections::BTreeMap;

/// Root node of a parsed `.ctst` file.
#[derive(Debug, Clone, Default)]
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
#[derive(Debug, Clone, Default)]
pub struct ComponentDecl {
    /// Component name.
    pub name: String,
    /// Template to inherit from (FROM keyword).
    pub from_template: Option<String>,
    /// Image source URI.
    pub image: Option<String>,
    /// Single exposed port.
    pub port: Option<u16>,
    /// Multiple exposed ports.
    pub ports: Vec<u16>,
    /// Memory limit string (e.g., "256MiB").
    pub memory: Option<String>,
    /// CPU shares string.
    pub cpu: Option<String>,
    /// Environment variables.
    pub env: BTreeMap<String, String>,
    /// Single volume mount.
    pub volume: Option<String>,
    /// Multiple volume mounts.
    pub volumes: Vec<String>,
    /// Command to run.
    pub command: Vec<String>,
    /// Read-only root filesystem.
    pub readonly: Option<bool>,
    /// Working directory.
    pub workdir: Option<String>,
    /// User to run as.
    pub user: Option<String>,
    /// Container hostname.
    pub hostname: Option<String>,
    /// Restart policy.
    pub restart: Option<String>,
    /// Network mode.
    pub network: Option<String>,
    /// Healthcheck configuration.
    pub healthcheck: Option<HealthcheckDecl>,
}

/// Healthcheck configuration inside a component.
#[derive(Debug, Clone)]
pub struct HealthcheckDecl {
    /// Command to run for health check.
    pub command: Vec<String>,
    /// Check interval.
    pub interval: Option<String>,
    /// Timeout per check.
    pub timeout: Option<String>,
    /// Number of retries.
    pub retries: Option<u32>,
    /// Start period before health checks begin.
    pub start_period: Option<String>,
}

/// A `CONNECT` declaration linking two components.
#[derive(Debug, Clone)]
pub struct ConnectionDecl {
    /// Source component name (depends on target).
    pub from: String,
    /// Target component name (started first).
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_file_default_is_empty() {
        let file = CompositionFile::default();
        assert!(file.imports.is_empty());
        assert!(file.components.is_empty());
        assert!(file.connections.is_empty());
    }

    #[test]
    fn component_decl_default_has_no_values() {
        let comp = ComponentDecl::default();
        assert!(comp.name.is_empty());
        assert!(comp.from_template.is_none());
        assert!(comp.image.is_none());
        assert!(comp.port.is_none());
        assert!(comp.ports.is_empty());
        assert!(comp.env.is_empty());
        assert!(comp.volumes.is_empty());
        assert!(comp.command.is_empty());
        assert!(comp.readonly.is_none());
        assert!(comp.healthcheck.is_none());
    }
}
