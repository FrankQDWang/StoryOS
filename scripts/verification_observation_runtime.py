"""Keep Grafana runtime data inside the checkout before starting observation."""

import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'target/observation'
COMPOSE = ['docker', 'compose', '-p', 'storyos-observation', '-f', str(ROOT / 'scripts/observation/compose.yaml')]


def main():
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


if __name__ == '__main__':
    main()
