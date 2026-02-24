//! Formatted output helpers for CLI commands.
//!
//! Provides consistent table formatting, colored status indicators,
//! and human-readable byte/duration formatting.

/// Formats a byte count into a human-readable string (e.g., "128 MiB").
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_displays_bytes() {
        assert_eq!(format_bytes(512), "512 B");
    }

    #[test]
    fn format_bytes_displays_kib() {
        assert_eq!(format_bytes(2048), "2.0 KiB");
    }

    #[test]
    fn format_bytes_displays_mib() {
        assert_eq!(format_bytes(134_217_728), "128.0 MiB");
    }

    #[test]
    fn format_bytes_displays_gib() {
        assert_eq!(format_bytes(2_147_483_648), "2.0 GiB");
    }
}
