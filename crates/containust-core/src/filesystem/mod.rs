//! Filesystem management for container isolation.
//!
//! Provides OverlayFS layer management, `pivot_root` for secure root
//! filesystem switching, and mount utilities.

pub mod mount;
pub mod overlayfs;
pub mod pivot_root;
