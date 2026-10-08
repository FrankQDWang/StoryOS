"""Run gh commands and retry a transient GitHub API failure a bounded number of times."""

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ATTEMPTS = 4
TRANSIENT = re.compile(r'\bEOF\b|connection reset|HTTP 50[234]\b', re.I)


def gh(*args, cwd=None, done=None):
    """Return the output of gh. Before a retry, done() can return the result of a write that the server accepted."""
    for attempt in range(1, ATTEMPTS + 1):
        if attempt > 1 and done and (result := done()):
            return result
        run = subprocess.run(['gh', *args], cwd=cwd, capture_output=True, text=True)
        if run.returncode == 0:
            return run.stdout
        if attempt == ATTEMPTS or not TRANSIENT.search(run.stderr):
            sys.stderr.write(run.stderr)
            raise subprocess.CalledProcessError(run.returncode, run.args, run.stdout, run.stderr)
        delay = float(os.environ.get('STORYOS_GH_RETRY_SECONDS', '1')) * 2 ** (attempt - 1)
        print(f"gh {' '.join(args[:2])}: transient failure, retry {attempt} of {ATTEMPTS - 1} in {delay:g} s: "
              f'{run.stderr.strip().splitlines()[-1]}', file=sys.stderr)
        time.sleep(delay)


def comment(pr, body):
    """Post a PR comment one time. Before a retry, look for a comment with the same first line."""
    heading = Path(body).read_text().split('\n', 1)[0]

    def posted():
        found = [c['url'] for c in json.loads(gh('pr', 'view', str(pr), '--json', 'comments'))['comments']
                 if c['body'].split('\n', 1)[0] == heading]
        return found[-1] if found else None

    return gh('pr', 'comment', str(pr), '--body-file', str(body), done=posted)
