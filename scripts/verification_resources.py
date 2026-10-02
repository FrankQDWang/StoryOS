"""Own a disposable checkout database for one command, including abandoned leases."""

import fcntl
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import uuid


def run(root, command):
    directory = root / 'target/verification/resources'
    directory.mkdir(parents=True, exist_ok=True)
    prefix = 'storyos-test-' + hashlib.sha256(str(root).encode()).hexdigest()[:20] + '-'
    with (directory / 'registry.lock').open('w') as registry:
        fcntl.flock(registry, fcntl.LOCK_EX)
        for path in directory.glob('*.json'):
            with path.open() as lease:
                try:
                    fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    continue
                name = json.load(lease)['container']
                if not name.startswith(prefix):
                    raise ValueError('Database lease belongs to another checkout')
                subprocess.run(['docker', 'rm', '-fv', name], check=True, stdout=subprocess.DEVNULL)
                path.unlink()
        name = prefix + uuid.uuid4().hex
        path = directory / f'{name}.json'
        lease = path.open('w')
        fcntl.flock(lease, fcntl.LOCK_EX)
        json.dump({'container': name, 'repository': str(root)}, lease)
        lease.flush()
    child = None
    interrupted = 0
    def stop(signum, _frame):
        nonlocal interrupted
        interrupted = signum
        if child is not None:
            try:
                os.killpg(child.pid, signum)
            except ProcessLookupError:
                pass
    previous = {sig: signal.signal(sig, stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        environment = {**os.environ, 'repository_root': str(root)}
        child = subprocess.Popen(['sh', '-ec', '''
. "$repository_root/scripts/lib/controlled-postgres.sh"
start_postgres "$1"
STORYOS_STORAGE_ADMIN_URL=$(postgres_admin_url "$1") "$repository_root/target/release-package/storyos-storage" >/dev/null
set_runtime_password "$1"
load_controlled_fixture "$1"
''', 'database-setup', name], cwd=root, env=environment, start_new_session=True, pass_fds=(lease.fileno(),))
        code = child.wait()
        if code or interrupted:
            return 128 + interrupted if interrupted else code
        port = subprocess.check_output(['docker', 'port', name, '5432/tcp'], text=True).strip().rsplit(':', 1)[1]
        environment.update(STORYOS_TEST_POSTGRES_CONTAINER=name,
            STORYOS_TEST_DATABASE_URL=f'postgres://storyos_runtime:runtime@127.0.0.1:{port}/postgres',
            STORYOS_TEST_ADMIN_DATABASE_URL=f'postgres://postgres:admin@127.0.0.1:{port}/postgres')
        child = subprocess.Popen(command, cwd=root, env=environment, start_new_session=True, pass_fds=(lease.fileno(),))
        code = child.wait()
        return 128 + interrupted if interrupted else code
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        try:
            subprocess.run(['docker', 'rm', '-fv', name], check=True, stdout=subprocess.DEVNULL)
            path.unlink()
        finally:
            lease.close()


if __name__ == '__main__':
    if not sys.argv[1:]:
        raise SystemExit('A database command is required')
    try:
        raise SystemExit(run(Path(__file__).resolve().parents[1], sys.argv[1:]))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
