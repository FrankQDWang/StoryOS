"""Render bounded views of existing verification facts."""

from collections import Counter
import json
import shlex

PAGE_SIZE = 8
MAX_BYTES = 16384
MAX_LINES = 80


def short(value):
    text = str(value)
    if len(json.dumps(text, ensure_ascii=True)) <= 240:
        return text
    prefix = ''
    for char in text:
        if len(json.dumps(prefix + char, ensure_ascii=True)) > 200:
            break
        prefix += char
    return prefix + f'... [{len(text)} chars]'


def summary(value, page=1, selection='all'):
    plan = value.get('plan', {})
    checks = plan.get('checks', [])
    failures = value.get('failed_stages', [])
    failed = set(failures)
    ordered = sorted(enumerate(checks), key=lambda pair: (
        0 if pair[1]['group'].replace(':', '-').replace('_', '-') in failed else 1 if pair[1]['status'] == 'pending' else 2, pair[0]))
    selected = [(i, c) for i, c in ordered if selection == 'all' or
                (selection == 'blocked' and c['status'] == 'pending') or
                (selection == 'ready' and c['status'] == 'ready')]
    pages = max(1, (len(selected) + PAGE_SIZE - 1) // PAGE_SIZE)
    if not 1 <= page <= pages:
        raise ValueError(f'Page must be between 1 and {pages}')
    output = {key: value[key] for key in ('version', 'status', 'decision', 'reasonCode', 'nextAction',
              'next_command', 'agentHint', 'execution', 'heartbeat_at', 'observedStatus', 'retainedStatus', 'retainedResultCurrent') if key in value}
    for key in ('run_id', 'report'):
        if key in value:
            output[key] = short(value[key])
    if 'supervision' in value:
        output['supervision'] = value['supervision']
    if 'prerequisites' in value:
        output['prerequisites'] = {name: {**item, **({'paths': [short(p) for p in item['paths']]} if 'paths' in item else {})}
                                   for name, item in value['prerequisites'].items()}
    output['changedInputs'] = [short(v) for v in value.get('changedInputs', [])[:PAGE_SIZE]]
    output['omittedChangedInputs'] = max(0, len(value.get('changedInputs', [])) - PAGE_SIZE)
    output['base'] = plan.get('base', value.get('base'))
    output['source'] = {k: plan['source'].get(k) for k in (
        'commit', 'tree', 'dirty', 'inputs_sha256', 'index_sha256', 'write_stamps_sha256')} if 'source' in plan else None
    output['planDigest'] = plan.get('digest')
    output['changedFileCount'] = len(plan['changes']) if 'changes' in plan else None
    output['counts'] = dict(Counter(c['status'] for c in checks))
    output['checks'] = [{'index': i, 'group': short(c['group']), 'status': c['status'],
                         'fileCount': len(c.get('files', [])),
                         'reason': short(c['reasons'][-1]) if c.get('reasons') else None} for i, c in
                        selected[(page - 1) * PAGE_SIZE:page * PAGE_SIZE]]
    output.update(page=page, pages=pages, selection=selection, matchingChecks=len(selected),
                  omittedChecks=len(selected) - len(output['checks']))
    fallback = ('Release package requires clean sources' if 'check' in plan and plan['source']['dirty']
                else 'Unknown prerequisite; inspect check details')
    reasons = Counter((c.get('reasons') or [fallback])[-1]
                      for c in checks if c['status'] == 'pending')
    output['blockedReasons'] = [{'reason': short(reason), 'checks': count} for reason, count in
                                list(reasons.items())[:PAGE_SIZE]]
    output['omittedReasonGroups'] = max(0, len(reasons) - PAGE_SIZE)
    output['failed_stages'] = [short(v) for v in failures[:PAGE_SIZE]]
    output['omittedFailedStages'] = max(0, len(failures) - PAGE_SIZE)
    base = shlex.quote(output['base'] or 'origin/main')
    workers = f" --workers {plan['workers']}" if type(plan.get('workers')) is int else ''
    query = f'python3 scripts/verification_plan.py summary --base {base}{workers}'
    if plan.get('digest'):
        query += ' --expected ' + plan['digest']
    output['inspect'] = {'checks': query + ' --select blocked --page 1',
                         'page': query + f' --select {selection} --page <page>',
                         'check': query + ' --index <index> --details',
                         'nextPage': query + f' --select {selection} --page {page + 1}' if page < pages else None,
                         'export': f'python3 scripts/verification_plan.py plan --base {base}{workers} --format json'}
    if 'check' in plan:
        output['inspect'] = {'details': 'python3 scripts/verification.py status --check ' +
                             shlex.quote(plan['check']) + ' --json --details'}
    elif not plan:
        output['inspect'] = {'details': 'python3 scripts/verification.py status --attempt ' +
                             shlex.quote(value['run_id']) + ' --json --details'}
    output['limits'] = {'bytes': MAX_BYTES, 'lines': MAX_LINES, 'pageSize': PAGE_SIZE,
                        'textFieldJsonBytes': 240}
    return output


def display(output, as_json):
    if as_json:
        rendered = json.dumps(output, ensure_ascii=True, separators=(',', ':'))
    else:
        rows = [f"Status: {output['status']}; decision: {output['decision']}; reason: {output['reasonCode']}"]
        rows.extend(f'{key}: {json.dumps(item, ensure_ascii=True)}' for key, item in output.items()
                    if key not in {'status', 'decision', 'reasonCode', 'next_command', 'agentHint'})
        rows.extend([f"Next: {output.get('next_command') or 'none'}", output['agentHint']])
        rendered = '\n'.join(rows)
    if len((rendered + '\n').encode()) > MAX_BYTES or len(rendered.splitlines()) > MAX_LINES:
        raise ValueError('Summary exceeds its output limit; use the explicit --details query')
    print(rendered)


def query_error(error):
    print(json.dumps({'version': 2, 'status': 'unknown', 'decision': 'blocked',
                      'reasonCode': 'query-failed', 'nextAction': None, 'error': short(error),
                      'errorLines': len(str(error).splitlines()),
                      'agentHint': 'Repeat this query with --details to read the full error.'}))
    return 1
