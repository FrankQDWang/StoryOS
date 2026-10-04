"""Proposal state axes from Manuscript State Machine sections 6, 7.4-7.6, and 8."""

import time
import random
from copy import deepcopy
from edits import Editor, units
from run import compare


class Proposal:
    def __init__(self, http, seed, differences, coverage, label):
        self.editor = Editor(http, seed, differences, coverage)
        self.http, self.differences, self.coverage = http, differences, coverage
        self.label = f'{seed}/proposal/{label}'
        self.prior_head = self.editor.revision
        initial = 'The narrator voice stays calm.' if label == 'draft_binding' else 'A quiet room before dawn.'
        self.editor.edit(units(initial), 'authoritative_applied', initial)
        if label in ['draft', 'draft_binding', 'ordered', 'bundle']:
            from mixed import prepare
            self.blocks = prepare(self.editor)
        self.body = self.editor.text
        self.start = self.editor.s.start
        status, assistance = http.request('GET', f'/api/v1/projects/{self.editor.s.project}/assistance')
        revision = assistance['assistance']['revision'] if status == 200 else '0'
        status, enabled = http.command('updateProjectAssistance',
            {'availability': 'available', 'expected_assistance_revision': revision},
            project_id=self.editor.s.project)
        assert status == 200, enabled
        target = {'kind': 'current_chapter', 'chapter_id': self.editor.chapter}
        if label in ['draft', 'draft_binding', 'ordered', 'bundle']:
            target = dict(kind='passage_collection', source_chapter_id=self.editor.chapter,
                targets=[dict(chapter_id=self.editor.chapter, base_authoritative_revision_id=self.editor.revision,
                              manuscript_block_ids=[b['manuscript_block_id'] for b in (self.blocks[:1] if label in ['draft', 'draft_binding'] else self.blocks)])])
        status, result = http.command('createAgentRun',
            dict(conversation={'kind': 'new'}, author_message={'text': 'Revise this passage: keep the narrator voice.' + {'ordered': ' in order', 'bundle': ' as a bundle'}.get(label, '')},
                 working_target=target,
                 instruction={'kind': 'absent'}, cause={'kind': 'author_request'}),
            project_id=self.editor.s.project)
        assert status == 202, result
        self.run_id = result['effect']['run_id']
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            _, run = http.request('GET', f'/api/v1/projects/{self.editor.s.project}/agent-runs/{self.run_id}')
            if run['status'] in ['completed', 'refused', 'cancelled', 'waiting']:
                break
            time.sleep(0.05)
        assert run['status'] == 'completed', run
        compare(self.label + '/fake output has no authority', False, run['decision']['authoritative'], differences)
        self.proposal_id = run['decision']['opened_proposal']['proposal_id']
        self.p = self.query()
        self.expected = {'generation': 'ready', 'validation': 'valid', 'closure': 'open', 'operation_resolution': 'pending'}
        # Generated candidate bytes are an external input; the model never copies fake implementation constants.
        self.candidate = self.p['candidate_text']
        compare(self.label + '/generated axes', self.expected, {key: self.p[key] for key in self.expected}, differences)
        _, chapter = http.request('GET', f'/api/v1/projects/{self.editor.s.project}/chapters/{self.editor.chapter}')
        compare(self.label + '/generation leaves prose', self.body, chapter['chapter']['current_revision']['body'], differences)
        coverage['createAgentRun:proposal_ready'] += 1
        self.editor.refresh()

    def query(self):
        status, response = self.http.request('GET',
            f'/api/v1/projects/{self.editor.s.project}/proposals/{self.proposal_id}')
        assert status == 200, response
        return response['proposal']

    def values(self, name):
        p, e = self.p, self.editor
        common = dict(proposal_revision_id=p['revision_id'], editor_session_id=e.session_id,
                      expected_target_revisions=[e.revision])
        if name == 'acceptProposal':
            return dict(proposal_revision_id=p['revision_id'], editor_session_id=e.session_id,
                validation_receipt_id=p['validation_receipt']['validation_receipt_id'],
                selected_operation_ids=[p['operation_id']], expected_authoritative_revision_id=e.revision)
        if name == 'rejectProposalOperations':
            common.update(selected_pending_operation_ids=[p['operation_id']],
                          rejection_reason={'kind': 'author_declined', 'note': {'kind': 'omitted'}})
        elif name == 'withdrawProposal':
            common.update(cause='author', expected_closure='open',
                          withdrawal_reason={'kind': 'author_withdrew', 'note': {'kind': 'omitted'}})
        elif name == 'reopenWithdrawnProposal':
            common.update(expected_closure='withdrawn', withdrawal_event_ref=self.withdrawal)
        elif name == 'reopenRejectedOperations':
            common.update(selected_rejected_operation_ids=[p['operation_id']], rejection_event_refs=self.rejections)
        elif name == 'replanProposal':
            return dict(conflicted_proposal_revision_id=p['revision_id'], expected_current_proposal_head=p['revision_id'],
                expected_current_target_revisions=[e.revision], replacement_operations=[p['operation_id']],
                source_condition=p['source_condition'] if p['source_condition']['kind'] != 'absent' else
                    {'kind': 'proposal_conflict', 'proposal_conflict_ref': self.http.identity()}, editor_session_id=e.session_id)
        return common

    def command(self, name, outcome, update=None, mutation=None):
        values = self.values(name)
        values.update(mutation or {})
        producer = values.get('cause') == 'current_producer'
        if producer:
            values.pop('editor_session_id')
        prior = deepcopy(self.p)
        before = len(self.differences)
        status, result = self.http.command(name, values, project_id=self.editor.s.project, proposal_id=self.proposal_id)
        effect = result.get('effect', {'kind': f'HTTP_{status}', 'reason': result.get('code')})
        key = name + ':' + effect['kind'] + (':' + effect['reason'] if effect.get('reason') else '')
        self.coverage[key] += 1
        compare(self.label + '/' + name + '/kind', outcome, effect['kind'], self.differences)
        if status == 200:
            applied = effect['kind'] in ['resolved', 'applied']
            if name == 'acceptProposal' and effect['kind'] in ['invalid', 'conflicted']:
                self.expected['validation'] = effect['kind']
            compare(self.label + '/' + name + '/Commit count', int(name == 'acceptProposal' and applied),
                    len(result['receipt']['authoritative_commit_ids']), self.differences)
            if applied:
                self.editor.actions += int(not producer)
                compare(self.label + '/' + name + '/Action', None if producer else str(self.editor.actions), effect.get('author_action_sequence'), self.differences)
                self.expected.update(update or {})
                if name == 'withdrawProposal':
                    self.withdrawal = effect['closure_event_refs'][0]
                if name == 'rejectProposalOperations':
                    self.rejections = effect['resolution_event_refs']
                if name == 'acceptProposal':
                    self.body = self.candidate
                    self.editor.text = self.body
                    self.editor.revision = effect['authoritative_revision']['revision_id']
            self.p = self.query()
            compare(self.label + '/' + name + '/axes', self.expected,
                    {key: self.p[key] for key in self.expected}, self.differences)
            compare(self.label + '/' + name + '/candidate', self.candidate, self.p['candidate_text'], self.differences)
            if applied and name in ['reopenWithdrawnProposal', 'reopenRejectedOperations', 'replanProposal']:
                compare(self.label + '/new Revision', True, prior['revision_id'] != self.p['revision_id'], self.differences)
            _, chapter = self.http.request('GET', f'/api/v1/projects/{self.editor.s.project}/chapters/{self.editor.chapter}')
            compare(self.label + '/' + name + '/prose', self.body, chapter['chapter']['current_revision']['body'], self.differences)
        for item in self.differences[before:]:
            item.update(seed_case=self.label, trace_start=self.start, trace_end=len(self.http.trace))
        return result

    def edit(self, outcome='proposal_revised', mutation=None):
        e, p = self.editor, self.p
        e.refresh()
        left = self.http.rng.randrange(len(self.candidate) + 1)
        replacement = 'quiet ' + str(self.http.rng.randrange(100))
        offset = len(self.candidate[:left].encode('utf-16-le')) // 2
        expected = self.candidate[:left] + replacement + self.candidate[left:]
        values = e.request(units(replacement, offset, offset))
        values.update(expected_proposal_head_revision_ids=[p['revision_id']], observed_ownership_partition='mixed',
            proposal_target=dict(proposal_id=self.proposal_id, operation_id=p['operation_id'],
                                 revision_id=p['revision_id'], manuscript_block_id=p['manuscript_block_id']))
        values.update(mutation or {})
        status, result = self.http.command('applyAuthorEdit', values, project_id=e.s.project)
        effect = result.get('effect', {'kind': f'HTTP_{status}'})
        self.coverage['applyAuthorEdit:' + effect['kind'] + (':' + effect['reason'] if effect.get('reason') else '')] += 1
        compare(self.label + '/candidate edit', outcome, effect['kind'], self.differences)
        if effect['kind'] == 'proposal_revised':
            e.actions += 1
            self.candidate = expected
            self.expected['validation'] = 'pending'
            self.p = self.query()
            if self.p['validation_receipt']['kind'] == 'present':
                compare(self.label + '/fresh validation identity', True,
                        self.p['validation_receipt']['validation_receipt_id'] != p['validation_receipt'].get('validation_receipt_id'), self.differences)
                # A-006: HTTP cannot observe the interval before a separate Core validation.
                self.expected['validation'] = 'valid'
            compare(self.label + '/edited candidate', expected, self.p['candidate_text'], self.differences)
            compare(self.label + '/edited axes', self.expected, {key: self.p[key] for key in self.expected}, self.differences)
            compare(self.label + '/candidate edit Commit', [], result['receipt']['authoritative_commit_ids'], self.differences)
        return result


def run(http, seed, differences, coverage, selected=None):
    modes = [selected] if selected else ['accept', 'withdraw', 'reject', 'edit', 'replan', 'invalid', 'stale', 'reopen_no_effect', 'conflict_reject', 'conflict_withdraw', 'refuse_replan', 'draft', 'edit_conflicts', 'producer', 'closed', 'reversal', 'duplicates', 'ordered', 'bundle']
    http.rng.shuffle(modes)
    for mode in modes:
        http.rng = random.Random(f'{seed}/proposal/{mode}')
        print(f'Seed {seed}: Proposal {mode}', flush=True)
        p = Proposal(http, seed, differences, coverage, mode)
        if mode == 'accept':
            p.command('acceptProposal', 'applied', {'operation_resolution': 'applied'})
        elif mode == 'withdraw':
            p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'})
            p.command('withdrawProposal', 'no_effect')
            p.command('reopenWithdrawnProposal', 'resolved', {'closure': 'open', 'validation': 'pending'})
            p.command('reopenWithdrawnProposal', 'no_effect')
        elif mode == 'reject':
            p.command('rejectProposalOperations', 'resolved', {'operation_resolution': 'rejected'})
            p.command('acceptProposal', 'refused')
            p.command('rejectProposalOperations', 'refused')
            p.command('reopenRejectedOperations', 'refused', mutation={'rejection_event_refs': [http.identity()]})
            p.command('reopenRejectedOperations', 'resolved', {'operation_resolution': 'pending', 'validation': 'pending'})
        elif mode == 'edit':
            p.edit()
        elif mode in ['replan', 'replan-undo']:
            p.command('acceptProposal', 'conflicted', mutation={'expected_authoritative_revision_id': p.prior_head})
            p.command('replanProposal', 'conflicted', mutation={'expected_current_target_revisions': [p.prior_head]})
            p.command('replanProposal', 'refused', mutation={'source_condition': {'kind': 'proposal_conflict', 'proposal_conflict_ref': http.identity()}})
            p.command('replanProposal', 'resolved', {'validation': 'pending'})
            if mode == 'replan-undo':
                e = p.editor
                e.refresh()
                status, response = http.command('undoLatestAuthorAction', dict(editor_session_id=e.session_id,
                    expected_authoritative_revision_id=e.revision,
                    expected_author_undo_frontier_sequence=e.session['author_undo_frontier_sequence']), project_id=e.s.project)
                compare('Replan Undo leaves authority', [], response['receipt']['authoritative_commit_ids'], differences)
                e.refresh()
                compare('Replan Undo leaves prose', e.text, e.session['base_snapshot']['materialized_revision']['body'], differences)
        elif mode == 'invalid':
            p.command('acceptProposal', 'invalid', mutation={'validation_receipt_id': http.identity()})
            p.command('acceptProposal', 'refused')
        elif mode == 'stale':
            p.withdrawal, p.rejections = http.identity(), [http.identity()]
            for name in ['acceptProposal', 'rejectProposalOperations', 'withdrawProposal', 'reopenWithdrawnProposal', 'reopenRejectedOperations']:
                p.command(name, 'refused', mutation={'proposal_revision_id': http.identity()})
        elif mode == 'reopen_no_effect':
            p.withdrawal = http.identity()
            p.command('reopenWithdrawnProposal', 'no_effect')
            p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'})
            p.command('reopenWithdrawnProposal', 'no_effect', mutation={'withdrawal_event_ref': http.identity()})
        elif mode == 'conflict_reject':
            p.command('rejectProposalOperations', 'conflicted', mutation={'expected_target_revisions': [p.prior_head]})
            p.command('rejectProposalOperations', 'resolved', {'operation_resolution': 'rejected'})
            p.command('reopenRejectedOperations', 'conflicted', mutation={'expected_target_revisions': [p.prior_head]})
        elif mode == 'conflict_withdraw':
            p.command('withdrawProposal', 'conflicted', mutation={'expected_target_revisions': [p.prior_head]})
            p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'})
            p.command('reopenWithdrawnProposal', 'conflicted', mutation={'expected_target_revisions': [p.prior_head]})
        elif mode == 'refuse_replan':
            p.command('replanProposal', 'refused')
            p.command('replanProposal', 'refused', mutation={'expected_current_proposal_head': http.identity()})
        elif mode == 'draft':
            from mixed import run as run_mixed
            run_mixed(p)
        elif mode == 'edit_conflicts':
            p.editor.edit(units('stale ownership'), 'conflicted')
            p.edit('refused', {'proposal_target': dict(proposal_id=p.proposal_id, operation_id=http.identity(),
                revision_id=p.p['revision_id'], manuscript_block_id=p.p['manuscript_block_id'])})
        elif mode == 'producer':
            cause = dict(cause='current_producer', withdrawal_reason={'kind': 'current_producer_withdrew'},
                producer=dict(kind='agent_run_decision', run_id=http.identity(), decision_id=http.identity()))
            p.command('withdrawProposal', 'no_effect', mutation=cause)
            cause['producer'] = dict(kind='agent_run_decision', run_id=p.run_id, decision_id=p.p['source']['decision_id'])
            p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'}, cause)
        elif mode == 'closed':
            p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'})
            p.command('rejectProposalOperations', 'refused')
            p.rejections = [http.identity()]
            p.command('reopenRejectedOperations', 'refused')
            p.command('replanProposal', 'refused')

        elif mode == 'reversal':
            prior = p.editor.text
            p.command('acceptProposal', 'applied', {'operation_resolution': 'applied'})
            e = p.editor
            e.history.append((prior, e.actions))
            e.refresh()
            e.edit(units('Later '), 'authoritative_applied', 'Later ' + e.text)
            e.undo()
            before = e.text
            response = e.undo('reversal_required')
            compare(p.label + '/reversal has no Commit', [], response['receipt']['authoritative_commit_ids'], differences)
            e.refresh()
            compare(p.label + '/reversal leaves authority', before, e.session['base_snapshot']['materialized_revision']['body'], differences)
            e.undo('unavailable')

        elif mode == 'duplicates':
            p.command('acceptProposal', 'refused', mutation={'selected_operation_ids': [p.p['operation_id']] * 2})
            p.command('rejectProposalOperations', 'refused', mutation={'selected_pending_operation_ids': [p.p['operation_id']] * 2})
            p.rejections = [http.identity()]
            p.command('reopenRejectedOperations', 'refused')

        elif mode in ['ordered', 'bundle']:
            operations = p.p['operations']
            assert len(operations) == 2
            target = p.blocks[1 if mode == 'ordered' else 0]['manuscript_block_id']
            selected = next(op['operation_id'] for op in operations if op['manuscript_block_id'] == target)
            p.command('acceptProposal', 'refused', mutation={'selected_operation_ids': [selected]})
            p.command('rejectProposalOperations', 'refused', mutation={'selected_pending_operation_ids': [selected]})

        elif mode == 'draft_binding':
            from draft_binding import run as binding
            binding(p)
        elif mode in ['proposal-undo', 'rejection-undo', 'withdrawal-undo']:
            original = p.candidate
            if mode == 'proposal-undo':
                p.edit()
            elif mode == 'rejection-undo':
                p.command('rejectProposalOperations', 'resolved', {'operation_resolution': 'rejected'})
            else:
                p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'})
            e = p.editor
            e.refresh()
            _, response = http.command('undoLatestAuthorAction', dict(editor_session_id=e.session_id,
                expected_authoritative_revision_id=e.revision,
                expected_author_undo_frontier_sequence=e.session['author_undo_frontier_sequence']), project_id=e.s.project)
            if mode == 'rejection-undo':
                effect = response['effect']
                allowed = effect['kind'] == 'compensated' or effect == {'kind': 'unavailable', 'reason': 'barrier'}
                compare('A-010: rejection handler or explicit Barrier', True, allowed, differences)
                expected_resolution = 'pending' if effect['kind'] == 'compensated' else 'rejected'
                compare('Rejection Undo result matches operation state', expected_resolution, p.query()['operation_resolution'], differences)
                coverage['undoLatestAuthorAction:' + effect['kind'] + (':' + effect['reason'] if effect.get('reason') else '')] += 1
                continue
            compare('Proposal Undo restores candidate', original, p.query()['candidate_text'], differences)
            compare('Proposal Undo has no authoritative Commit', [], response['receipt']['authoritative_commit_ids'], differences)
            revision = response['effect']['authoritative_revision']
            before = len(differences)
            compare('D-004: Undo response preserves immutable authoritative payload',
                {'revision_id': e.revision, 'body': e.text},
                {key: revision[key] for key in ['revision_id', 'body']}, differences)
            for entry in differences[before:]:
                entry.update(seed_case=p.label, trace_start=p.start, trace_end=len(http.trace))
            e.refresh()
            compare('Proposal Undo leaves canonical prose', e.text, e.session['base_snapshot']['materialized_revision']['body'], differences)
            coverage['undoLatestAuthorAction:compensated'] += 1
