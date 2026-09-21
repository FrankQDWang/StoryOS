"""Check retained workflow graphs through public verification commands."""

import json
import unittest

import verification_plan_tests
import verification_evidence_tests


class GraphPlanTests(unittest.TestCase):
    def setUp(self):
        self.fixture = verification_plan_tests.FilePlanTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.install_runner_fixture()
        self.path = self.fixture.add_test('first.test.ts')
        self.other = self.fixture.add_test('other.test.ts')
        self.policy = json.loads(self.fixture.policy_path.read_text())
        self.policy['complete'] = {'stages': ['foundation-tests'], 'groups': {'node-contract': ['foundation-tests']}}
        self.policy['workflow'] = {
            'version': 1, 'operations': {'node-install': {'type': 'setup', 'requires': []}},
            'profiles': {'node-contract': {'requires': ['node-install']}, 'policy': {}, 'contracts': {}, 'web-typecheck': {}},
            'stage_types': {'foundation-tests': 'aggregate'}, 'targeted': {}}
        self.save()
        self.fixture.repo.git('add', '.')
        self.fixture.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                              'commit', '--quiet', '-m', 'Declare workflow.')
        self.fixture.base = self.fixture.repo.git('rev-parse', 'HEAD')
        self.path.write_text(self.path.read_text() + '// changed\n')

    def save(self):
        self.fixture.policy_path.write_text(json.dumps(self.policy))

    def test_retained_daily_graph_has_full_membership_and_selected_preparation(self):
        result = self.fixture.cli('run')
        self.assertEqual(result.returncode, 0, result.stderr)
        plan = self.fixture.repo.report()['plan']
        graph = plan['graph']
        nodes = {n['id']: n for n in graph['nodes']}
        first = 'file:node-contract:apps/web/test/node-contract/first.test.ts'
        other = 'file:node-contract:apps/web/test/node-contract/other.test.ts'
        self.assertEqual({first, other} & nodes.keys(), {first, other})
        self.assertTrue(nodes[first]['selected'])
        self.assertFalse(nodes[other]['selected'])
        self.assertTrue(nodes['check:node-install']['selected'])
        self.assertEqual(nodes['check:node-install']['type'], 'setup')
        self.assertIn({'from': 'check:node-contract', 'to': first, 'type': 'member'}, graph['relations'])
        self.assertIn({'from': 'check:node-install', 'to': 'check:node-contract'}, graph['dependencies'])
        self.assertEqual(graph['identity']['source'], plan['source'])
        self.assertTrue(graph['identity']['plan_sha256'])
        old_ids = set(nodes)
        self.other.rename(self.other.with_name('renamed.test.ts'))
        changed = json.loads(self.fixture.cli('plan').stdout)['graph']
        self.assertEqual(old_ids - {n['id'] for n in changed['nodes']}, {other})
        self.assertNotEqual(graph['identity'], changed['identity'])


    def test_complete_and_targeted_plans_cover_obligations_without_execution(self):
        result = self.fixture.cli('plan', '--profile', 'complete')
        self.assertEqual(result.returncode, 0, result.stderr)
        graph = json.loads(result.stdout)['graph']
        self.assertTrue(all(n['selected'] for n in graph['nodes'] if n['type'] == 'test-file'))
        self.assertTrue(next(n for n in graph['nodes'] if n['id'] == 'check:foundation-tests')['selected'])
        self.policy['targeted'] = {'sample': {'command': ['echo', 'sample'], 'clean': False}}
        self.policy['workflow']['targeted'] = {'sample': ['check:foundation-tests']}
        self.save()
        result = self.fixture.repo.cli('status', '--check', 'sample', '--json')
        self.assertEqual(result.returncode, 0, result.stderr)
        nodes = json.loads(result.stdout)['plan']['graph']['nodes']
        self.assertTrue(all(n['selected'] for n in nodes if n['type'] == 'test-file'))
        self.other.unlink()
        result = self.fixture.repo.cli('status', '--check', 'sample', '--json')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(str(self.other.relative_to(self.fixture.root)), json.loads(result.stdout)['plan']['test_files'])
        self.assertFalse(list(self.fixture.root.glob('target/verification/*/report.json')))

    def test_invalid_workflow_fails_at_the_public_plan_boundary(self):
        original = json.loads(json.dumps(self.policy))
        mutations = [
            lambda p: p['workflow']['operations']['node-install'].update(requires=['absent']),
            lambda p: p['workflow']['operations']['node-install'].update(requires=['node-contract']),
            lambda p: p['workflow']['profiles']['node-contract'].update(requires=['node-install', 'node-install']),
            lambda p: p['complete']['groups'].clear(),
        ]
        for mutate in mutations:
            self.policy = json.loads(json.dumps(original))
            mutate(self.policy)
            self.save()
            result = self.fixture.cli('plan')
            self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertFalse(list(self.fixture.root.glob('target/verification/*/report.json')))


    def test_shared_file_dependencies_and_resets_are_distinct_from_membership(self):
        policy = self.policy
        policy['complete']['stages'].append('http-files')
        policy['complete']['groups']['node-postgresql'] = ['http-files']
        policy['rules'].insert(0, {'pattern': 'apps/web/test/node-postgresql/*.test.ts', 'kind': 'web-test', 'group': 'node-postgresql'})
        policy['shared_phases'] = [
            {'name': name, 'group': 'node-postgresql', 'stage': 'http-files', 'prepare': prepare}
            for name, prepare in [('first', 'none'), ('second', 'reset-challenge')]]
        policy['workflow']['operations']['database-setup'] = {'type': 'setup', 'requires': []}
        policy['workflow']['operations']['database-cleanup'] = {'type': 'cleanup', 'requires': ['database-setup'], 'after': ['node-postgresql']}
        policy['workflow']['profiles']['node-postgresql'] = {'requires': ['database-setup']}
        policy['workflow']['profiles']['release-package'] = {}
        paths = ['apps/web/test/node-postgresql/' + name + '.test.ts' for name in ['a', 'b', 'c']]
        for path, phase, after in zip(paths, ['first', 'first', 'second'], [[], paths[:1], []]):
            file = self.fixture.root / path
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('// Verification: ' + json.dumps({'phase': phase, 'after': after}) + '\n')
        self.save()
        result = self.fixture.cli('plan')
        self.assertEqual(result.returncode, 0, result.stderr)
        graph = json.loads(result.stdout)['graph']
        self.assertIn({'from': 'file:node-postgresql:' + paths[0], 'to': 'file:node-postgresql:' + paths[1]}, graph['dependencies'])
        self.assertIn({'from': 'prepare:second', 'to': 'phase:second'}, graph['dependencies'])
        self.assertIn({'from': 'check:database-setup', 'to': 'prepare:second'}, graph['dependencies'])
        self.assertIn({'from': 'check:node-postgresql', 'to': 'check:database-cleanup', 'when': 'both-selected'}, graph['dependencies'])
        self.assertNotIn({'from': 'check:http-files', 'to': 'check:node-postgresql', 'type': 'contains'}, graph['relations'])
        self.assertTrue(next(n for n in graph['nodes'] if n['id'] == 'prepare:second')['selected'])
        (self.fixture.root / paths[0]).unlink()
        self.assertNotEqual(self.fixture.cli('plan').returncode, 0)


    def test_complete_report_graph_is_retained_and_stale_graph_evidence_is_refused(self):
        fixture = verification_evidence_tests.CandidateEvidenceTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        policy_path = fixture.root / 'docs/agents/verification-policy.json'
        policy = json.loads(policy_path.read_text())
        policy['workflow'] = {'version': 1, 'operations': {}, 'profiles': {},
                              'stage_types': {'sample': 'aggregate'}, 'targeted': {}}
        policy_path.write_text(json.dumps(policy))
        fixture.commit()
        report = json.loads(fixture.prepare('--policy-reviewed').read_text())
        self.assertEqual(fixture.check().returncode, 0)
        self.assertTrue(report['graph']['identity']['plan_sha256'])
        report['graph'] = {'version': 1, 'sha256': '0' * 64}
        fixture.write_report(report)
        self.assertIn('graph', fixture.check().stderr)
        report['graph'] = {'nodes': []}
        fixture.write_report(report)
        self.assertIn('graph', fixture.check().stderr)


if __name__ == '__main__':
    unittest.main()
