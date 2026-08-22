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


def historical_reusable_owner_issues(tickets: list[dict], policy: dict) -> set[str]:
    if not isinstance(policy, dict):
        raise ValueError("Stage 1 tracker verification policy is not an object")
    responsibility_ids = policy.get("historical_reusable_owner_responsibility_ids")
    if (
        not isinstance(responsibility_ids, list)
        or not responsibility_ids
        or not all(isinstance(item, str) and item for item in responsibility_ids)
        or len(responsibility_ids) != len(set(responsibility_ids))
    ):
        raise ValueError("historical reusable-owner responsibility IDs are invalid")
    definitions = {
        ticket.get("responsibility_id"): ticket
        for ticket in tickets
        if isinstance(ticket, dict)
    }
    if len(definitions) != len(tickets):
        raise ValueError("Stage 1 ticket responsibility IDs are invalid")
    missing = set(responsibility_ids) - definitions.keys()
    if missing:
        raise ValueError(f"historical reusable-owner responsibility IDs are unknown: {sorted(missing)}")
    issues = {definitions[item].get("issue") for item in responsibility_ids}
    if len(issues) != len(responsibility_ids) or not all(
        canonical_issue_url(issue) for issue in issues
    ):
        raise ValueError("historical reusable-owner Issue bindings are invalid")
    return issues


def child_binding_errors(
    tickets: list[dict], historical_issues: set[str], live_children: list[str]
) -> list[str]:
    expected_live = {
        ticket.get("issue")
        for ticket in tickets
        if ticket.get("issue") not in historical_issues
    }
    if (
        not all(canonical_issue_url(item) for item in live_children)
        or len(live_children) != len(set(live_children))
        or set(live_children) - historical_issues != expected_live
    ):
        return ["Stage 1 parent native sub-issues drifted"]
    return []


def tickets_requiring_live_verification(
    tickets: list[dict], historical_issues: set[str], live_children: list[str]
) -> list[dict]:
    live_child_set = set(live_children)
    return [
        ticket
        for ticket in tickets
        if ticket["issue"] not in historical_issues or ticket["issue"] in live_child_set
    ]


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

    reusable_issue = "https://github.com/FrankQDWang/StoryOS/issues/209"
    tickets = [
        {"responsibility_id": "LIVE", "issue": issue},
        {"responsibility_id": "HISTORICAL", "issue": reusable_issue},
    ]
    policy = {"historical_reusable_owner_responsibility_ids": ["HISTORICAL"]}
    historical_issues = historical_reusable_owner_issues(tickets, policy)
    assert historical_issues == {reusable_issue}
    assert child_binding_errors(tickets, historical_issues, [issue]) == []
    assert child_binding_errors(tickets, historical_issues, [issue, reusable_issue]) == []
    assert child_binding_errors(tickets, historical_issues, [reusable_issue])
    assert child_binding_errors(tickets, historical_issues, [issue, issue_108])
    assert tickets_requiring_live_verification(tickets, historical_issues, [issue]) == [tickets[0]]
    assert tickets_requiring_live_verification(
        tickets, historical_issues, [issue, reusable_issue]
    ) == tickets
    for invalid_policy in (
        {},
        {"historical_reusable_owner_responsibility_ids": None},
        {"historical_reusable_owner_responsibility_ids": ["UNKNOWN"]},
        {"historical_reusable_owner_responsibility_ids": ["HISTORICAL", "HISTORICAL"]},
    ):
        try:
            historical_reusable_owner_issues(tickets, invalid_policy)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid historical reusable-owner policy must fail closed")


def main() -> None:
    declarations = json.loads(CROSSWALK.read_text(encoding="utf-8"))["declarations"]
    contract = declarations["delivery_contract"]
    tickets = contract["tickets"]
    try:
        historical_issues = historical_reusable_owner_issues(
            tickets, declarations["tracker_verification"]
        )
    except (KeyError, ValueError) as error:
        raise SystemExit(f"Stage 1 tracker binding verification failed:\n- {error}") from error
    parent = contract["parent_specification"]
    parent_number = int(parent["issue"].rsplit("/", 1)[1])
    parent_actual = api(f"issues/{parent_number}")
    errors = issue_errors(parent, parent_actual, api(f"issues/{parent_number}/dependencies/blocked_by"))
    live_children = [item["html_url"] for item in api(f"issues/{parent_number}/sub_issues")]
    errors.extend(child_binding_errors(tickets, historical_issues, live_children))
    for ticket in tickets_requiring_live_verification(tickets, historical_issues, live_children):
        number = int(ticket["issue"].rsplit("/", 1)[1])
        errors.extend(issue_errors(ticket, api(f"issues/{number}"), api(f"issues/{number}/dependencies/blocked_by")))
    if errors:
        raise SystemExit("Stage 1 tracker binding verification failed:\n- " + "\n- ".join(errors))
    live_count = len(tickets) - len(historical_issues)
    print(
        f"verified parent #{parent_number}, {live_count} live Stage 1 ticket bindings, "
        f"and {len(historical_issues)} historical reusable-owner binding"
    )


if __name__ == "__main__":
    self_test() if sys.argv[1:] == ["--self-test"] else main()
