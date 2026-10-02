"""Publish disposable read copies of retained records for every repository worktree."""

from functools import lru_cache
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


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
