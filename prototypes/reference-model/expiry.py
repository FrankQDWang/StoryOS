"""Real-clock Challenge expiry through the public boundary (protocol section 4.3)."""

from datetime import datetime
import hashlib
import time
from edits import Editor, units
from replay import replay
from run import ROUTES, compare, wire


def run(http, seed, differences, coverage):
    e = Editor(http, seed, differences, coverage)
    settled = []
    for index in range(20):
        text = f'Settled {seed}/{index}'
        e.edit(units(text, 0, len(e.text)), 'authoritative_applied', text)
        settled.append(http.commands['applyAuthorEdit'])
    route = ROUTES['applyAuthorEdit']
    path = route['path'].format(project_id=e.s.project)
    challenge_path = f'/api/v1/projects/{e.s.project}/anti-forgery-challenges'
    pending = []
    for index in range(20):
        body = dict(command_schema=route['schemas']['request'], **http.meta(), **e.request(units(f'Expired {index}')))
        key = http.identity()
        challenge_body = dict(method=route['method'], route_template=route['path'],
            command_schema=route['schemas']['request'], idempotency_key=key,
            canonical_command_digest=dict(algorithm='sha256', profile='storyos.command.applyAuthorEdit.jcs.v1',
                value_hex_lowercase=hashlib.sha256(wire(body)).hexdigest()))
        status, challenge = http.request('POST', challenge_path, challenge_body)
        assert status == 200, challenge
        pending.append((body, key, challenge))
    deadline = max(datetime.fromisoformat(c['expires_at'].replace('Z', '+00:00')).timestamp() for _, _, c in pending) + 1
    while time.time() < deadline:
        print(f'Waiting for real Challenge expiry: {int(deadline - time.time())} seconds', flush=True)
        time.sleep(min(30, max(0, deadline - time.time())))
    for (body, key, challenge), saved in zip(pending, settled):
        status, response = http.request('POST', path, body,
            {'Idempotency-Key': key, 'X-StoryOS-Anti-Forgery': challenge['nonce']})
        refused = status >= 400 and 'receipt' not in response
        compare('Expired pending Challenge has no authority', True, refused, differences)
        coverage['challenge:expired_pending_refused'] += int(refused)
        replay(http, saved, 'applyAuthorEdit_after_expiry', differences, coverage)
    e.refresh()
    compare('Expiry leaves settled prose', e.text, e.session['base_snapshot']['materialized_revision']['body'], differences)
