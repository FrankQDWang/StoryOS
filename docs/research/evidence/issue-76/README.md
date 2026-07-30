# Issue #76 evidence bundle

This directory freezes the disposable apparatus, raw observations, derived
summary, environment, and hashes used by
`representative-writing-path-performance-and-storage-growth-envelope.md`.
Nothing here is StoryOS product code, a production implementation, a normative
default, or a service-level promise.

## Reproduce

Prerequisites are macOS with Google Chrome at its standard application path,
Node.js 24 or later, Python 3.14 or later, a running Docker-compatible daemon,
and the pinned `postgres:16` image. Override the Chrome binary with
`STORYOS_BENCH_CHROME` when necessary.

```sh
cd docs/research/evidence/issue-76/apparatus
./run.sh
python3 hash-evidence.py
```

`run.sh` recreates the browser, PostgreSQL, environment, and summary files.
Each run intentionally replaces the prior successful observation set. The
hash command must be run last.

## Files

| File | Role |
| --- | --- |
| `apparatus/workload.json` | Versioned workload, sample counts, payload sizes, resource caps, and model inputs |
| `apparatus/browser-benchmark.mjs` | Headless-Chrome/IndexedDB and delayed-loopback instrument |
| `apparatus/correct-browser-evidence.py` | One-time audited correction of the frozen journal comparator without resampling 421 timing rows |
| `apparatus/distributions.py` | Shared nearest-rank distribution derivation used by the summary rebuild |
| `apparatus/evidence_files.py` | Canonical manifest inclusion rule shared by hashing and verification |
| `apparatus/postgres-benchmark.py` | Isolated PostgreSQL relation/index/WAL/compaction/dump/restore instrument |
| `apparatus/summarize.py` | Nearest-rank percentile and storage-model derivation |
| `apparatus/verify-evidence.py` | Non-mutating raw/summary/report, comparator, provenance, restore-identity, and complete-manifest checks |
| `browser-measurements.jsonl` | 422 raw browser observations, including one environment row |
| `browser-evidence-correction.json` | Source/corrected hashes, exact formula and values, target line, and aggregate identity for 421 unchanged rows |
| `postgres-relation-growth.csv` | Raw table-heap, attached-index, and total-relation bytes by phase |
| `postgres-operations.jsonl` | Raw operation duration, WAL, compaction, and logical restore observations |
| `environment.json` | Sanitized host, runtime, image, and workload identity |
| `summary.json` | Full raw-derived distributions, storage models, critical claims, and 71 exact report fragments |
| `MANIFEST.sha256` | SHA-256 identity of every included source and evidence file except itself |

The verifier rebuilds the entire summary from raw inputs, checks all 71
machine-derived report fragments after whitespace normalization, proves the
421-row frozen-browser preservation, and requires the manifest keys to equal
the complete includable file set before checking each digest.

The frozen Issue #69 browser/recovery evidence is not copied into this bundle.
It remains at `../issue-69/` and is cited by the report as fixed mechanism
evidence.

## Evidence boundaries

- Trusted `keyboard.insertText` observations are browser automation, not human
  typing. Synthetic composition dispatch is not a real operating-system IME.
- `input_to_double_raf_ms` ends at a double-animation-frame surrogate. It is not
  content-specific paint or display scan-out. The recorded Event Timing list is
  empty because ordinary entries below the API's threshold are censored.
- IndexedDB durations end at a transaction-level `complete` event requested
  with `durability: "strict"`; this remains a browser durability hint.
- Network rows use two delayed loopback HTTP responses and one unreachable
  loopback port. They are not real Internet or controlled-cloud observations.
- The two PostgreSQL profiles are CPU/memory caps on the same local virtualized
  Docker host. The 2-vCPU/2-GiB profile is a controlled-cloud surrogate only.
- PostgreSQL relations are synthetic record-family envelopes, not a StoryOS
  production schema. Logical `pg_dump`/`pg_restore` is not a physical backup,
  WAL replay, Recovery Visibility Proof, RPO, or RTO test.
- `navigator.storage.estimate()` is retained only as a rough origin-level
  estimate. Application-serialized bytes are the attributable journal measure.

## Comparator correction provenance

Independent review found that the original generator counted two copies of the
post-edit text while naming the field as pre-edit plus post-edit. The generator
now captures both lengths. To avoid mixing a new nondeterministic timing run
with the frozen report, the correction script transformed only the single
`journal_growth` row from `main@61593be`:

```sh
python3 apparatus/correct-browser-evidence.py \
  --source-main 61593be96d7915db13772b0dff5618c31a088dae
```

For 1,000 constant 33-byte patches, the corrected formula is
`33 × 1000² = 33,000,000` bytes. The provenance record proves the other 421
JSONL rows were byte-identical. A fresh full `run.sh` execution naturally
creates a new timing population; its final verifier is expected to reject stale
report fragments until every affected table and narrative is regenerated and
reviewed. It must not be compared as if it were the frozen run.
