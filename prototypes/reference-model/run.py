"""Seeded HTTP comparison against a contract-only model. No product imports."""

import argparse
from collections import Counter
from copy import deepcopy
import hashlib
import http.cookiejar
import json
import os
from pathlib import Path
import random
import subprocess
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / 'generated/json-schema/storyos-public-release-1'
CATALOG = json.loads((ROOT / 'docs/foundation/versioned-protocol-release-1-route-catalog.json').read_text())
ROUTES = {entry['operation_id']: entry for entry in CATALOG['operations']}


def wire(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()


class HTTP:
    def __init__(self, url, rng, trace):
        self.url, self.rng, self.trace = url, rng, trace
        self.replay = False
        self.commands = {}
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        self.opener.open(url).read()
        profile = self.request('GET', '/api/v1/protocol')[1]
        self.client = profile['release_identity']['web_client_contract_revision']
        manifest = json.loads((ROOT / 'target/release-package/web/manifest.json').read_text())
        self.security = manifest['security_policy_revision']

    def identity(self):
        value = (1791134461000 << 80) | self.rng.getrandbits(80)
        value = (value & ~(15 << 76)) | (7 << 76)
        value = (value & ~(3 << 62)) | (2 << 62)
        return str(uuid.UUID(int=value))

    def meta(self):
        return dict(client_contract_revision=self.client, security_policy_revision=self.security,
                    correlation_id=self.identity())

    def request(self, method, path, body=None, headers=None):
        request = urllib.request.Request(self.url + path, data=None if body is None else wire(body),
            headers={'Origin': self.url, 'Content-Type': 'application/json', **(headers or {})}, method=method)
        try:
            response = self.opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        raw = response.read()
        self.last_raw = raw
        result = json.loads(raw)
        safe = {k: v for k, v in result.items() if k != 'nonce'}
        self.trace.append(dict(method=method, path=path, request=body, status=response.status, response=safe))
        return response.status, result

    def command(self, name, values, **targets):
        route = ROUTES[name]
        schema = route['schemas']['request']
        stem = schema.split('.')[2]
        body = {'command_schema': schema, stem.replace('-', '_') + '_input': {**self.meta(), **values}}
        if name in ['createEditorSession', 'takeOverProjectWriter', 'applyAuthorEdit']:
            body = {'command_schema': schema, **self.meta(), **values}
        key = self.identity()
        if name == 'createProject':
            challenge_path = '/api/v1/anti-forgery-challenges'
            challenge_body = {**body, 'idempotency_key': key}
            status, challenge = self.request('POST', challenge_path, challenge_body)
            if status != 200:
                return status, challenge
            body['prospective_project_id'] = challenge['prospective_project_id']
        else:
            digest = dict(algorithm='sha256', profile=f'storyos.command.{name}.jcs.v1',
                          value_hex_lowercase=hashlib.sha256(wire(body)).hexdigest())
            challenge_path = f"/api/v1/projects/{targets['project_id']}/anti-forgery-challenges"
            challenge_body = dict(method=route['method'], route_template=route['path'], command_schema=schema,
                                  canonical_command_digest=digest, idempotency_key=key)
            status, challenge = self.request('POST', challenge_path, challenge_body)
            if status != 200:
                return status, challenge
        path = route['path'].format(**targets)
        self.last_challenge = (challenge_path, challenge_body, challenge)
        if self.replay:
            retry_status, retry = self.request('POST', challenge_path, challenge_body)
            equal = retry_status == 200 and retry == challenge
            compare(name + '/Challenge exact retry', True, equal, self.differences)
            self.coverage['challenge:exact_retry:' + str(equal)] += 1
        headers = {'Idempotency-Key': key, 'X-StoryOS-Anti-Forgery': challenge['nonce']}
        status, response = self.request(route['method'], path, body, headers)
        self.last_command = (route['method'], path, body, headers, self.last_raw)
        self.commands[name] = self.last_command
        if self.replay and status in [200, 202]:
            retry_status, _ = self.request(route['method'], path, body, headers)
            equal = retry_status == status and self.last_raw == self.last_command[4]
            compare(name + '/Command exact retry bytes', True, equal, self.differences)
            self.coverage['replay:' + name + ':' + str(equal)] += 1
        return status, response


class Structure:
    # ADR 0018, 0025, 0028, 0029; GLOSSARY: Empty Project create.
    def __init__(self):
        self.tree_revision, self.volumes, self.actions = 0, [], 0

    def create(self, kind, title, identity, parent=None):
        self.tree_revision += 1
        self.actions += 1
        siblings = self.volumes if kind == 'volume' else next(v['chapters'] for v in self.volumes if v['volume_id'] == parent)
        node = {kind + '_id': identity, 'title': title, 'order': str(len(siblings) + 1)}
        if kind == 'volume':
            node['chapters'] = []
        siblings.append(node)
        return dict(kind='authoritative_applied', tree_revision=str(self.tree_revision), order=node['order'], title=title)


def compare(label, expected, actual, differences):
    if expected != actual:
        differences.append(dict(label=label, expected=deepcopy(expected), actual=deepcopy(actual)))


def chain(http, seed, differences, coverage):
    model = Structure()
    title = f'Reference {seed}'
    status, response = http.command('createProject', {'title': title})
    compare('createProject HTTP', 200, status, differences)
    if status != 200:
        return
    project = response['project']['project_id']
    compare('Empty Project', {'project_id': project, 'title': title, 'open': {'kind': 'empty'}}, response['project'], differences)
    coverage['createProject:created'] += 1
    status, initial = http.request('GET', f'/api/v1/projects/{project}/manuscript/tree')
    compare('Empty tree HTTP', 200, status, differences)
    compare('Empty tree', [], initial['volumes'], differences)
    # The contract does not define the initial number. Check increments from this opaque baseline.
    model.tree_revision = int(initial['tree_revision'])
    parent = None
    for kind in ['volume', 'chapter']:
        name = 'create' + kind.title()
        title = http.rng.choice(['Dawn', 'Night', 'Chapter']) + str(http.rng.randrange(10000))
        targets = {'project_id': project, **({'volume_id': parent} if parent else {})}
        status, response = http.command(name, {'title': title, 'expected_tree_revision': str(model.tree_revision)}, **targets)
        compare(name + ' HTTP', 200, status, differences)
        if status != 200:
            return
        effect = response['effect']
        identity = effect[kind + '_id']
        expected = model.create(kind, title, identity, parent)
        compare(name + ' effect', expected, {key: effect[key] for key in expected}, differences)
        receipt = response['receipt']
        compare(name + ' commit count', 1, len(receipt['authoritative_commit_ids']), differences)
        compare(name + ' action sequence', str(model.actions), receipt['author_action_sequence'], differences)
        coverage[name + ':' + effect['kind']] += 1
        status, tree = http.request('GET', f'/api/v1/projects/{project}/manuscript/tree')
        compare(name + ' query HTTP', 200, status, differences)
        compare(name + ' tree', {'tree_revision': str(model.tree_revision), 'volumes': model.volumes},
                {key: tree[key] for key in ['tree_revision', 'volumes']}, differences)
        parent = identity


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seed', type=int, default=1)
    parser.add_argument('--count', type=int, default=1)
    parser.add_argument('--stage', default='bootstrap')
    parser.add_argument('--case')
    parser.add_argument('--replay', action='store_true')
    parser.add_argument('--output', default='target/reference-model/bootstrap.json')
    args = parser.parse_args()
    package = ROOT / 'target/release-package'
    manifest_bytes = (package / 'web/manifest.json').read_bytes()
    package_manifest = json.loads(manifest_bytes)
    provenance = dict(package_source_commit=package_manifest['source_commit'],
        web_manifest_sha256=hashlib.sha256(manifest_bytes).hexdigest(),
        server_sha256=hashlib.sha256((package / 'storyos-server').read_bytes()).hexdigest(),
        harness_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip())
    environment = {**os.environ, 'STORYOS_DATABASE_URL': os.environ['STORYOS_TEST_DATABASE_URL'],
        'STORYOS_BOOTSTRAP_SESSIONS': json.dumps({'reference-model': '018f0000-0000-7001-8000-000000000001'}),
        'STORYOS_CHALLENGE_SECRET': 'reference-model-disposable-database-secret',
        'STORYOS_WORKER': '1' if args.stage == 'proposals' else '0'}
    server = subprocess.Popen([str(package / 'storyos-server'), '--bind', '127.0.0.1:0', '--web-root', str(package / 'web')],
        env=environment, stdout=subprocess.PIPE, text=True)
    trace, differences, coverage = [], [], Counter()
    try:
        line = server.stdout.readline().strip()
        assert line.startswith('STORYOS_SERVER_URL='), line
        http = HTTP(line.split('=', 1)[1], random.Random(args.seed), trace)
        http.replay, http.coverage, http.differences = args.replay, coverage, differences
        for seed in range(args.seed, args.seed + args.count):
            http.rng = random.Random(seed)
            if args.stage == 'bootstrap':
                chain(http, seed, differences, coverage)
            elif args.stage == 'structure':
                from structure import run
                run(http, seed, differences, coverage, args.case)
            elif args.stage == 'edits':
                from edits import run
                run(http, seed, differences, coverage, args.case)
            elif args.stage == 'proposals':
                from proposals import run
                run(http, seed, differences, coverage, args.case)
            elif args.stage == 'replay':
                from replay import run
                run(http, seed, differences, coverage, args.case)
    finally:
        server.terminate()
        server.wait(timeout=15)
        result = dict(seed=args.seed, count=args.count, stage=args.stage, case=args.case, base='479224809cdaae997cda51cb8853e3fafa242b65',
                      coverage=dict(coverage), differences=differences, trace=trace, provenance=provenance)
        output = ROOT / args.output
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'coverage': dict(coverage), 'differences': differences}))


if __name__ == '__main__':
    main()
