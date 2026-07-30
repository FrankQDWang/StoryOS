# Representative Writing-Path Performance and Storage-Growth Envelope

## Status and authority

- Research date: 2026-07-30
- Evidence owner:
  [Measure the Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76)
- Contract revision: `canonical-map-2026-07-23`
- Execution baseline:
  `main@8ad66d95790189f04e9fce634b76b57385a13ee2`
- Baseline tree: `d5d1ebf737a1f3dab7d4993dcceb0ef85ff03080`
- Authority: research evidence and owner-routed recommendations only

This report is not a StoryOS performance specification, production
implementation, service-level agreement, retention rule, recovery promise, or
capacity commitment. It introduces no product Rust or TypeScript. Every value
remains non-normative until the single owner named in the recommendation matrix
accepts or rejects it in that owner's tracked contract.

## Executive conclusion

The evidence establishes a reproducible lower-risk measurement baseline, not a
production envelope:

- a real Chrome 150 browser instrument completed trusted English/Unicode input,
  strict IndexedDB transactions, offline journaling, 200 KB chapters, a 2.4 MB
  120-chapter synthetic novel, chapter switching, cold reload, and delayed
  acknowledgement/Event convergence;
- the frozen Issue #69 apparatus remains the only real operating-system Chinese
  Pinyin and editor recovery mechanism evidence;
- isolated PostgreSQL 16.14 instruments completed under 4-vCPU/4-GiB and
  2-vCPU/2-GiB local container caps, attributed heap/index bytes, measured WAL
  deltas, demonstrated ordinary-vacuum versus rewrite behavior, and validated
  six logical dump/restore count-and-sequence identities;
- the measured synthetic schema projects about `92.8 MiB` for 20,000 annual
  commands and `278.4 MiB` for 60,000 annual commands across four small envelope
  families. Those figures exclude every unmodelled table, WAL retention,
  backups, bloat, archives, and the browser journal;
- a 4 KiB payload sensitivity probe ranged from `299.008` to `5,791.744`
  total-relation bytes per record depending on compressibility. Payload shape,
  TOAST, indexes, and future schema therefore dominate any single linear
  capacity estimate;
- the data can define candidate experiment bands. It cannot yet justify a
  production latency target, timeout, compaction cadence, replay window,
  resource minimum, RPO, or RTO.

## Evidence classes

| Class | Meaning in this report | Examples |
| --- | --- | --- |
| Instrumented observation | A real named interface was executed and timed or sized on the recorded environment | Chrome trusted `insertText`, IndexedDB transaction `complete`, PostgreSQL size functions, LSN difference, `pg_dump`/`pg_restore` |
| Controlled synthetic | A disposable mechanism substitute or artificial condition was executed | script-created composition events, delayed loopback responses, fake acknowledgement/Event pairing, local CPU/memory-capped container |
| Modelled projection | Measured per-record values were multiplied by an explicit assumed workload | 20,000- and 60,000-command annual storage projections |
| Frozen mechanism evidence | A prior disposable experiment is reused without claiming a new population | Issue #69 real macOS Pinyin, crash/reload/reconnect/resync, long-session correctness |

These classes are never pooled into one distribution.

## Environment and reproducibility

The current instrument ran on macOS 15.6.1, Apple M4, 10 logical CPUs, 32 GiB
host memory, Chrome `150.0.7871.187`, Python `3.14.4`, Docker server `29.4.0`,
and PostgreSQL `16.14` from image ID
`sha256:88777d7cb0db2e0160fcf36277608f42920e517409316e2dbeafe6c844cb08ca`.
The Chrome run exposed a 4.1 GiB JavaScript heap limit and recorded one final
used-heap observation; it did not measure peak browser or container resident
memory.

The workload, apparatus, environment, raw rows, summary, and SHA-256 identities
are under [`evidence/issue-76`](evidence/issue-76/README.md). Reproduce from a
running Docker-compatible host with:

```sh
cd docs/research/evidence/issue-76/apparatus
./run.sh
```

That command creates a new observation population and finishes with fresh-mode
raw/summary/schema/restore/manifest validation; it does not validate this
frozen report or its one-time correction provenance. Audit the checked-in
bundle and report non-destructively with the default frozen mode:

```sh
python3 verify-evidence.py
```

The apparatus uses a fixed workload seed, but wall-clock timings, browser quota
estimates, randomized low-compressibility payloads, and dump bytes will vary.
The raw files, not the rounded tables below, are authoritative observations.
Percentiles use the nearest-rank method. Browser groups retain 20-40 measured
samples after five warm-ups where applicable; PostgreSQL profiles retain three
independent container samples. With `n=30`, p99 is the maximum, not a stable
population-tail estimate. With `n=3`, PostgreSQL p95 is also the maximum.

`summary.json` now carries 71 raw-derived report fragments. The default frozen
audit rebuilds the complete summary, then checks those fragments against every
checked-in browser and PostgreSQL measurement table and associated numeric
narrative in this report. Fresh mode deliberately omits report and historical
correction-provenance checks, so it cannot approve this frozen report. Other
numeric text is classified separately as a workload input, environment
identity, frozen Issue #69 fact, existing contract bound, identifier, or
explicitly non-normative recommendation band.

External measurement boundaries and primary sources are recorded separately in
[Representative Writing-Path Performance: Primary-Source Measurement Boundaries](representative-writing-path-performance-primary-sources.md).

## Workload

| Surface | Workload and retained sample count | Evidence class |
| --- | --- | --- |
| English/Unicode input | Alternating trusted Playwright `keyboard.insertText` into 10/50/200 KB `contenteditable`; 5 warm-ups then 30 samples per chapter size | Instrumented browser automation; Unicode insertion is not OS IME |
| Chinese composition | 30 script-created composition boundaries plus trusted text insertion; Issue #69 supplies three real macOS Pinyin compositions | Controlled synthetic plus frozen real-IME mechanism evidence |
| Browser journal | One strict IndexedDB transaction per input; 1,000 sequential 33-byte UTF-8 patches with a full checkpoint every 100 intents | Instrumented browser API with synthetic records |
| Novel navigation | 120 decimal-text chapters × 20 KB = 2.4 MB; 40 switches and 20 page reload/cold opens | Controlled synthetic |
| Network/offline | 30 samples for each 0/5, 10/30, 30/10, 30/100, 100/30, 75/250, and 250/75 ms delayed acknowledgement/Event pair; 30 unreachable-loopback journal samples | Controlled delayed loopback, not network emulation or cloud |
| Recovery | Issue #69 gap, replay-floor, crash-before-commit, crash-after-commit, reload, writer-fence, reconnect/resync, and long-session cases | Frozen disposable mechanism evidence |
| PostgreSQL growth | 1K/10K/50K records per Revision, Receipt, Activity Event, and Application Wire Record family; compressible and low-compressibility payload variants | Controlled synthetic schema |
| Payload sensitivity | 10K rows at 4 KiB, both highly compressible and low-compressibility | Controlled synthetic |
| Replay/checkpoint | 50K Event scan versus 5K Event tail after one checkpoint | Controlled synthetic |
| Compaction | 50K 2-KiB Event payloads; delete 90%, ordinary vacuum, then `VACUUM FULL` | Controlled synthetic |
| Backup/restore | Three custom-format logical dump/restores per resource profile, restored into a fresh database with count and sequence-sum equality | Instrumented logical PostgreSQL path |
| Annual storage | 20K and 60K commands/year multiplied by observed four-family total-relation bytes | Modelled projection |

The 200 KB chapter, 2.4 MB novel, and 1,000-intent session are representative
test points, not hard maxima or a claim about all novels. The apparatus does not
contain Tiptap, StoryOS Core, a product protocol, a production database schema,
or a real controlled-cloud endpoint.

## Browser observations

### Input, durability, and navigation

| Path | n | p50 | p95 | p99/max | Interpretation |
| --- | ---: | ---: | ---: | ---: | --- |
| 10 KB input → double `requestAnimationFrame` | 30 | 11.6 ms | 14.3 ms | 14.8 ms | Frame-boundary surrogate |
| 50 KB input → double `requestAnimationFrame` | 30 | 11.2 ms | 13.0 ms | 13.3 ms | Frame-boundary surrogate |
| 200 KB input → double `requestAnimationFrame` | 30 | 4.7 ms | 7.4 ms | 7.5 ms | Non-monotonic phase/cache evidence |
| 10 KB strict IndexedDB `complete` | 30 | 0.2 ms | 0.3 ms | 1.2 ms | Browser-visible strict-hint transaction completion |
| 50 KB strict IndexedDB `complete` | 30 | 0.2 ms | 0.8 ms | 1.0 ms | Same |
| 200 KB strict IndexedDB `complete` | 30 | 1.0 ms | 1.4 ms | 1.4 ms | Same |
| Synthetic composition → double rAF | 30 | 10.5 ms | 13.5 ms | 14.6 ms | Not OS IME |
| Synthetic composition journal `complete` | 30 | 0.2 ms | 0.8 ms | 0.9 ms | Not OS IME |
| Offline journal `complete` | 30 | 0.2 ms | 0.5 ms | 0.8 ms | Unreachable loopback, journal still local |
| 20 KB chapter switch → double rAF | 40 | 15.7 ms | 16.7 ms | 17.0 ms | IndexedDB lookup plus replacement |
| 20 KB cold reload/open → double rAF | 20 | 9.0 ms | 12.9 ms | 13.0 ms | Warm browser/profile, page reload |

`input_to_double_raf_ms` is not a production input-to-paint number. The browser
recorded no ordinary Event Timing entries: the API censors interactions below
its 16 ms minimum observer threshold, and double rAF is sensitive to frame
phase. The faster 200 KB result confirms that this instrument cannot infer
chapter-size scaling. It also does not identify changed pixels or display
scan-out. A product-backed run must register Event Timing before input, report
censored counts, bind trusted event identity to an exact semantic text
assertion, and add an independently attributable paint/mutation boundary.

The Issue #69 real macOS Pinyin checkpoint observed three completed composition
units producing the exact 18-byte final text. Journal completion ranged
`2.7-4.6 ms`; Receipt settlement ranged `3.4-10.4 ms`; native undo reversed the
three compositions; reload recovered the exact post-undo Head. Three
compositions are mechanism evidence, not an IME latency distribution.

### Acknowledgement, Event, offline, and convergence

| Slowest delayed channel | Pair order | n | convergence p50 | p95 | max |
| ---: | --- | ---: | ---: | ---: | ---: |
| 5 ms | loopback acknowledgement first | 30 | 6.4 ms | 6.8 ms | 7.0 ms |
| 30 ms | acknowledgement first | 30 | 31.9 ms | 32.7 ms | 33.3 ms |
| 30 ms | Event first | 30 | 31.8 ms | 32.6 ms | 32.7 ms |
| 100 ms | acknowledgement first | 30 | 102.9 ms | 103.8 ms | 103.8 ms |
| 100 ms | Event first | 30 | 102.3 ms | 103.6 ms | 104.1 ms |
| 250 ms | acknowledgement first | 30 | 252.7 ms | 254.2 ms | 255.9 ms |
| 250 ms | Event first | 30 | 252.6 ms | 253.4 ms | 253.6 ms |

The measured pair converges only after both responses, and configured-delay
overhead ranged `0.3-5.9 ms` in this run. This proves that the local projection
can be tested independently of delayed settlement and that order must not be
assumed. It does not measure DNS, TLS, loss, bandwidth, server queueing,
PostgreSQL commit, Core behavior, reconnect, or a real Event stream. No timeout
or cloud latency value can be derived from it.

Issue #69 separately froze exact mechanism outcomes for Event gaps,
replay-floor misses, Snapshot resynchronization, pre-commit and
post-commit/pre-response crashes, Receipt-first reload reconciliation,
writer-generation fencing, and reconnect/resync. Its fake Core establishes
state-machine behavior only, never production availability or performance.

### Browser journal growth

The 1,000-intent run wrote one actual 33-byte UTF-8 patch per intent and a full
text checkpoint every 100 intents:

- attributable serialized journal records: `274,373 bytes`;
- hypothetical before-plus-after full-document copies: `33,000,000 bytes`;
- amplification avoided by the sampled patch/checkpoint representation:
  `120.3×` relative to that deliberately naive comparator.

The ratio is not a production database compression ratio. Only one checkpoint
cadence and one patch size were sampled. `navigator.storage.estimate().usage`
fell from `288,658` to `136,689` bytes after the preceding clear/rewrite cycle,
which demonstrates why the rough origin-level estimate cannot be attributed as
physical journal growth.

The comparator was deterministically corrected after independent review:
[`browser-evidence-correction.json`](evidence/issue-76/browser-evidence-correction.json)
binds the original `main@61593be` raw SHA-256, formula, prior and corrected
values, corrected line, and an aggregate SHA-256 for the other 421 byte-identical
rows. No timing observation was resampled.

Issue #69's different 240-intent long session retained `276,265` serialized
bytes before settlement, then coalesced all 240 one-character intents into one
`6,079`-byte command and converged with no lost, duplicated, or reordered
characters. That fixed point supports bounded coalescing as a mechanism, not
240 as a default.

## PostgreSQL observations

### Relation and index attribution

The synthetic tables bind one owner/project pair and contain a record ID,
project sequence, SHA-256 digest, payload, primary key, and one project-sequence
index. At 50K rows, the observed per-table totals were identical across the two
resource caps:

| Record family and sampled payload | Heap | Attached indexes | Total | Total/record |
| --- | ---: | ---: | ---: | ---: |
| Revision, 1,024 B | 58,515,456 B | 5,947,392 B | 64,503,808 B | 1,290.076 B at 50K |
| Receipt, 640 B | 40,960,000 B | 5,947,392 B | 46,948,352 B | 938.967 B at 50K |
| Activity Event, 768 B | 45,514,752 B | 5,947,392 B | 51,503,104 B | 1,030.062 B at 50K |
| Application Wire Record, 1,024 B | 58,515,456 B | 5,947,392 B | 64,503,808 B | 1,290.076 B at 50K |

The summary's conservative observed upper slopes across 1K/10K/50K are
`1,368.064`, `1,015.808`, `1,114.112`, and `1,368.064` bytes/record
respectively. Compressible and low-compressibility variants collapse to the
same size at these sub-TOAST payloads; this is not proof that payload shape is
irrelevant.

At 4 KiB, total relation bytes per row ranged from `299.008` for highly
compressible values to `5,791.744` for low-compressibility values. The
`19.37×` span includes TOAST and attached indexes and is the stronger
sensitivity warning.

### Modelled annual growth and WAL

Multiplying only the four conservative small-envelope slopes gives:

| Assumed annual author commands | Revision | Receipt | Event | Wire record | Four-family total |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 20,000 | 26.09 MiB | 19.38 MiB | 21.25 MiB | 26.09 MiB | 92.81 MiB |
| 60,000 | 78.28 MiB | 58.13 MiB | 63.75 MiB | 78.28 MiB | 278.44 MiB |

This is a dimensional model, not a forecast. It assumes one record in each
family per command, linear growth, and the synthetic indexes. It excludes
project/author tables, current-state materializations, payload side tables,
snapshots, checkpoints, proposals, runs, tools, models, migrations, dead
tuples, fill factor, WAL retention, backups, archives, browser data, and
multiple command/Event cardinalities.

The isolated cluster generated deterministic LSN deltas under
`fsync=on`, `full_page_writes=on`, `synchronous_commit=on`, and an explicit
checkpoint after each loaded size:

| Records per each base family/shape | Additional 4 KiB probe | WAL delta |
| ---: | --- | ---: |
| 1,000 | no | 9,736,352 B |
| 10,000 | 10K rows × two shapes | 150,234,888 B |
| 50,000 | no | 486,853,520-486,853,576 B |

These deltas cover eight base tables because both payload shapes were loaded,
plus the stated probe at 10K. They are not WAL retained in `pg_wal`, and must
not be divided into a production per-command promise. The isolation removes
other application sessions, but checkpoint full-page images and PostgreSQL
background activity remain part of the recorded envelope.

### Checkpoint, compaction, and restore

Scanning 50K Events versus a 5K tail after a checkpoint produced:

| Profile | n | full scan p50/max | bounded-tail p50/max |
| --- | ---: | ---: | ---: |
| 4 vCPU / 4 GiB | 3 | 7.603/13.283 ms | 5.934/10.728 ms |
| 2 vCPU / 2 GiB surrogate | 3 | 7.600/7.918 ms | 6.510/6.705 ms |

The improvement is small relative to sample noise and synthetic sequential-scan
behavior. It cannot select a checkpoint cadence.

Deleting 90% of 50K low-compressibility 2-KiB Event payloads followed by
ordinary vacuum changed total relation bytes from `148,160,512` to
`148,176,896`; space became reusable but was not returned to the operating
system. `VACUUM FULL` rewrote it to `14,843,904` bytes, a `89.98%` reduction
from the pre-compaction Event file size. The 45,000 retained compaction-floor
records occupied another `7,626,752` bytes, so Event plus floor fell from
`148,176,896` to `22,470,656` bytes, an `84.84%` net reduction rather than
`89.98%`. Across three runs per profile, delete/floor work ranged
`181.5-761.2 ms`, rewrite ranged `136.7-227.6 ms`, loading generated
`184,967,424` WAL bytes, and compaction generated
`34,812,464-34,812,696` WAL bytes.
`VACUUM FULL` takes an exclusive lock and needs rewrite scratch space; the
apparatus did not capture peak scratch bytes or concurrent-author disruption.

After compaction, the source database was `82,484,247` bytes. Six custom-format
logical dumps were `60,268,999-60,272,182` bytes. Dump duration was
`2.667-3.216 s`; restore was `1.118-2.112 s`. Every fresh restore reproduced
exactly 50,000 Event rows and sequence sum `1,250,025,000`.

That proves only the sampled logical archive path. It is not the Foundation
Recovery Profile's physical base backup, continuous WAL archive, host-loss
restore, release-candidate Recovery Visibility Proof, zero acknowledged-loss
test, fifteen-minute RPO, or two-hour RTO evidence.

### Bounded resources

All 6 database samples completed inside the declared 4-vCPU/4-GiB and
2-vCPU/2-GiB container caps. This is an upper admission fact for this disposable
workload, not a peak-use measurement and not a minimum deployment size. Both
profiles ran on the same local virtualized host; the second is a
controlled-cloud surrogate, not controlled-cloud evidence. A real
personal-cloud envelope still needs disclosed endpoint, region, storage class,
network path, background load, cgroup telemetry, database settings, and peak
CPU/RSS/I/O/scratch observations.

## Existing hard bounds versus recommendations

The Foundation Recovery Profile already requires zero acknowledged-data loss
for ordinary process or power crashes, host/disk-loss RPO at most 15 minutes,
and RTO at most two hours. Those are existing hard safety/recovery contract
values owned by PostgreSQL storage and proved by the deterministic/recovery
gate owner. This report neither validates nor relaxes them.

Author-experience targets answer how responsive a writing path should feel.
Experimental tuning defaults answer how batching, checkpointing, or compaction
might be configured while testing. Neither category may override safety,
authority, journal-before-submission, Receipt/Event truth, recovery fencing, or
hard protocol limits.

## Recommendation and acceptance matrix

Each row has exactly one normative owner. A range is a candidate experiment or
planning band; it is not accepted merely by appearing here.

| Evidence | Non-normative recommended range or disposition | Single normative owner | Required accept/reject action |
| --- | --- | --- | --- |
| Strict IndexedDB p95 `0.3-1.4 ms`; real Pinyin samples `2.7-4.6 ms` | Trial a product-backed journal target band of p95 `≤5-10 ms` and p99 `≤10-20 ms` at 10/50/200 KB, with failures counted | [Web Editor Session owner #70](https://github.com/FrankQDWang/StoryOS/issues/70) | Reopen only if adopting; run real Tiptap/IME/browser-profile trials and write or reject one value in the session contract |
| 240 intents coalesced safely once; 1,000-intent run sampled checkpoint-every-100 only | Do not adopt `240` or `100`; sweep `64/128/256` intents, `64/128/256 KiB`, and `250/500/750 ms` idle independently | [Web Editor Session owner #70](https://github.com/FrankQDWang/StoryOS/issues/70) | Select only after lossless undo/reload/crash trials, or record that cadence remains implementation policy |
| Double-rAF p95 `7.4-14.3 ms`, switch p95 `16.7 ms`, cold reload p95 `12.9 ms`, all on a disposable editor | Trial product author-experience bands of input-to-visible p95 `≤16-33 ms`, chapter switch p95 `≤100-250 ms`, and cold open p95 `≤500-1,000 ms` | [AI-independent release owner #62](https://github.com/FrankQDWang/StoryOS/issues/62) | Measure trusted product interactions with censored Event Timing and semantic paint assertions; accept, tighten, widen, or reject per release stage |
| Delayed loopback convergence followed the slower channel plus `~0.3-5.9 ms`; no real network/Core | Reject any absolute timeout or cloud latency derived here; trial reporting as `slower observed channel + 25-100 ms` local processing budget only after real Core instrumentation | [Versioned Protocol owner #58](https://github.com/FrankQDWang/StoryOS/issues/58) | Decide whether a versioned convergence/timeout field is needed; validate on real route and never turn timeout into truth |
| Four small families model `4,866.048 B/command`; 4-KiB sensitivity implies up to `5,791.744 B/record` | Capacity sensitivity band: `5-24 KiB/command` for these four families only, before explicit headroom | [PostgreSQL storage owner #56](https://github.com/FrankQDWang/StoryOS/issues/56) | Replace synthetic shapes with exact schema/DTO corpus, add all relations/WAL/backups/bloat, and accept or reject a planning band—not a hard payload limit |
| 20K/60K commands model `92.8/278.4 MiB/year`, excluding major classes | Use `0.1-0.3 GiB/project-year` only as the small-envelope baseline term in a multi-term capacity model | [PostgreSQL storage owner #56](https://github.com/FrankQDWang/StoryOS/issues/56) | Add observed project cardinalities and recovery-chain footprint before declaring deployment capacity |
| 90% delete + ordinary vacuum did not shrink files; rewrite reduced sampled Event relation `89.98%` with exclusive lock | Do not adopt 90%; sweep live fractions `10/25/50%` and replay tails `1K/5K/10K` under concurrent load | [Retention/archival owner #64](https://github.com/FrankQDWang/StoryOS/issues/64) | Choose retention/replay/compaction semantics only after proving floors, archive chain, lock/scratch budget, and reader safety |
| Browser distributions have n=20-40; database distributions n=3 | For performance evidence, retain `100-300` samples per browser/environment path after warm-up; use at least `3-10` independent database/restore runs, while semantic gates remain deterministic | [Deterministic verification owner #60](https://github.com/FrankQDWang/StoryOS/issues/60) | Adopt or reject evidence-volume rules and required percentile method; never make wall-clock timing decide semantic correctness |
| 6 logical restores matched count and sequence sum; no physical/WAL recovery | Reject these timings as RPO/RTO evidence | [Deterministic verification owner #60](https://github.com/FrankQDWang/StoryOS/issues/60) | Require the storage owner's exact physical backup/WAL restore drill and Recovery Visibility Proof for the release profile |

The suggested sweep points are deliberately sparse logarithmic or staged
experiment inputs. They are not disguised defaults. A rejected recommendation
requires no contract edit beyond the owner recording its rejection when that
owner executes.

## Uncertainty and invalid extrapolations

The following claims are explicitly unsupported:

- that headless Chrome represents foreground human typing, all browsers,
  display refresh rates, power modes, accessibility stacks, or real IMEs;
- that double rAF is content paint, or that missing Event Timing entries are
  zero-duration interactions;
- that IndexedDB `strict` survives every operating-system, device-controller,
  power, quota, eviction, clearing, or application failure;
- that a fake Core, delayed loopback response, local Docker cap, or one machine
  is a production Core, network, controlled cloud, or service guarantee;
- that n=3 estimates a stable database tail, or n=30 estimates a stable p99;
- that the mean can replace the reported p50/p95/p99/max distributions;
- that serialized UTF-8, `navigator.storage.estimate()`, relation bytes, WAL
  bytes, dump bytes, and backup bytes are interchangeable;
- that the synthetic record cardinality, payload shapes, table/index layout, or
  annual command counts match production;
- that ordinary vacuum returns disk space, that `VACUUM FULL` is safe online,
  or that one 90% deletion selects a compaction policy;
- that a logical restore validates physical backup, continuous WAL, RPO, RTO,
  or all application invariants;
- that successful completion under resource caps measured peak CPU, memory,
  I/O, temporary space, or safe multi-project concurrency.

## Independent self-audit

- **Units:** raw storage is bytes and presentation uses binary MiB/GiB. The
  macOS apparatus `ru_maxrss` field is recorded as bytes, not mislabeled KiB.
  The requested 32-byte Chinese patch is explicitly recorded as 33 actual
  UTF-8 bytes.
- **Samples and tails:** warm-ups are excluded where declared; every table shows
  `n`; nearest-rank p95/p99 and maxima remain visible; no tail claim relies on a
  mean.
- **Cache/frame bias:** the non-monotonic 200 KB double-rAF result is retained
  and treated as disqualifying evidence for a size-scaling or production-paint
  claim.
- **IME and backend identity:** real Pinyin is cited only from Issue #69;
  synthetic composition and fake acknowledgement/Event/Core paths are labelled.
- **Cloud assumptions:** the 2-vCPU/2-GiB run is named a same-host surrogate and
  never a real controlled-cloud measurement.
- **Storage attribution:** journal serialization, rough browser usage, heap,
  indexes, total relation, WAL, dump, and modelled bytes remain separate.
- **Compression/TOAST:** a 4-KiB two-shape sensitivity probe prevents the
  sub-TOAST equality from being generalized.
- **Compaction and restore:** pre/post file bytes, lock-bearing rewrite,
  generated WAL, logical archive bytes, durations, and restored identities are
  separate observations.
- **Reproducibility:** workload, source, dependency lock, environment, raw rows,
  derivation, commands, and SHA-256 manifest are checked in. The verifier
  rebuilds all of `summary.json` from raw inputs, checks every machine-derived
  report fragment, and requires exact equality between the manifest key set and
  the includable file set; usernames, credentials, serial numbers, and
  `.reference/**` are excluded.

## Handoff

Issue #76 can close when this evidence bundle and report merge. Numeric adoption
remains with #70, #58, #56, #64, #62, and #60 in their existing serial order.
The next Wayfinder frontier is reported from the live map after merge; this
report does not claim or execute it.
