//! Reports each `tracing` form in the workspace crates that can record the `Display` or `Debug`
//! text of a value (ADR 0047). `make check-fast` runs it with `--tracing` (ADR 0070).

mod tracing_capture;

use std::path::Path;
use std::process::{Command, ExitCode};

const INVALID_INPUT: u8 = 2;

fn main() -> ExitCode {
    let arguments = std::env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    if !matches!(arguments.as_slice(), [mode] if mode == "--tracing") {
        eprintln!("usage: storyos-literal-guard --tracing");
        return ExitCode::from(INVALID_INPUT);
    }
    match tracing_captures() {
        Ok(0) => ExitCode::SUCCESS,
        Ok(captures) => {
            eprintln!("{captures} tracing forms can record author text or a secret (ADR 0047)");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("storyos-literal-guard: {error}");
            ExitCode::from(INVALID_INPUT)
        }
    }
}

fn tracing_captures() -> Result<usize, String> {
    let root = git(Path::new("."), &["rev-parse", "--show-toplevel"])?;
    let root = Path::new(root.trim_end());
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
