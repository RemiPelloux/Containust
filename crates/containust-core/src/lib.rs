//! # containust-core
//!
//! Low-level Linux isolation primitives for the Containust runtime.
//!
//! This crate provides safe abstractions over:
//! - **Namespaces**: PID, Mount, Network, User, IPC, UTS isolation.
//! - **Cgroups v2**: CPU, memory, and I/O resource limiting.
//! - **Filesystem**: `OverlayFS` layer management and `pivot_root`.
//! - **Capabilities**: Linux capability dropping for least-privilege execution.
//!
//! All unsafe system calls are encapsulated in safe wrappers with
//! proper error handling and `// SAFETY:` documentation.

pub mod capability;
pub mod cgroup;
pub mod filesystem;
pub mod namespace;
