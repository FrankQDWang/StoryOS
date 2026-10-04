"""Idempotency, Command Challenge budgets, and writer fences from protocol sections 4 and 7."""

from copy import deepcopy
import hashlib
import time
from edits import Editor, units
from run import ROUTES, compare, wire
from structure import Scenario


def replay(http, saved, name, differences, coverage):
    method, path, body, headers, raw = saved
    status, response = http.request(method, path, body, headers)
    equal = status in [200, 202] and http.last_raw == raw
    compare(name + '/delayed immutable acknowledgement', True, equal, differences)
    coverage['delayed_replay:' + name + ':' + str(equal)] += 1
    return response


def run(http, seed, differences, coverage, selected=None):
    if selected == 'minimals':
        for index, case in enumerate(['chapter-after-session', 'session-after-edit', 'project-undo']):
            run(http, seed + index, differences, coverage, case)
        return
    e = Editor(http, seed, differences, coverage)
    if selected == 'chapter-after-session':
        replay(http, http.commands['createChapter'], 'createChapter', differences, coverage)
        return
    if selected == 'session-after-edit':
        saved = http.commands['createEditorSession']
        e.edit(units('Keep this sentence.'), 'authoritative_applied', 'Keep this sentence.')
        replay(http, saved, 'createEditorSession', differences, coverage)
        return
    if selected == 'project-undo':
        e.edit(units('Keep this sentence.'), 'authoritative_applied', 'Keep this sentence.')
        e.s.execute('updateProject', {'title': 'Renamed ' + str(seed)})
        e.refresh()
        _, result = http.command('undoLatestAuthorAction', dict(editor_session_id=e.session_id,
            expected_authoritative_revision_id=e.revision,
            expected_author_undo_frontier_sequence=e.session['author_undo_frontier_sequence']), project_id=e.s.project)
        _, chapter = http.request('GET', f'/api/v1/projects/{e.s.project}/chapters/{e.chapter}')
        compare('Project rename must not be skipped by Author Undo', e.text, chapter['chapter']['current_revision']['body'], differences)
        coverage['project_rename_undo:' + result['effect']['kind']] += 1
        return
    print(f'Seed {seed}: writer takeover and delayed replay', flush=True)
    old_creates = {name: http.commands[name] for name in ['createProject', 'createVolume', 'createChapter']}
    e.edit(units('Before takeover'), 'authoritative_applied', 'Before takeover')
    old_edit = http.commands['applyAuthorEdit']
    e.s.execute('updateProject', {'title': 'Renamed ' + str(seed)})
    for name, saved in old_creates.items():
        replay(http, saved, name, differences, coverage)
    status, second = http.command('createEditorSession', {}, project_id=e.s.project)
    compare('Secondary Editor Session', 'read_only', second['writer']['kind'], differences)
    new_id = second['editor_session']['editor_session_id']
    generation = e.session['writer']['writer_generation']
    values = dict(editor_session_id=new_id, observed_writer_generation=generation,
                  editor_contract_revision='storyos.editor-contract.release-1.v3')
    status, response = http.command('takeOverProjectWriter', values, project_id=e.s.project, editor_session_id=new_id)
    result = response['result']
    compare('Takeover kind', 'takeover_applied', result['kind'], differences)
    compare('Takeover generation increases', True, int(result['resulting_writer_generation']) > int(generation), differences)
    compare('Takeover grants requester', new_id, result['resulting_editor_session_id'], differences)
    compare('Takeover creates no Author Action', None, response['receipt']['author_action_sequence'], differences)
    coverage['takeOverProjectWriter:takeover_applied'] += 1
    old_session_id = e.session_id
    status, stale = http.command('applyAuthorEdit', e.request(units('Stale writer')), project_id=e.s.project)
    compare('Stale writer pre-Admission refusal', True, status >= 400 and 'receipt' not in stale, differences)
    coverage['writer:stale_refused'] += 1
    e.refresh()
    compare('Old writer fenced', 'read_only', e.session['writer']['kind'], differences)
    e.session_id = new_id
    e.refresh()
    compare('New writer active', 'current_writer', e.session['writer']['kind'], differences)
    e.edit(units('New ' + str(seed), 0, len(e.text)), 'authoritative_applied', 'New ' + str(seed))
    method, path, body, headers, raw = http.commands['applyAuthorEdit']
    altered_body = deepcopy(body)
    altered_body['author_edit_units'][0]['normalized_primitives'][0]['text'] = 'Changed request'
    for label, payload, sent_headers in [
        ('wrong_nonce', body, {**headers, 'X-StoryOS-Anti-Forgery': '0' * 64}),
        ('changed_body', altered_body, headers),
        ('new_key_old_nonce', body, {**headers, 'Idempotency-Key': http.identity()}),
    ]:
        refused_status, refused_result = http.request(method, path, payload, sent_headers)
        compare(label + '/no fresh authority', True, refused_status >= 400 and 'receipt' not in refused_result, differences)
        coverage['replay_binding:' + label + ':refused'] += int(refused_status >= 400 and 'receipt' not in refused_result)
    replay(http, old_edit, 'applyAuthorEdit', differences, coverage)
    _, chapter = http.request('GET', f'/api/v1/projects/{e.s.project}/chapters/{e.chapter}')
    compare('Replay does not restore old prose', e.text, chapter['chapter']['current_revision']['body'], differences)
    values['editor_session_id'] = old_session_id
    status, refused = http.command('takeOverProjectWriter', values, project_id=e.s.project, editor_session_id=old_session_id)
    compare('Stale takeover pre-Admission refusal', True, status >= 400 and 'receipt' not in refused, differences)
    coverage['takeOverProjectWriter:stale_before_admission'] += 1

    challenge_path, challenge_body, challenge = http.last_challenge
    changed = deepcopy(challenge_body)
    changed['canonical_command_digest']['value_hex_lowercase'] = '0' * 64
    status, problem = http.request('POST', challenge_path, changed)
    compare('Challenge changed digest conflicts', 409, status, differences)
    coverage['challenge:changed_digest_conflict'] += 1

    print(f'Seed {seed}: independent Challenge rate classes', flush=True)
    s = Scenario(http, seed, 'rate', differences, coverage)
    path = f'/api/v1/projects/{s.project}/anti-forgery-challenges'
    for name, capacity, label in [('createVolume', 10, 'shared'), ('applyAuthorEdit', 120, 'author_edit')]:
        route = ROUTES[name]
        window = int(time.time() // 60)
        statuses = []
        for index in range(capacity + 1):
            body = dict(method=route['method'], route_template=route['path'],
                command_schema=route['schemas']['request'], idempotency_key=http.identity(),
                canonical_command_digest=dict(algorithm='sha256', profile=f'storyos.command.{name}.jcs.v1',
                    value_hex_lowercase=hashlib.sha256(wire([seed, label, index])).hexdigest()))
            status, response = http.request('POST', path, body)
            statuses.append(status)
        if int(time.time() // 60) != window:
            coverage['challenge:' + label + ':window_boundary_observed'] += 1
        else:
            compare(label + ' inclusive capacity', [200] * capacity + [429], statuses, differences)
            coverage['challenge:' + label + ':capacity_' + str(capacity)] += int(statuses == [200] * capacity + [429])
