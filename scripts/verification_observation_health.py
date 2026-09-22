"""Retain independent, bounded health observations without executor authority."""

from contextlib import closing
from datetime import datetime, timezone
import json
import sqlite3
import threading
import time
import urllib.request


class Probe:
    def __init__(self, database, collector, grafana, output):
        self.database, self.collector, self.grafana, self.output = database, collector, grafana, output
        self.lock, self.ready, self.stop = threading.Lock(), threading.Event(), threading.Event()
        self.state = {}
        self.thread = threading.Thread(target=self.watch, daemon=True)

    def watch(self):
        while not self.stop.is_set():
            state = {}
            for name in ('collector', 'query', 'grafana'):
                item = {'status': 'unavailable'}
                try:
                    if name == 'collector':
                        with self.collector.open('rb') as source:
                            raw = source.read(65537)
                        if len(raw) > 65536:
                            raise ValueError('Health record exceeds the limit')
                        value = json.loads(raw)
                        if not isinstance(value, dict):
                            raise ValueError('Invalid collector health')
                        item = {key: value[key] for key in ('checked_at', 'status', 'pending', 'records',
                                'seconds', 'database_bytes', 'error') if key in value}
                        if item.get('status') not in {'ok', 'unavailable'} or 'checked_at' not in item:
                            raise ValueError('Invalid collector health')
                    elif name == 'query':
                        with closing(sqlite3.connect(self.database.as_uri() + '?mode=ro', uri=True, timeout=1)) as connection:
                            deadline = time.monotonic() + 2
                            connection.set_progress_handler(lambda: time.monotonic() > deadline, 1000)
                            if connection.execute('PRAGMA application_id').fetchone()[0] != 749:
                                raise ValueError('Not an observation database')
                            item = {'status': 'ok', 'records': connection.execute('SELECT count(*) FROM records').fetchone()[0]}
                    else:
                        with urllib.request.urlopen(self.grafana + '/api/health', timeout=2) as response:
                            raw = response.read(4097)
                        value = json.loads(raw) if len(raw) <= 4096 else None
                        if not isinstance(value, dict) or value.get('database') != 'ok':
                            raise ValueError('Grafana health is unavailable')
                        item = {'status': 'ok'}
                except (OSError, ValueError, TypeError, sqlite3.Error):
                    item = {'status': 'unavailable'}
                item.setdefault('checked_at', datetime.now(timezone.utc).isoformat())
                state[name] = item
            state['probe_checked_at'] = datetime.now(timezone.utc).isoformat()
            try:
                self.output.parent.mkdir(parents=True, exist_ok=True)
                temporary = self.output.with_suffix('.tmp')
                temporary.write_text(json.dumps(state, allow_nan=False))
                temporary.replace(self.output)
                state['storage'] = 'ok'
            except (OSError, ValueError):
                state['storage'] = 'unavailable'
            with self.lock:
                self.state = state
            self.ready.set()
            self.stop.wait(10)

    def snapshot(self):
        self.ready.wait(7)
        with self.lock:
            state = json.loads(json.dumps(self.state))
        for name in ('collector', 'query', 'grafana'):
            item = state.setdefault(name, {'status': 'unavailable'})
            try:
                stamp = datetime.fromisoformat(item['checked_at'])
                age = (datetime.now(timezone.utc) - stamp).total_seconds()
                item['age_seconds'] = age
                item['reported_status'] = item['status']
                if age < 0 or age > 30:
                    item['status'] = 'stale'
            except (KeyError, ValueError, TypeError):
                item['status'], item['age_seconds'] = 'unavailable', None
        return {**state, 'stale_after_seconds': 30,
                'coverage': 'Managed local verification only; no remote CI or arbitrary shell coverage.'}
