"""Publish disposable read copies of retained records for every repository worktree."""

from functools import lru_cache
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.error
import urllib.request


def supervision(report=None, *, url='http://127.0.0.1:3754'):
    """Report collection health and exact record receipt without granting admission."""
    result = {'status': 'unavailable', 'receipt': 'not-requested',
              'nextAction': ['make', 'observe-status']}
    try:
        with urllib.request.urlopen(url + '/api/v1/health', timeout=1) as response:
            health = json.load(response)
        result.update(status='ok' if all(health.get(k, {}).get('status') == 'ok'
                      for k in ('collector', 'query')) else 'unavailable',
                      collector=health.get('collector', {}).get('status', 'unavailable'))
        if report:
            path = Path(report)
            result['receipt'] = 'pending'
            with urllib.request.urlopen(url + '/api/v1/runs/' + path.parent.name, timeout=1) as response:
                detail = json.load(response)
            result['receipt'] = ('current' if detail.get('source_sha256') ==
                hashlib.sha256(path.read_bytes()).hexdigest() else 'behind')
            result['timing'] = detail.get('timing', {'reason': 'projection-upgrade-required'})
        if result['status'] == 'ok' and result['receipt'] in {'current', 'not-requested'}:
            result['nextAction'] = None
    except (OSError, ValueError, TypeError):
        pass
    if result['status'] == 'ok' and result['receipt'] in {'pending', 'behind'}:
        result['status'] = 'awaiting-receipt'
    return result


@lru_cache(maxsize=16)
def shared_directory(root):
    common = subprocess.check_output(['git', 'rev-parse', '--git-common-dir'], cwd=root, text=True).strip()
    return (root / common).resolve() / 'storyos-observation'


def publish(path):
    directory = next((p for p in path.parents if p.name == 'verification' and p.parent.name == 'target'), None)
    if directory is None:
        return
    relative = path.relative_to(directory)
    if path.name != 'report.json' and not {'steps', 'nodes', 'requests'}.intersection(relative.parts):
        return
    root = directory.parent.parent
    shared = shared_directory(root)
    destination = shared / 'records' / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as temporary:
        staged = Path(temporary.name)
    try:
        shutil.copyfile(path, staged)
        staged.replace(destination)
    finally:
        staged.unlink(missing_ok=True)
    sources = shared / 'records/sources'
    sources.mkdir(exist_ok=True)
    source = sources / (hashlib.sha256(str(root).encode()).hexdigest() + '.json')
    if not source.exists():
        with tempfile.NamedTemporaryFile(mode='w', dir=sources, delete=False) as output:
            json.dump({'repository': str(root), 'records': str(directory)}, output)
            staged = Path(output.name)
        staged.replace(source)


def import_worktrees(root):
    listing = subprocess.check_output(['git', 'worktree', 'list', '--porcelain', '-z'], cwd=root, text=True)
    records = shared_directory(root) / 'records'
    (records / 'sources').mkdir(parents=True, exist_ok=True)
    for field in listing.split('\0'):
        if field.startswith('worktree '):
            checkout = Path(field.removeprefix('worktree ')).resolve()
            directory = checkout / 'target/verification'
            reports = sorted(directory.glob('*/report.json'), key=lambda p: p.stat().st_mtime, reverse=True)
            selected = reports[:32]
            for report in selected:
                publish(report)
                for pattern in ('steps/*.json', 'nodes/*.json'):
                    for path in report.parent.glob(pattern):
                        publish(path)
            for path in sorted(directory.glob('requests/*.json'), key=lambda p: p.stat().st_mtime, reverse=True)[:32]:
                publish(path)
            source = records / 'sources' / (hashlib.sha256(str(checkout).encode()).hexdigest() + '.json')
            source.write_text(json.dumps({'repository': str(checkout), 'records': str(directory),
                'retained_runs_at_import': len(reports), 'imported_runs': len(selected),
                'older_runs_in_original_directory': max(0, len(reports) - len(selected))}))
    return records
