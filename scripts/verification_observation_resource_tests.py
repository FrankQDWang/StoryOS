"""Exercise checkout resource ownership through public commands."""

import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import unittest


SOURCE = Path(__file__).resolve().parent.parent


class ResourceCommandTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        tools = self.directory / 'tools'
        tools.mkdir()
        self.state = self.directory / 'docker.json'
        self.state.write_text(json.dumps({'storyos-dev-postgres': {'port': 5999, 'writes': 0}}))
        docker = tools / 'docker'
        docker.write_text(f'#!{sys.executable}\n' + '''import json, os, pathlib, sys
path = pathlib.Path(os.environ['RESOURCE_DOCKER_STATE'])
state = json.loads(path.read_text())
args = sys.argv[1:]
if args[0] == 'run':
    name = args[args.index('--name') + 1]
    state[name] = {'port': 6000 + len(state), 'writes': 0}
elif args[0] == 'rm':
    state.pop(args[-1], None)
elif args[0] == 'port':
    print('127.0.0.1:' + str(state[args[1]]['port']))
elif args[0] == 'logs':
    print('PostgreSQL init process complete')
elif args[0] == 'exec':
    name = args[2] if args[1] == '-i' else args[1]
    if 'psql' in args:
        state[name]['writes'] += 1
path.write_text(json.dumps(state))
''')
        docker.chmod(0o755)
        self.environment = {**os.environ, 'PATH': f'{tools}{os.pathsep}{os.environ["PATH"]}',
                            'STORYOS_VERIFICATION_RUN': str(self.directory / 'run'),
                            'RESOURCE_DOCKER_STATE': str(self.state), 'PYTHONDONTWRITEBYTECODE': '1'}

    def checkout(self, name):
        root = self.directory / name
        (root / 'scripts/lib').mkdir(parents=True)
        shutil.copy(SOURCE / 'scripts/dev-postgres.sh', root / 'scripts')
        shutil.copy(SOURCE / 'scripts/lib/controlled-postgres.sh', root / 'scripts/lib')
        fixture = root / 'crates/storyos-adapter-postgres/tests/fixture.sql'
        fixture.parent.mkdir(parents=True)
        fixture.write_text('SELECT 1;\n')
        storage = root / 'target/release-package/storyos-storage'
        storage.parent.mkdir(parents=True)
        storage.write_text('#!/bin/sh\nexit 0\n')
        storage.chmod(0o755)
        return root

    def command(self, root, *args):
        return subprocess.run(args, cwd=root, env=self.environment, text=True, capture_output=True, timeout=20)

    def test_development_database_lifecycle_preserves_other_checkouts_and_legacy_data(self):
        first, second = self.checkout('first'), self.checkout('second')
        started = self.command(first, 'sh', 'scripts/dev-postgres.sh', 'up', '--interactive')
        self.assertEqual(started.returncode, 0, started.stderr)
        retained = json.loads(self.state.read_text())
        for action in ('up', 'reload', 'down'):
            result = self.command(second, 'sh', 'scripts/dev-postgres.sh', action,
                                  *(['--interactive'] if action == 'up' else []))
            self.assertEqual(result.returncode, 0, result.stderr)
            current = json.loads(self.state.read_text())
            self.assertEqual({name: current.get(name) for name in retained}, retained)
            environment = self.command(first, 'sh', 'scripts/dev-postgres.sh', 'env')
            self.assertEqual((environment.returncode, environment.stdout), (0, started.stdout))
        alias = self.directory / 'alias'
        alias.symlink_to(first, target_is_directory=True)
        self.assertEqual(self.command(alias, 'sh', 'scripts/dev-postgres.sh', 'env').stdout, started.stdout)
        self.assertEqual(json.loads(self.state.read_text())['storyos-dev-postgres'], {'port': 5999, 'writes': 0})

    def test_unowned_database_start_requires_explicit_interactive_mode(self):
        root = self.checkout('unowned')
        before = self.state.read_bytes()
        result = self.command(root, 'sh', 'scripts/dev-postgres.sh', 'up')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('run command', result.stderr)
        self.assertEqual(self.state.read_bytes(), before)

    def test_scoped_database_removes_only_its_resources_on_success_and_failure(self):
        root = self.checkout('scoped')
        shutil.copy(SOURCE / 'scripts/verification_resources.py', root / 'scripts')
        for code in (0, 7):
            result = self.command(root, 'sh', 'scripts/dev-postgres.sh', 'run',
                                  sys.executable, '-c',
                                  f'import os; assert os.environ["STORYOS_TEST_POSTGRES_CONTAINER"]; exit({code})')
            self.assertEqual(result.returncode, code, result.stderr)
            self.assertEqual(json.loads(self.state.read_text()),
                             {'storyos-dev-postgres': {'port': 5999, 'writes': 0}})
            self.assertEqual(list((root / 'target/verification/resources').glob('*.json')), [])

    def test_scoped_database_cleans_up_after_interruption(self):
        root = self.checkout('interrupted')
        shutil.copy(SOURCE / 'scripts/verification_resources.py', root / 'scripts')
        with socket.socket() as barrier:
            barrier.bind(('127.0.0.1', 0))
            barrier.listen(1)
            barrier.settimeout(10)
            child = ('import socket; s=socket.create_connection(("127.0.0.1", '
                     f'{barrier.getsockname()[1]})); s.recv(1)')
            process = subprocess.Popen(['sh', 'scripts/dev-postgres.sh', 'run', sys.executable, '-c', child],
                                       cwd=root, env=self.environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                connection, _ = barrier.accept()
                with connection:
                    process.terminate()
                    process.communicate(timeout=10)
                self.assertEqual(process.returncode, 143)
                self.assertEqual(list((root / 'target/verification/resources').glob('*.json')), [])
                self.assertEqual(json.loads(self.state.read_text()),
                                 {'storyos-dev-postgres': {'port': 5999, 'writes': 0}})
            finally:
                if process.poll() is None:
                    process.kill()
                process.communicate(timeout=10)

    def test_scoped_database_reclaims_an_abandoned_lease(self):
        import hashlib
        root = self.checkout('abandoned')
        shutil.copy(SOURCE / 'scripts/verification_resources.py', root / 'scripts')
        name = 'storyos-test-' + hashlib.sha256(str(root.resolve()).encode()).hexdigest()[:20] + '-dead'
        state = json.loads(self.state.read_text())
        state[name] = {'port': 6001, 'writes': 0}
        self.state.write_text(json.dumps(state))
        leases = root / 'target/verification/resources'
        leases.mkdir(parents=True)
        (leases / 'dead.json').write_text(json.dumps({'container': name}))
        result = self.command(root, 'sh', 'scripts/dev-postgres.sh', 'run', 'true')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(list(leases.glob('*.json')), [])
        self.assertEqual(json.loads(self.state.read_text()),
                         {'storyos-dev-postgres': {'port': 5999, 'writes': 0}})

    def test_observation_lifecycle_refuses_foreign_and_unknown_owners_before_mutation(self):
        first, second = self.checkout('owner'), self.checkout('worker')
        for root in (first, second):
            shutil.copy(SOURCE / 'Makefile', root)
            shutil.copytree(SOURCE / 'scripts/observation', root / 'scripts/observation')
            for name in ('runtime', 'app', 'dashboard', 'compare'):
                shutil.copy(SOURCE / f'scripts/verification_observation_{name}.py', root / 'scripts')
        docker = self.directory / 'tools/docker'
        docker.write_text(f'#!{sys.executable}\n' + '''import json, os, pathlib, sys
path = pathlib.Path(os.environ['RESOURCE_DOCKER_STATE'])
state = json.loads(path.read_text())
args = sys.argv[1:]
if args[0] == 'inspect':
    print(json.dumps(state['containers']))
elif args[0] == 'ps' or ('compose' in args and 'ps' in args):
    print('grafana')
else:
    state['mutations'].append(args)
    path.write_text(json.dumps(state))
''')
        for labels in ({'com.docker.compose.project.working_dir': str(first / 'scripts/observation')}, {}):
            expected = {'containers': [{'Config': {'Labels': labels,
                        'Env': ['GF_PATHS_DATA=/var/lib/grafana']}}], 'mutations': []}
            self.state.write_text(json.dumps(expected))
            for command in (['make', 'observe-start'], ['make', 'observe-stop'],
                            [sys.executable, 'scripts/verification_observation_runtime.py']):
                with self.subTest(labels=labels, command=command):
                    result = self.command(second, *command)
                    self.assertNotEqual(result.returncode, 0, result.stdout)
                    self.assertIn('observation owner', result.stderr.lower())
                    self.assertEqual(json.loads(self.state.read_text()), expected)
                    self.assertFalse((second / 'target/observation/grafana-import').exists())
                    self.assertFalse((second / 'target/observation/grafana').exists())
        self.state.write_text(json.dumps({'containers': [{'Config': {'Labels': {
            'com.docker.compose.project.working_dir': str(first / 'scripts/observation')}}}], 'mutations': []}))
        result = self.command(first, 'make', 'observe-stop')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([args[-1] for args in json.loads(self.state.read_text())['mutations']], ['down'])
        (first / 'target/observation/grafana').mkdir(parents=True)
        (first / 'scripts/verification_records.py').write_text('def import_worktrees(root): return root\n')
        (first / 'scripts/verification_observation.py').write_text(
            "import json, os\nfrom pathlib import Path\n"
            "state=json.loads(Path(os.environ['RESOURCE_DOCKER_STATE']).read_text())\n"
            "assert state['mutations'][-1][-1] == 'stop', 'Import requires stopped services'\n")
        for action in ('start', 'rebuild'):
            result = self.command(first, sys.executable, 'scripts/verification_observation_runtime.py', action)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_overlapping_smoke_commands_keep_separate_projects_through_cleanup(self):
        with socket.socket() as barrier:
            barrier.bind(('127.0.0.1', 0))
            barrier.listen(2)
            barrier.settimeout(30)
            self.state.write_text(json.dumps({'live': [], 'commands': []}))
            docker = self.directory / 'tools/docker'
            docker.write_text(f'#!{sys.executable}\n' + '''import fcntl, json, os, pathlib, socket, sys
args = sys.argv[1:]
project = args[args.index('-p') + 1]
action = next(value for value in ('up', 'port', 'down') if value in args)
path = pathlib.Path(os.environ['RESOURCE_DOCKER_STATE'])
with path.open('r+') as output:
    fcntl.flock(output, fcntl.LOCK_EX)
    state = json.load(output)
    state['commands'].append([project, action])
    if action == 'up':
        state['live'].append(project)
    elif action == 'down':
        state['live'] = [name for name in state['live'] if name != project]
    output.seek(0)
    json.dump(state, output)
    output.truncate()
if action == 'port':
    with socket.socket() as connection:
        connection.settimeout(30)
        connection.connect(('127.0.0.1', int(os.environ['RESOURCE_BARRIER'])))
        connection.sendall(json.dumps([project, os.getppid()]).encode())
        connection.shutdown(socket.SHUT_WR)
        connection.recv(1)
    raise SystemExit(7)
''')
            environment = {**self.environment, 'RESOURCE_BARRIER': str(barrier.getsockname()[1])}
            processes = [subprocess.Popen([sys.executable, 'scripts/verification_observation_smoke.py'],
                         cwd=SOURCE, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
                         for _ in range(2)]
            connections = []
            try:
                projects, owners = [], []
                for _ in processes:
                    connection, _ = barrier.accept()
                    connection.settimeout(30)
                    connections.append(connection)
                    with connection.makefile('rb') as request:
                        project, owner = json.load(request)
                    projects.append(project)
                    owners.append(owner)
                connections[0].sendall(b'x')
                first = next(process for process in processes if process.pid == owners[0])
                first.communicate(timeout=30)
                self.assertEqual(json.loads(self.state.read_text())['live'], [projects[1]])
                connections[1].sendall(b'x')
                results = [process.communicate(timeout=30) for process in processes]
                self.assertEqual([process.returncode for process in processes], [1, 1], results)
                self.assertEqual(len(set(projects)), 2, projects)
                state = json.loads(self.state.read_text())
                self.assertEqual(state['live'], [])
                self.assertEqual({project: [action for name, action in state['commands'] if name == project]
                                  for project in projects}, {project: ['up', 'port', 'down'] for project in projects})
            finally:
                for connection in connections:
                    connection.close()
                for process in processes:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.communicate(timeout=10)


if __name__ == '__main__':
    unittest.main()
