# Representative Writing-Path Performance: Primary-Source Measurement Boundaries

## Status and authority

- Research date: 2026-07-30
- Scope: external primary-source constraints for Issue #76 measurement design
- Sources: web standards, MDN, Chrome/Chromium documentation, and PostgreSQL
  documentation
- Authority: evidence-supporting only

This note does not choose a StoryOS product limit, default, budget, service
level, retention rule, or recovery guarantee. It identifies what the cited
interfaces can measure, what their results can support, and which
extrapolations they cannot support. Any numeric observation produced by a
StoryOS apparatus remains an observation until the relevant normative owner
accepts or rejects it.

The PostgreSQL links below use the `current` documentation path, which resolved
to PostgreSQL 18 on the research date. A reproducible run must additionally
record and cite the exact server and client-tool version that it tested rather
than assuming that `current` will remain PostgreSQL 18.

## 1. Browser event timing and input-to-paint

### Primary sources

1. [W3C Event Timing API](https://www.w3.org/TR/event-timing/)
2. [W3C Paint Timing](https://www.w3.org/TR/paint-timing/)
3. [MDN `PerformanceEventTiming`](https://developer.mozilla.org/en-US/docs/Web/API/PerformanceEventTiming)

### What the sources support

- Event Timing considers a bounded event set that includes `keydown`,
  `beforeinput`, `input`, `compositionstart`, `compositionupdate`, and
  `compositionend`. It excludes untrusted events and continuous events such as
  `mousemove`, `pointermove`, `touchmove`, and `wheel`. A real Chinese or
  English input run can therefore observe eligible browser input and
  composition events, while a script-created event with `isTrusted == false`
  cannot supply Event Timing evidence.
- `PerformanceEventTiming.duration` is intended to measure from the estimated
  physical input time (`Event.timeStamp`) to the next rendering update of the
  associated document. `processingStart - startTime` isolates input delay,
  `processingEnd - processingStart` isolates synchronous event dispatch, and
  the remainder of `duration` includes the delay until the following rendering
  update.
- `duration` has 8 ms granularity. Ordinary `event` entries are exposed at a
  default threshold of 104 ms. An observer may request a lower threshold, but
  the specification clamps it to at least 16 ms.
- Registering an observer with a lower threshold affects future entries. The
  buffered event timeline still contains only entries at or above the default
  threshold; `first-input` is a special case and is reported regardless of the
  threshold. A benchmark that registers late can therefore silently censor
  fast non-first interactions.
- Paint Timing defines paint as the browser's rendering update and calls the
  timestamp a best-effort point in a complex rendering pipeline, typically as
  late as the user agent can observe (with frame submission to the operating
  system recommended).
- `interactionId` groups related event entries into interactions, but its
  numeric value is deliberately unsuitable as an exact interaction counter.
  Per-run counts must come from the benchmark's own explicit workload identity,
  not from arithmetic on `interactionId`.

### Measurement consequences

A reproducible input-to-paint probe should record at least:

- browser name and exact build, operating system, display refresh rate, power
  state, foreground/background visibility, viewport, and hardware;
- whether input came from a real operating-system IME, browser automation, or
  script construction, plus observed event type and `isTrusted`;
- observer registration time and options, including `durationThreshold` and
  whether `buffered` was used;
- raw `startTime`, `processingStart`, `processingEnd`, `duration`,
  `interactionId`, event type, target identity, and workload step identity;
- warm-up samples separately from retained samples, and the full retained
  distribution rather than only a mean;
- an independent semantic assertion that the expected editor text/state was
  present, because the timing entry does not perform that assertion.

The 16 ms minimum threshold means Event Timing alone produces a
left-censored distribution for ordinary interactions faster than that
threshold. The 8 ms granularity also makes fine differences inside one bucket
unresolvable. Report censored counts and interval-valued observations rather
than treating missing entries as zero or rounded values as exact.

### What the sources do not support

- They do not prove that the expected Chinese or English text was correct,
  durably journaled, authoritative in Core, acknowledged, or converged.
- They do not identify which exact pixels changed. “Next paint” is a document
  rendering boundary, not a content-specific paint assertion.
- They do not guarantee a physical display scan-out timestamp or include human
  perception and IME candidate-selection time.
- They do not turn synthetic composition dispatch into real OS IME evidence.
- They do not justify a StoryOS latency target, percentile, SLA, or universal
  browser result from one browser build, machine, refresh rate, or sample.

## 2. IndexedDB transaction completion and durability

### Primary sources

1. [W3C Indexed Database API 3.0, transactions](https://www.w3.org/TR/IndexedDB/#transactions)
2. [W3C Indexed Database API 3.0, committing a transaction](https://www.w3.org/TR/IndexedDB/#commit-transaction)
3. [MDN `IDBTransaction.complete`](https://developer.mozilla.org/en-US/docs/Web/API/IDBTransaction/complete_event)
4. [MDN `IDBTransaction.durability`](https://developer.mozilla.org/en-US/docs/Web/API/IDBTransaction/durability)

### What the sources support

- IndexedDB reads and writes occur in transactions. Requests within one
  transaction execute in request order, and commit must atomically write all
  transaction changes or none of them.
- A successful individual request is not evidence that its enclosing
  transaction committed. The specification explicitly directs callers to wait
  for the transaction's `complete` event because the transaction may still
  fail after a request's `success` event.
- An inactive transaction auto-commits after all its requests and their
  results have been handled and no new request was added. An explicit
  `commit()` starts commit without waiting for request results to be handled by
  script.
- A `complete` event fires only after the transaction has successfully
  committed. A journal-duration measurement can therefore end at
  transaction-level `complete`, provided it separately records `abort` and
  `error` instead of dropping failed samples.
- Durability is a transaction option and a user-agent hint:
  - `strict` permits success only after the user agent verifies outstanding
    changes were written to persistent storage;
  - `relaxed` permits success once changes were written to the operating
    system, without later verification;
  - `default` delegates the choice to the user agent's storage-bucket default.
- The specification describes a typical `strict` implementation as flushing
  operating-system I/O buffers before `complete`, while also warning of latency
  and battery cost. The terms “hint”, “may”, and “typical implementation” are
  material qualifications.

### Measurement consequences

For every journal sample, the apparatus should preserve:

- exact browser build and whether the requested durability option is supported;
- requested durability and the transaction's observed `durability` property;
- transaction scope, operation and record counts, serialized application
  payload bytes, and whether indexes were updated;
- monotonic start, request-success (if useful), transaction-complete, abort,
  and error observations;
- warm/cold state, outstanding concurrency, origin persistence state, and
  available storage estimate;
- a separately named failure injection: renderer/process termination, browser
  termination, operating-system crash, or power loss are not interchangeable.

The transaction `complete` event is the correct browser-visible durability
boundary for that transaction. A benchmark must not label request `success`,
an awaited request wrapper, a DOM update, or a timer as “journal durable”.

### What the sources do not support

- `complete` with `default` or `relaxed` does not prove that bytes reached
  physical media before power loss.
- Even `strict` is specified as a hint to the user agent. It gives stronger
  confidence, not a hardware-independent guarantee against every storage
  controller, operating-system, device, or power failure.
- Transaction atomicity does not protect the origin from quota exhaustion,
  browser eviction, explicit user clearing, application deletion, or a bug
  that writes the wrong logical record.
- A fake Core or a local browser journal does not establish production Core,
  PostgreSQL, network acknowledgement, or end-to-end recovery performance.
- One browser's completion latency does not justify a cross-browser durability
  budget.

## 3. Controlled network emulation and real-network evidence

### Primary sources

1. [Chrome DevTools: custom network throttling](https://developer.chrome.com/docs/devtools/settings/throttling#network)
2. [Chrome DevTools: Network features reference](https://developer.chrome.com/docs/devtools/network/reference#throttling)
3. [Chrome DevTools Protocol, Network domain](https://chromedevtools.github.io/devtools-protocol/tot/Network/)
4. [Chrome DevTools Protocol stability policy](https://chromedevtools.github.io/devtools-protocol/)

### What the sources support

- Chrome DevTools custom profiles can control named download and upload rates
  and latency. Current DevTools also exposes packet loss, queue length, and
  reordering controls for packet-related/WebRTC testing.
- Chrome DevTools can emulate an offline state and predefined or custom slow
  connections. These are controlled, repeatable browser test conditions when
  the exact profile and browser build are recorded.
- In the DevTools Protocol, the older
  `Network.emulateNetworkConditions` command is deprecated. The newer
  experimental split uses `emulateNetworkConditionsByRule` for matched request
  conditions and `overrideNetworkState` for `navigator.onLine` and
  `navigator.connection`. Request throttling and page-visible navigator state
  are therefore distinct controls.
- The protocol defines latency, aggregate download/upload throughput, offline,
  and connection-type inputs. The newer rule API can associate a rule ID with
  requests, which can make applied-condition evidence inspectable.
- Chrome's tip-of-tree protocol changes frequently and carries no backward
  compatibility guarantee. A reproducible harness must bind the browser build,
  negotiated protocol, command names, and full condition values.

### Measurement consequences

A controlled synthetic network run should record:

- exact browser build, DevTools Protocol version, commands, rule identifiers,
  and all profile values with units;
- whether cache was disabled, whether a new connection was established, the
  protocol in use, and whether a service worker handled the request;
- requested versus observed offline/navigator state;
- per-request timing plus the application's submission, acknowledgement,
  Event, reconnect, resync, and convergence timestamps;
- both directions independently; a single “latency” label is insufficient for
  asymmetric upload/download or acknowledgement tests;
- full distributions and failure counts for each condition, with warm-up
  isolated.

The following is a methodological inference from the bounded controls above,
not a claim made by Chrome documentation: a synthetic profile is suitable for
repeatable mechanism comparisons, but it cannot represent the joint
distribution of real DNS, connection setup, TLS, routing, congestion,
carrier/Wi-Fi behavior, server queuing, regional distance, device scheduling,
and transient loss. A controlled-cloud claim therefore needs separately
labelled real endpoint runs from disclosed locations and networks. Synthetic
and real-network samples must not be pooled as if they had the same sampling
process.

### What the sources do not support

- Selecting a “4G” preset does not prove representative performance for all
  real 4G users, locations, carriers, or radios.
- Emulated offline does not by itself prove recovery from a physical interface
  transition, captive portal, DNS failure, suspended device, half-open
  connection, server outage, or process crash.
- A request delay does not prove Receipt durability, Event ordering,
  convergence, or recovery correctness; the application must instrument those
  boundaries.
- Tip-of-tree CDP behavior is not a stable cross-browser contract.
- A controlled local result does not justify a production controlled-cloud
  latency or availability commitment.

## 4. PostgreSQL relation, index, WAL, backup, and restore size

### Primary sources

1. [PostgreSQL database-object size functions](https://www.postgresql.org/docs/current/functions-admin.html#FUNCTIONS-ADMIN-DBSIZE)
2. [PostgreSQL WAL control and LSN-difference functions](https://www.postgresql.org/docs/current/functions-admin.html#FUNCTIONS-ADMIN-BACKUP)
3. [PostgreSQL WAL internals](https://www.postgresql.org/docs/current/wal-internals.html)
4. [PostgreSQL directory-listing functions](https://www.postgresql.org/docs/current/functions-admin.html#FUNCTIONS-ADMIN-GENFILE)
5. [PostgreSQL routine vacuuming](https://www.postgresql.org/docs/current/routine-vacuuming.html)
6. [PostgreSQL `pg_basebackup`](https://www.postgresql.org/docs/current/app-pgbasebackup.html)
7. [PostgreSQL backup manifest file objects](https://www.postgresql.org/docs/current/backup-manifest-files.html)
8. [PostgreSQL `pg_verifybackup`](https://www.postgresql.org/docs/current/app-pgverifybackup.html)
9. [PostgreSQL `pg_dump`](https://www.postgresql.org/docs/current/app-pgdump.html)
10. [PostgreSQL `pg_restore`](https://www.postgresql.org/docs/current/app-pgrestore.html)

### Relation and index attribution

PostgreSQL's size functions return raw byte counts:

- `pg_relation_size(relation[, fork])` measures one relation fork; the default
  is the main fork.
- `pg_table_size(table)` includes the table, TOAST, free-space map, and
  visibility map, but excludes indexes.
- `pg_indexes_size(table)` totals the indexes attached to the table.
- `pg_total_relation_size(table)` equals table size plus attached-index size.
- `pg_database_size(database)` measures the database's total disk use.
- `pg_column_size(value)` measures an individual stored value and reflects
  compression when applied to a table value.

These functions support before/after byte deltas per concrete table and index.
They do not know StoryOS semantic categories. Attribution to Revision, Receipt,
Event, checkpoint, or other record classes requires an explicit, versioned
mapping from each category to its concrete relations and indexes. Shared
relations need a disclosed allocation method; otherwise only the whole
relation can be claimed as measured.

Partitioned data requires enumeration of the actual partition tree and
per-relation measurements. A parent relation's size is not evidence that all
child storage and child indexes were included. Raw bytes should remain the
machine-readable unit; `pg_size_pretty` is presentation only and uses powers of
two.

### WAL generated versus WAL retained

- An LSN is a byte position in the WAL stream. `pg_wal_lsn_diff(end, start)`
  returns the number of bytes between two WAL locations.
- `pg_current_wal_insert_lsn()` is the logical insertion end,
  `pg_current_wal_lsn()` is the write end, and
  `pg_current_wal_flush_lsn()` is the location known to have reached durable
  storage. The chosen pair must be recorded and used consistently.
- `pg_ls_waldir()` lists ordinary files currently present in `pg_wal`, with
  each file's size. That is a retained-directory footprint, not the same
  quantity as WAL bytes generated by a workload.
- WAL is cluster-wide. A before/after LSN difference also includes concurrent
  sessions and background work. A workload attribution needs an isolated
  cluster or an explicitly measured control/background envelope.
- With `full_page_writes`, the first modification of a page after a checkpoint
  can log the entire page. Checkpoint placement and warm-up can therefore
  materially change WAL deltas. The apparatus must record checkpoint and
  configuration state rather than treating runs at different checkpoint
  phases as exchangeable.
- WAL segment files are normally fixed-size and can be recycled. A
  `pg_wal` directory size delta can be zero while the LSN advances, or can jump
  by a segment while the workload generated less than a segment. It must not be
  substituted for workload WAL generation.

### Checkpoint, vacuum, and compaction observations

PostgreSQL standard `VACUUM` makes dead-row space reusable but usually does not
return it to the operating system. `VACUUM FULL` rewrites the table, can return
space, takes an exclusive lock, and needs temporary space for the new copy.
Consequently:

- unchanged relation-file bytes after ordinary vacuum do not prove that no
  space became reusable;
- reduced bytes after a rewriting operation must be reported with its lock,
  duration, temporary peak, index changes, and workload pause;
- a one-time post-rewrite minimum is not a steady-state growth rate;
- no PostgreSQL source here chooses which operation should implement a StoryOS
  checkpoint or compaction policy.

### Backup and restore observations

- `pg_basebackup` produces a whole-cluster physical backup. Full backups copy
  cluster files; format, compression, tablespaces, and WAL inclusion change
  the output bytes. A measured size must state plain versus tar, compression
  method/level/location, tablespace mapping, and `--wal-method`.
- A base-backup manifest lists file size, modification time, and optional
  checksum, but excludes included WAL files and the manifest itself. Summing
  manifest file sizes is therefore not the complete delivered-backup size.
- `pg_verifybackup` checks a base backup against its manifest, but PostgreSQL
  explicitly says that this cannot perform every check a running server will
  perform and that a test restore is still required.
- `pg_dump` creates a consistent logical export of one database. Its script and
  archive formats encode reconstruction commands/data rather than physical
  relation layout. `pg_restore` rebuilds from non-plain archives.

A defensible backup/restore record should retain actual output bytes, elapsed
and CPU time, peak scratch space, format and compression settings, WAL
inclusion, manifest and verification results, then restore into a fresh
disclosed target and measure the restored relations/indexes/database plus
application-level identities and counts. Logical dump bytes, physical backup
bytes, source relation bytes, and restored relation bytes are different
quantities and should remain separate columns.

### What the sources do not support

- A row payload's UTF-8 or JSON byte length does not predict its PostgreSQL
  relation footprint; tuple/page overhead, TOAST, indexes, compression, free
  space, and dead versions intervene.
- A total database delta does not attribute bytes to a StoryOS record class.
- A single LSN delta under concurrent activity does not isolate StoryOS WAL.
- `CHECKPOINT` does not mean that WAL generation or retained `pg_wal` footprint
  will immediately shrink by the same amount.
- Ordinary `VACUUM` does not establish an on-disk compaction ratio.
- Backup-manifest verification alone is not a successful restore.
- A successful restore does not by itself establish a recovery-time objective
  or production backup policy.

## 5. Browser quota, usage estimates, persistence, and eviction

### Primary sources

1. [WHATWG Storage Standard, usage and quota](https://storage.spec.whatwg.org/#usage-and-quota)
2. [WHATWG Storage Standard, storage pressure](https://storage.spec.whatwg.org/#storage-pressure)
3. [MDN Storage API](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API)
4. [MDN storage quotas and eviction criteria](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria)

### What the sources support

- Storage usage is an implementation-defined rough estimate. User agents can
  use deduplication, compression, and privacy padding, so
  `navigator.storage.estimate().usage` is not an exact physical byte count.
- Storage quota is an implementation-defined conservative estimate. The
  standard deliberately does not expose exact device free space, partly for
  privacy.
- Browser-managed origin data is best-effort by default. Under storage
  pressure, a user agent should clear best-effort local storage buckets,
  ideally in a way that least affects the user.
- A granted persistent-storage permission changes the default bucket to
  persistent. Persistent data is protected from automatic best-effort
  eviction, but can still be cleared by the user; under continuing pressure,
  the user agent can ask the user to clear remaining persistent buckets.
- Browser quotas and persistence-grant behavior vary. Private browsing can use
  different quotas and usually removes its stored data when the private
  session ends.
- A write that exceeds the applicable quota can fail with
  `QuotaExceededError`.

### Measurement consequences

Browser storage evidence should preserve three distinct byte measures:

1. application-level serialized journal records and indexes, computed by a
   versioned serializer;
2. the origin-level `navigator.storage.estimate()` result, labelled an estimate
   and sampled before and after;
3. any browser-profile filesystem observation, labelled
   implementation-specific and not attributed to the journal unless the
   browser layout and isolation make that attribution auditable.

Each sample should also record exact browser/profile, origin, browsing mode,
`navigator.storage.persisted()` state, the result of any `persist()` request,
other storage APIs used by the origin, device total/free-space context where
the test environment can disclose it, and every quota error. A fresh isolated
profile is necessary for a clean synthetic delta; a normal author profile is a
different population and should be reported separately.

### What the sources do not support

- `estimate().usage` does not prove IndexedDB physical bytes, journal-only
  bytes, or bytes that will remain after compaction.
- `estimate().quota` is not a portable fixed capacity and is not a promise that
  every write below that number will succeed.
- One fill-to-failure experiment does not establish a cross-browser or
  cross-device quota.
- A granted persistence request is not authorization to omit a server-side
  recovery path; user clearing and application failures remain possible.
- Rare observed eviction on one machine does not prove that eviction cannot
  happen.
- Incognito/private-mode behavior cannot be generalized to the normal author
  profile, or vice versa.

## 6. Cross-cutting evidence rules derived from the sources

These are measurement-method consequences, not StoryOS normative decisions:

| Evidence kind | It can support | It cannot support alone |
| --- | --- | --- |
| `PerformanceEventTiming` | eligible trusted-event delay, dispatch time, and thresholded/rounded next-render duration on the recorded browser | correct text, exact pixel visibility, journal durability, Core convergence, or universal latency |
| IndexedDB transaction `complete` | browser-visible atomic transaction commit under the recorded durability hint | physical-media survival for every failure, quota survival, or server durability |
| Chrome network emulation | repeatable behavior under the exact recorded synthetic controls | a real user/network distribution or production-cloud guarantee |
| PostgreSQL relation-size functions | byte footprint of named relations/indexes at a sampled instant | semantic attribution without a relation map, logical live bytes, or steady-state growth |
| WAL LSN difference | WAL-stream bytes between two recorded positions | workload-only WAL in a shared cluster or current retained-directory bytes |
| `pg_ls_waldir()` | current ordinary-file footprint of `pg_wal` | WAL generated by one workload |
| backup output plus manifest/verification | artifact bytes and a bounded integrity check under the recorded format | successful restore or recovery objective |
| test restore | recoverability and measured restore behavior in the recorded environment | all future environments or a production recovery guarantee |
| `navigator.storage.estimate()` | origin-level rough usage/quota estimates | exact IndexedDB or journal bytes, fixed capacity, or eviction immunity |

Every result using these sources should preserve environment, exact versions,
workload identity, sample count, raw observations, exclusions/failures,
distribution or interval, reproducible command or harness entry point, and
limitations. Synthetic, directly instrumented, and modelled/projected values
must remain visibly separate. Means must not replace tail distributions, and a
missing thresholded browser event must not be encoded as a zero-duration
sample.
