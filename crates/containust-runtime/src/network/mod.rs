//! Named networks, shared netns, loopback, and `/etc/hosts` for CONNECT.

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{
    ensure_loopback, ensure_shared_netns, join_netns, network_ns_path, write_container_hosts,
};

/// Normalized network mode for a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkMode {
    /// Share the host network namespace.
    Host,
    /// Private empty network namespace.
    None,
    /// Shared project network (bridge or named).
    Shared(String),
}

impl NetworkMode {
    /// Parses a `.ctst` `network` property.
    ///
    /// Unspecified / empty → private netns (`none`). Explicit `bridge` or a
    /// custom name selects a shared project netns.
    #[must_use]
    pub fn parse(raw: Option<&str>) -> Self {
        let trimmed = raw.map(str::trim).filter(|s| !s.is_empty());
        match trimmed {
            None | Some("none") => Self::None,
            Some("host") => Self::Host,
            Some("bridge") => Self::Shared("bridge".into()),
            Some(name) => Self::Shared(name.to_string()),
        }
    }

    /// Returns true for the host network mode.
    #[must_use]
    pub const fn is_host(&self) -> bool {
        matches!(self, Self::Host)
    }

    /// Shared network name, if any.
    #[must_use]
    pub const fn shared_name(&self) -> Option<&str> {
        match self {
            Self::Shared(name) => Some(name.as_str()),
            Self::Host | Self::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_mode_parse_defaults_to_private() {
        assert_eq!(NetworkMode::parse(None), NetworkMode::None);
        assert_eq!(
            NetworkMode::parse(Some("bridge")),
            NetworkMode::Shared("bridge".into())
        );
    }

    #[test]
    fn network_mode_parse_host_and_none() {
        assert_eq!(NetworkMode::parse(Some("host")), NetworkMode::Host);
        assert_eq!(NetworkMode::parse(Some("none")), NetworkMode::None);
    }

    #[test]
    fn network_mode_parse_custom_name() {
        assert_eq!(
            NetworkMode::parse(Some("backend")),
            NetworkMode::Shared("backend".into())
        );
    }
}
