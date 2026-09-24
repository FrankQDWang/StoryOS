#!/usr/bin/env python3
"""Run one policy-approved complete Web stage pair within a resource budget."""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


STAGES = {'foundation-tests': 'web-foundation', 'project-scope': 'project-scope'}


def physical_memory():
    if sys.platform == 'darwin':
        return int(subprocess.check_output(['sysctl', '-n', 'hw.memsize'], text=True))
    return os.sysconf('SC_PHYS_PAGES') * os.sysconf('SC_PAGE_SIZE')


def order(graph, stages):
    edges = {(edge['from'], edge['to']) for edge in graph['dependencies']}
    def reaches(start, end):
        seen = {start}
        while True:
            next_nodes = seen | {to for source, to in edges if source in seen}
            if end in next_nodes:
                return True
            if next_nodes == seen:
                return False
            seen = next_nodes
    first, second = ('check:' + stage for stage in stages)
    if reaches(second, first):
        return list(reversed(stages)), False
    return stages, not reaches(first, second)


def validate(policy):
    if 'complete_overlap' not in policy.get('workflow', {}):
        return
    overlap = policy['workflow']['complete_overlap']
    stages = overlap['stages']
    if (len(stages) != 2 or set(stages) != set(STAGES) or overlap['max_workers'] != 2
            or overlap['prerequisite'] != 'release-package'):
        raise ValueError('Unsupported complete overlap pair')
    resources = overlap['resources']
    if set(resources) != set(stages):
        raise ValueError('The complete overlap resource declarations are incomplete')
    for stage in stages:
        entry = resources[stage]
        if (set(entry) != {'reads', 'writes', 'cpu', 'memory_bytes'}
                or not isinstance(entry['reads'], list) or not isinstance(entry['writes'], list)
                or any(not isinstance(name, str) or not name for name in entry['reads'] + entry['writes'])
                or not isinstance(entry['cpu'], int) or entry['cpu'] < 1
                or not isinstance(entry['memory_bytes'], int) or entry['memory_bytes'] < 1):
            raise ValueError(f'Invalid complete overlap resources: {stage}')


def admission(policy, graph, profile, comparison_mode=None):
    validate(policy)
    overlap = policy['workflow']['complete_overlap']
    stages = overlap['stages']
    selected = {node['id'] for node in graph['nodes'] if node['selected']}
    if not {'check:' + stage for stage in stages} <= selected:
        raise ValueError('The Web stages are not selected in the retained graph')
    if 'check:release-package' not in selected:
        raise ValueError('The release package is missing from the retained graph')
    required = {'check:' + stage: {edge['from'] for edge in graph['dependencies']
                                    if edge['to'] == 'check:' + stage and edge['from'].startswith('check:')}
                for stage in stages}
    if any(deps - {'check:release-package', *('check:' + stage for stage in stages)} for deps in required.values()):
        raise ValueError('The Web pair has an unhandled graph prerequisite')
    if not all('check:release-package' in deps for deps in required.values()):
        raise ValueError('The Web pair must depend on the release package')
    stage_order, independent = order(graph, stages)
    resources = overlap['resources']
    left, right = (resources[stage] for stage in stages)
    conflict = bool(set(left['writes']) & (set(right['reads']) | set(right['writes']))
                    or set(right['writes']) & set(left['reads']))
    cpu = sum(item['cpu'] for item in resources.values())
    memory = sum(item['memory_bytes'] for item in resources.values())
    budget = (cpu <= overlap['cpu_budget'] <= (os.cpu_count() or 0)
              and memory <= overlap['memory_budget_bytes'] <= physical_memory())
    selected_mode = (profile == 'complete' and comparison_mode != 'serial'
                     or profile == 'targeted' and comparison_mode == 'overlap')
    return stage_order, selected_mode and independent and not conflict and budget


def child_command(stage):
    return ['make', '--no-print-directory', '-o', 'release-package', STAGES[stage]]


def child_environment(stage):
    environment = os.environ.copy()
    if stage == 'project-scope':
        environment['CARGO_BUILD_JOBS'] = '4'
        environment['RUST_TEST_THREADS'] = '4'
    return environment


def run(stages, concurrent):
    active = {}
    interrupted = 0
    def stop(signum, _frame):
        nonlocal interrupted
        interrupted = signum
        for child in active.values():
            try:
                os.killpg(child.pid, signum)
            except ProcessLookupError:
                pass
    previous = {sig: signal.signal(sig, stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    results = {}
    def settle(stage, child, code):
        deadline = time.monotonic() + 30
        while True:
            try:
                os.killpg(child.pid, 0)
            except ProcessLookupError:
                break
            if time.monotonic() >= deadline:
                os.killpg(child.pid, signal.SIGKILL)
                code = 1
                break
            time.sleep(0.05)
        results[stage] = code
        active.pop(stage)
    try:
        for stage in stages:
            if interrupted:
                break
            child = subprocess.Popen(child_command(stage), env=child_environment(stage), start_new_session=True)
            active[stage] = child
            if not concurrent:
                settle(stage, child, child.wait())
                if results[stage] != 0:
                    break
        while active:
            for stage, child in list(active.items()):
                code = child.poll()
                if code is not None:
                    settle(stage, child, code)
            if active:
                time.sleep(0.05)
    except OSError:
        stop(signal.SIGTERM, None)
        for child in active.values():
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
        raise
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    if interrupted:
        return 128 + interrupted
    return next((code for stage in stages if (code := results.get(stage, 0)) != 0), 0)


def main():
    run_path = os.environ.get('STORYOS_VERIFICATION_RUN')
    if not run_path:
        raise ValueError('The Web overlap requires an owned verification run')
    root = Path.cwd().resolve()
    directory = Path(run_path).resolve()
    if directory.parent != root / 'target/verification':
        raise ValueError('The Web overlap requires the checkout run directory')
    report = json.loads((directory / 'report.json').read_text())
    policy = json.loads((root / 'docs/agents/verification-policy.json').read_text())
    graph = report.get('graph') or (report.get('plan') or {}).get('graph')
    if not graph:
        raise ValueError('The retained verification graph is required')
    stages, concurrent = admission(policy, graph, report['profile'],
                                   os.environ.get('STORYOS_VERIFICATION_COMPARE'))
    prerequisite = policy['workflow']['complete_overlap']['prerequisite']
    steps = [json.loads(path.read_text()) for path in (directory / 'steps').glob('*.json')]
    if not any(item['stage'] == prerequisite and item['status'] == 'passed' for item in steps):
        raise ValueError('The release package must pass before Web children start')
    print('Web stage mode: ' + ('bounded overlap' if concurrent else 'serial'), flush=True)
    return run(stages, concurrent)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, TypeError) as error:
        sys.exit(f'{error}\n')
