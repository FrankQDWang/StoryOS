"""Name the failed steps, the reason, the log, and the known flakes of a verification run that did not pass."""

from datetime import date
import json
from pathlib import Path
import re

PATH_LIMIT = 20
REGISTER = "docs/agents/flaky-tests.json"
SHAPES = ({"issue", "file", "test", "main", "symptom"}, {"issue", "rust", "main", "symptom"})
VITEST = re.compile(r"^\s*(?:FAIL|✗|×)\s+(?:\|[^|]*\|\s+)?(\S+\.[cm]?[jt]sx?)(.*)$")
CARGO = re.compile(r"^test (\S+) \.\.\. FAILED$")
FRAME = re.compile(r"^\s+at (?:.*\()?(?:file://)?(/\S+?):\d+:\d+\)?$")
DIFF = re.compile(r"^\+\s+(\w+): ")
ANSI = re.compile(r"\x1b\[[0-9;]*m")


def validate(entries):
    """Raise ValueError when the flake register does not have the shape that the verification guide documents."""
    if not isinstance(entries, list):
        raise ValueError("The flake register must be a JSON list")
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict) or set(entry) not in SHAPES:
            raise ValueError(f"Flake register entry {index} needs issue, main, symptom, and file and test or rust")
        main, text = entry["main"], [value for key, value in entry.items() if key not in {"issue", "main"}]
        if (type(entry["issue"]) is not int or entry["issue"] < 1
                or not all(isinstance(value, str) and value.strip() and "\n" not in value for value in text)
                or not isinstance(main, dict) or set(main) != {"passed", "runs", "commit", "date"}
                or not all(type(main[key]) is int for key in ("passed", "runs"))
                or not 0 <= main["passed"] <= main["runs"] or main["runs"] < 1
                or not isinstance(main["commit"], str) or not re.fullmatch(r"[0-9a-f]{7,40}", main["commit"])
                or not isinstance(main["date"], str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", main["date"])):
            raise ValueError(f"Flake register entry {index} has an invalid field value")
        date.fromisoformat(main["date"])


def failed_tests(text):
    """Return the failed test identities that vitest and cargo test lines name in a step log."""
    found, lines = [], ANSI.sub("", text).splitlines()
    for index, line in enumerate(lines):
        if match := VITEST.match(line):
            rest = re.sub(r"\s+\d+(?:\.\d+)?m?s$", "", match[2].strip())
            found.append((match[1], rest[1:].strip() if rest.startswith(">") else ""))
        elif match := CARGO.match(line):
            found.append(("", match[1]))
        elif line.startswith("error during close"):
            block = lines[index + 1:]
            end = next((position for position, item in enumerate(block) if FRAME.match(item)), len(block))
            frame = FRAME.match(block[end])[1] if end < len(block) else ""
            keys = [match[1] for match in map(DIFF.match, block[:end]) if match] or [""]
            found.extend((frame, "global teardown > " + key) for key in keys)
    return list(dict.fromkeys(found))


def matches(entry, identity):
    """Return whether one failed test identity is the test of one register entry."""
    file, test = identity
    if "rust" in entry:
        return not file and test == entry["rust"]
    return bool(file) and test == entry["test"] and (
        file == entry["file"] or entry["file"].endswith("/" + file) or file.endswith("/" + entry["file"]))


def known(report, failed):
    """Return the register entries when each failed test in the failed step logs matches one, else an empty list."""
    try:
        entries = json.loads((Path(report["repository"]) / REGISTER).read_text())
        validate(entries)
        identities = [identity for step in failed if step.get("log")
                      for identity in failed_tests(Path(step["log"]).read_text(errors="replace"))]
    except (KeyError, OSError, ValueError):
        return []
    matched = [next((entry for entry in entries if matches(entry, identity)), None) for identity in identities]
    if not matched or None in matched:
        return []
    return list({json.dumps(entry, sort_keys=True): entry for entry in matched}.values())


def flake_note(report):
    """Return the final-line marker of the known flakes in a report, or an empty string."""
    notes = [f"#{entry['issue']} ({entry['main']['passed']}/{entry['main']['runs']} on main {entry['main']['commit']})"
             for entry in report.get("known_flake", [])]
    return "; known flake: " + ", ".join(dict.fromkeys(notes)) if notes else ""


def changed(before, after):
    """Return the sorted keys whose value differs between two path-to-state mappings."""
    return sorted(path for path in before.keys() | after.keys() if before.get(path) != after.get(path))


def describe(report, directory):
    """Add failed_steps, failure_reason, changed_paths, and known_flake to a report whose status is not passed."""
    if report["status"] == "passed":
        return
    unfinished = [step for step in report.get("steps", []) if step["status"] not in {"passed", "cached"}]
    parents = {step.get("parent") for step in unfinished}
    failed = [step for step in unfinished if step.get("id") not in parents]
    child = directory / "failure.json"
    detail = json.loads(child.read_text()) if child.is_file() else {}
    paths = report.get("changed_paths", [])
    if report["status"] == "source-changed":
        reason = "Verification inputs changed during the run"
    elif failed:
        first = failed[0]
        reason = (f"The {first['stage']} step exceeded its {first.get('budget_seconds')}s budget"
                  if first.get("budget_exceeded") else
                  f"The {first['stage']} step stopped with status {first['status']} and exit code {first.get('exit_code')}")
    elif detail:
        reason, paths = detail["reason"], detail.get("changed_paths", [])
    elif report.get("error"):
        reason = report["error"].rstrip(".")
    elif report["status"] == "infrastructure-failed":
        reason = f"The verification command did not start because of {report['process'].get('launch_error')}"
    elif report["status"] == "interrupted":
        reason = "A signal stopped the run before it completed"
    elif report["status"] == "pending":
        reason = "One or more selected checks are pending"
    elif report["status"] == "incomplete":
        reason = "The run did not record a passed result for each required step"
    else:
        reason = f"The verification command stopped with exit code {report.get('exit_code')} and no failed step"
    if paths:
        more = len(paths) - PATH_LIMIT
        reason += ": " + ", ".join(paths[:PATH_LIMIT]) + (f", and {more} more" if more > 0 else "")
        report["changed_paths"] = paths[:PATH_LIMIT]
    report.update(failed_steps=[step["stage"] for step in failed], failure_reason=reason + ".",
                  failure_log=(failed[0].get("log") if failed else None) or report.get("log"))
    if flakes := known(report, failed):
        report["known_flake"] = flakes
