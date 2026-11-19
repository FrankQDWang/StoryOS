#!/usr/bin/env python3
"""Run the shared Web test phases of `make check` in order; each file declares its phase on its first line."""

import graphlib
import json
import os
from pathlib import Path
import subprocess
import sys

PHASES = [
    ("http-bootstrap", "node-postgresql", None),
    ("author-edit-process-cut", "node-process-cut", "reset_command_challenge_rate_windows"),
    ("http-main", "node-postgresql", "reset_command_challenge_rate_windows"),
    ("readable-export-process-cut", "node-process-cut", "reload_controlled_fixture"),
    ("project-export-process-cut", "node-process-cut", "reload_controlled_fixture"),
]
HEADER = "// Verification:"


def plan(root):
    projects = {name: project for name, project, _ in PHASES}
    declarations = {}
    for project in sorted(set(projects.values())):
        for file in sorted((root / "apps/web/test" / project).rglob("*.test.ts")):
            path = str(file.relative_to(root))
            first = file.read_text().split("\n", 1)[0]
            if not first.startswith(HEADER):
                raise ValueError(f"{path}: the first line must declare the shared phase")
            declaration = json.loads(first.removeprefix(HEADER))
            if (not isinstance(declaration, dict) or set(declaration) != {"phase", "after"}
                    or projects.get(declaration["phase"]) != project
                    or not isinstance(declaration["after"], list)):
                raise ValueError(f"{path}: invalid shared phase declaration")
            declarations[path] = declaration
    result = []
    for name, project, prepare in PHASES:
        graph = {path: data["after"] for path, data in declarations.items() if data["phase"] == name}
        for path, after in graph.items():
            if any(dependency not in graph for dependency in after):
                raise ValueError(f"{path}: an earlier file is missing or in another phase")
        if not graph:
            raise ValueError(f"The shared phase is empty: {name}")
        sorter = graphlib.TopologicalSorter(graph)
        sorter.prepare()
        files = []
        while sorter.is_active():
            ready = sorted(sorter.get_ready())
            files.extend(ready)
            sorter.done(*ready)
        result.append({"name": name, "project": project, "prepare": prepare, "files": files})
    return result


def run(root, phase):
    environment = {**os.environ, "repository_root": str(root)}
    if phase["prepare"]:
        subprocess.run(["sh", "-ec", '. "$repository_root/scripts/lib/controlled-postgres.sh"; '
                        '"$1" "$STORYOS_TEST_POSTGRES_CONTAINER"', "prepare", phase["prepare"]],
                       env=environment, check=True)
    files = [path.removeprefix("apps/web/") for path in phase["files"]]
    output = root / "target/check" / f"{phase['name']}.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    print(f"Shared phase {phase['name']}: {len(files)} files", flush=True)
    subprocess.run(["pnpm", "--dir", "apps/web", "exec", "vitest", "run", "--project", phase["project"], *files,
                    "--passWithNoTests=false", "--allowOnly=false", "--reporter=default", "--reporter=json",
                    f"--outputFile={output}"],
                   env={**environment, "STORYOS_VITEST_FILE_ORDER": ":".join(files)}, check=True)
    suites = json.loads(output.read_text()).get("testResults", [])
    if ({suite["name"] for suite in suites} != {str(root / path) for path in phase["files"]}
            or any(not any(test["status"] == "passed" for test in suite["assertionResults"]) for suite in suites)):
        raise ValueError(f"{phase['name']}: a listed file did not run or has no passing test")


def main():
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    try:
        phases = plan(root)
        if sys.argv[1:] == ["--list"]:
            print(json.dumps([{"name": phase["name"], "files": phase["files"]} for phase in phases], indent=2))
            return 0
        if sys.argv[1:] or not os.environ.get("STORYOS_TEST_POSTGRES_CONTAINER"):
            raise ValueError("usage: scripts/dev-postgres.sh with python3 scripts/check-phases.py [--list]")
        for phase in phases:
            run(root, phase)
        return 0
    except (ValueError, OSError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        print(f"check-phases: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
