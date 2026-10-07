"""Derive operation tables from runtime counters and actual PostgreSQL plans."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

lab = Path(__file__).resolve().parent
out = lab / 'out'
destination = Path(sys.argv[1]) if len(sys.argv) > 1 else out / 'evidence'
destination.mkdir(parents=True, exist_ok=True)
rows = []
for directory in sorted(out.iterdir()):
    if not directory.is_dir() or not directory.name.isdigit():
        continue
    log = (directory / 'postgres.log').read_text()
    plans = {}
    intervals = []
    markers = list(re.finditer(r'LOG:  BASELINE_(START|END) (\d+):([^\s]+)', log))
    for start, end in zip(markers, markers[1:]):
        section = log[start.end():end.start()]
        captured = []
        for prefix in re.finditer(r'LOG:  duration: [\d.]+ ms  plan:\n', section):
            plan, _ = json.JSONDecoder().raw_decode(section[prefix.end():].lstrip())
            captured.append(plan)
        if start[1] == 'START' and end[1] == 'END':
            plans[start[3]] = captured
        elif captured:
            intervals.append({'after': start[3], 'before': end[3], 'classification': 'setup_or_deferred_not_attributed', 'plans': captured})
    def scans(node):
        own = 0
        if node.get('Relation Name') and 'Scan' in node['Node Type']:
            own = (node.get('Actual Rows', 0) + node.get('Rows Removed by Filter', 0) + node.get('Rows Removed by Index Recheck', 0)) * node.get('Actual Loops', 0)
        return own + sum(scans(child) for child in node.get('Plans', []))
    (destination / f'{directory.name}-inter-window-plans.json.gz').write_bytes(gzip.compress(json.dumps(intervals, ensure_ascii=False, separators=(',', ':')).encode(), mtime=0))
    for path in sorted(directory.glob('*.json')):
        data = json.loads(path.read_text())
        if 'operation' not in data:
            continue
        captured = plans.get(data['operation'], [])
        result = {k: v for k, v in data.items() if k != 'statements'}
        result.update(plan_count=len(captured), plan_scan_row_visits=sum(scans(p['Plan']) for p in captured),
                      response_bytes=sum(w['response_bytes'] for w in data['wire']), http_requests=len(data['wire']))
        rows.append(result)
        packed = json.dumps({'observation': data, 'plans': captured}, ensure_ascii=False, separators=(',', ':')).encode()
        (destination / f'{directory.name}-{data["operation"]}.json.gz').write_bytes(gzip.compress(packed, mtime=0))
    for name in ['import.json', 'coordinates.json', 'export-result.json', 'browser-error.json', 'planner-state.json', 'tree-order.json', 'web-proposal-setup.json', 'proposal-setup.json'] + [p.name for p in directory.glob('web-*-failure.json')]:
        path = directory / name
        if path.exists():
            shutil.copyfile(path, destination / f'{directory.name}-{name}')
shutil.copyfile(out / 'corpus-manifest.json', destination / 'corpus-manifest.json')
(destination / 'operations.json').write_text(json.dumps(rows, ensure_ascii=False, indent=2) + '\n')
columns = ['sql_calls', 'sql_rows', 'plan_scan_row_visits', 'shared_hit', 'shared_read', 'response_bytes']
table = ['# Measured operation counters', '', 'Cell: SQL calls / returned-or-affected rows / plan scan row visits / buffer hits / buffer reads / response bytes.', '', '| Operation | 30,000 | 300,000 | 1,000,000 | 3,000,000 |', '| --- | --- | --- | --- | --- |']
for operation in sorted({r['operation'] for r in rows}):
    cells = []
    for scale in [30000, 300000, 1000000, 3000000]:
        row = next((r for r in rows if r['operation'] == operation and r['scale'] == scale), None)
        cells.append('unmeasured' if row is None else (('FAIL; ' if row.get('error') else 'REFUSED; ' if row['outcome'] == 'refused' else '') + ' / '.join(str(row[k]) for k in columns)))
    table.append('| ' + operation + ' | ' + ' | '.join(cells) + ' |')
(destination / 'TABLE.md').write_text('\n'.join(table) + '\n')
idb = ['# IndexedDB observations', '', 'Cell: returned records / write calls / stored records. Repeated reads count again. Stored totals are a post-window census.', '', 'Web windows with delayed editor remount can be lower bounds. FAIL retains the failed window counts. Per-store details are in operations.json.', '', '| Operation | 30,000 | 300,000 | 1,000,000 | 3,000,000 |', '| --- | --- | --- | --- | --- |']
for operation in sorted({r['operation'] for r in rows if r.get('browser')}):
    cells = []
    for scale in [30000, 300000, 1000000, 3000000]:
        row = next((r for r in rows if r['operation'] == operation and r['scale'] == scale), None)
        browser = row.get('browser') if row else None
        cells.append('unmeasured' if not browser else ('FAIL; ' if row.get('error') else '') + ' / '.join(str(browser[k]) for k in ['returned_records', 'writes', 'record_count']))
    idb.append('| ' + operation + ' | ' + ' | '.join(cells) + ' |')
(destination / 'INDEXEDDB.md').write_text('\n'.join(idb) + '\n')
manifest = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(destination.iterdir()) if p.is_file() and p.name != 'SHA256.json'}
(destination / 'SHA256.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'{len(rows)} observed operation rows written to {destination}')
