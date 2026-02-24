//! Integration tests for `ctst build`.

#[test]
fn build_simple_ctst_file_succeeds() {
    // Will parse examples/simple_container.ctst and verify layer generation
}

#[test]
fn build_with_missing_file_returns_error() {
    // Verify proper error when .ctst file does not exist
}

#[test]
fn build_with_invalid_syntax_returns_parse_error() {
    // Verify parser reports actionable errors on malformed input
}
