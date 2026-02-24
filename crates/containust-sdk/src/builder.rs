//! Fluent API for configuring and launching containers.

use containust_common::error::Result;
use containust_common::types::ContainerId;
use containust_runtime::container::Container;

/// Builder for configuring a container before launch.
#[derive(Debug)]
pub struct ContainerBuilder {
    name: String,
    image: Option<String>,
    command: Vec<String>,
    env: Vec<(String, String)>,
    memory_limit: Option<u64>,
    cpu_shares: Option<u64>,
    readonly_rootfs: bool,
}

impl ContainerBuilder {
    /// Creates a new builder with the given container name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            image: None,
            command: Vec::new(),
            env: Vec::new(),
            memory_limit: None,
            cpu_shares: None,
            readonly_rootfs: true,
        }
    }

    /// Sets the image source URI.
    #[must_use]
    pub fn image(mut self, uri: impl Into<String>) -> Self {
        self.image = Some(uri.into());
        self
    }

    /// Sets the command to run inside the container.
    #[must_use]
    pub fn command(mut self, cmd: Vec<String>) -> Self {
        self.command = cmd;
        self
    }

    /// Adds an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Sets the memory limit in bytes.
    #[must_use]
    pub const fn memory_limit(mut self, bytes: u64) -> Self {
        self.memory_limit = Some(bytes);
        self
    }

    /// Sets CPU shares (relative weight).
    #[must_use]
    pub const fn cpu_shares(mut self, shares: u64) -> Self {
        self.cpu_shares = Some(shares);
        self
    }

    /// Sets whether the root filesystem should be read-only.
    #[must_use]
    pub const fn readonly_rootfs(mut self, readonly: bool) -> Self {
        self.readonly_rootfs = readonly;
        self
    }

    /// Builds and returns the configured container (does not start it).
    ///
    /// # Errors
    ///
    /// Returns an error if required fields (image) are missing.
    pub fn build(self) -> Result<Container> {
        let _image =
            self.image
                .ok_or_else(|| containust_common::error::ContainustError::Config {
                    message: "image source is required".to_string(),
                })?;

        let mut container = Container::new(ContainerId::new(self.name), self.command);
        container.env = self.env;

        if let Some(mem) = self.memory_limit {
            container.limits.memory_bytes = Some(mem);
        }
        if let Some(cpu) = self.cpu_shares {
            container.limits.cpu_shares = Some(cpu);
        }

        Ok(container)
    }
}
