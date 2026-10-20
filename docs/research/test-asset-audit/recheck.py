#!/usr/bin/env python3
"""Recheck the DELETE, MERGE and MOVE verdicts of REPORT.md against one recheck commit.

The script reads only Git objects of the audit baseline and the recheck commit, so the
output does not change when the worktree or main moves. Mechanical facts come from Git:
the current location of each audited test, a body comparison with the baseline, the
current location of each cited covering test, and the commit that removed a test.
recheck-notes.json holds the manual judgment for each row that the mechanical rule
cannot settle. A row without a note is "applies" only when the audited test and every
cited covering test are present with an unchanged body.

Usage:
  python3 docs/research/test-asset-audit/recheck.py           # write the table files
  python3 docs/research/test-asset-audit/recheck.py --check   # fail if a file differs
  python3 docs/research/test-asset-audit/recheck.py --todo    # list rows without a note
"""

import json
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

BASE = "479224809cdaae997cda51cb8853e3fafa242b65"
RECHECK = "e448962b848c5a5b7461ca0bad7bb640d57b1f3e"
DIR = "docs/research/test-asset-audit"
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
MAX_BYTES = 64 * 1024

VERDICTS = ("DELETE", "MERGE", "MOVE")
STATUSES = ("applies", "changed", "obsolete")

# One proposed ticket for each crate or test suite.
TICKETS = [
    ("core", "crates/storyos-core/", "core"),
    ("application", "crates/storyos-application/", "rust"),
    ("contracts", "crates/storyos-contracts/", "rust"),
    ("adapter-postgres", "crates/storyos-adapter-postgres/", "rust"),
    ("server", "crates/storyos-server/", "rust"),
    ("node-postgresql", "apps/web/test/node-postgresql/", "web"),
    ("browser-source", "apps/web/test/browser-source/", "web"),
    ("browser-exact-dist", "apps/web/test/browser-exact-dist/", "web"),
    ("node-contract", "apps/web/test/node-contract/", "web"),
    ("node-process-cut", "apps/web/test/node-process-cut/", "web"),
]
OUTPUTS = {"core": "recheck-core.md", "rust": "recheck-rust.md", "web": "recheck-web.md"}

_cache = {}


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)


def show(rev, path):
    key = (rev, path)
    if key not in _cache:
        r = git("show", f"{rev}:{path}")
        _cache[key] = r.stdout.split("\n") if r.returncode == 0 else None
    return _cache[key]


def short(path):
    for prefix in ("apps/web/test/", "crates/"):
        if path.startswith(prefix):
            return path[len(prefix):]
    return path


def ticket_of(path):
    for code, prefix, _ in TICKETS:
        if path.startswith(prefix):
            return code
    raise SystemExit(f"no ticket for {path}")


def parse_report():
    rows, current = [], None
    for line in show(RECHECK, f"{DIR}/REPORT.md"):
        m = re.match(r"^#### (\S+)\s*$", line)
        if m:
            current = m.group(1)
            continue
        if not (line.startswith("| ") and current):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split(" | ")]
        if len(cells) < 6 or cells[2] not in VERDICTS:
            continue
        m = re.match(r"^(\d+)\s+—\s+(.*)$", cells[1])
        rows.append({
            "id": cells[0], "file": current, "line": int(m.group(1)), "test": m.group(2),
            "verdict": cells[2], "reason": cells[3],
            "covers": re.findall(r"([\w./-]+\.(?:rs|ts|tsx|mjs)):(\d+)", cells[5]),
        })
    return rows


DECL = re.compile(
    r"""\bfn\s+(\w+)\s*[(<]|\b(?:it|test)(?:\.each\(.*?\))?(?:\.\w+)?\(\s*(['"`])(.*?)\2""")


def find_name(lines, name, near=None):
    """Return the 0-based line of the declaration of a named test, or None."""
    quoted = [i for i, l in enumerate(lines) if any(q + name + q in l for q in "'\"`")]
    hits = [i for i, l in enumerate(lines) if re.search(r"\bfn " + re.escape(name) + r"\b", l)]
    hits += [i for i in quoted if re.search(r"\b(?:it|test)\b[\w.]*\(", " ".join(
        lines[max(0, i - 3):i + 1]))]
    hits = sorted(set(hits)) or quoted
    if not hits:
        return None
    if near is None:
        return hits[0]
    return min(hits, key=lambda i: abs(i - near))


def decl_at(lines, line):
    """Return (index, name) of the test declaration at or nearest before a 1-based line."""
    for j in range(line - 1, min(len(lines), line + 4)):
        m = DECL.search(lines[j])
        if m:
            return j, m.group(1) or m.group(3)
    for j in range(min(line - 1, len(lines) - 1), -1, -1):
        m = DECL.search(lines[j])
        if m:
            return j, m.group(1) or m.group(3)
    return None, None


def body(lines, start):
    """Return the stripped lines of a test from its declaration to its closing line."""
    for back in range(0, 4):
        if start - back >= 0 and re.search(r"\bfn |\b(?:it|test)\b[\w.]*\(", lines[start - back]):
            start -= back
            break
    rust = re.search(r"\bfn ", lines[start]) is not None
    indent = len(lines[start]) - len(lines[start].lstrip())
    # Attributes such as #[ignore], #[should_panic] or #[cfg] change what a test proves.
    first = start
    while rust and first > 0 and lines[first - 1].strip().startswith("#["):
        first -= 1
    out = [line.strip() for line in lines[first:start + 1]]
    for line in lines[start + 1:]:
        out.append(line.strip())
        stripped = line.strip()
        if not stripped or len(line) - len(line.lstrip()) != indent:
            continue
        if (rust and stripped == "}") or (
                not rust and re.match(r"^[})].*\)[;,]?$", stripped)):
            break
    return out


def removal(path, name):
    """Return 'sha7 subject' of the first commit after BASE whose file lacks the name."""
    log = git("log", "--reverse", "--format=%H %h %s", f"{BASE}..{RECHECK}", "--", path)
    for entry in log.stdout.splitlines():
        full, sha, subject = entry.split(" ", 2)
        lines = show(full, path)
        if lines is None or find_name(lines, name) is None:
            return f"{sha} {subject}"
    return None


def locate(path, name, near=None):
    """Return the current (path, 1-based line) of a named test in its own file.

    A test with the same name in another file can be a different test, so a test
    that left its file always goes to manual judgment.
    """
    lines = show(RECHECK, path)
    if lines is not None:
        i = find_name(lines, name, near)
        if i is not None:
            return path, i + 1
    return None, None


def cover_state(path, line):
    base = show(BASE, path)
    if base is None:
        return {"ref": f"{path}:{line}", "state": "unknown"}
    i, name = decl_at(base, int(line))
    if name is None:
        return {"ref": f"{path}:{line}", "state": "unknown"}
    cur, cur_line = locate(path, name, i)
    if cur is None:
        return {"ref": f"{path}:{line}", "name": name, "state": "missing",
                "removed": removal(path, name)}
    same = body(base, i) == body(show(RECHECK, cur), cur_line - 1)
    return {"ref": f"{path}:{line}", "name": name, "now": f"{cur}:{cur_line}",
            "state": "same" if same else "edited"}


def bound_stages(path):
    """Return the stage numbers whose crosswalk binds the test file by path."""
    hits = git("grep", "-l", "-F", f'"{path}"', RECHECK, "--",
               "crates/storyos-contracts/src").stdout
    return sorted({m.group(1) for h in hits.splitlines()
                   for m in [re.search(r"stage(\d)_crosswalk", h)] if m})


def facts(row):
    base = show(BASE, row["file"])
    i = find_name(base, row["test"], row["line"] - 1)
    cur, cur_line = locate(row["file"], row["test"], i)
    f = {"now": None, "body": None, "removed": None}
    if cur is not None:
        f["now"] = f"{cur}:{cur_line}"
        if i is not None:
            f["body"] = "same" if body(base, i) == body(show(RECHECK, cur), cur_line - 1) \
                else "edited"
    elif i is not None:
        f["removed"] = removal(row["file"], row["test"])
    f["covers"] = [cover_state(p, l) for p, l in row["covers"]]
    f["bound"] = bound_stages(row["file"])
    return f


REF = re.compile(r"((?:apps/web/test/|crates/)?[\w./-]+\.(?:rs|ts|tsx|mjs)):(\d+)")


def check_ref(row_id, ref):
    """Refuse a manual path:line that does not name a test declaration at RECHECK."""
    path, line = ref.rsplit(":", 1)
    for candidate in (path, "apps/web/test/" + path, "crates/" + path):
        lines = show(RECHECK, candidate)
        if lines is not None:
            break
    else:
        raise SystemExit(f"{row_id}: {path} does not exist at the recheck commit")
    line = int(line)
    if not 1 <= line <= len(lines):
        raise SystemExit(f"{row_id}: {ref} is outside the file at the recheck commit")
    is_test = re.search(r"(_tests\.rs|\.test\.[mt]s|/tests/[\w/]+\.rs)$", candidate)
    if is_test and not any(DECL.search(l) or re.search(r"\.each\(", l)
                           for l in lines[line - 1:line + 4]):
        raise SystemExit(f"{row_id}: {ref} is not a test declaration at the recheck commit")


def check_note(row_id, note):
    if note.get("location"):
        check_ref(row_id, note["location"])
    for path, line in REF.findall(note.get("proof") or ""):
        check_ref(row_id, f"{path}:{line}")


def needs_note(row, f):
    return not (f["now"] and f["body"] == "same"
                and all(c["state"] == "same" for c in f["covers"]))


def resolve(row, f, note):
    if note is None:
        if needs_note(row, f):
            return None
        note = {"status": "applies"}
    check_note(row["id"], note)
    status = note["status"]
    if status not in STATUSES:
        raise SystemExit(f"{row['id']}: bad status {status}")
    loc = note.get("location") or f["now"]
    removed = note.get("removed") or f["removed"]
    if status == "obsolete":
        if f["now"] and not note.get("location"):
            raise SystemExit(f"{row['id']}: obsolete, but the test is present at {f['now']}")
        where = f"removed in {removed.split(' ')[0]}" if removed else "removed"
        path = row["file"]
    else:
        if not loc:
            raise SystemExit(f"{row['id']}: {status} needs a current location")
        path, line = loc.rsplit(":", 1)
        where = f":{line}" if path == row["file"] else f"moved to {short(loc)}"
        path = row["file"] if path == row["file"] else path
    proof = note.get("proof")
    if proof is None and status != "obsolete":
        proof = "; ".join(short(c["now"]) for c in f["covers"] if c.get("now"))
    elif proof:
        proof = "; ".join(short(p.strip()) for p in proof.split(";"))
    text = note.get("note", "")
    if f["bound"]:
        text = (text + " " if text else "") + "Path bound by stage " + "/".join(f["bound"]) \
            + " crosswalk."
    ticket = note.get("ticket") or ("—" if status == "obsolete" else ticket_of(path))
    return {"id": row["id"], "verdict": f"{row['verdict']} {row['reason']}", "status": status,
            "group": row["file"], "where": where, "proof": proof or "—",
            "note": text or "—", "ticket": ticket, "kind": row["verdict"],
            "area": ticket_of(row["file"])}


def cell(text):
    return text.replace("|", "\\|").replace("\n", " ")


def render_table(results, kind):
    out = []
    for code, prefix, k in TICKETS:
        if k != kind:
            continue
        area = [r for r in results if r["area"] == code]
        if not area:
            continue
        out += [f"## {code}", ""]
        for group in sorted({r["group"] for r in area}):
            out += [f"### {short(group)}", "",
                    "| ID | Verdict | Status | Location | Proof test after change | Note | Ticket |",
                    "|---|---|---|---|---|---|---|"]
            for r in sorted((r for r in area if r["group"] == group), key=lambda r: r["id"]):
                out.append("| " + " | ".join(cell(r[c]) for c in (
                    "id", "verdict", "status", "where", "proof", "note", "ticket")) + " |")
            out.append("")
    head = [f"# Test asset audit recheck: {kind} tables", "",
            f"Generated by `recheck.py` from REPORT.md. Audit baseline `{BASE[:8]}`, "
            f"recheck commit `{RECHECK[:8]}`. [RECHECK.md](RECHECK.md) gives the method, the "
            "status definitions and the proposed tickets.", "",
            "Paths omit the prefix `apps/web/test/` or `crates/`. A section heading names the "
            "audited file at the baseline. Location is the current line in that file, the "
            "new path when the test moved, or the commit that removed the test. Proof test "
            "after change is the test that keeps proof of the behavior after the verdict is "
            "applied: the covering test for DELETE, the receiving test for MERGE, and the "
            "target boundary for MOVE.", ""]
    return "\n".join(head + out)


def build(notes):
    rows = parse_report()
    counts = Counter(r["verdict"] for r in rows)
    if counts != Counter({"DELETE": 330, "MERGE": 52, "MOVE": 5}):
        raise SystemExit(f"unexpected verdict counts {counts}")
    unknown = set(notes) - {r["id"] for r in rows}
    if unknown:
        raise SystemExit(f"notes for unknown rows: {sorted(unknown)}")
    results, todo = [], []
    for row in rows:
        f = facts(row)
        r = resolve(row, f, notes.get(row["id"]))
        if r is None:
            todo.append((row, f))
        else:
            results.append(r)
    return rows, results, todo


def summary(results):
    by_area = defaultdict(Counter)
    for r in results:
        by_area[r["area"]][r["status"]] += 1
        if r["status"] != "obsolete" and r["ticket"] == r["area"]:
            by_area[r["area"]]["open-" + r["kind"]] += 1
    lines = ["| Crate or suite | Rows | applies | changed | obsolete |",
             "|---|---:|---:|---:|---:|"]
    total = Counter()
    for code, _, _ in TICKETS:
        c = by_area[code]
        n = c["applies"] + c["changed"] + c["obsolete"]
        total.update({"n": n, **{s: c[s] for s in STATUSES}})
        lines.append(f"| {code} | {n} | {c['applies']} | {c['changed']} | {c['obsolete']} |")
    lines.append(f"| Total | {total['n']} | {total['applies']} | {total['changed']} | "
                 f"{total['obsolete']} |")
    tickets = ["| Proposed ticket | Scope | applies | changed | Open DELETE | Open MERGE | "
               "Open MOVE | Table |", "|---|---|---:|---:|---:|---:|---:|---|"]
    for code, prefix, kind in TICKETS:
        open_rows = [r for r in results if r["ticket"] == code and r["status"] != "obsolete"]
        c = Counter(r["status"] for r in open_rows)
        k = Counter(r["kind"] for r in open_rows)
        tickets.append(f"| Apply the {code} audit verdicts | `{prefix}` | {c['applies']} | "
                       f"{c['changed']} | {k['DELETE']} | {k['MERGE']} | {k['MOVE']} | "
                       f"[{OUTPUTS[kind]}]({OUTPUTS[kind]}#{code}) |")
    return "\n".join(lines), "\n".join(tickets)


def main():
    notes = json.loads((HERE / "recheck-notes.json").read_text())
    rows, results, todo = build(notes)
    if "--todo" in sys.argv:
        for row, f in todo:
            print(json.dumps({"id": row["id"], "verdict": row["verdict"], "file": row["file"],
                              "line": row["line"], "test": row["test"], **f}))
        print(f"{len(todo)} rows need a note", file=sys.stderr)
        return
    if todo:
        raise SystemExit(f"{len(todo)} rows need a note; run with --todo")
    status_table, ticket_table = summary(results)
    template = (HERE / "recheck-summary.md.in").read_text()
    files = {"RECHECK.md": template.replace("{{STATUS_TABLE}}", status_table)
             .replace("{{TICKET_TABLE}}", ticket_table)}
    for kind, name in OUTPUTS.items():
        files[name] = render_table(results, kind)
    failed = False
    for name, text in files.items():
        size = len(text.encode())
        if size > MAX_BYTES:
            raise SystemExit(f"{name} is {size} bytes, above {MAX_BYTES}")
        path = HERE / name
        if "--check" in sys.argv:
            if not path.exists() or path.read_text() != text:
                print(f"{name} differs from the generated text")
                failed = True
        else:
            path.write_text(text)
            print(f"{name}: {size} bytes")
    if failed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
