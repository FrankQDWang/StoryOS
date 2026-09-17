"""Plan and run conservative daily verification from changed files."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

import verification


def cargo_targets(root, changes, files):
    rust = [path for path in changes if files.get(path, {}).get("group", "").startswith("cargo:")]
    if not rust:
        return {}
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--locked", "--offline", "--format-version", "1"], cwd=root))
    packages = {p["name"]: p for p in metadata["packages"]}
    directories = {Path(p["manifest_path"]).parent.resolve(): name for name, p in packages.items()}
    owners = {name for directory, name in directories.items()
              if any((root / path).is_relative_to(directory) for path in rust)}
    affected = set(owners)
    if any(files[path]["kind"] != "rust-test" for path in rust):
        while True:
            consumers = {name for name, p in packages.items() if any(
                dep.get("path") and directories.get(Path(dep["path"]).resolve()) in affected
                for dep in p["dependencies"])}
            if consumers <= affected:
                break
            affected.update(consumers)
    return {name: {"directory": str(Path(packages[name]["manifest_path"]).parent.relative_to(root)),
                   "targets": [target["name"] for target in packages[name]["targets"] if target["test"]]}
            for name in sorted(affected)}


def build_plan(root, base, workers=None):
    base = verification.git(root, "rev-parse", "--verify", f"{base}^{{commit}}")
    source = verification.source_identity(root)
    files = {item["path"]: item for item in verification.inventory(root)["files"]}
    changes = set()
    for arguments in (("diff", base, "HEAD"), ("diff", "--cached", "HEAD"), ("diff",)):
        output = subprocess.check_output(
            ["git", *arguments, "--name-only", "--no-renames", "-z", "--"], cwd=root)
        changes.update(output.decode().split("\0"))
    changes.update(subprocess.check_output(
        ["git", "ls-files", "--others", "--exclude-standard", "-z"], cwd=root).decode().split("\0"))
    changes.discard("")
    if not changes:
        raise ValueError("No changed inputs; there is no selected test run")
    policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
    limit = min(policy.get("daily_workers", 2), os.cpu_count() or 1)
    workers = limit if workers is None else workers
    if not 1 <= workers <= limit:
        raise ValueError(f"The daily worker budget must be between 1 and {limit}")
    targets = cargo_targets(root, changes, files)
    selected, complete = {}, []
    for path in sorted(changes):
        item = files.get(path, {})
        group = item.get("group", "complete")
        marker = policy.get("file_profiles", {}).get(group.split(":")[0])
        if group.startswith("cargo:"):
            owner = next((name for name, data in targets.items() if path.startswith(data["directory"] + "/")), None)
            if owner is None:
                raise ValueError(f"No Cargo target owns {path}")
            group = f"cargo:{owner}"
            if any(re.search(r"#\s*\[\s*(?:ignore|cfg_attr)\b", file.read_text())
                   for file in (root / targets[owner]["directory"]).rglob("*.rs")):
                marker = None
        if ((root / path).is_file() and item.get("kind") in {"web-test", "rust-test"}
                and marker and (root / path).read_text().startswith(marker + "\n")):
            selected.setdefault(group, []).append(path)
        else:
            complete.append(f"{path}: deleted, shared, production, or undeclared isolated input")
    checks = [{"group": "policy", "files": [], "reasons": ["Validate input ownership and the runner"]}]
    if "node-contract" in selected:
        checks.append({"group": "web-typecheck", "files": selected["node-contract"],
                       "reasons": ["Prepare locked Web dependencies and check test types"]})
    checks.extend({"group": group, "files": paths,
                   "reasons": [f"{path}: declared repository-only test" for path in paths]}
                  for group, paths in sorted(selected.items()))
    if complete:
        checks = [{"group": "complete", "files": [], "reasons": complete}]
    plan = {"version": 1, "base": base, "source": source, "changes": sorted(changes), "checks": checks,
            "workers": workers,
            "cargo_targets": targets,
            "test_files": sorted(path for path, item in files.items()
                                 if item["kind"].endswith("-test") and (root / path).is_file())}
    if source != verification.source_identity(root):
        raise ValueError("Inputs changed while building the plan")
    plan["digest"] = hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest()
    return plan


def execute_plan(root, plan):
    directory = Path(os.environ["STORYOS_VERIFICATION_RUN"])
    for check in plan["checks"]:
        group = check["group"]
        if group == "complete":
            command = ["make", "verify-local-steps"]
        elif group == "policy":
            command = ["make", "verify-policy"]
        elif group == "web-typecheck":
            command = ["make", "web-typecheck"]
        elif group.startswith("cargo:"):
            command = ["cargo", "test", "--locked", "--tests", "--all-features", "-p", group.removeprefix("cargo:"),
                       "--jobs", str(plan["workers"])]
            artifacts = subprocess.check_output(
                [*command, "--no-run", "--message-format=json"], cwd=root, text=True)
            (directory / f"{group.replace(':', '-')}-artifacts.jsonl").write_text(artifacts)
            compiled = set()
            for line in artifacts.splitlines():
                artifact = json.loads(line)
                if (artifact.get("reason") == "compiler-artifact" and artifact["profile"]["test"]
                        and artifact.get("executable")):
                    dependencies = Path(artifact["executable"]).with_suffix(".d").read_text()
                    compiled.update((root / entry[:-1].replace("\\ ", " ")).resolve()
                                    for entry in dependencies.splitlines()
                                    if entry.endswith(":") and not entry.startswith("#"))
            missing = sorted(path for path in check["files"] if (root / path).resolve() not in compiled)
            if missing:
                raise ValueError(f"Selected files were not compiled into a test target: {missing}")
            listing = subprocess.check_output([*command, "--", "--list", "--format", "terse"], cwd=root, text=True)
            print(listing, flush=True)
            if not any(line.endswith(": test") for line in listing.splitlines()):
                raise ValueError("The selected Cargo target contains no tests")
            command.extend(["--", "--test-threads", str(plan["workers"])])
        elif group == "node-contract":
            output = directory / "vitest.json"
            command = ["pnpm", "--dir", "apps/web", "exec", "vitest", "run", "--project", group,
                       *[str(root / path) for path in check["files"]], "--passWithNoTests=false",
                       "--allowOnly=false", f"--maxWorkers={plan['workers']}", "--reporter=default",
                       "--reporter=json", f"--outputFile={output}"]
        else:
            raise ValueError(f"Unsupported execution group: {group}")
        code = verification.step(root, group.replace(":", "-").replace("_", "-").lower(), command)
        if code:
            return code
        if group == "node-contract":
            result = json.loads(output.read_text())
            suites = result.get("testResults", [])
            expected = {str(root / path) for path in check["files"]}
            if (result.get("success") is not True or {suite["name"] for suite in suites} != expected
                    or any(not any(test["status"] == "passed" for test in suite["assertionResults"])
                           for suite in suites)):
                raise ValueError("Selected files were missing or had no passing tests")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("plan", "run", "execute"))
    parser.add_argument("--base", default="origin/main")
    parser.add_argument("--plan", type=Path)
    parser.add_argument("--expected")
    parser.add_argument("--workers", type=int)
    parser.add_argument("--no-cache", action="store_true")
    args = parser.parse_args()
    try:
        root = Path(verification.git(Path.cwd(), "rev-parse", "--show-toplevel"))
        plan = build_plan(root, args.base, args.workers)
        if ((args.plan and json.loads(args.plan.read_text()) != plan)
                or (args.expected and args.expected != plan["digest"])):
            raise ValueError("The verification plan is stale or has been changed")
        if args.action == "plan":
            print(json.dumps(plan, indent=2))
            return 0
        if args.action == "execute":
            return execute_plan(root, plan)
        command = [sys.executable, str(Path(__file__).resolve()), "execute", "--base", plan["base"],
                   "--expected", plan["digest"], "--workers", str(plan["workers"])]
        return verification.run(root, command, plan=plan, no_cache=args.no_cache)
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
