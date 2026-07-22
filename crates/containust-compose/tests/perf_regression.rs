//! Parse / resolve wall-clock budgets for wide compositions.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use containust_compose::parser::parse_ctst;
use containust_compose::resolver::resolve_connections;

const PARSE_BUDGET: Duration = Duration::from_millis(200);
const RESOLVE_BUDGET: Duration = Duration::from_millis(200);
const COMPONENT_COUNT: usize = 80;

fn wide_composition() -> String {
    let mut out = String::with_capacity(COMPONENT_COUNT * 96);
    for index in 0..COMPONENT_COUNT {
        let _ = writeln!(out, "COMPONENT c{index} {{");
        let _ = writeln!(out, "  image = \"alpine:3.21\"");
        let _ = writeln!(out, "}}");
        if index > 0 {
            let _ = writeln!(out, "CONNECT c{index} -> c0");
        }
    }
    out
}

#[test]
fn parse_wide_composition_within_budget() {
    let source = wide_composition();
    let start = Instant::now();
    let file = parse_ctst(&source).expect("parse");
    let elapsed = start.elapsed();
    assert_eq!(file.components.len(), COMPONENT_COUNT);
    assert!(
        elapsed < PARSE_BUDGET,
        "parse took {elapsed:?}, budget {PARSE_BUDGET:?}"
    );
}

#[test]
fn resolve_wide_composition_within_budget() {
    let file = parse_ctst(&wide_composition()).expect("parse");
    let start = Instant::now();
    let resolved = resolve_connections(&file).expect("resolve");
    let elapsed = start.elapsed();
    assert_eq!(resolved.len(), COMPONENT_COUNT);
    assert!(
        elapsed < RESOLVE_BUDGET,
        "resolve took {elapsed:?}, budget {RESOLVE_BUDGET:?}"
    );
}
