"""Name the failed steps, the reason, and the log of a verification run that did not pass."""

import json

PATH_LIMIT = 20


def changed(before, after):
    """Return the sorted keys whose value differs between two path-to-state mappings."""
    return sorted(path for path in before.keys() | after.keys() if before.get(path) != after.get(path))


def describe(report, directory):
    """Add failed_steps, failure_reason, and changed_paths to a report whose status is not passed."""
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


def final_line(report, report_path):
    """Return the last printed line of a run."""
    summary = f"Verification {report['status']}: {report['duration_seconds']:.2f}s"
    if report["status"] == "passed":
        return f"{summary}; report: {report_path}"
    return (f"{summary}; failed step: {', '.join(report['failed_steps']) or 'none'}; "
            f"reason: {report['failure_reason'].removesuffix('.')}; log: {report['failure_log'] or 'none'}; report: {report_path}")
