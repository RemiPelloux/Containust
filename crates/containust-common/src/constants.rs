//! System-wide constants and default paths.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Default base directory for Containust data on Linux with root access.
pub const SYSTEM_DATA_DIR: &str = "/var/lib/containust";

/// Returns the data directory, preferring `$HOME/.containust` for non-root
/// or non-Linux environments, falling back to `/var/lib/containust`.
fn resolve_data_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let user_dir = PathBuf::from(home).join(".containust");
        if std::fs::create_dir_all(&user_dir).is_ok() {
            return user_dir;
        }
    }
    PathBuf::from(SYSTEM_DATA_DIR)
}

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Returns the resolved data directory for this session.
pub fn data_dir() -> &'static PathBuf {
    DATA_DIR.get_or_init(resolve_data_dir)
}

/// Returns the default state file path.
pub fn default_state_file() -> String {
    data_dir().join("state.json").to_string_lossy().into_owned()
}

/// Returns the default image store path.
pub fn default_image_store() -> PathBuf {
    data_dir().join("images")
}

/// Returns the default rootfs directory.
pub fn default_rootfs_dir() -> PathBuf {
    data_dir().join("rootfs")
}

/// Legacy default data directory (Linux system path).
pub const DEFAULT_DATA_DIR: &str = "/var/lib/containust";
/// Legacy default state file path (Linux system path).
pub const DEFAULT_STATE_FILE: &str = "/var/lib/containust/state.json";
/// Legacy default image store path (Linux system path).
pub const DEFAULT_IMAGE_STORE: &str = "/var/lib/containust/images";
/// Legacy default rootfs directory (Linux system path).
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
