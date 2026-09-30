#!/usr/bin/env python3
"""Read bounded native issue relationships without reading execution contracts."""

import argparse
import json
from pathlib import Path
import runpy
import sys

sys.dont_write_bytecode = True

from verification_summary import MAX_BYTES, MAX_LINES, PAGE_SIZE, short

api = runpy.run_path(str(Path(__file__).with_name('verify-stage1-ticket-bindings.py')))['api']


def issue(value):
    if not isinstance(value, dict) or type(value.get('number')) is not int or not isinstance(value.get('title'), str):
        raise ValueError('Incomplete issue identity')
    return {'number': value['number'], 'title': short(value['title']),
            'state': value.get('state') if value.get('state') in ('open', 'closed') else 'unknown'}


def dependencies(number, relation):
    result = {}
    for page in range(1, 101):
        values = api(f'issues/{number}/dependencies/{relation}?per_page=100&page={page}')
        if not isinstance(values, list) or len(values) > 100:
            raise ValueError('Dependency page is not a list')
        for value in values:
            item = issue(value)
            if item['number'] in result:
                raise ValueError('Dependency pages changed during the query; retry')
            result[item['number']] = item
        if len(values) < 100:
            return sorted(result.values(), key=lambda item: (item['state'] == 'closed', item['number']))
    raise ValueError('Dependency page limit reached; relationship state is unknown')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('number', type=int)
    parser.add_argument('--relation', choices=('blocked_by', 'blocking'), default='blocked_by')
    parser.add_argument('--page', type=int, default=1)
    parser.add_argument('--format', choices=('text', 'json'), default='text')
    args = parser.parse_args()
    try:
        if args.number < 1:
            raise ValueError('Use a positive issue number')
        item = issue(api(f'issues/{args.number}'))
        relations = {name: dependencies(args.number, name) for name in ('blocked_by', 'blocking')}
        selected = relations[args.relation]
        pages = max(1, (len(selected) + PAGE_SIZE - 1) // PAGE_SIZE)
        if not 1 <= args.page <= pages:
            raise ValueError(f'Page must be between 1 and {pages}')
        unknown = sum(i['state'] == 'unknown' for i in relations['blocked_by'])
        opened = sum(i['state'] == 'open' for i in relations['blocked_by'])
        output = {'version': 1, 'issue': item, 'dependencyState': 'unknown' if unknown or item['state'] == 'unknown' else
                  'blocked' if opened else 'unblocked', 'openBlockers': opened, 'unknownBlockers': unknown,
                  'counts': {name: len(values) for name, values in relations.items()},
                  'relation': args.relation, 'page': args.page, 'pages': pages,
                  'items': selected[(args.page - 1) * PAGE_SIZE:args.page * PAGE_SIZE]}
        output['omitted'] = len(selected) - len(output['items'])
        output['next'] = (f'python3 scripts/tracker_query.py {args.number} --relation {args.relation} --page {args.page + 1}'
                          if args.page < pages else None)
        output['inspect'] = f'python3 scripts/tracker_query.py {args.number} --relation blocking --page 1'
        output['contract'] = f'gh issue view {args.number} --repo FrankQDWang/StoryOS --json title,body,labels,assignees,comments'
        output['hint'] = 'Read the real ticket, parent and tracked execution contracts before Claim. This query does not authorize execution.'
        rendered = json.dumps(output, ensure_ascii=True, separators=(',', ':')) if args.format == 'json' else '\n'.join(
            f'{key}: {json.dumps(value, ensure_ascii=True)}' for key, value in output.items())
        if len((rendered + '\n').encode()) > MAX_BYTES or len(rendered.splitlines()) > MAX_LINES:
            raise ValueError('Query exceeds its output limit')
        print(rendered)
        return 0
    except (ValueError, OSError, SystemExit) as error:
        print(json.dumps({'dependencyState': 'unknown', 'error': short(error), 'hint': 'Read failed; retry before Claim.'}))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
