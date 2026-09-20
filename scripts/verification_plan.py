"""Plan and run conservative daily verification from changed files."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

import verification
import verification_cache
import verification_daily


def cargo_targets(root, changes, files, revisions):
    rust = [path for path in changes if path.startswith("crates/") or path in {"Cargo.toml", "Cargo.lock"}]
    if not rust:
        return {}
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--locked", "--offline", "--format-version", "1"], cwd=root))
    packages = {p["name"]: p for p in metadata["packages"]}
    directories = {Path(p["manifest_path"]).parent.resolve(): name for name, p in packages.items()}
    owners = {name for directory, name in directories.items()
              if any((root / path).is_relative_to(directory) for path in rust)}
    prior_dependencies = {}
    for revision in dict.fromkeys(revisions):
        for path in verification.git(root, "ls-tree", "-r", "--name-only", revision, "crates").splitlines():
            if path.endswith("/Cargo.toml"):
                manifest = tomllib.loads(verification.git(root, "show", f"{revision}:{path}"))
                prior_dependencies.setdefault(manifest["package"]["name"], set()).update(
                    value.get("package", name) for section in ("dependencies", "dev-dependencies", "build-dependencies")
                    for name, value in manifest.get(section, {}).items() if isinstance(value, dict))
    affected = set(owners)
    if any(path in {"Cargo.toml", "Cargo.lock"} for path in rust):
        affected.update(packages)
    for revision in revisions:
        for path in rust:
            if path.count("/") < 2:
                continue
            manifest = "/".join(path.split("/")[:2]) + "/Cargo.toml"
            old = subprocess.run(["git", "show", f"{revision}:{manifest}"], cwd=root, capture_output=True, text=True)
            if old.returncode == 0:
                affected.add(tomllib.loads(old.stdout)["package"]["name"])
    if any(files.get(path, {}).get("kind") != "rust-test" or not (root / path).exists() for path in rust):
        while True:
            consumers = {name for name, p in packages.items() if any(
                dep.get("path") and directories.get(Path(dep["path"]).resolve()) in affected
                for dep in p["dependencies"])}
            consumers.update(name for name, deps in prior_dependencies.items() if deps & affected)
            if consumers <= affected:
                break
            affected.update(consumers)
    if "storyos-adapter-postgres" in affected:
        affected.update(packages)
    return {name: {"directory": str(Path(packages[name]["manifest_path"]).parent.relative_to(root)),
                   "targets": [target["name"] for target in packages[name]["targets"] if target["test"]]}
            for name in sorted(affected & packages.keys())}


def build_plan(root, base, workers=None, *, allow_empty=False):
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
    if not changes and not allow_empty:
        raise ValueError("No changed inputs; there is no selected test run")
    policy = json.loads((root / "docs/agents/verification-policy.json").read_text())
    limit = min(policy.get("daily_workers", 2), os.cpu_count() or 1)
    workers = limit if workers is None else workers
    if not 1 <= workers <= limit:
        raise ValueError(f"The daily worker budget must be between 1 and {limit}")
    previous = [{item["path"]: item for item in verification.inventory(root, revision)["files"]}
                for revision in dict.fromkeys([base, source["commit"]])]
    ownership = {path: item for mapping in [*previous, files] for path, item in mapping.items()}
    targets = cargo_targets(root, changes, ownership, [base, source["commit"]])
    policy["daily_consumers"] = policy.get("daily_consumers", []) + [rule
        for revision in dict.fromkeys([base, source["commit"]])
        for rule in json.loads(verification.git(root, "show", f"{revision}:docs/agents/verification-policy.json")).get("daily_consumers", [])
        if rule not in policy.get("daily_consumers", [])]
    checks = verification_daily.checks(root, changes, files, previous, targets, policy, source["dirty"])
    plan = {"version": 1, "base": base, "source": source, "changes": sorted(changes), "checks": checks,
            "workers": workers, "historical_estimate_seconds": None,
            "preparation": ["web-typecheck"] if any(c["group"] == "web-typecheck" for c in checks) else [],
            "cargo_targets": targets,
            "test_files": sorted(path for path, item in files.items()
                                 if item["kind"].endswith("-test") and (root / path).is_file())}
    if any(c.get("requires_package") and c["status"] == "ready" for c in checks):
        plan["preparation"].append("release-package")
    if any(c["group"] in {"node-postgresql", "node-process-cut"} for c in checks):
        phases = verification.verification_shared.plan(root, policy, list(files.values()))
        plan["shared_phases"] = [phase for phase in phases if any(c["group"] == phase["group"] for c in checks)]
        for check in checks:
            if check["group"] in {"node-postgresql", "node-process-cut"}:
                check["files"] = [path for phase in phases if phase["group"] == check["group"] for path in phase["files"]]
    if source != verification.source_identity(root):
        raise ValueError("Inputs changed while building the plan")
    for path in sorted((root / "target/verification").glob("*/report.json"), reverse=True):
        try:
            report = json.loads(path.read_text())
            prior = report.get("plan", {})
            if (report["status"] in {"passed", "pending"} and report["profile"] == "daily"
                    and all(prior.get(key) == plan[key] for key in ("checks", "workers", "test_files"))):
                plan["historical_estimate_seconds"] = report["duration_seconds"]
                break
        except (OSError, ValueError, KeyError):
            continue
    plan["digest"] = hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest()
    return plan


def execute_plan(root, plan):
    directory = Path(os.environ["STORYOS_VERIFICATION_RUN"])
    cargo_done = package_done = database_done = False
    os.environ["CARGO_BUILD_JOBS"] = str(plan["workers"])
    for check in plan["checks"]:
        group = check["group"]
        if check.get("status") == "pending":
            print(f"Daily scope pending: {group}; inspect make verify-plan BASE={plan['base']}", flush=True)
            continue
        if check.get("requires_package") and not package_done:
            code = verification.step(root, "release-package", [sys.executable, "scripts/package-release.py"])
            if code:
                return code
            package_done = True
        if group == "policy":
            command = ["make", "verify-policy"]
        elif group == "contracts":
            command = ["make", "verify-contract-inputs"]
        elif group in {"database", "node-postgresql", "node-process-cut"}:
            if database_done:
                continue
            database_done = True
            command = ["sh", "scripts/verify-daily-database.sh", *[c["group"] for c in plan["checks"]
                       if c["group"] in {"database", "node-postgresql", "node-process-cut"} and c["status"] == "ready"]]
        elif group == "web-typecheck":
            command = ["make", "web-typecheck"]
        elif group.startswith("cargo:"):
            if cargo_done:
                continue
            cargo_done = True
            groups = [c for c in plan["checks"] if c["group"].startswith("cargo:")]
            selection = (["--workspace"] if "storyos-adapter-postgres" in plan["cargo_targets"] else
                         [arg for c in groups for arg in ("-p", c["group"].removeprefix("cargo:"))])
            command = ["cargo", "test", "--locked", "--all-targets", "--all-features", *selection,
                       "--jobs", str(plan["workers"])]
            compiled = verification.cargo_test_inputs(root, command)
            missing = sorted(path for c in groups for path in c["files"] if (root / path).resolve() not in compiled)
            if missing:
                raise ValueError(f"Selected files were not compiled into a test target: {missing}")
            listing = subprocess.check_output([*command, "--", "--list", "--format", "terse"], cwd=root, text=True)
            print(listing, flush=True)
            if not any(line.endswith(": test") for line in listing.splitlines()):
                raise ValueError("The selected Cargo target contains no tests")
            command.extend(["--", "--test-threads", str(plan["workers"])])
        elif group in {"node-contract", "browser-source"}:
            output = directory / ("vitest.json" if group == "node-contract" else "browser-source.json")
            dependencies = verification_cache.outputs(root)
            (directory / "dependencies.json").write_text(json.dumps(dependencies))
            command = ["pnpm", "--dir", "apps/web", "exec", "vitest", "run", "--project", group,
                       *[str(root / path) for path in check["files"]], "--passWithNoTests=false",
                       "--allowOnly=false", "--cache=false", f"--maxWorkers={plan['workers']}", "--reporter=default",
                       "--reporter=json", f"--outputFile={output}"]
        else:
            raise ValueError(f"Unsupported execution group: {group}")
        code = verification.step(root, group.replace(":", "-").replace("_", "-").lower(), command)
        if code:
            return code
        if group in {"node-contract", "browser-source"}:
            if dependencies != verification_cache.outputs(root):
                raise ValueError("Installed dependencies changed during the selected tests")
            result = json.loads(output.read_text())
            suites = result.get("testResults", [])
            expected = {str(root / path) for path in check["files"]}
            if (result.get("success") is not True or {suite["name"] for suite in suites} != expected
                    or any(not any(test["status"] == "passed" for test in suite["assertionResults"])
                           for suite in suites)):
                raise ValueError("Selected files were missing or had no passing tests")
    return 2 if any(check.get("status") == "pending" for check in plan["checks"]) else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("plan", "status", "run", "execute"))
    parser.add_argument("--format", choices=("json", "text"), default="json")
    parser.add_argument("--issue", type=int)
    parser.add_argument("--pr", type=int)
    parser.add_argument("--base", default="origin/main")
    parser.add_argument("--plan", type=Path)
    parser.add_argument("--expected")
    parser.add_argument("--workers", type=int)
    parser.add_argument("--no-cache", action="store_true")
    args = parser.parse_args()
    try:
        root = Path(verification.git(Path.cwd(), "rev-parse", "--show-toplevel"))
        plan = build_plan(root, args.base, args.workers, allow_empty=args.action == "status")
        if ((args.plan and json.loads(args.plan.read_text()) != plan)
                or (args.expected and args.expected != plan["digest"])):
            raise ValueError("The verification plan is stale or has been changed")
        if args.action in {"plan", "status"}:
            if args.action == "plan" and args.format == "json":
                print(json.dumps(plan, indent=2))
            else:
                verification.verification_status.display(verification.verification_status.status(root, plan), args.format == "json")
            return 0
        if args.action == "execute":
            return execute_plan(root, plan)
        command = [sys.executable, str(Path(__file__).resolve()), "execute", "--base", plan["base"],
                   "--expected", plan["digest"], "--workers", str(plan["workers"])]
        return verification.run(root, command, plan=plan, no_cache=args.no_cache,
                                context={"issue": args.issue, "pr": args.pr})
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
