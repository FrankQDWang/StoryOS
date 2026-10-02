"""Manage the singleton observation service only from its owner checkout."""

import argparse
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'target/observation'
COMPOSE = ['docker', 'compose', '-p', 'storyos-observation', '-f', str(ROOT / 'scripts/observation/compose.yaml')]


def prepare():
    grafana = OUTPUT / 'grafana'
    if not grafana.exists():
        container = subprocess.check_output([*COMPOSE, 'ps', '-aq', 'grafana'], text=True).strip()
        if container:
            data = json.loads(subprocess.check_output(['docker', 'inspect', container]))[0]
            source = next(item.split('=', 1)[1] for item in data['Config']['Env'] if item.startswith('GF_PATHS_DATA='))
            subprocess.run([*COMPOSE, 'stop', 'grafana'], check=True)
            staging = OUTPUT / 'grafana-import'
            staging.mkdir(parents=True, exist_ok=True)
            subprocess.run(['docker', 'cp', f'{container}:{source}/.', str(staging)], check=True)
            staging.rename(grafana)
        else:
            grafana.mkdir(parents=True)
        for path in (grafana, *grafana.rglob('*')):
            if not path.is_symlink():
                path.chmod(path.stat().st_mode | (0o777 if path.is_dir() else 0o666))
    (OUTPUT / 'health').mkdir(parents=True, exist_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', nargs='?', default='prepare', choices=('prepare', 'start', 'rebuild', 'stop', 'status'))
    action = parser.parse_args().action
    # The fixed loopback ports belong to one local owner across all checkouts.
    descriptor = os.open(f'/tmp/storyos-observation-{os.getuid()}.lock',
                         os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        containers = subprocess.check_output(['docker', 'ps', '--all', '--quiet', '--filter',
            'label=com.docker.compose.project=storyos-observation'], text=True).split()
        if containers:
            records = json.loads(subprocess.check_output(['docker', 'inspect', *containers]))
            for record in records:
                labels = record.get('Config', {}).get('Labels') or {}
                owner = labels.get('com.docker.compose.project.working_dir')
                if not owner or Path(owner).resolve() != ROOT / 'scripts/observation':
                    raise ValueError(f'Observation owner is {owner or "unknown"}; use that checkout to stop it. '
                                     'No service or retained data was changed.')
        if action in ('prepare', 'start', 'rebuild'):
            prepare()
        if action in ('start', 'rebuild'):
            subprocess.run([*COMPOSE, 'stop'], check=True)
            (ROOT / 'target/verification').mkdir(parents=True, exist_ok=True)
            (OUTPUT / 'data').mkdir(parents=True, exist_ok=True)
            import verification_records
            records = verification_records.import_worktrees(ROOT)
            os.environ['STORYOS_OBSERVATION_RECORDS'] = str(records)
            subprocess.run([sys.executable, str(ROOT / 'scripts/verification_observation.py'),
                            'rebuild' if action == 'rebuild' else 'collect',
                            '--records', str(records)], check=True)
            subprocess.run([*COMPOSE, 'up', '-d', '--build'], check=True)
        elif action == 'stop':
            subprocess.run([*COMPOSE, 'down'], check=True)
        elif action == 'status':
            subprocess.run([*COMPOSE, 'ps'], check=True)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
