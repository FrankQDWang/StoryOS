#!/usr/bin/env python3
"""Fail closed when the live Stage 1 tracker drifts from the checked-in contract."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
CROSSWALK = ROOT / "generated/evidence/stage1-contract-crosswalk.json"
REPOSITORY = "repos/FrankQDWang/StoryOS"


def api(path: str):
    result = subprocess.run(
        ["gh", "api", f"{REPOSITORY}/{path}"], check=True, capture_output=True, text=True
    )
    return json.loads(result.stdout)


def body_sha256(body: str) -> str:
    return hashlib.sha256(body.replace("\r\n", "\n").encode()).hexdigest()


def issue_errors(expected: dict, actual: dict, blockers: list[dict]) -> list[str]:
    expected_blockers = expected["blocked_by"]
    if not isinstance(expected_blockers, list):
        expected_blockers = [] if expected_blockers is None else [expected_blockers]
    checks = {
        "URL": (actual.get("html_url"), expected["issue"]),
        "native parent": (actual.get("parent_issue_url"), expected["parent"].replace("https://github.com/", "https://api.github.com/repos/")),
        "title": (actual.get("title"), expected["title"]),
        "body SHA-256": (body_sha256(actual.get("body") or ""), expected["issue_body_sha256"]),
        "native blocked-by": (sorted(item["html_url"] for item in blockers), expected_blockers),
    }
    return [f"{expected['issue']} {name} drifted" for name, values in checks.items() if values[0] != values[1]]


def self_test() -> None:
    issue = "https://github.com/FrankQDWang/StoryOS/issues/103"
    parent = "https://github.com/FrankQDWang/StoryOS/issues/100"
    expected = {"issue": issue, "parent": parent, "title": "ticket", "issue_body_sha256": body_sha256("body"), "blocked_by": None}
    actual = {"html_url": issue, "parent_issue_url": parent.replace("https://github.com/", "https://api.github.com/repos/"), "title": "ticket", "body": "body"}
    assert issue_errors(expected, actual, []) == []
    assert "body SHA-256 drifted" in issue_errors(expected, {**actual, "body": "drift"}, [])[0]
    assert "native blocked-by drifted" in issue_errors(expected, actual, [{"html_url": issue}])[0]


def main() -> None:
    contract = json.loads(CROSSWALK.read_text(encoding="utf-8"))["declarations"]["delivery_contract"]
    parent = contract["parent_specification"]
    parent_number = int(parent["issue"].rsplit("/", 1)[1])
    parent_actual = api(f"issues/{parent_number}")
    errors = issue_errors(parent, parent_actual, api(f"issues/{parent_number}/dependencies/blocked_by"))
    live_children = sorted(item["html_url"] for item in api(f"issues/{parent_number}/sub_issues"))
    expected_children = sorted(ticket["issue"] for ticket in contract["tickets"])
    if live_children != expected_children:
        errors.append(f"{parent['issue']} native sub-issues drifted")
    for ticket in contract["tickets"]:
        number = int(ticket["issue"].rsplit("/", 1)[1])
        errors.extend(issue_errors(ticket, api(f"issues/{number}"), api(f"issues/{number}/dependencies/blocked_by")))
    if errors:
        raise SystemExit("Stage 1 tracker binding verification failed:\n- " + "\n- ".join(errors))
    print(f"verified parent #{parent_number} and {len(contract['tickets'])} live Stage 1 ticket bindings")


if __name__ == "__main__":
    self_test() if sys.argv[1:] == ["--self-test"] else main()
