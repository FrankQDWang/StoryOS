"""Source-Draft binding drift after a Proposal compensation, from section 10.1."""

from copy import deepcopy
import time
from run import compare, ROUTES


def run(p):
    from mixed import run as preserve
    e, http = p.editor, p.http
    draft, draft_path = preserve(p, finish=False)
    p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'})
    status, admitted = http.command('createAgentRun', dict(conversation={'kind': 'new'},
        author_message={'text': 'Revise this phrase: narrator voice'},
        working_target={'kind': 'current_chapter', 'chapter_id': e.chapter},
        instruction={'kind': 'absent'}, cause={'kind': 'author_request'}), project_id=e.s.project)
    assert status == 202, admitted
    run_id = admitted['effect']['run_id']
    deadline = time.monotonic() + 40
    while time.monotonic() < deadline:
        _, result = http.request('GET', f'/api/v1/projects/{e.s.project}/agent-runs/{run_id}')
        if result['status'] == 'completed':
            break
        time.sleep(0.05)
    p.proposal_id = result['decision']['opened_proposal']['proposal_id']
    p.p = p.query()
    p.candidate = p.p['candidate_text']
    p.expected = dict(generation='ready', validation='valid', closure='open', operation_resolution='pending')
    anchors = deepcopy(p.p['anchors'])
    assert anchors
    # Retained public Anchors are opaque source proof, not model-computed expected state.
    p.command('withdrawProposal', 'resolved', {'closure': 'withdrawn'})
    e.refresh()
    values = dict(draft_id=draft['draft_id'], source_current_draft_revision_id=draft['draft_revision_id'],
        source_draft_payload_digest=draft['payload_digest'], expected_source_draft_closure='open',
        selected_payload_range={'kind': 'whole_draft_payload'}, proposal_kind='inline_edit',
        chapter_id=e.chapter, target_refs=[anchors[0]['manuscript_block_id']],
        expected_target_revisions=[e.revision], anchors=anchors,
        editor_session_id=e.session_id, writer_generation=e.session['writer']['writer_generation'])
    status, response = http.command('expandRefusedEditDraftToProposal', values,
        project_id=e.s.project, draft_id=draft['draft_id'])
    assert status == 200 and response['effect']['kind'] == 'proposal_created_from_draft', response
    e.actions += 1
    compare('Draft expansion action', str(e.actions), response['receipt']['author_action_sequence'], p.differences)
    compare('Draft expansion no authority', [], response['receipt']['authoritative_commit_ids'], p.differences)
    p.coverage['expandRefusedEditDraftToProposal:proposal_created_from_draft'] += 1
    p.proposal_id = response['effect']['proposal_id']
    p.p = p.query()
    p.candidate = 'Complete mixed intent.'
    compare('Expansion preserves complete replacement', p.candidate, p.p['candidate_text'], p.differences)
    original = p.candidate
    p.expected = dict(generation='ready', validation='pending', closure='open', operation_resolution='pending')
    p.edit()
    e.refresh()
    _, response = http.command('undoLatestAuthorAction', dict(editor_session_id=e.session_id,
        expected_authoritative_revision_id=e.revision,
        expected_author_undo_frontier_sequence=e.session['author_undo_frontier_sequence']), project_id=e.s.project)
    compare('Proposal edit compensation', 'compensated', response['effect']['kind'], p.differences)
    compare('Proposal compensation no Commit', [], response['receipt']['authoritative_commit_ids'], p.differences)
    compare('Proposal compensation restores candidate', original, p.query()['candidate_text'], p.differences)
    p.coverage['undoLatestAuthorAction:compensated'] += 1
    e.actions += 1
    response = e.undo('conflicted')
    compare('Expansion Undo exact source binding', 'source_binding_changed', response['effect'].get('reason'), p.differences)
    _, query = http.request('GET', draft_path)
    compare('Conflicted Undo does not partly reopen source', 'closed', query['draft']['closure'], p.differences)
    e.refresh()
    compare('Binding drift leaves prose', e.text, e.session['base_snapshot']['materialized_revision']['body'], p.differences)
