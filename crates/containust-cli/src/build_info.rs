//! Build-time identity embedded into `ctst --version`.

/// Short `SemVer` from Cargo.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Multi-line version with git SHA and build date for `--version` / long help.
#[must_use]
pub const fn long_version() -> &'static str {
    concat!(
        env!("CARGO_PKG_VERSION"),
        "\ngit=",
        env!("CONTAINUST_GIT_SHA"),
        "\nbuilt=",
        env!("CONTAINUST_BUILD_DATE"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn long_version_includes_git_and_built() {
        let text = long_version();
        assert!(text.contains("git="));
        assert!(text.contains("built="));
    }
}
