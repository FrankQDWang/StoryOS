"""Mixed ownership preservation and Draft compensation, from the Refused Edit Input profile."""

from copy import deepcopy
import hashlib
from run import compare, wire, ROUTES
from edits import units


def prepare(editor):
    editor.refresh()
    first = deepcopy(editor.session['base_snapshot']['materialized_revision']['blocks'][0])
    second = dict(manuscript_block_id=editor.http.identity(), block_kind='paragraph', text='Tail.')
    unit = units('')[0]
    unit['normalized_primitives'] = [
        dict(kind='split_block', manuscript_block_id=first['manuscript_block_id'],
             new_manuscript_block_id=second['manuscript_block_id'], offset=len(first['text'])),
        dict(kind='replace_block_selection', manuscript_block_id=second['manuscript_block_id'],
             **{'from': 0, 'to': 0}, text=second['text'])]
    status, response = editor.http.command('applyAuthorEdit', editor.request([unit]), project_id=editor.s.project)
    assert status == 200 and response['effect']['kind'] == 'authoritative_applied', response
    revision = response['effect']['authoritative_revision']
    compare('Split identity and ordered replacement', [first, second], revision['blocks'], editor.differences)
    editor.actions += 1
    editor.revision = revision['revision_id']
    # The semantic model owns Blocks. The body string is an opaque display projection here.
    editor.text = revision['body']
    editor.refresh()
    return [first, second]


def run(proposal):
    p, e, http = proposal.p, proposal.editor, proposal.http
    candidate_size = len(proposal.candidate.encode('utf-16-le')) // 2
    tail = proposal.blocks[1]
    sources = [dict(owner=dict(kind='proposal', proposal_id=proposal.proposal_id, operation_id=p['operation_id'],
                    revision_id=p['revision_id'], manuscript_block_id=p['manuscript_block_id']),
                    coordinate_profile='storyos.editor.utf16-code-unit.v1', **{'from': 0, 'to': candidate_size},
                    block_kind='paragraph', source_text=proposal.candidate),
               dict(owner=dict(kind='manuscript', manuscript_block_id=tail['manuscript_block_id']),
                    coordinate_profile='prosemirror-token-utf16.v1', **{'from': 0, 'to': len(tail['text'])},
                    block_kind='paragraph', source_text=tail['text'])]
    edit_units = [dict(normalized_primitives=[dict(kind='replace_structured_selection',
        replacement=[dict(block_kind='paragraph', text='Complete mixed intent.')])],
        selection_snapshot=dict(coordinate_profile='storyos.editor.ordered-source.v1', **{'from': 0, 'to': len(tail['text'])},
            ordered_selection=dict(sources=sources, anchor=dict(source_index=0, source_offset=0),
                                   head=dict(source_index=1, source_offset=len(tail['text'])))))]
    values = e.request(edit_units)
    values.update(expected_proposal_head_revision_ids=[p['revision_id']], observed_ownership_partition='mixed')
    status, response = http.command('applyAuthorEdit', values, project_id=e.s.project)
    kind = response.get('effect', {}).get('kind', f'HTTP_{status}')
    proposal.coverage['applyAuthorEdit:' + kind] += 1
    compare('Whole mixed intent becomes Draft', 'refused_to_draft', kind, proposal.differences)
    if kind != 'refused_to_draft':
        return
    compare('Mixed intent creates no Commit', [], response['receipt']['authoritative_commit_ids'], proposal.differences)
    compare('Mixed intent creates no Action', None, response['receipt']['author_action_sequence'], proposal.differences)
    draft_id = response['effect']['draft_id']
    path = ROUTES['getRefusedEditDraft']['path'].format(project_id=e.s.project, draft_id=draft_id)
    _, query = http.request('GET', path)
    draft = query['draft']
    compare('Draft preserves every unit', edit_units, draft['payload']['author_edit_units'], proposal.differences)
    compare('Draft payload digest', hashlib.sha256(wire(draft['payload'])).hexdigest(), draft['payload_digest'], proposal.differences)
    _, chapter = http.request('GET', f'/api/v1/projects/{e.s.project}/chapters/{e.chapter}')
    compare('Mixed intent leaves Blocks unchanged', proposal.blocks, chapter['chapter']['current_revision']['blocks'], proposal.differences)
    compare('Mixed intent leaves candidate unchanged', proposal.candidate, proposal.query()['candidate_text'], proposal.differences)
    values = dict(draft_id=draft_id, draft_kind='refused_edit', source_current_draft_revision_id=draft['draft_revision_id'],
        source_draft_payload_digest=draft['payload_digest'], expected_closure='open', close_reason='abandoned',
        editor_session_id=e.session_id, writer_generation=e.session['writer']['writer_generation'])
    status, closed = http.command('closeEditorFlowDraft', values, project_id=e.s.project, draft_id=draft_id)
    assert status == 200, closed
    e.actions += 1
    compare('Draft close Action', str(e.actions), closed['receipt']['author_action_sequence'], proposal.differences)
    _, query = http.request('GET', path)
    compare('Draft closed', 'closed', query['draft']['closure'], proposal.differences)
    e.undo('draft_compensated')
    _, query = http.request('GET', path)
    compare('Draft Undo restores open closure', 'open', query['draft']['closure'], proposal.differences)
