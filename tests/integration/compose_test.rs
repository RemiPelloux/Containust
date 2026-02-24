//! Integration tests for .ctst composition and dependency graph resolution.

#[test]
fn compose_resolves_dependency_order() {
    // Parse web_stack.ctst and verify db/cache start before api
}

#[test]
fn compose_detects_cyclic_dependencies() {
    // Verify that circular CONNECT declarations produce an error
}

#[test]
fn compose_auto_wires_environment_variables() {
    // Verify that CONNECT generates the expected env vars
}
