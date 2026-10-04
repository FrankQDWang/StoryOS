"""Structure contracts: ADRs 0018 and 0025-0030, route catalog, generated effects."""

from copy import deepcopy
from run import Structure, compare


class Scenario:
    def __init__(self, http, seed, case, differences, coverage):
        self.http, self.coverage, self.differences = http, coverage, differences
        self.label = f'{seed}/{case}'
        self.model = Structure()
        self.title = f'Reference {self.label}'
        self.archived = False
        self.project_revision = None
        self.current = None
        self.start = len(http.trace)
        status, response = http.command('createProject', {'title': self.title})
        assert status == 200, response
        self.project = response['project']['project_id']
        coverage['createProject:created'] += 1
        status, tree = http.request('GET', f'/api/v1/projects/{self.project}/manuscript/tree')
        assert status == 200, tree
        compare(self.label + '/empty', [], tree['volumes'], differences)
        self.model.tree_revision = int(tree['tree_revision'])
        self.snapshot = tree['snapshot']
        self.last = response

    def execute(self, name, values, outcome='authoritative_applied', reason=None, **targets):
        model = self.model
        if name in ['updateVolume', 'updateChapter']:
            kind = 'volume' if name == 'updateVolume' else 'chapter'
            nodes = model.volumes if kind == 'volume' else [c for v in model.volumes for c in v['chapters']]
            node = next((n for n in nodes if n[kind + '_id'] == targets[kind + '_id']), {'title': 'Missing', 'order': '1'})
            values = {'title': node['title'], 'order': node['order'], **values}
        if name in ['createVolume', 'createChapter', 'updateVolume', 'updateChapter', 'deleteVolume', 'deleteChapter']:
            values = {'expected_tree_revision': str(model.tree_revision), **values}
        elif name in ['updateProject', 'archiveProject']:
            # Initial Project Revision is not exposed by getProject. One is a probe input, not an oracle.
            values = {'expected_project_revision': str(self.project_revision or 1), **values}
        expected = dict(kind=outcome, **({'reason': reason} if reason else {}))
        before = len(self.differences)
        prior_snapshot = self.snapshot
        status, response = self.http.command(name, values, project_id=self.project, **targets)
        observed = response.get('effect', {'kind': f'HTTP_{status}', 'reason': response.get('code')})
        self.coverage[name + ':' + observed['kind'] + (':' + observed['reason'] if observed.get('reason') else '')] += 1
        if reason == 'invalid_title':
            compare(self.label + '/invalid title before Admission', 400, status, self.differences)
        elif reason == 'empty_project':
            # A-003: the contract does not order the empty-Project and invalid-join refusals.
            compare(self.label + '/empty Project refusal', 'refused', observed['kind'], self.differences)
        else:
            compare(self.label + '/' + name + '/outcome', expected,
                    {key: observed.get(key) for key in expected}, self.differences)
        if status != 200 or observed['kind'] != outcome:
            for item in self.differences[before:]:
                item.update(seed_case=self.label, trace_start=self.start, trace_end=len(self.http.trace))
            return False
        applied = outcome == 'authoritative_applied'
        if applied and name.startswith('create'):
            kind = 'volume' if name == 'createVolume' else 'chapter'
            identity = observed[kind + '_id']
            expected_create = model.create(kind, values['title'], identity, targets.get('volume_id'))
            compare(self.label + '/' + name + '/effect', expected_create,
                    {key: observed.get(key) for key in expected_create}, self.differences)
        elif applied and name in ['updateVolume', 'updateChapter', 'deleteVolume', 'deleteChapter']:
            kind = 'volume' if 'Volume' in name else 'chapter'
            identity = targets[kind + '_id']
            siblings = model.volumes if kind == 'volume' else next(v['chapters'] for v in model.volumes if any(c['chapter_id'] == identity for c in v['chapters']))
            node = next(n for n in siblings if n[kind + '_id'] == identity)
            siblings.remove(node)
            if name.startswith('update'):
                node['title'] = values.get('title', node['title'])
                siblings.insert(int(values.get('order', node['order'])) - 1, node)
            for rank, sibling in enumerate(siblings, 1):
                sibling['order'] = str(rank)
            model.tree_revision += 1
            model.actions += 1
        elif applied and name == 'setCurrentChapter':
            self.current = values['chapter_id']
            model.actions += 1
        elif applied and name in ['updateProject', 'archiveProject']:
            if self.project_revision is not None:
                compare(self.label + '/project revision increment', self.project_revision + 1, int(observed['revision']), self.differences)
            self.project_revision = int(observed['revision'])
            if name == 'updateProject':
                self.title = values['title']
            else:
                self.archived = True
        receipt = response['receipt']
        compare(self.label + '/Receipt result', outcome, receipt['result'], self.differences)
        if name not in ['updateProject', 'archiveProject']:
            compare(self.label + '/Commit count', int(applied and name != 'setCurrentChapter'), len(receipt['authoritative_commit_ids']), self.differences)
            compare(self.label + '/Author Action', str(model.actions) if applied else None, receipt['author_action_sequence'], self.differences)
        if not applied:
            for field in ['authoritative_revision_ids', 'proposal_revision_ids', 'authoritative_commit_ids', 'draft_artifact_refs', 'condition_refs']:
                compare(self.label + '/' + field, [], receipt[field], self.differences)
        status, tree = self.http.request('GET', f'/api/v1/projects/{self.project}/manuscript/tree')
        if status == 200:
            compare(self.label + '/tree', {'tree_revision': str(model.tree_revision), 'volumes': model.volumes},
                    {key: tree[key] for key in ['tree_revision', 'volumes']}, self.differences)
            if not applied:
                compare(self.label + '/no Activity', prior_snapshot['project_activity_position'], tree['snapshot']['project_activity_position'], self.differences)
            self.snapshot = tree['snapshot']
        elif not self.archived:
            compare(self.label + '/tree HTTP', 200, status, self.differences)
        status, project = self.http.request('GET', f'/api/v1/projects/{self.project}')
        if status == 200:
            compare(self.label + '/Project title', self.title, project['project']['title'], self.differences)
            if self.current is not None:
                compare(self.label + '/Current Chapter', self.current, project['project']['open'].get('current_chapter_id'), self.differences)
        self.last = response
        for item in self.differences[before:]:
            item.update(seed_case=self.label, trace_start=self.start, trace_end=len(self.http.trace))
        return True

    def volume(self):
        assert self.execute('createVolume', {'title': 'Volume ' + str(self.http.rng.randrange(100000))})
        return self.last['effect']['volume_id']

    def chapter(self, volume):
        assert self.execute('createChapter', {'title': 'Chapter ' + str(self.http.rng.randrange(100000))}, volume_id=volume)
        return self.last['effect']['chapter_id']


def cases():
    from run import SCHEMAS
    import json
    result = []
    for name in ['createVolume', 'createChapter', 'updateVolume', 'updateChapter', 'deleteVolume', 'deleteChapter', 'setCurrentChapter', 'updateProject', 'archiveProject']:
        stem = ''.join('-' + char.lower() if char.isupper() else char for char in name)
        definitions = json.loads((SCHEMAS / (stem + '-response.schema.json')).read_text())['$defs']
        effect = definitions[name[0].upper() + name[1:] + 'Effect']
        for branch in effect['oneOf']:
            kind = branch['properties']['kind']['const']
            reason = branch['properties'].get('reason')
            result.extend((name, kind, item) for item in definitions[reason['$ref'].split('/')[-1]]['enum']) if reason else result.append((name, kind, None))
    return result


def run(http, seed, differences, coverage, selected=None):
    if selected == 'zero-revision':
        s = Scenario(http, seed, selected, differences, coverage)
        status, response = http.command('createVolume', dict(title='Zero revision probe', expected_tree_revision='0'), project_id=s.project)
        coverage['schema_probe:createVolume:HTTP_' + str(status)] += 1
        return
    sequence = cases()
    http.rng.shuffle(sequence)
    for name, outcome, reason in sequence:
        case = ':'.join(filter(None, [name, outcome, reason]))
        if selected and selected not in case:
            continue
        print(f'Seed {seed}: {case}', flush=True)
        s = Scenario(http, seed, case, differences, coverage)
        targets, values = {}, {}
        if name not in ['createVolume', 'updateProject', 'archiveProject']:
            volume = s.volume()
            if 'Chapter' in name and name != 'createChapter':
                chapter = s.chapter(volume)
                targets['chapter_id'] = chapter
            else:
                targets['volume_id'] = volume
        if name in ['createVolume', 'createChapter', 'updateProject']:
            values['title'] = 'Name ' + str(http.rng.randrange(100000))
        if name in ['updateVolume', 'updateChapter']:
            values['title'] = 'Changed ' + str(http.rng.randrange(100000))
        if name == 'setCurrentChapter':
            _, session = http.command('createEditorSession', {}, project_id=s.project)
            _, chapter_query = http.request('GET', f'/api/v1/projects/{s.project}/chapters/{chapter}')
            current = session['base_snapshot']['chapter_id']
            if outcome == 'authoritative_applied':
                chapter = s.chapter(volume)
                _, chapter_query = http.request('GET', f'/api/v1/projects/{s.project}/chapters/{chapter}')
            values = dict(chapter_id=chapter, expected_current_chapter_id=current,
                expected_target_revision_id=chapter_query['chapter']['current_revision']['revision_id'],
                editor_session_id=session['editor_session']['editor_session_id'])
            targets = {}
        if reason == 'unchanged':
            values = {}
        elif reason == 'title_unchanged':
            values['title'] = s.title
        elif reason in ['already_removed', 'already_archived']:
            assert s.execute(name, {}, **targets)
        elif reason == 'nonempty_volume':
            s.chapter(volume)
        elif reason == 'archived_project':
            assert s.execute('archiveProject', {})
        elif reason in ['stale_tree_revision', 'stale_project_revision']:
            field = 'expected_tree_revision' if reason == 'stale_tree_revision' else 'expected_project_revision'
            values[field] = '999999'
        elif reason == 'invalid_title':
            values['title'] = ''
        elif reason == 'invalid_order':
            values['order'] = '999999'
        elif reason in ['invalid_volume_join', 'invalid_chapter_join']:
            field = 'volume_id' if reason == 'invalid_volume_join' else 'chapter_id'
            (values if name == 'setCurrentChapter' else targets)[field] = http.identity()
        elif reason == 'invalid_placement':
            values['placement'] = {'kind': http.rng.choice(['before', 'after']), 'chapter_id': http.identity()}
        elif reason == 'stale_current_chapter':
            values['expected_current_chapter_id'] = http.identity()
        elif reason == 'wrong_target_head':
            values['expected_target_revision_id'] = http.identity()
        elif reason == 'empty_project':
            assert s.execute('deleteChapter', {}, chapter_id=chapter)
        s.execute(name, values, outcome, reason, **targets)
    if selected:
        return
    s = Scenario(http, seed, 'random-walk', differences, coverage)
    for _ in range(9):
        volumes = s.model.volumes
        options = ['createVolume']
        if volumes:
            options += ['createChapter', 'updateVolume', 'deleteVolume']
        chapters = [(v, c) for v in volumes for c in v['chapters']]
        if chapters:
            options += ['updateChapter', 'deleteChapter']
        name = http.rng.choice(options)
        title = http.rng.choice(['Night', 'Dawn', 'Scene']) + str(http.rng.randrange(100000))
        if name == 'createVolume':
            s.execute(name, {'title': title})
        elif name in ['createChapter', 'updateVolume', 'deleteVolume']:
            volume = http.rng.choice(volumes)
            values = {'title': title} if name != 'deleteVolume' else {}
            if name == 'updateVolume':
                values['order'] = str(http.rng.randrange(1, len(volumes) + 1))
            refused = name == 'deleteVolume' and bool(volume['chapters'])
            s.execute(name, values, 'refused' if refused else 'authoritative_applied',
                      'nonempty_volume' if refused else None, volume_id=volume['volume_id'])
        else:
            volume, chapter = http.rng.choice(chapters)
            values = {'title': title, 'order': str(http.rng.randrange(1, len(volume['chapters']) + 1))} if name == 'updateChapter' else {}
            s.execute(name, values, chapter_id=chapter['chapter_id'])
