"""Author Edit and Undo expectations from Manuscript State Machine sections 7.2 and 10."""

from copy import deepcopy
from run import compare
from structure import Scenario


def units(text, start=0, end=0):
    return [dict(normalized_primitives=[dict(kind='replace_selection', **{'from': start, 'to': end}, text=text)],
                 selection_snapshot=dict(coordinate_profile='storyos.editor.utf16-code-unit.v1', **{'from': start, 'to': end}))]


class Editor:
    def __init__(self, http, seed, differences, coverage):
        self.s = Scenario(http, seed, 'author-edit', differences, coverage)
        self.http, self.differences, self.coverage = http, differences, coverage
        volume = self.s.volume()
        self.chapter = self.s.chapter(volume)
        status, self.session = http.command('createEditorSession', {}, project_id=self.s.project)
        assert status == 200, self.session
        self.session_id = self.session['editor_session']['editor_session_id']
        self.text = self.session['base_snapshot']['materialized_revision']['body']
        self.revision = self.session['base_snapshot']['authoritative_head_revision_id']
        self.actions, self.history, self.local = self.s.model.actions, [], 0

    def refresh(self):
        status, self.session = self.http.request('GET',
            f'/api/v1/projects/{self.s.project}/editor-sessions/{self.session_id}')
        assert status == 200, self.session
        return self.session

    def request(self, edit_units, **overrides):
        self.local += 1
        base = self.session['base_snapshot']
        return dict(editor_session_id=self.session_id,
                    writer_generation=self.session['writer']['writer_generation'],
                    chapter_id=self.chapter, expected_authoritative_revision_id=self.revision,
                    expected_proposal_head_revision_ids=[], target_refs=base['target_refs'],
                    observed_ownership_partition='authoritative',
                    editor_contract_revision='storyos.editor-contract.release-1.v3',
                    undo_group_id=self.http.identity(), completed_intent_record_id=self.http.identity(),
                    local_intent_sequence=str(self.local), author_edit_units=edit_units, **overrides)

    def edit(self, edit_units, outcome, expected_text=None, mutation=None):
        values = self.request(edit_units)
        values.update(mutation or {})
        start = len(self.differences)
        status, response = self.http.command('applyAuthorEdit', values, project_id=self.s.project)
        effect = response.get('effect', {'kind': f'HTTP_{status}', 'reason': response.get('code')})
        self.coverage['applyAuthorEdit:' + effect['kind'] + (':' + effect['reason'] if effect.get('reason') else '')] += 1
        compare(self.s.label + '/edit kind', outcome, effect['kind'], self.differences)
        if status == 200:
            receipt = response['receipt']
            applied = outcome == 'authoritative_applied'
            compare(self.s.label + '/edit Commit', int(applied), len(receipt['authoritative_commit_ids']), self.differences)
            if applied and effect['kind'] == outcome:
                self.history.append((self.text, self.actions + 1))
                self.actions += 1
                self.text = expected_text
                self.revision = effect['authoritative_revision']['revision_id']
                compare(self.s.label + '/edit response body', self.text, effect['authoritative_revision']['body'], self.differences)
            compare(self.s.label + '/edit action', str(self.actions) if applied else None, receipt['author_action_sequence'], self.differences)
            status, chapter = self.http.request('GET', f'/api/v1/projects/{self.s.project}/chapters/{self.chapter}')
            compare(self.s.label + '/edit read', self.text, chapter['chapter']['current_revision']['body'], self.differences)
            self.refresh()
            compare(self.s.label + '/editor base', self.text, self.session['base_snapshot']['materialized_revision']['body'], self.differences)
        for entry in self.differences[start:]:
            entry.update(seed_case=self.s.label, trace_start=self.s.start, trace_end=len(self.http.trace))
        return response

    def undo(self, outcome='compensated', mutation=None):
        self.refresh()
        values = dict(editor_session_id=self.session_id,
            expected_authoritative_revision_id=self.revision,
            expected_author_undo_frontier_sequence=self.session['author_undo_frontier_sequence'])
        values.update(mutation or {})
        start = len(self.differences)
        status, response = self.http.command('undoLatestAuthorAction', values, project_id=self.s.project)
        effect = response.get('effect', {'kind': f'HTTP_{status}', 'reason': response.get('code')})
        self.coverage['undoLatestAuthorAction:' + effect['kind'] + (':' + effect['reason'] if effect.get('reason') else '')] += 1
        allowed = outcome if isinstance(outcome, tuple) else (outcome,)
        compare(self.s.label + '/undo kind allowed', True, effect['kind'] in allowed, self.differences)
        if effect['kind'] == 'compensated':
            prior, source_sequence = self.history.pop()
            compare(self.s.label + '/undo source', str(source_sequence), effect['source_sequence'], self.differences)
            self.actions += 1
            compare(self.s.label + '/undo action', str(self.actions), effect['author_action_sequence'], self.differences)
            compare(self.s.label + '/undo text', prior, effect['authoritative_revision']['body'], self.differences)
            self.text, self.revision = prior, effect['authoritative_revision']['revision_id']
            self.refresh()
            compare(self.s.label + '/undo base', prior, self.session['base_snapshot']['materialized_revision']['body'], self.differences)
            if self.history:
                compare(self.s.label + '/newest-first frontier', str(self.history[-1][1]),
                        self.session['author_undo_frontier_sequence'], self.differences)
        for entry in self.differences[start:]:
            entry.update(seed_case=self.s.label, trace_start=self.s.start, trace_end=len(self.http.trace))
        return response


def run(http, seed, differences, coverage, selected=None):
    editor = Editor(http, seed, differences, coverage)
    print(f'Seed {seed}: Author Edit and Undo', flush=True)
    editor.edit(units(''), 'no_effect')
    editor.edit(units('bad', 0, 99999), 'refused')
    empty = units('')[0]
    empty['normalized_primitives'] = []
    editor.edit([empty], 'refused')
    editor.edit(units('target'), 'HTTP_422', mutation={'target_refs': ['manuscript:' + http.identity()]})
    original_revision = editor.revision
    for index in range(4):
        text = editor.text
        left = http.rng.randrange(len(text) + 1)
        right = http.rng.randrange(left, len(text) + 1)
        replacement = http.rng.choice(['Dawn', '夜色', '🌙', 'A quiet room']) + str(http.rng.randrange(100))
        start = len(text[:left].encode('utf-16-le')) // 2
        end = len(text[:right].encode('utf-16-le')) // 2
        expected = text[:left] + replacement + text[right:]
        editor.edit(units(replacement, start, end), 'authoritative_applied', expected)
        if index == 0:
            editor.edit(units('stale'), 'conflicted', mutation={'expected_authoritative_revision_id': original_revision})
    editor.undo('conflicted', {'expected_author_undo_frontier_sequence': '999999'})
    editor.undo('conflicted', {'expected_authoritative_revision_id': http.identity()})
    editor.undo()
    # A-005: the contract does not define the next direct-edit handler's eligibility after compensation.
    editor.undo(('compensated', 'conflicted'))
    fixture = '018f0000-0000-7001-8000-000000000002'
    if not hasattr(http, 'fixture_session'):
        status, http.fixture_session = http.command('createEditorSession', {}, project_id=fixture)
        assert status == 200, http.fixture_session
    session = http.fixture_session
    assert session.get('author_undo_frontier_sequence') is None
    status, result = http.command('undoLatestAuthorAction', dict(
        editor_session_id=session['editor_session']['editor_session_id'], expected_author_undo_frontier_sequence='1',
        expected_authoritative_revision_id=session['base_snapshot']['authoritative_head_revision_id']), project_id=fixture)
    compare('No frontier Undo', {'kind': 'unavailable', 'reason': 'no_frontier'}, result['effect'], differences)
    coverage['undoLatestAuthorAction:unavailable:no_frontier'] += 1
