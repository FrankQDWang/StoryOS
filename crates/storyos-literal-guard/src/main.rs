//! Reports positional opaque literal arguments that have no `/*param*/` comment.
//! The guard checks only the added lines of the Rust files that changed after the merge base.
//! It also reports each `tracing` form in the workspace crates that can record the `Display` or
//! `Debug` text of a value (ADR 0047).

mod finder;
mod tracing_capture;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::Path;
use std::process::{Command, ExitCode};

const EXEMPTIONS: &str = "docs/agents/rust-literal-exemptions.json";
const INVALID_INPUT: u8 = 2;

/// Method names whose sole parameter has the method name, so a sole literal argument passes.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Exemptions {
    methods: BTreeSet<String>,
}

fn main() -> ExitCode {
    let arguments = std::env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    let [base] = arguments.as_slice() else {
        eprintln!("usage: storyos-literal-guard <base-revision>");
        return ExitCode::from(INVALID_INPUT);
    };
    match run(base) {
        Ok((0, 0)) => ExitCode::SUCCESS,
        Ok((literals, captures)) => {
            if literals > 0 {
                eprintln!(
                    "{literals} positional literal arguments need a /*param*/ comment or a rename to a self-documenting API"
                );
            }
            if captures > 0 {
                eprintln!("{captures} tracing forms can record author text or a secret (ADR 0047)");
            }
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("storyos-literal-guard: {error}");
            ExitCode::from(INVALID_INPUT)
        }
    }
}

fn run(base: &str) -> Result<(usize, usize), String> {
    let root = git(Path::new("."), &["rev-parse", "--show-toplevel"])?;
    let root = Path::new(root.trim_end());
    let exemptions = serde_json::from_str::<Exemptions>(&read(&root.join(EXEMPTIONS))?)
        .map_err(|error| format!("{EXEMPTIONS}: {error}"))?;
    let merge_base = git(root, &["merge-base", base, "HEAD"])?;
    let diff = git(
        root,
        &[
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "--find-renames",
            "--diff-filter=d",
            merge_base.trim_end(),
            "--",
            "*.rs",
        ],
    )?;
    let mut added = BTreeMap::<String, Vec<Range<usize>>>::new();
    let mut path = String::new();
    for line in diff.lines() {
        if let Some(name) = line.strip_prefix("+++ b/") {
            name.clone_into(&mut path);
        } else if line.starts_with("@@ ") {
            let lines = line
                .split(' ')
                .nth(/*n*/ 2)
                .and_then(|range| range.strip_prefix('+'))
                .ok_or_else(|| format!("unexpected diff hunk header: {line}"))?;
            let (start, length) = lines.split_once(',').unwrap_or((lines, "1"));
            let start = start
                .parse::<usize>()
                .map_err(|error| format!("{line}: {error}"))?;
            let length = length
                .parse::<usize>()
                .map_err(|error| format!("{line}: {error}"))?;
            added
                .entry(path.clone())
                .or_default()
                .push(start..start + length);
        }
    }
    let untracked = git(
        root,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "*.rs",
        ],
    )?;
    for path in untracked.split('\0').filter(|path| !path.is_empty()) {
        added
            .entry(path.to_owned())
            .or_default()
            .push(1..usize::MAX);
    }
    let mut count = 0;
    for (path, lines) in &added {
        let source = read(&root.join(path))?;
        let mut findings = finder::find(&source, &exemptions.methods)
            .map_err(|error| format!("{path}: {error}"))?;
        findings.retain(|finding| lines.iter().any(|range| range.contains(&finding.line)));
        for finding in &findings {
            let finder::Finding {
                line,
                column,
                callee,
                literal,
            } = finding;
            println!(
                "{path}:{line}:{column}: positional-literal: argument `{literal}` of `{callee}` has no /*param*/ comment"
            );
        }
        count += findings.len();
        proc_macro2::extra::invalidate_current_thread_spans();
    }
    Ok((count, tracing_captures(root)?))
}

fn tracing_captures(root: &Path) -> Result<usize, String> {
    let files = git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "crates/*.rs",
        ],
    )?;
    let mut count = 0;
    for path in files.split('\0').filter(|path| !path.is_empty()) {
        let Ok(source) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        let findings =
            tracing_capture::find(&source).map_err(|error| format!("{path}: {error}"))?;
        for tracing_capture::Finding {
            line,
            column,
            reason,
        } in &findings
        {
            println!("{path}:{line}:{column}: tracing-capture: {reason}");
        }
        count += findings.len();
        proc_macro2::extra::invalidate_current_thread_spans();
    }
    Ok(count)
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {}: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim_end()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("git {}: {error}", arguments.join(" ")))
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}
