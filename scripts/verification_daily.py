"""Select bounded daily obligations from current and prior file ownership."""

import fnmatch
import re


GROUPS = {"policy", "contracts", "web-typecheck", "node-contract", "browser-source",
          "database", "node-postgresql", "node-process-cut", "exact-dist", "recovery", "pending"}
PACKAGE = {"browser-source", "node-postgresql", "node-process-cut", "database"}


def validate(policy):
    for name, entry in policy.get("targeted", {}).items():
        if (not re.fullmatch(r"[a-z][a-z0-9-]*", name) or set(entry) != {"command", "clean"}
                or type(entry['clean']) is not bool or not isinstance(entry['command'], list)
                or not entry['command'] or not all(isinstance(arg, str) for arg in entry['command'])
                or any(arg in {'verify-local', 'verify-local-steps', 'verify'} for arg in entry['command'])):
            raise ValueError("Invalid targeted check; complete execution is not a targeted entry")
    rules = policy.get("daily_consumers", [])
    for rule in rules:
        if (set(rule) != {"pattern", "groups"} or not isinstance(rule["pattern"], str)
                or not isinstance(rule["groups"], list) or not rule["groups"]
                or any(group not in GROUPS for group in rule["groups"])):
            raise ValueError("Invalid daily consumer rule; update verification-policy.json")


def checks(root, changes, files, previous, targets, policy, dirty):
    rules = policy.get("daily_consumers", [])
    selected = {"policy": {"group": "policy", "files": [], "reasons": ["Validate input ownership and the runner"]}}
    consumers = {group for data in targets.values() for rule in rules
                 if fnmatch.fnmatchcase(data["directory"] + "/src/", rule["pattern"])
                 for group in rule["groups"]}
    for path in sorted(changes):
        items = [mapping[path] for mapping in [files, *previous] if path in mapping]
        if not items:
            raise ValueError(f"Unclassified prior input: {path}; update verification-policy.json")
        groups = {group for rule in rules if fnmatch.fnmatchcase(path, rule["pattern"]) for group in rule["groups"]}
        groups.update(consumers)
        if path.startswith("crates/") or path in {"Cargo.toml", "Cargo.lock"}:
            groups.update(f"cargo:{name}" for name in targets)
        for item in items:
            group = item["group"]
            if group.startswith("cargo:"):
                groups.update(f"cargo:{name}" for name in targets)
            elif group in {"node-contract", "browser-source", "node-postgresql", "node-process-cut"}:
                groups.add(group)
            elif group == "browser-exact-dist":
                groups.add("exact-dist")
            elif not groups:
                groups.add({"verification-tools": "policy", "contracts": "contracts"}.get(group, "pending"))
        if path.startswith("crates/") and not any(path.startswith(data["directory"] + "/") for data in targets.values()):
            groups.add("pending")
        if not groups:
            groups.add("pending")
        for group in sorted(groups):
            check = selected.setdefault(group, {"group": group, "files": [], "reasons": []})
            check["reasons"].append(f"{path}: current and prior ownership")
            check["files"].append(path)
    for name, data in targets.items():
        if ("database" not in selected and any(re.search(r"#\s*\[\s*(?:ignore|cfg_attr)\b", p.read_text())
                                               for p in (root / data["directory"]).rglob("*.rs"))):
            selected["pending"] = {"group": "pending", "files": [], "reasons": [f"{name}: conditional or ignored tests need an explicit resource scope"]}
    for group, check in list(selected.items()):
        if group.startswith("cargo:"):
            directory = targets[group.removeprefix("cargo:")]["directory"]
            check["files"] = sorted(path for path, item in files.items() if item["kind"] == "rust-test"
                                      and path.startswith(directory + "/") and (root / path).is_file())
        elif group in {"node-contract", "browser-source"}:
            marker = policy.get("file_profiles", {}).get(group)
            isolated = marker and all((root / path).is_file() and (root / path).read_text().startswith(marker + "\n")
                                      for path in check["files"])
            if not isolated:
                check["files"] = sorted(path for path, item in files.items()
                                          if item["group"] == group and (root / path).is_file())
            check["requires_package"] = not (marker and check["files"] and all(
                (root / path).read_text().startswith(marker + "\n") for path in check["files"]))
        if group in PACKAGE:
            check["requires_package"] = True
        if group in {"exact-dist", "recovery", "pending"} or (dirty and check.get("requires_package")):
            check["status"] = "pending"
            check["reasons"].append("Use the reviewed candidate workflow for this proof" if group in
                                    {"exact-dist", "recovery", "pending"} else "Release package requires clean sources")
        else:
            check["status"] = "ready"
    if any(group in selected for group in PACKAGE | {"node-contract", "web-typecheck"}):
        selected.setdefault("web-typecheck", {"group": "web-typecheck", "files": [],
                                               "reasons": ["Prepare locked Web dependencies and check types"], "status": "ready"})
    order = ["policy", "contracts", "web-typecheck", *sorted(g for g in selected if g.startswith("cargo:")),
             "node-contract", "browser-source", "database", "node-postgresql", "node-process-cut", "exact-dist", "recovery", "pending"]
    return [selected[group] for group in order if group in selected]
