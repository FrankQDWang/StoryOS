"""Export policy-owned workflow membership without changing execution."""

import fnmatch
import graphlib
import hashlib
import json
import subprocess

import verification_shared


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def attach(root, plan, policy, files, revision=None):
    workflow = policy.get('workflow')
    if workflow is None:
        return
    if workflow['version'] != 1:
        raise ValueError('Unsupported workflow version')
    nodes, dependencies, relations, ordering = {}, set(), set(), set()

    def node(name, kind, **fields):
        if name in nodes or kind not in {'check', 'aggregate', 'test-file', 'build', 'setup', 'reset', 'cleanup'}:
            raise ValueError(f'Duplicate identity or invalid workflow type: {name}')
        nodes[name] = {'id': name, 'type': kind, 'selected': False, **fields}
        return name

    stages = policy.get('complete', {}).get('stages', [])
    groups = policy.get('complete', {}).get('groups', {})
    operations = workflow['operations']
    profiles = workflow['profiles']
    if set(operations) & (set(profiles) | set(stages)):
        raise ValueError('Duplicate workflow operation identity')
    if set(workflow['targeted']) != set(policy.get('targeted', {})):
        raise ValueError('Missing targeted workflow membership')
    if set(workflow['stage_types']) - set(stages):
        raise ValueError('Workflow type has no mandatory stage')
    for name in sorted(set(stages) | set(operations) | set(profiles)):
        kind = operations.get(name, {}).get('type', workflow['stage_types'].get(name, 'aggregate' if name in profiles else 'check'))
        node('check:' + name, kind, profile=name)
    for name, entry in {**profiles, **operations}.items():
        required = entry.get('requires', [])
        if len(required) != len(set(required)):
            raise ValueError(f'Duplicate dependency: {name}')
        dependencies.update(('check:' + dep, 'check:' + name) for dep in required)
        ordering.update(('check:' + dep, 'check:' + name) for dep in entry.get('after', []))
    for check in policy.get('targeted', {}):
        node('targeted:' + check, 'aggregate', profile='targeted:' + check)
        relations.update(('targeted:' + check, member, 'contains') for member in workflow['targeted'].get(check, []))
    for check, entry in profiles.items():
        relations.update(('check:' + check, 'check:' + member, 'contains') for member in entry.get('members', []))
    current = [item for item in files if item['kind'].endswith('-test')
               and item['kind'] not in {'historical-test', 'prototype-test'}
               and (revision or (root / item['path']).is_file())]
    file_ids = {}
    for item in current:
        group, path = item['group'], item['path']
        owner = 'check:' + group
        if owner not in nodes:
            node(owner, 'aggregate', profile=group)
        name = node(f'file:{group}:{path}', 'test-file', profile=group, path=path,
                    execution='member-only')
        file_ids[path] = name
        relations.add((owner, name, 'member'))
        for stage in groups.get(group.split(':')[0], []):
            if 'check:' + stage != owner and group not in {'node-postgresql', 'node-process-cut'}:
                relations.add(('check:' + stage, owner, 'contains'))
        if group.startswith('cargo:'):
            dependencies.update(('check:' + dep, owner) for dep in profiles.get('cargo', {}).get('requires', []))
    previous = None
    for phase in verification_shared.plan(root, policy, current, revision):
        name = node('phase:' + phase['name'], 'aggregate', profile=phase['group'])
        relations.add(('check:' + phase['stage'], name, 'contains'))
        dependencies.update(('check:' + dep, name) for dep in profiles.get(phase['group'], {}).get('requires', []))
        if previous:
            ordering.add((previous, name))
        if phase['prepare'] != 'none':
            reset = node('prepare:' + phase['name'], 'reset', operation=phase['prepare'])
            dependencies.add((reset, name))
            dependencies.update(('check:' + dep, reset) for dep in profiles.get(phase['group'], {}).get('requires', []))
            if previous:
                ordering.add((previous, reset))
        for path in phase['files']:
            relations.add((name, file_ids[path], 'member'))
            source = (subprocess.check_output(['git', 'show', f'{revision}:{path}'], cwd=root, text=True)
                      if revision else (root / path).read_text())
            declaration = json.loads(source.splitlines()[0].removeprefix('// Verification:'))
            dependencies.update((file_ids[dep], file_ids[path]) for dep in declaration['after'])
        previous = name
    for before, after in dependencies | ordering | {(a, b) for a, b, _ in relations}:
        if before not in nodes or after not in nodes:
            raise ValueError(f'Dangling workflow edge: {before} -> {after}')
    graph = {name: set() for name in nodes}
    for before, after in dependencies | ordering:
        graph[after].add(before)
    graphlib.TopologicalSorter(graph).prepare()
    containment = {name: set() for name in nodes}
    for parent, child, _ in relations:
        containment[child].add(parent)
    graphlib.TopologicalSorter(containment).prepare()
    if set(file_ids) != set(plan['test_files']) - {i['path'] for i in files if i['kind'] in {'historical-test', 'prototype-test'}}:
        raise ValueError('Workflow is missing discovered test files')
    selected = set()
    if 'stages' in plan:
        selected.update('check:' + stage for stage in stages)
    elif 'check' in plan:
        selected.add('targeted:' + plan['check'])
    if 'stages' in plan or 'check' in plan:
        while True:
            expanded = selected | {b for a, b, _ in relations if a in selected}
            if expanded == selected:
                break
            selected = expanded
    else:
        for check in plan['checks']:
            owner = 'check:' + check['group']
            if owner not in nodes:
                raise ValueError(f'Missing workflow profile: {check["group"]}')
            selected.add(owner)
            selected.update(file_ids[path] for path in check['files'] if path in file_ids)
            if check.get('requires_package'):
                selected.add('check:release-package')
            selected.update('check:' + member for member in profiles.get(check['group'], {}).get('members', []))
            if check['group'] == 'policy':
                selected.update(file_ids[i['path']] for i in current if i['group'] == 'verification-tools')
    while True:
        expanded = selected | {a for a, b in dependencies if b in selected}
        expanded.update(b for a, b, kind in relations if a in selected and kind == 'contains' and
                        ('stages' in plan or 'check' in plan or nodes[a]['type'] in {'build', 'setup'} or a == 'check:release-package'))
        expanded.update(a for a, b, kind in relations if b in selected and kind == 'member')
        expanded.update('check:' + name for name, entry in operations.items()
                        if entry['type'] == 'cleanup' and any('check:' + dep in selected for dep in entry.get('requires', [])))
        if expanded == selected:
            break
        selected = expanded
    command = plan.get('command', [])
    if 'unittest' in command and 'discover' in command and '-p' in command:
        pattern = command[command.index('-p') + 1]
        selected -= {name for name in selected if nodes[name]['type'] == 'test-file'
                     and not fnmatch.fnmatchcase(nodes[name]['path'].rsplit('/', 1)[-1], pattern)}
    for name in selected:
        nodes[name]['selected'] = True
    plan['graph'] = {
        'version': 1,
        'identity': {'source': plan.get('source', {'tree': plan.get('tree')}),
                     'policy_sha256': plan.get('policy_sha256', hashlib.sha256((root / 'docs/agents/verification-policy.json').read_bytes()).hexdigest()),
                     'membership_sha256': digest(plan['test_files']), 'plan_sha256': digest({k: v for k, v in plan.items() if k != 'historical_estimate_seconds'})},
        'nodes': [nodes[key] for key in sorted(nodes)],
        'dependencies': [{'from': a, 'to': b} for a, b in sorted(dependencies)] +
                        [{'from': a, 'to': b, 'when': 'both-selected'} for a, b in sorted(ordering)],
        'relations': [{'from': a, 'to': b, 'type': kind} for a, b, kind in sorted(relations)]}


def evidence(value):
    """Publish graph digests while local reports retain complete snapshots."""
    if isinstance(value, dict):
        return {key: ({'version': 1, 'sha256': digest(item)} if key == 'graph' and item is not None
                      else evidence(item)) for key, item in value.items()}
    if isinstance(value, list):
        return [evidence(item) for item in value]
    return value


def bind_attempt(report, stage, node_id=None):
    """Bind an executor boundary to its retained graph, without expanding members."""
    plan = report.get('plan') or {}
    graph = report.get('graph') or plan.get('graph')
    if not graph:
        return {}
    nodes = {node['id']: node for node in graph['nodes']}
    name = node_id or 'check:' + stage
    if name not in nodes:
        return {}
    node = nodes[name]
    checks = [check for check in plan.get('checks', []) if check['group'] == node.get('profile')
              or (name == 'check:cargo' and check['group'].startswith('cargo:'))]
    return {'node_version': 1, 'run_id': report['run_id'], 'graph_sha256': digest(graph),
            'node_id': name, 'selection_reason': [reason for check in checks for reason in check.get('reasons', [])]
            or ['Registered command boundary'],
            'execution_scope': checks or {'profile': node.get('profile'), 'node': name}}
