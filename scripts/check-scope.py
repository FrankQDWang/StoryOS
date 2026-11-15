#!/usr/bin/env python3
"""Print the scope of make check (ADR 0070): `documents` when the change has only documents, or `all`.

The change is the diff from the merge base of the base argument to HEAD. It has only documents when each changed
path is under docs/ or is a rule document at the root, and no file under crates/, apps/, scripts/, .github/, and no
Makefile, has the path as text. A file that names a document can read it, so its change gets all the steps.
An empty change is not a document change."""
import subprocess
import sys

ROOT_DOCUMENTS = {"GLOSSARY.md", "AGENTS.md", "CODING_STANDARDS.md", "CLAUDE.md"}
READERS = ["crates", "apps", "scripts", ".github", ":(glob)**/Makefile"]


def git(*arguments):
    return subprocess.run(["git", *arguments], check=True, capture_output=True, text=True).stdout


def named(path):
    """True when a tracked file under READERS has the path as text."""
    search = subprocess.run(["git", "grep", "-q", "-F", "-e", path, "--", *READERS], capture_output=True, text=True)
    if search.returncode > 1:
        raise SystemExit(f"git grep failed for {path}: {search.stderr.strip()}")
    return search.returncode == 0


def documents_only(paths):
    return bool(paths) and all((path.startswith("docs/") or path in ROOT_DOCUMENTS) and not named(path) for path in paths)


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: scripts/check-scope.py <base>")
    merge_base = git("merge-base", sys.argv[1], "HEAD").strip()
    paths = [path for path in git("diff", "--name-only", "--no-renames", "-z", merge_base, "HEAD").split("\0") if path]
    print("documents" if documents_only(paths) else "all")


if __name__ == "__main__":
    main()
