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
ISSUE_URL_PREFIX = "https://github.com/FrankQDWang/StoryOS/issues/"


def api(path: str):
    result = subprocess.run(
        ["gh", "api", f"{REPOSITORY}/{path}"], check=True, capture_output=True, text=True
    )
    return json.loads(result.stdout)


def body_sha256(body: str) -> str:
    return hashlib.sha256(body.replace("\r\n", "\n").encode()).hexdigest()


def canonical_issue_url(value) -> bool:
    if not isinstance(value, str) or not value.startswith(ISSUE_URL_PREFIX):
        return False
    number = value.removeprefix(ISSUE_URL_PREFIX)
    return number.isdigit() and not number.startswith("0")


def issue_errors(expected: dict, actual: dict, blockers: list[dict]) -> list[str]:
    expected_blockers = expected["blocked_by"]
    checks = {
        "URL": (actual.get("html_url"), expected["issue"]),
        "native parent": (actual.get("parent_issue_url"), expected["parent"].replace("https://github.com/", "https://api.github.com/repos/")),
        "title": (actual.get("title"), expected["title"]),
        "body SHA-256": (body_sha256(actual.get("body") or ""), expected["issue_body_sha256"]),
    }
    errors = [f"{expected['issue']} {name} drifted" for name, values in checks.items() if values[0] != values[1]]
    if (
        not isinstance(expected_blockers, list)
        or not all(canonical_issue_url(item) for item in expected_blockers)
        or len(expected_blockers) != len(set(expected_blockers))
    ):
        errors.append(f"{expected['issue']} native blocked-by contract is invalid")
        return errors
    actual_blockers = [item.get("html_url") for item in blockers if isinstance(item, dict)]
    if (
        len(actual_blockers) != len(blockers)
        or not all(canonical_issue_url(item) for item in actual_blockers)
        or len(actual_blockers) != len(set(actual_blockers))
        or sorted(actual_blockers) != sorted(expected_blockers)
    ):
        errors.append(f"{expected['issue']} native blocked-by drifted")
    return errors


def self_test() -> None:
    issue = "https://github.com/FrankQDWang/StoryOS/issues/103"
    issue_56 = "https://github.com/FrankQDWang/StoryOS/issues/56"
    issue_108 = "https://github.com/FrankQDWang/StoryOS/issues/108"
    parent = "https://github.com/FrankQDWang/StoryOS/issues/100"
    expected = {"issue": issue, "parent": parent, "title": "ticket", "issue_body_sha256": body_sha256("body"), "blocked_by": []}
    actual = {"html_url": issue, "parent_issue_url": parent.replace("https://github.com/", "https://api.github.com/repos/"), "title": "ticket", "body": "body"}
    assert issue_errors(expected, actual, []) == []
    assert "body SHA-256 drifted" in issue_errors(expected, {**actual, "body": "drift"}, [])[0]
    assert "native blocked-by drifted" in issue_errors(expected, actual, [{"html_url": issue}])[0]
    two_blockers = {**expected, "blocked_by": [issue_56, issue_108]}
    assert issue_errors(two_blockers, actual, [{"html_url": issue_108}, {"html_url": issue_56}]) == []
    assert issue_errors(two_blockers, actual, [{"html_url": issue_56}])
    assert issue_errors(two_blockers, actual, [{"html_url": issue_56}, {"html_url": issue_108}, {"html_url": issue}])
    assert issue_errors({**expected, "blocked_by": None}, actual, [])
    assert issue_errors({**expected, "blocked_by": issue_56}, actual, [{"html_url": issue_56}])
    assert issue_errors({**expected, "blocked_by": [issue_56, issue_56]}, actual, [{"html_url": issue_56}, {"html_url": issue_56}])
    foreign = "https://github.com/another-owner/another-repository/issues/56"
    assert issue_errors({**expected, "blocked_by": [foreign]}, actual, [{"html_url": foreign}])


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
