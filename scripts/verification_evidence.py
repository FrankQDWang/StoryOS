"""Publish and check complete candidate evidence within the local execution trust boundary."""

import argparse
import base64
import gzip
import io
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys

import verification


PREFIX = "<!-- storyos-candidate-evidence:v1 -->\n"
PROTECTED = ["AGENTS.md", "**/AGENTS.md", "CONTEXT.md", "**/CONTEXT.md", "Makefile", "scripts", ".github", ".cursor", ".agents", ".githooks", ".gitignore", "rustfmt.toml", "docs/agents",
             "Cargo.toml", "crates/*/Cargo.toml", "docs/foundation", "Cargo.lock", "package.json", "pnpm-*.yaml", "apps/web/*config*", "apps/web/package.json"]


def api(path, data=None, *, pages=False):
    command = ["gh", "api", path, *(["--paginate", "--slurp"] if pages else [])]
    if data is not None:
        command.extend(["--method", "POST", "--input", "-"])
    return json.loads(subprocess.check_output(command, input=json.dumps(data).encode() if data is not None else None))


def unpack(body):
    if not body.startswith(PREFIX):
        raise ValueError("A structured candidate evidence report is required")
    return json.loads(body[len(PREFIX):])


def gate(root):
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    number = int(event["pull_request"]["number"] if "pull_request" in event else event["issue"]["number"])
    repository = os.environ["GITHUB_REPOSITORY"]
    route = f"repos/{repository}"
    pull = api(f"{route}/pulls/{number}")
    head, base = pull["head"]["sha"], pull["base"]["sha"]

    def status(state, description):
        api(f"{route}/statuses/{head}", {"state": state, "context": "candidate-evidence", "description": description[:140],
            "target_url": f"https://github.com/{repository}/actions/runs/{os.environ['GITHUB_RUN_ID']}"})

    status("pending", "Checking complete candidate evidence")
    try:
        baseline = verification.git(root, "rev-parse", "HEAD")
        if pull["state"] == "open" and base != baseline:
            raise ValueError("The protected baseline changed; publish evidence again")
        candidate = "refs/storyos/evidence-candidate"
        verification.git(root, "fetch", "--no-tags", "--depth=2", "origin", f"+{pull['merge_commit_sha'] if pull.get('merged') else f'refs/pull/{number}/merge'}:{candidate}")
        if verification.git(root, "rev-list", "--parents", "-n", "1", candidate).split()[1:] != [base, head]:
            raise ValueError("The synthetic merge no longer matches the pull request")
        comments = [comment for page in api(f"{route}/issues/{number}/comments?per_page=100", pages=True) for comment in page]
        for comment in reversed(comments):
            if (comment["body"].startswith(PREFIX) and comment["author_association"] in {"OWNER", "MEMBER", "COLLABORATOR"}
                    and api(f"{route}/collaborators/{comment['user']['login']}/permission")["permission"] in {"admin", "write", "maintain"}):
                packet = unpack(comment["body"])
                break
        else:
            raise ValueError("No authorized complete candidate report was published")
        if packet.get("pr") not in {None, number}:
            raise ValueError("Evidence belongs to another PR")
        check(root, packet, candidate, baseline, head, base)
        if packet.get("admission_version"):
            import verification_reviews
            verification_reviews.sentinel(route, head, base, verification.git(root, "rev-parse", f"{candidate}^{{tree}}"))
        latest = api(f"{route}/pulls/{number}")
        if (latest["head"]["sha"], latest["base"]["sha"]) != (head, base):
            raise ValueError("The candidate changed during validation")
        status("success", "Current complete candidate evidence passed")
    except Exception as error:
        status("failure", f"Candidate evidence refused: {type(error).__name__}")
        raise


def check(root, packet, candidate, baseline, head, base, *, policy_review_required=True, emit=True):
    expected = verification.complete_plan(root, candidate, base, with_graph=True)
    graph = expected.pop("graph", None)
    if (packet["head"], packet["base"], packet["baseline"]) != (head, base, baseline):
        raise ValueError("Stale candidate or protected baseline")
    changed = verification.git(root, "diff", "--name-only", baseline, candidate, "--", *PROTECTED)
    with gzip.GzipFile(fileobj=io.BytesIO(base64.b64decode(packet["report"], validate=True))) as archive:
        raw = archive.read(2_000_001)
    if len(raw) > 2_000_000:
        raise ValueError("Evidence exceeds the report size limit")
    report = json.loads(raw)
    if graph is not None and report.get("graph") not in (graph, {"version": 1, "sha256": verification.verification_graph.digest(graph)}):
        raise ValueError("Evidence workflow graph is missing or stale")
    if (type(report["version"]) is not int or report["version"] != 1 or report["profile"] != "complete" or report["status"] != "passed"
            or report["command"] != ["make", "verify-local-steps"] or report["cache"]["status"] != "disabled"
            or set(report["environment"]) != {"system", "machine", "python"}
            or any(not isinstance(value, str) or not value for value in report["environment"].values())
            or report["plan"] != expected or report["inventory"] != verification.inventory(root, candidate)):
        raise ValueError("Evidence is not a current complete verification report")
    if report.get("rust_test_files", []) != sorted(item["path"] for item in report["inventory"]["files"] if item["kind"] == "rust-test"):
        raise ValueError("Rust test files are missing from current compiler artifacts")
    source = report["source_start"]
    if (source != report["source_end"] or source["dirty"] is not False or source["tree"] != expected["tree"]
            or source["commit"] not in {head, verification.git(root, "rev-parse", candidate)}
            or any(not re.fullmatch(r"[0-9a-f]{64}", source[key])
                   for key in ("inputs_sha256", "write_stamps_sha256", "index_sha256"))):
        raise ValueError("Candidate source identity changed or does not match")
    import verification_reviews
    verification_reviews.check_report(root, report, candidate, base, head, packet.get("pr"))
    if policy_review_required and changed and not report.get("admission") and packet.get("policy_review") != {"tree": expected["tree"], "standards": "PASS", "spec": "PASS"}:
        raise ValueError("Policy inputs changed; explicit independent Standards and Spec review is required")
    if bool(report.get("admission")) != (packet.get("admission_version") == 1):
        raise ValueError("Evidence admission version is inconsistent")
    steps = report["steps"]
    if not steps or {step["stage"] for step in steps} != set(expected["stages"]):
        raise ValueError("Mandatory verification stages are missing or unreviewed")
    for item in [report, *steps]:
        duration = item["duration_seconds"]
        if (item["status"] != "passed" or type(item["exit_code"]) is not int or item["exit_code"] != 0
                or type(duration) not in (int, float) or not math.isfinite(duration) or duration < 0
                or not isinstance(item["command"], list) or not item["command"]
                or not all(isinstance(arg, str) and arg for arg in item["command"])):
            raise ValueError("Failed, interrupted or malformed execution record")
    if any(step["duration_seconds"] > report["duration_seconds"] for step in steps):
        raise ValueError("Stage duration exceeds the complete run")
    groups = json.loads(verification.git(root, "show", f"{candidate}:docs/agents/verification-policy.json"))["complete"]["groups"]
    for item in report["inventory"]["files"]:
        if item["kind"] not in {"web-test", "verification-test"}:
            continue
        commands = [step["command"] for step in steps if step["stage"] in groups[item["group"]]]
        if item["kind"] == "verification-test":
            if ["python3", "-m", "unittest", "discover", "-s", "scripts", "-p", "*_tests.py"] not in commands:
                raise ValueError(f"No recorded discovery covers {item['path']}")
            continue
        project = [command for command in commands if command[:6] == ["pnpm", "--dir", "apps/web", "exec", "vitest", "run"]
                   and any(pair == ["--project", item["group"]] for pair in [command[i:i + 2] for i in range(len(command))])]
        if not any(not any(re.search(r"\.(test|spec)\.[cm]?[jt]sx?$", arg) for arg in command)
                   or any(arg == item["path"].removeprefix("apps/web/") or arg.endswith("/" + item["path"]) for arg in command)
                   for command in project):
            raise ValueError(f"No recorded execution covers {item['path']}")
    if emit:
        print(f"Candidate evidence passed: {expected['tree']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "check", "publish", "gate"))
    parser.add_argument("--report", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--candidate", default="HEAD")
    for name in ("head", "base", "baseline"):
        parser.add_argument(f"--{name}")
    parser.add_argument("--pr", type=int)
    parser.add_argument("--policy-reviewed", action="store_true")
    args = parser.parse_args()
    try:
        root = Path(verification.git(Path.cwd(), "rev-parse", "--show-toplevel"))
        if args.action == "gate":
            gate(root)
            return
        if args.action == "publish":
            repository = subprocess.check_output(["gh", "repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner"], text=True).strip()
            pull = api(f"repos/{repository}/pulls/{args.pr}")
            args.head, args.base = pull["head"]["sha"], pull["base"]["sha"]
            args.candidate = "refs/storyos/evidence-candidate"
            verification.git(root, "fetch", "origin", "main", f"+{pull['merge_commit_sha'] if pull.get('merged') else f'refs/pull/{args.pr}/merge'}:{args.candidate}")
            args.baseline = verification.git(root, "rev-parse", "origin/main")
        if not all((args.head, args.base, args.baseline)):
            raise ValueError("The candidate head, base and protected baseline are required")
        if args.action in {"prepare", "publish"}:
            report = json.loads(args.report.read_bytes())
            packet = {"pr": args.pr, "head": args.head, "base": args.base, "baseline": args.baseline,
                      "summary": {"tree": report["source_start"]["tree"], "command": "make verify-local",
                                  "result": report["status"], "clean": not report["source_start"]["dirty"]},
                      "report": base64.b64encode(gzip.compress(json.dumps(verification.verification_graph.evidence(report)).encode(), mtime=0)).decode()}
            if report.get("admission"):
                packet["admission_version"] = 1
            policy = json.loads(verification.git(root, "show", f"{args.candidate}:docs/agents/verification-policy.json"))
            if policy["complete"].get("admission") or report.get("admission"):
                check(root, packet, args.candidate, args.baseline, args.head, args.base, emit=False)
            if args.policy_reviewed:
                packet["policy_review"] = {"tree": report["source_start"]["tree"], "standards": "PASS", "spec": "PASS"}
            body = PREFIX + json.dumps(packet, indent=2)
            if len(body) > 60_000:
                raise ValueError("Evidence exceeds the publication size limit")
            if args.action == "publish":
                if report.get("admission"):
                    import verification_reviews
                    verification_reviews.admission(root, report)
                check(root, packet, args.candidate, args.baseline, args.head, args.base)
                print(api(f"repos/{repository}/issues/{args.pr}/comments", {"body": body})["html_url"])
            else:
                print(body)
        else:
            check(root, unpack(args.evidence.read_text()), args.candidate, args.baseline, args.head, args.base)
    except (OSError, ValueError, KeyError, TypeError, EOFError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Candidate evidence refused: {error}\n")


if __name__ == "__main__":
    main()
