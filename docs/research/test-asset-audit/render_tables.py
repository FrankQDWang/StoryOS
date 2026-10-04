"""Render manually reviewed audit rows; do not infer verdicts."""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent


def render(name):
    rows = json.loads((OUT / f"{name}.json").read_text())
    paths = sorted({row["path"] for row in rows})
    text = [f"# {name} test verdicts", "",
            f"Reviewed: {len(rows)} cases in {len(paths)} files. See PROGRESS.md for directory completion.",
            "", "Reason codes: [METHOD.md](METHOD.md). Locations use the fixed audit baseline.", ""]
    for path in paths:
        text += [f"## {path}", "",
                 "| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |",
                 "|---|---|---|---|---|---|---|"]
        for row in rows:
            if row["path"] != path:
                continue
            values = [row["id"], f'{row["line"]} — {row["test"]}', row["verdict"],
                      row["reason"], row["evidence"], row["coverage"]]
            text.append("| " + " | ".join(value.replace("|", "\\|") for value in values) + " |")
        text.append("")
    (OUT / f"{name}.md").write_text("\n".join(text) + "\n")


def append(name, groups):
    target = OUT / f"{name}.json"
    rows = json.loads(target.read_text()) if target.exists() else []
    prefix = {"core": "CO", "application": "AP", "contracts": "CT", "server": "SV",
              "adapter": "AD"}[name]
    for path, entries in groups:
        assert not any(row["path"] == path for row in rows), path
        source = (ROOT / path).read_text()
        tests = list(re.finditer(r'#\[(?:\w+::)?test(?:\([^\]]*\))?\][\s\S]*?\b(?:async\s+)?fn\s+(\w+)\s*\(', source))
        assert len(tests) == len(entries), (path, len(tests), len(entries))
        all_delete = all(entry[0] == "DELETE" for entry in entries)
        for index, (match, entry) in enumerate(zip(tests, entries)):
            verdict, reason, evidence, coverage = entry
            assert verdict in {"DELETE", "KEEP", "MERGE", "MOVE"}, verdict
            start = source[:match.start()].count("\n") + 1
            declaration = source.index("fn ", match.start())
            closing = re.search(r"^}", source[match.end():], re.M)
            assert closing, path
            end = source[:match.end() + closing.end()].count("\n") + 1
            remove = None
            if all_delete:
                remove = [1, len(source.splitlines())] if index == 0 else None
            elif verdict == "DELETE":
                remove = [start, end]
            for cited, line in re.findall(r"([\w/.-]+\.(?:rs|ts)):(\d+)", coverage):
                assert 1 <= int(line) <= len((ROOT / cited).read_text().splitlines()), coverage
            rows.append(dict(id=f"{prefix}{len(rows)+1:03}", path=path,
                             line=source[:declaration].count("\n")+1, test=match[1],
                             verdict=verdict, reason=reason, evidence=evidence,
                             coverage=coverage, remove=remove))
    target.write_text(json.dumps(rows, indent=2) + "\n")
    render(name)


if __name__ == "__main__":
    if len(sys.argv) == 2:
        render(sys.argv[1])
    else:
        append(sys.argv[1], json.loads(sys.argv[2]))
