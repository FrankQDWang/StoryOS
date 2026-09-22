"""Build the local Grafana App from tracked source only."""

import hashlib
import json
from pathlib import Path
import shutil
import subprocess


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'scripts/observation/app'
OUTPUT = ROOT / 'target/observation/plugins/storyos-supervision-app'


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    code = '\n'.join((SOURCE / name).read_text() for name in ('model.js', 'views.js', 'drawer-model.js', 'drawer-view.js', 'health.js', 'app.js'))
    code = code.replace('__STYLE_DIGEST__', hashlib.sha256((SOURCE / 'style.css').read_bytes()).hexdigest()[:12])
    (OUTPUT / 'module.js').write_text("define(['react','@grafana/data'], async function(React,grafana){\n" + code +
        '\nreturn {plugin:new grafana.AppPlugin().setRootPage(App)};\n});\n')
    shutil.copyfile(SOURCE / 'style.css', OUTPUT / 'style.css')
    (OUTPUT / 'plugin.json').write_text(json.dumps({'type': 'app', 'name': 'StoryOS 仓库监督',
        'id': 'storyos-supervision-app', 'autoEnabled': True, 'info': {'description': 'Read-only local verification',
        'author': {'name': 'StoryOS'}, 'keywords': ['observation'], 'logos': {'small': '', 'large': ''},
        'version': '1.0.0', 'updated': '2026-09-22'}, 'dependencies': {'grafanaDependency': '>=12.1.1', 'plugins': []},
        'includes': [{'type': 'page', 'name': '仓库监督', 'path': '/a/storyos-supervision-app',
                      'addToNav': True, 'defaultNav': True, 'role': 'Viewer'}]}, ensure_ascii=False, indent=2)+'\n')
    subprocess.run(['node', '--check', str(OUTPUT / 'module.js')], check=True)
    print(OUTPUT)


if __name__ == '__main__':
    main()
