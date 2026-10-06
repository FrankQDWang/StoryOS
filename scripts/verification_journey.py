#!/usr/bin/env python3
"""Run one exact-dist journey file with the project-scope procedure and print its pass count table."""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys

import verification_status

JOURNEYS = "apps/web/test/browser-exact-dist"


def journey_file(root, name):
    """Return the record name and the apps/web-relative path of one exact-dist journey file."""
    if not name or "/" in name or name.startswith("."):
        raise ValueError(f"FILE must name one file of {JOURNEYS}, for example FILE=s2-statistics")
    stem = name.removesuffix(".ts").removesuffix(".test").removesuffix(".integration")
    matches = [stem + suffix for suffix in (".integration.test.ts", ".test.ts")
               if (root / JOURNEYS / (stem + suffix)).is_file()]
    if len(matches) != 1:
        raise ValueError(f"FILE={name} must match exactly one test file of {JOURNEYS}; it matches {len(matches)}")
    return stem, "test/browser-exact-dist/" + matches[0]


def count(name, value, lowest, highest):
    if not value.isdecimal() or not lowest <= int(value) <= highest:
        raise ValueError(f"{name} must be an integer from {lowest} to {highest}; it is {value!r}")
    return int(value)


def stream(command, log, root):
    with log.open("w") as output, subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE,
                                                   stderr=subprocess.STDOUT, text=True) as process:
        for line in process.stdout:
            sys.stdout.write(line)
            output.write(line)
        return process.wait()


def table(name, rows, runs, record):
    passed = sum(row["status"] == "passed" for row in rows)
    seconds = [row["seconds"] for row in rows]
    duration = f"{min(seconds)}-{max(seconds)} s" if seconds else "none"
    lines = [f"Journey {name}", "run  status  seconds  load1m"]
    lines += [f"{row['run']:<4} {row['status']:<7} {row['seconds']:<8} {row['load1m']}" for row in rows]
    lines.append(f"Passed: {passed} of {runs}; duration: {duration}; records: {record}")
    return "\n".join(lines)


def run(root, name, runs, load, runner=stream):
    stem, test = journey_file(root, name)
    runs = count("RUNS", runs, 1, 100)
    load = count("LOAD", load, 0, os.cpu_count() or 1)
    unmet = verification_status.refusal(verification_status.clean_tree(root, True, "A journey run"))
    if unmet:
        raise ValueError(unmet)
    record = root / "target/verification/journeys" / stem / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    record.mkdir(parents=True)
    command = ["sh", str(root / "scripts/verify-journey.sh"), test, str(runs), str(load), str(record)]
    code = runner(command, record / "journey.log", root)
    tsv = record / "runs.tsv"
    rows = [dict(zip(("run", "status", "seconds", "load1m"), line.split("\t")))
            for line in (tsv.read_text().splitlines() if tsv.exists() else [])]
    for row in rows:
        row.update(run=int(row["run"]), seconds=int(row["seconds"]))
    passed = sum(row["status"] == "passed" for row in rows)
    summary = {"file": test, "runs": runs, "load": load, "passed": passed, "exit_code": code, "rows": rows}
    (record / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(table(stem, rows, runs, record), flush=True)
    return 0 if code == 0 and passed == runs else (code or 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--file", required=True)
    parser.add_argument("--runs", default="1")
    parser.add_argument("--load", default="0")
    arguments = parser.parse_args()
    try:
        root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
        return run(root, arguments.file, arguments.runs, arguments.load)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
