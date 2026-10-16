//! Every `.ctst` example in the README, `docs/` and `site/` must plan
//! cleanly with the real CLI: no parse/validation error and no warning
//! (e.g. a `CONNECT` across networks). Examples that `IMPORT` files that
//! only exist in the reader's project are allowed to stop there.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sources() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = vec![root.join("README.md")];
    for dir in ["docs", "site", "site/docs"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        files.extend(
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|ext| ext == "md" || ext == "html")
                }),
        );
    }
    files.sort();
    files
}

/// Code blocks of a Markdown file (fenced) or an HTML page (`<pre><code>`).
fn code_blocks(path: &Path, text: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    if path.extension().is_some_and(|ext| ext == "md") {
        let mut current: Option<(usize, Vec<&str>)> = None;
        for (index, line) in text.lines().enumerate() {
            let fence = line.trim_start().starts_with("```");
            match (&mut current, fence) {
                (None, true) => current = Some((index + 1, Vec::new())),
                (Some(_), true) => {
                    let (start, lines) = current.take().unwrap();
                    blocks.push((start, lines.join("\n")));
                }
                (Some((_, lines)), false) => lines.push(line),
                (None, false) => {}
            }
        }
    } else {
        let mut rest = text;
        while let Some(open) = rest.find("<pre") {
            let Some(code) = rest[open..].find("<code>") else {
                break;
            };
            let start = open + code + "<code>".len();
            let Some(end) = rest[start..].find("</code>") else {
                break;
            };
            let line = text.len() - rest.len() + start;
            let line = text[..line].lines().count();
            blocks.push((line, unescape_html(&rest[start..start + end])));
            rest = &rest[start + end..];
        }
    }
    blocks
}

fn unescape_html(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Whether a block is a `.ctst` composition (not shell, YAML, or prose).
fn is_composition(block: &str) -> bool {
    let has_component = block.lines().any(|line| {
        let line = line.trim_start();
        // `COMPONENT <name> …` grammar notation is not a composition.
        line.strip_prefix("COMPONENT ")
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic()))
            && line.contains('{')
    });
    let foreign = block.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("services:") || line.starts_with("$ ") || line.starts_with("ctst ")
    });
    has_component && !foreign
}

#[test]
fn documented_compositions_plan_cleanly() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let mut failures = Vec::new();
    let mut checked = 0;
    for path in sources() {
        let text = std::fs::read_to_string(&path).expect("read doc");
        for (line, block) in code_blocks(&path, &text) {
            if !is_composition(&block) {
                continue;
            }
            checked += 1;
            let file = scratch.path().join(format!("snippet-{checked}.ctst"));
            std::fs::write(&file, &block).expect("write snippet");
            let output = Command::new(env!("CARGO_BIN_EXE_ctst"))
                .arg("plan")
                .arg(&file)
                .output()
                .expect("run ctst plan");
            let report = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let missing_import = report.contains("import file not found");
            let problem = report
                .lines()
                .find(|l| l.starts_with("error") || l.contains("warning:"))
                .filter(|_| !missing_import);
            if let Some(problem) = problem {
                let source = path.strip_prefix(repo_root()).unwrap_or(&path);
                failures.push(format!("{}:{line}: {}", source.display(), problem.trim()));
            }
        }
    }
    assert!(
        checked > 40,
        "expected to find the documented examples, found {checked}"
    );
    assert!(
        failures.is_empty(),
        "invalid documented .ctst examples:\n{}",
        failures.join("\n")
    );
}

#[test]
fn shipped_examples_plan_cleanly() {
    let mut examples: Vec<PathBuf> = ["examples", "examples/templates"]
        .iter()
        .filter_map(|dir| std::fs::read_dir(repo_root().join(dir)).ok())
        .flat_map(|entries| entries.filter_map(Result::ok).map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "ctst"))
        .collect();
    examples.sort();
    assert!(examples.len() > 10, "examples not found");
    let failures: Vec<String> = examples
        .iter()
        .filter_map(|file| {
            let output = Command::new(env!("CARGO_BIN_EXE_ctst"))
                .arg("plan")
                .arg(file)
                .output()
                .expect("run ctst plan");
            let report = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            (!output.status.success() || report.contains("warning:"))
                .then(|| format!("{}: {}", file.display(), report.trim()))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
