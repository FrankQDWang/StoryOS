"""Append manual verdicts for plain top-level Web test declarations."""

import json
import re
import sys
from pathlib import Path

from render_tables import OUT, ROOT, render


def append(name, groups):
    target = OUT / f"{name}.json"
    rows = json.loads(target.read_text()) if target.exists() else []
    prefix = {"node-postgresql": "NP", "browser-source": "BS",
              "browser-exact-dist": "BD", "node-process-cut": "PC"}[name]
    for path, entries in groups:
        assert not any(row["path"] == path for row in rows), path
        source = (ROOT / path).read_text()
        declarations = list(re.finditer(r'^\s*(?:test|it)(?:\.each|\()', source, re.M))
        tests = list(re.finditer(r'^(?:test|it)\("((?:[^"\\]|\\.)*)",', source, re.M))
        assert len(tests) == len(declarations) == len(entries), (path, len(tests), len(declarations), len(entries))
        all_delete = all(entry[0] == "DELETE" for entry in entries)
        for index, (match, entry) in enumerate(zip(tests, entries)):
            verdict, reason, evidence, coverage = entry
            assert verdict in {"KEEP", "DELETE", "MERGE", "MOVE"}, verdict
            start = source[:match.start()].count("\n") + 1
            closing = re.search(r'^\}\);', source[match.end():], re.M)
            assert closing, path
            end_offset = match.end() + closing.end()
            if index + 1 < len(tests):
                assert end_offset < tests[index + 1].start(), path
            end = source[:end_offset].count("\n") + 1
            remove = None
            if all_delete:
                remove = [1, len(source.splitlines())] if index == 0 else None
            elif verdict == "DELETE":
                remove = [start, end]
            for cited, line in re.findall(r'([\w/.-]+\.(?:rs|ts)):(\d+)', coverage):
                assert 1 <= int(line) <= len((ROOT / cited).read_text().splitlines()), coverage
            rows.append(dict(id=f"{prefix}{len(rows)+1:03}", path=path, line=start,
                             test=json.loads('"' + match[1] + '"'), verdict=verdict,
                             reason=reason, evidence=evidence, coverage=coverage,
                             remove=remove))
    target.write_text(json.dumps(rows, indent=2) + "\n")
    render(name)


if __name__ == "__main__":
    append(sys.argv[1], json.loads(sys.argv[2]))
