#!/usr/bin/env python3
"""Run one Codex review round with the Standards and Spec axes on a pull request (ADR 0070)."""

import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
PROMPT = ROOT / "docs/agents/review-prompt.md"
ROUNDS = 3
AXES = ("Standards", "Spec")
# The same heading as the verdict comments of make review-round, so the two tools count the same rounds.
VERDICT = re.compile(r'## (Standards|Spec) review, round (\d+): (PASS|FAIL)')
HEAD = re.compile(r"^(?:Head|Candidate: head) `([0-9a-f]{40})`", re.M)
EVIDENCE = re.compile(r"Check Evidence Line: tier=(\S+) commit=([0-9a-f]{40}) tree=(\S+) result=(\S+) time=\S+ duration=\d+s")
# GitHub fills closingIssuesReferences only for a pull request into the default branch.
CLOSES = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?):? #(\d+)", re.I)
LINKS = re.compile(r"^## Review verdicts\r?\n.*?(?=^## |\Z)", re.M | re.S)
TRANSIENT = re.compile(r"\bEOF\b|connection reset|HTTP 50[234]\b|TLS handshake timeout|SSL_ERROR|i/o timeout"
                       r"|proxyconnect|remote end hung up unexpectedly", re.I)
# The total seconds that one GitHub call retries a transient network failure.
RETRY = 60
# The seconds between two reads of the verify check, and the wait of one Codex status call.
POLL = 20
CODEX_WAIT_MS = 600000
CODEX_WAITS = 6
INPUTS = """- The request gives the base and head commits, the Check Evidence Line, the scoped diff command, the
  pull request body, and the bodies of the issues that the pull request closes.
- The working tree is at the head commit. Review the scoped diff.
- Compare the commit of the Check Evidence Line with the head. A line that does not name the head is
  missing required evidence."""
BROKER = "openai-codex/codex/.*/scripts/app-server-broker.mjs"
LAST_ROUND = f"""PR {{pr}} had {ROUNDS} review rounds, and a fourth round is not permitted.
Send the open blocking findings to the coordinator session. The coordinator can accept a small fix:
1. The coordinator reviews the fix in a PR comment that names the new head.
2. Add a test that fails before the fix.
3. Run make check on the new head, put its Check Evidence Line in the PR body, and let the GitHub verify check pass.
Then the coordinator review comment replaces the verdicts of round {ROUNDS}."""
# A change to the diff of the ticket after a passed round needs a new round. A merge of the base does not.
BASE_MERGE = ("When a later merge of the base changes only the head and not the diff of this ticket, do not start a new round: "
              "send the Check Evidence Line of the new head and the GitHub verify check to the coordinator.")


class Refused(Exception):
    """The round does not start or does not complete."""


def run(command, done=None):
    """Return the output of a command. Retry it while its error output is a transient network failure.

    Before a retry, done() can return the result of a write that the server accepted, so a retry never
    does the write two times."""
    deadline, pause = time.monotonic() + RETRY, 1
    while True:
        process = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        if process.returncode == 0:
            return process.stdout
        remaining = deadline - time.monotonic()
        if not TRANSIENT.search(process.stderr) or remaining <= 0:
            raise Refused(f"{' '.join(command[:3])} failed: {process.stderr.strip()}")
        print(f"{' '.join(command[:3])}: transient failure, retry in {min(pause, remaining):.0f} s: "
              f"{process.stderr.strip().splitlines()[-1]}", file=sys.stderr)
        time.sleep(min(pause, remaining))
        pause *= 2
        if done and (result := done()):
            return result


def gh_json(*args):
    return json.loads(run(["gh", *args]))


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def pull_request(pr):
    return gh_json("pr", "view", str(pr), "--json",
                   "url,state,body,closingIssuesReferences,comments,headRefOid,baseRefOid,baseRefName")


def rounds_of(pull):
    """Map each round number to {axis: (result, head, comment URL)} from the verdict comments."""
    rounds = {}
    for comment in pull["comments"]:
        if match := VERDICT.match(comment["body"]):
            head = HEAD.search(comment["body"])
            rounds.setdefault(int(match[2]), {})[match[1]] = (match[3], head and head[1], comment["url"])
    return rounds


def evidence_line(body, head):
    """The Check Evidence Line of the PR body that is merge evidence for the head."""
    found = list(EVIDENCE.finditer(body))
    for line in reversed(found):
        tier, commit, tree, result = line.groups()
        if commit == head and result == "PASS" and tree != "DIRTY" and tier in ("check", "check-full"):
            return line[0]
    seen = ", ".join(f"tier={m[1]} commit={m[2][:12]} tree={m[3][:12]} result={m[4]}" for m in found) or "none"
    raise Refused(f"No Check Evidence Line of make check or make check-full with result=PASS and a clean tree names the head "
                  f"{head} (lines in the PR body: {seen}). Run make check on the head first, then put its line in the PR body.")


def wait_verify(head, seconds):
    """Return when a GitHub verify check of the head passed. Stop when the newest check completed without success."""
    deadline, shown = time.monotonic() + seconds, None
    while True:
        checks = gh_json("api", f"repos/{{owner}}/{{repo}}/commits/{head}/check-runs?check_name=verify&filter=all")["check_runs"]
        checks.sort(key=lambda check: check["id"], reverse=True)
        if any(c["status"] == "completed" and c["conclusion"] == "success" for c in checks):
            return
        link = checks[0]["html_url"] if checks else "no verify check yet"
        if checks and checks[0]["status"] == "completed":
            raise Refused(f"The verify check of the head {head} completed with {checks[0]['conclusion']}: {link}")
        if time.monotonic() >= deadline:
            raise Refused(f"The verify check of the head {head} did not pass in {seconds} s: {link}. Run the command again")
        state = f"{checks[0]['status']} {link}" if checks else link
        if state != shown:
            print(f"Waiting up to {seconds} s for the verify check of the head {head}: {state}", file=sys.stderr, flush=True)
            shown = state
        time.sleep(min(POLL, max(0, deadline - time.monotonic())))


def codex_scripts():
    """The scripts directory of the newest installed Codex plugin version."""
    versions = sorted(Path.home().glob(".claude/plugins/cache/openai-codex/codex/*/scripts/codex-companion.mjs"),
                      key=lambda p: [int(n) if n.isdecimal() else 0 for n in re.split(r"[.-]", p.parents[1].name)])
    if not versions:
        raise Refused("The Codex plugin is not installed; run /codex:setup")
    return versions[-1].parent


def codex(*args):
    return json.loads(subprocess.check_output(["node", str(codex_scripts() / "codex-companion.mjs"), *args, "--json"],
                                              cwd=ROOT, text=True))


def verdict(job):
    """Wait for one Codex job, then read the lists of the last fenced json block of its answer."""
    for _ in range(CODEX_WAITS):
        snapshot = codex("status", job, "--wait", "--timeout-ms", str(CODEX_WAIT_MS))
        if not snapshot["waitTimedOut"]:
            break
    else:
        raise Refused(f"Codex job {job} did not finish; read it with /codex:status {job}")
    if snapshot["job"]["status"] != "completed":
        raise Refused(f"Codex job {job} is {snapshot['job']['status']}; read it with /codex:result {job}")
    stored = codex("result", job)
    output = (stored.get("storedJob") or {}).get("result", {}).get("rawOutput", "")
    blocks = re.findall(r"```json\s*\n(.*?)\n```", output, re.S)
    value = json.loads(blocks[-1] if blocks else output)
    if not all(isinstance(value.get(k), list) and all(isinstance(i, str) for i in value[k])
               for k in ("blocking", "non_blocking", "evidence")):
        raise Refused(f"Codex job {job} did not return the blocking, non_blocking, and evidence lists")
    value["thread"] = stored["storedJob"].get("threadId") or stored["job"].get("threadId")
    return value


def stop_brokers():
    """Stop the Codex brokers whose working directory is in this worktree, and their app servers.

    Two parallel jobs can start two brokers, and the plugin session end stops only one. Other worktrees
    and ChatGPT.app do not match. A failure writes a warning and does not stop the round."""
    try:
        subprocess.run(["node", str(codex_scripts() / "session-lifecycle-hook.mjs"), "SessionEnd"],
                       input=json.dumps({"cwd": str(ROOT)}), text=True, capture_output=True)
        brokers = subprocess.run(["pgrep", "-f", BROKER], capture_output=True, text=True).stdout.split()
        parents = [line.split() for line in subprocess.check_output(["ps", "-eo", "pid=,ppid="], text=True).splitlines()]
        for broker in (pid for pid in brokers if inside_worktree(pid)):
            for pid in [child for child, parent in parents if parent == broker] + [broker]:
                try:
                    os.kill(int(pid), signal.SIGTERM)
                except ProcessLookupError:
                    pass
    except (Refused, OSError, subprocess.SubprocessError) as error:
        print(f"Warning: the Codex brokers did not stop: {error}", file=sys.stderr)


def inside_worktree(pid):
    listed = subprocess.run(["lsof", "-a", "-p", pid, "-d", "cwd", "-Fn"], capture_output=True, text=True).stdout
    directory = next((os.path.realpath(line[1:]) for line in listed.splitlines() if line.startswith("n")), None)
    return directory is not None and (directory == str(ROOT) or directory.startswith(f"{ROOT}{os.sep}"))


def request(pr, number, pull, line, directory):
    """Write the review request: the commits, the Check Evidence Line, the PR body, and the closed issues."""
    head, base = pull["headRefOid"], pull["baseRefOid"]
    parts = [f"# Review request: PR {pr}, round {number}\n",
             f"- Pull request: {pull['url']}",
             f"- Base: `{pull['baseRefName']}` at `{base}`",
             f"- Head: `{head}`",
             f"- Check Evidence Line: `{line}`",
             f"- Scoped diff: `git diff {base}...{head}`",
             "- The request has no `guards` field. Wording findings stay non-blocking.\n",
             f"# Pull request\n\n{pull['body']}"]
    for issue in sorted({i["number"] for i in pull["closingIssuesReferences"]} | {int(n) for n in CLOSES.findall(pull["body"])}):
        value = gh_json("issue", "view", str(issue), "--json", "number,title,body")
        parts.append(f"# Issue #{value['number']}: {value['title']}\n\n{value['body']}")
    path = directory / "request.md"
    path.write_text("\n".join(parts) + "\n")
    return path


def prompt(axis, path):
    """The review prompt of one axis with the inputs of this request in place of the request JSON inputs."""
    text = PROMPT.read_text().replace("{{axis}}", axis).replace("{{request}}", str(path))
    intro, inputs = text[text.index("You are the independent"):].split("\n## Inputs\n", 1)
    axes, rules = inputs[inputs.index("- Standards axis:"):].split("\n## Rules\n", 1)
    return f"{intro}\n## Inputs\n\n{INPUTS}\n{axes}\n## Rules\n{rules}"


def listed(items, numbered=True):
    return "\n".join(f"{n}. {item}" if numbered else f"- {item}" for n, item in enumerate(items, 1)) or "None."


def post(pr, body):
    """Post one PR comment. Before a retry, look for a comment with the same heading, head, and Codex thread."""
    start = body.split("\n", 4)[:4]

    def posted():
        found = [c["url"] for c in pull_request(pr)["comments"] if c["body"].split("\n", 4)[:4] == start]
        return found[-1] if found else None

    return run(["gh", "pr", "comment", str(pr), "--body", body], done=posted).strip()


def link_verdicts(pr, pull):
    """Link the verdict comments from the `## Review verdicts` section of the PR body, and return the rounds.

    Only that section changes, and the PR body changes only when the links change."""
    body, rounds, section = pull["body"], rounds_of(pull), "## Review verdicts\n\n"
    for number, axes in sorted(rounds.items()):
        head = next((head for _, head, _ in axes.values() if head), "unknown")
        section += (f"- Round {number}, head `{head[:12]}`: "
                    + ", ".join(f"{axis} {axes[axis][0]} {axes[axis][2]}" for axis in AXES if axis in axes) + "\n")
    if match := LINKS.search(body):
        new = body[:match.start()] + section + ("\n" if match.end() < len(body) else "") + body[match.end():]
    else:
        new = body + ("\n" if body.endswith("\n") else "\n\n") + section
    if rounds and new != body:
        path = ROOT / "target/review" / f"pr-{pr}" / "body.md"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(new)
        run(["gh", "api", "-X", "PATCH", f"repos/{{owner}}/{{repo}}/pulls/{pr}", "-F", f"body=@{path}", "--silent"])
    return rounds


def review(pr, wait):
    pull = pull_request(pr)
    head, rounds = pull["headRefOid"], rounds_of(pull)
    if pull["state"] == "OPEN":
        # This repairs the links after a failed update of the PR body. A closed PR stays as it is.
        link_verdicts(pr, pull)
    last = max((n for n, axes in rounds.items() if len(axes) == len(AXES)), default=0)
    number = last + 1
    previous = rounds.get(last, {})
    if previous and all(result == "PASS" and round_head == head for result, round_head, _ in previous.values()):
        raise Refused(f"Round {last} passed on the two axes, and the head {head} did not change. Only a blocking finding "
                      "or a changed head starts a new round. Send the PR link and the verdict links to the coordinator session.")
    if number > ROUNDS:
        raise Refused(LAST_ROUND.format(pr=pr))
    # A round with only one verdict comment runs again on the two axes. The newer comment of an axis counts.
    print(f"Round {number} of {ROUNDS} for PR {pr}", flush=True)
    line = evidence_line(pull["body"], head)
    if git("rev-parse", "HEAD") != head or git("status", "--porcelain", "--untracked-files=all"):
        raise Refused(f"Check out the head {head} of PR {pr} in a clean working tree, then run the command again")
    wait_verify(head, wait)
    run(["git", "fetch", "--no-tags", "origin", pull["baseRefName"]])
    directory = ROOT / "target/review" / f"pr-{pr}" / f"round-{number}"
    directory.mkdir(parents=True, exist_ok=True)
    path = request(pr, number, pull, line, directory)
    jobs = {}
    try:
        for axis in AXES:
            (directory / f"{axis.lower()}-prompt.md").write_text(prompt(axis.lower(), path))
            jobs[axis] = codex("task", "--background", "--fresh", "--prompt-file",
                               str(directory / f"{axis.lower()}-prompt.md"))["jobId"]
        verdicts = {axis: verdict(job) for axis, job in jobs.items()}
    finally:
        stop_brokers()
    for axis, value in verdicts.items():
        result = "FAIL" if value["blocking"] else "PASS"
        post(pr, f"## {axis} review, round {number}: {result}\n\n"
                 f"Head `{head}`.\n"
                 f"Reviewer: Codex plugin, new read-only thread `{value['thread']}`.\n"
                 f"Check Evidence Line: `{line}`.\n\n"
                 f"### Blocking\n\n{listed(value['blocking'])}\n\n"
                 f"### Non-blocking\n\n{listed(value['non_blocking'])}\n\n"
                 f"### Evidence\n\n{listed(value['evidence'], numbered=False)}\n")
    results = link_verdicts(pr, pull_request(pr))[number]
    print(f"Round {number} of {ROUNDS} for PR {pr}. Request: {path}")
    for axis, (result, _, url) in results.items():
        print(f"{axis}: {result} {url}")
    if all(result == "PASS" for result, _, _ in results.values()):
        print(f"Next: send the PR link and the verdict links to the coordinator session. {BASE_MERGE}")
    elif number < ROUNDS:
        print(f"Next: fix the blocking findings, commit, push, run make check, put its line in the PR body, "
              f"and run make review PR={pr} again.")
    else:
        print("Next: this was the last round. Send the open blocking findings to the coordinator session.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, required=True)
    parser.add_argument("--wait", type=int, default=1200, help="the seconds to wait for the GitHub verify check of the head")
    args = parser.parse_args()
    try:
        review(args.pr, args.wait)
    except (Refused, OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Review refused: {error}\n")


if __name__ == "__main__":
    main()
