"""Discover and run serial shared Web phases from file declarations."""

import argparse
import graphlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def plan(root, policy, files, revision=None):
    phases = policy.get("shared_phases", [])
    names = set()
    for phase in phases:
        if (not isinstance(phase, dict) or set(phase) != {"name", "group", "stage", "prepare"}
                or not all(isinstance(value, str) for value in phase.values())
                or not re.fullmatch(r"[a-z][a-z0-9-]*", phase["name"])
                or phase["name"] in names or phase["group"] not in {"node-postgresql", "node-process-cut"}
                or phase["prepare"] not in {"none", "reset-challenge", "reload-fixture"}
                or not re.fullmatch(r"[a-z][a-z0-9-]*", phase["stage"])
                or ("complete" in policy and phase["stage"] not in
                    policy["complete"]["groups"].get(phase["group"], []))):
            raise ValueError("Invalid or duplicate shared phase in verification-policy.json")
        names.add(phase["name"])
    declarations = {}
    for item in files:
        path = item["path"]
        if item["kind"] != "web-test" or item["group"] not in {"node-postgresql", "node-process-cut"}:
            continue
        if not revision and not (root / path).exists():
            continue
        source = (subprocess.check_output(["git", "show", f"{revision}:{path}"], cwd=root, text=True)
                  if revision else (root / path).read_text())
        lines = [line for line in source.splitlines() if line.startswith("// Verification:")]
        if len(lines) != 1 or not source.startswith(lines[0] + "\n"):
            raise ValueError(f"{path}: require one first-line shared phase declaration")
        declaration = json.loads(lines[0].removeprefix("// Verification:"))
        if (not isinstance(declaration, dict) or set(declaration) != {"phase", "after"}
                or not isinstance(declaration["phase"], str) or declaration["phase"] not in names
                or not isinstance(declaration["after"], list)
                or any(not isinstance(dep, str) for dep in declaration["after"])
                or len(declaration["after"]) != len(set(declaration["after"]))
                or next(p for p in phases if p["name"] == declaration["phase"])["group"] != item["group"]):
            raise ValueError(f"{path}: invalid shared phase or dependencies")
        declarations[path] = declaration
    for path, declaration in declarations.items():
        for dependency in declaration["after"]:
            if dependency not in declarations or declarations[dependency]["phase"] != declaration["phase"]:
                raise ValueError(f"{path}: dangling or cross-phase dependency: {dependency}")
    result = []
    for phase in phases:
        graph = {path: data["after"] for path, data in declarations.items() if data["phase"] == phase["name"]}
        if not graph:
            raise ValueError(f"Required shared phase is empty: {phase['name']}")
        sorter = graphlib.TopologicalSorter(graph)
        sorter.prepare()
        ordered = []
        while sorter.is_active():
            ready = sorted(sorter.get_ready())
            ordered.extend(ready)
            sorter.done(*ready)
        result.append({**phase, "files": ordered})
    return result


def main():
    import verification

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("plan", "run"))
    args = parser.parse_args()
    try:
        root = Path(verification.git(Path.cwd(), "rev-parse", "--show-toplevel"))
        policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
        phases = plan(root, policy, verification.inventory(root)["files"])
        if args.action == "plan":
            print(json.dumps(phases, indent=2))
            return 0
        if not phases or not os.environ.get("STORYOS_TEST_POSTGRES_CONTAINER"):
            raise ValueError("Shared execution requires a phase policy and a prepared PostgreSQL fixture")
        for phase in phases:
            prepare = {"none": None, "reset-challenge": "reset_command_challenge_rate_windows",
                       "reload-fixture": "reload_controlled_fixture"}[phase["prepare"]]
            if prepare:
                subprocess.run(["sh", "-ec", '. "$repository_root/scripts/lib/controlled-postgres.sh"; '
                                '"$1" "$STORYOS_TEST_POSTGRES_CONTAINER"', "shared-prepare", prepare],
                               cwd=root, env={**os.environ, "repository_root": str(root)}, check=True)
            files = [path.removeprefix("apps/web/") for path in phase["files"]]
            os.environ["STORYOS_VITEST_FILE_ORDER"] = ":".join(files)
            code = verification.step(root, phase["stage"],
                                     ["pnpm", "--dir", "apps/web", "exec", "vitest", "run",
                                      "--project", phase["group"], *files])
            if code:
                return code
        return 0
    except (ValueError, OSError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
