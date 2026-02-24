//! System-wide constants and default paths.

/// Default base directory for Containust data.
pub const DEFAULT_DATA_DIR: &str = "/var/lib/containust";

/// Default path for the state index file.
pub const DEFAULT_STATE_FILE: &str = "/var/lib/containust/state.json";

/// Default path for the image/layer storage.
pub const DEFAULT_IMAGE_STORE: &str = "/var/lib/containust/images";

/// Default path for container rootfs mounts.
pub const DEFAULT_ROOTFS_DIR: &str = "/var/lib/containust/rootfs";

/// Cgroups v2 unified hierarchy mount point.
pub const CGROUP_V2_PATH: &str = "/sys/fs/cgroup";

/// File extension for Containust composition files.
pub const CTST_EXTENSION: &str = ".ctst";

/// SHA-256 digest length in hex characters.
pub const SHA256_HEX_LENGTH: usize = 64;

/// Maximum number of layers in an image.
pub const MAX_IMAGE_LAYERS: usize = 128;

/// Application name used in CLI output and state files.
pub const APP_NAME: &str = "containust";

/// Binary name for the CLI.
pub const BIN_NAME: &str = "ctst";
