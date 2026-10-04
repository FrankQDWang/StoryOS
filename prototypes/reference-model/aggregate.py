"""Build outcome counts from retained public runs. This is a report tool, not a harness test."""

from collections import Counter
import gzip
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORT = ROOT / 'docs/research/reference-model'
SCHEMAS = ROOT / 'generated/json-schema/storyos-public-release-1'

GROUPS = {
    'stage-1': ['createVolume', 'createChapter', 'updateVolume', 'updateChapter', 'deleteVolume', 'deleteChapter', 'setCurrentChapter', 'updateProject', 'archiveProject'],
    'stage-2': ['applyAuthorEdit', 'undoLatestAuthorAction'],
    'stage-3': ['acceptProposal', 'rejectProposalOperations', 'withdrawProposal', 'reopenWithdrawnProposal', 'reopenRejectedOperations', 'replanProposal'],
    'stage-4': ['takeOverProjectWriter'],
}


def variants(name):
    stem = ''.join('-' + c.lower() if c.isupper() else c for c in name)
    data = json.loads((SCHEMAS / (stem + '-response.schema.json')).read_text())['$defs']
    typename = name[0].upper() + name[1:] + ('Result' if name == 'takeOverProjectWriter' else 'Effect')
    for branch in data[typename]['oneOf']:
        kind = branch['properties']['kind']['const']
        reason = branch['properties'].get('reason')
        reasons = data[reason['$ref'].split('/')[-1]]['enum'] if reason else [None]
        for value in reasons:
            yield ':'.join(filter(None, [name, kind, value]))


def main():
    manifest = json.loads((REPORT / 'evidence-manifest.json').read_text())
    limits = json.loads((REPORT / 'reachability.json').read_text())
    counts, allocation = Counter(), Counter()
    differences, receipts = [], set()
    for filename in manifest['counted_evidence']:
        path = REPORT / filename
        data = json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_text())
        counts.update(data['coverage'])
        differences.extend(dict(evidence=filename, **d) for d in data['differences'])
        for call in data['trace']:
            response = call.get('response', {})
            receipt = response.get('receipt', {})
            identity = receipt.get('receipt_id')
            if not identity or identity in receipts:
                continue
            receipts.add(identity)
            name = receipt.get('command_kind')
            if name in ['createProject', 'updateProject', 'archiveProject']:
                effect = response.get('effect', {}).get('kind')
                success = (name == 'createProject' and 'project' in response) or effect == 'authoritative_applied'
                if success:
                    allocation[name + ':missing_action'] += int(receipt.get('author_action_sequence') is None)
    result = {}
    for stage, names in GROUPS.items():
        rows = []
        for name in names:
            for variant in variants(name):
                count = counts[variant]
                limit = limits.get(variant)
                rows.append(dict(outcome=variant, hits=count,
                    status='threshold_met' if count >= 20 else ('reachability_limit' if limit else 'GAP'),
                    reachability=limit))
        result[stage] = rows
    (REPORT / 'coverage.json').write_text(json.dumps(dict(stages=result, all_counts=dict(sorted(counts.items())),
        project_allocation_audit=dict(allocation), differences=differences), ensure_ascii=False, indent=2) + '\n')
    for stage, rows in result.items():
        print(stage, Counter(r['status'] for r in rows))
        for row in rows:
            if row['status'] == 'GAP':
                print(row['outcome'], row['hits'])


if __name__ == '__main__':
    main()
