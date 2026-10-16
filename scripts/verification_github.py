"""Run gh and git network commands and retry a transient network failure for a bounded time.

A read can retry freely. A write retries only with a done() check that finds the result of an
earlier attempt that the server accepted, so a retry never does the write two times.
"""

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

# The total wait of one call in seconds. WAIT keeps the four attempts at 1, 2, and 4 s of a short call.
WAIT = 7
# The admission calls of review-round and verify-local, verify-tracker, the tracker query, and the git
# fetch of a review candidate wait longer, because a local proxy can fail for some tens of seconds.
ADMISSION_WAIT = 60
TRANSIENT = re.compile(r'\bEOF\b|connection reset|HTTP 50[234]\b|TLS handshake timeout|SSL_ERROR|i/o timeout'
                       r'|proxyconnect|remote end hung up unexpectedly', re.I)
GIT_RETRY = Path(__file__).with_name('git-retry.sh')


def delays(wait):
    """The pauses before each retry: 1, 2, 4 s and so on, with the last pause cut to the remaining wait."""
    result = []
    while sum(result) < wait:
        result.append(min(2 ** len(result), wait - sum(result)))
    return result


def run(command, *, cwd=None, done=None, wait=WAIT):
    """Return the completed process of a command, and retry it while its error output is transient.

    Before a retry, done() can return the output of a write that the server accepted. A permanent
    failure, or a transient failure after the total wait, writes the original error output and raises
    CalledProcessError."""
    pauses = delays(wait)
    for attempt in range(len(pauses) + 1):
        if attempt and done and (result := done()):
            return subprocess.CompletedProcess(command, 0, result, '')
        process = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
        if process.returncode == 0:
            return process
        if attempt == len(pauses) or not TRANSIENT.search(process.stderr):
            sys.stderr.write(process.stderr)
            raise subprocess.CalledProcessError(process.returncode, process.args, process.stdout, process.stderr)
        # The test scale shortens the real pause and keeps the number of attempts.
        delay = pauses[attempt] * float(os.environ.get('STORYOS_GH_RETRY_SECONDS', '1'))
        print(f"{' '.join(command[:3])}: transient failure, retry {attempt + 1} of {len(pauses)} in {delay:g} s: "
              f'{process.stderr.strip().splitlines()[-1]}', file=sys.stderr)
        time.sleep(delay)


def gh(*args, cwd=None, done=None, wait=WAIT):
    """Return the output of gh. Before a retry, done() can return the result of a write that the server accepted."""
    return run(['gh', *args], cwd=cwd, done=done, wait=wait).stdout


def comment(pr, body):
    """Post a PR comment one time. Before a retry, look for a comment with the same first line."""
    heading = Path(body).read_text().split('\n', 1)[0]

    def posted():
        found = [c['url'] for c in json.loads(gh('pr', 'view', str(pr), '--json', 'comments', wait=ADMISSION_WAIT))['comments']
                 if c['body'].split('\n', 1)[0] == heading]
        return found[-1] if found else None

    return gh('pr', 'comment', str(pr), '--body-file', str(body), done=posted, wait=ADMISSION_WAIT)


def fetch(root, *args):
    """Fetch through git-retry.sh, so the verification commands and the agents use one bounded retry."""
    return subprocess.check_output([str(GIT_RETRY), 'fetch', *args], cwd=root, text=True)


if __name__ == '__main__' and sys.argv[1:2] == ['git']:
    # git-retry.sh selects the git command. The output goes through after the last attempt.
    try:
        completed = run(['git', *sys.argv[2:]], wait=ADMISSION_WAIT)
    except subprocess.CalledProcessError as error:
        sys.stdout.write(error.stdout)
        raise SystemExit(error.returncode) from None
    sys.stdout.write(completed.stdout)
    sys.stderr.write(completed.stderr)
