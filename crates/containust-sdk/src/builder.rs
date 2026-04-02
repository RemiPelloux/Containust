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

        let name = self.name.clone();
        let mut container = Container::new(ContainerId::new(self.name), name, self.command);
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use crate::builder::ContainerBuilder;

    #[test]
    fn builder_new_stores_name_and_defaults() {
        let builder = ContainerBuilder::new("test-container");
        assert_eq!(builder.name, "test-container");
        assert!(builder.image.is_none());
        assert!(builder.command.is_empty());
        assert!(builder.env.is_empty());
        assert!(builder.memory_limit.is_none());
        assert!(builder.cpu_shares.is_none());
        assert!(builder.readonly_rootfs);
    }

    #[test]
    fn builder_name_accepts_string() {
        let builder = ContainerBuilder::new(String::from("from-string"));
        assert_eq!(builder.name, "from-string");
    }

    #[test]
    fn builder_image_sets_uri() {
        let builder = ContainerBuilder::new("app").image("file:///opt/alpine");
        assert_eq!(builder.image, Some("file:///opt/alpine".to_string()));
    }

    #[test]
    fn builder_image_accepts_string() {
        let builder = ContainerBuilder::new("app").image(String::from("tar:///tmp/img.tar"));
        assert_eq!(builder.image, Some("tar:///tmp/img.tar".to_string()));
    }

    #[test]
    fn builder_command_replaces_vec() {
        let cmd = vec![
            "/usr/bin/myapp".into(),
            "--config".into(),
            "/etc/app.toml".into(),
        ];
        let builder = ContainerBuilder::new("app").command(cmd.clone());
        assert_eq!(builder.command, cmd);
    }

    #[test]
    fn builder_env_accumulates_variables() {
        let builder = ContainerBuilder::new("app")
            .env("APP_ENV", "production")
            .env("LOG_LEVEL", "warn")
            .env("PORT", "8080");
        assert_eq!(builder.env.len(), 3);
        assert!(
            builder
                .env
                .contains(&("APP_ENV".into(), "production".into()))
        );
        assert!(builder.env.contains(&("LOG_LEVEL".into(), "warn".into())));
        assert!(builder.env.contains(&("PORT".into(), "8080".into())));
    }

    #[test]
    fn builder_env_accepts_string_types() {
        let builder = ContainerBuilder::new("app").env(String::from("KEY"), String::from("value"));
        assert_eq!(builder.env[0], ("KEY".to_string(), "value".to_string()));
    }

    #[test]
    fn builder_memory_limit_stores_bytes() {
        let builder = ContainerBuilder::new("app").memory_limit(256 * 1024 * 1024);
        assert_eq!(builder.memory_limit, Some(268_435_456));
    }

    #[test]
    fn builder_cpu_shares_stores_weight() {
        let builder = ContainerBuilder::new("app").cpu_shares(512);
        assert_eq!(builder.cpu_shares, Some(512));
    }

    #[test]
    fn builder_readonly_rootfs_toggles() {
        let default_true = ContainerBuilder::new("app");
        assert!(default_true.readonly_rootfs);

        let set_false = ContainerBuilder::new("app").readonly_rootfs(false);
        assert!(!set_false.readonly_rootfs);

        let set_true = ContainerBuilder::new("app").readonly_rootfs(true);
        assert!(set_true.readonly_rootfs);
    }

    #[test]
    fn builder_chained_fluent_api() {
        let builder = ContainerBuilder::new("web")
            .image("file:///opt/images/web")
            .command(vec!["./server".into(), "--port".into(), "3000".into()])
            .env("NODE_ENV", "production")
            .memory_limit(512 * 1024 * 1024)
            .cpu_shares(1024)
            .readonly_rootfs(false);

        assert_eq!(builder.name, "web");
        assert_eq!(builder.image, Some("file:///opt/images/web".into()));
        assert_eq!(builder.command, vec!["./server", "--port", "3000"]);
        assert_eq!(builder.env[0], ("NODE_ENV".into(), "production".into()));
        assert_eq!(builder.memory_limit, Some(536_870_912));
        assert_eq!(builder.cpu_shares, Some(1024));
        assert!(!builder.readonly_rootfs);
    }

    #[test]
    fn builder_build_missing_image_returns_error() {
        let builder = ContainerBuilder::new("no-image");
        let result = builder.build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("image source is required"));
    }

    #[test]
    fn builder_build_with_image_succeeds() {
        let container = ContainerBuilder::new("valid")
            .image("file:///tmp/rootfs")
            .command(vec!["/bin/sh".into()])
            .env("HOME", "/root")
            .memory_limit(128 * 1024 * 1024)
            .cpu_shares(256)
            .build()
            .expect("build should succeed");

        assert_eq!(container.name, "valid");
        assert_eq!(container.command, vec!["/bin/sh"]);
        assert_eq!(container.env.len(), 1);
        assert_eq!(container.limits.memory_bytes, Some(134_217_728));
        assert_eq!(container.limits.cpu_shares, Some(256));
    }

    #[test]
    fn builder_build_transitions_env_to_runtime() {
        let container = ContainerBuilder::new("env-test")
            .image("file:///tmp/rootfs")
            .env("DB_HOST", "localhost")
            .env("DB_PORT", "5432")
            .build()
            .expect("build should succeed");

        assert!(
            container
                .env
                .contains(&("DB_HOST".into(), "localhost".into()))
        );
        assert!(container.env.contains(&("DB_PORT".into(), "5432".into())));
    }

    #[test]
    fn builder_build_applies_memory_limit() {
        let container = ContainerBuilder::new("mem")
            .image("file:///tmp/rootfs")
            .memory_limit(64 * 1024)
            .build()
            .expect("build should succeed");

        assert_eq!(container.limits.memory_bytes, Some(65_536));
    }

    #[test]
    fn builder_build_applies_cpu_shares() {
        let container = ContainerBuilder::new("cpu")
            .image("file:///tmp/rootfs")
            .cpu_shares(128)
            .build()
            .expect("build should succeed");

        assert_eq!(container.limits.cpu_shares, Some(128));
    }

    #[test]
    fn builder_build_empty_name_succeeds() {
        let container = ContainerBuilder::new("")
            .image("file:///tmp/rootfs")
            .build()
            .expect("build should succeed");

        assert_eq!(container.name, "");
    }

    #[test]
    fn builder_debug_output() {
        let builder = ContainerBuilder::new("debug-test");
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("ContainerBuilder"));
    }
}
