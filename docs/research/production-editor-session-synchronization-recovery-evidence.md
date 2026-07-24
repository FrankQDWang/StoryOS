# Production Editor Session, Synchronization, and Recovery Evidence

## Status and authority

- Issue: [#69 — Validate Production Editor Session, Synchronization, and
  Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/69)
- Contract revision: `canonical-map-2026-07-23`
- Canonical baseline tested: `main@fa2978c6848ce0bf6d1b2807e464b27dea09fcde`
- Automated evidence: complete, `24/24` scenarios passed
- Real macOS Chinese Pinyin checkpoint: complete

This is experimental evidence, not a second editor/session contract. The
normative owners remain the current foundation documents and Issues #46 and
#70. The disposable page is an inspection instrument only; it is not a proposed
StoryOS editor UI, design direction, frontend dependency, or product
implementation. Its layout and styling have no product-design significance and
must not be inherited by a production editor.

## Decision

Use **completed semantic editor intent** as the correctness boundary and permit
**bounded idle coalescing** only while every target, ownership, expected Head,
writer generation, admission, editor contract, and undo-group binding remains
equal.

The evidence eliminates:

1. **one Core command per normalized ProseMirror transaction** because a single
   composition was split across four transactions and non-semantic transaction
   detail leaked into the durable boundary;
2. **fixed time-window batching** because it crossed composition, clipboard,
   structure, explicit-command, Scope, chapter, target, ownership, Head,
   writer-generation, admission, editor-contract, and undo-group boundaries.

One command per completed semantic intent remains a correct conservative
fallback. Bounded idle coalescing passed the same correctness gates and reduced
the representative six semantic intents to five command units; in the
continuous 240-intent probe it reduced 240 journaled intents to one Core
command. A production implementation must retain a maximum size/time bound and
flush at every listed boundary.

## What was separated in the trace

The trace never overloads “saved.” It names these distinct observations:

1. browser intent completed;
2. IndexedDB journal persistence started;
3. journal transaction durably completed;
4. non-authoritative local pending projection applied;
5. submission started;
6. Core transition committed;
7. Receipt durably settled;
8. Project Activity observed;
9. projection converged;
10. journal became GC-eligible;
11. journal entry was collected.

`OutcomeUnknown`, submission pause, Snapshot load, journal reconciliation,
Recovery Draft preservation, and submission reauthorization are separate
recovery stages.

Across all 2,100 automated trace records, the independent lifecycle gate
confirmed:

- every network submission followed durable journal persistence for all
  included intents;
- every journal collection followed a durable Receipt, projection convergence,
  and an explicit GC-eligibility observation;
- no trace exceeded the 5,000-record per-scenario bound.

## Disposable apparatus

The primary-source harness is retained at
[`codex/issue-69-editor-session-harness@0e188f3`](https://github.com/FrankQDWang/StoryOS/commit/0e188f3)
and is not part of this evidence branch.

Its executable source is
[`2c67c9abe126a17c6c02fb66ab63b843ecfb4a63`](https://github.com/FrankQDWang/StoryOS/commit/2c67c9abe126a17c6c02fb66ab63b843ecfb4a63).
It contains:

- a real Tiptap `3.27.3` contenteditable input surface;
- a real browser IndexedDB Local Edit Journal with a cross-tab atomic local
  order;
- immediate pending projection distinct from Core authority;
- an independently persisted and restartable fake Core representing document
  authority, Heads, Receipts, Author Action order, Project Activity, replay
  floor, and writer generation;
- controlled HTTP/Activity ordering, delay, acknowledgement loss, duplicates,
  gaps, replay-floor misses, pre-commit crashes, post-commit/pre-response
  crashes, Snapshot resynchronization, and writer takeover;
- deterministic JSONL, JSON, and CSV export.

Run it from that branch:

```sh
cd prototypes/editor-session-recovery-lab
npm install
npm run evidence
```

The full frozen automated run is under
`prototypes/editor-session-recovery-lab/artifacts/runs/issue69-2026-07-24T07-24-28-542Z/`
on the harness branch. Its full `trace.jsonl` SHA-256 is
`6d3d342c6f9b8792e937d3af102442bafd11ad9341fd241f4cf29d46b599e20f`.

## Candidate evidence

| Candidate | Representative groups | Boundary violations | Disposition |
| --- | ---: | --- | --- |
| normalized transaction | 7 | composition fragmented | eliminated |
| completed semantic intent | 6 | none | correct conservative fallback |
| bounded idle within equal bindings | 5 | none | selected |
| fixed 500 ms window | 1 | four observed hard crossings in the representative stream; every frozen binding class failed the focused matrix | eliminated |

The focused binding matrix separately verified project Scope, chapter, target,
ownership, expected Head, writer generation, admission identity, admission
expiry, editor-contract version, undo group, composition, paste, cut, drop,
structural operation, and explicit command.

## Correctness and recovery matrix

| Area | Evidence | Result |
| --- | --- | --- |
| Chinese IME | repository owner, macOS Simplified Chinese Pinyin candidate UI, real composition events | exact 18-byte text; three candidate commits remained three commands and Author Actions |
| English typing, backward delete, forward delete, selection replace | native `beforeinput`, keyboard selection, real Tiptap transactions | exact editor/Core convergence |
| paste and cut | native Chrome clipboard shortcuts and clipboard events | hard boundaries; exact convergence |
| drop | native Chrome drag/drop path and `DataTransfer` observation | hard boundary; exact convergence |
| author undo and redo | native keyboard into Tiptap history | explicit semantic boundaries; no durable generic-redo claim |
| HTTP before Activity | Receipt trace index 9; Activity index 10 | one Receipt, one Author Action, one convergence |
| Activity before HTTP | Activity trace index 8; Receipt index 10 | one Receipt, one Author Action, one convergence |
| duplicate Activity and duplicate command | duplicate Activity ignored; repeated idempotency key returned the same Receipt | one Author Action |
| acknowledgement loss | Activity/Core commit survived; client entered `OutcomeUnknown`; Receipt lookup settled | no blind retry, one Author Action |
| sequence gap | submission paused; Snapshot loaded; journal reconciled | no inferred event, exact convergence |
| replay-floor miss | HTTP 409 drove pause and Snapshot | exact convergence |
| Core crash before commit | first reconciliation required reauthorization; equal bindings then permitted one retry | one commit and one Author Action |
| Core crash after atomic commit, before response | Receipt/Head/Action/Activity survived restart | one commit and one Author Action |
| reload after journal durability, before network | one durable record recovered and reauthorized because all bindings matched | exact text committed |
| reload with fenced writer generation | no automatic resubmit | exact copyable Recovery Draft, zero authority change |
| reload after Receipt, before Activity visibility | Receipt lookup preceded current-Head retry decisions; Snapshot confirmed settlement | one Author Action, journal GC after convergence |
| secondary tab | read-only until explicit takeover | no second writer |
| takeover and stale writer | writer generation incremented; stale submission refused | old text preserved as Draft; zero stale Author Action |
| chapter switch | current chapter flushed before switch | independent Heads and exact text in both chapters |
| authoritative outcome | typed Receipt with Author Action sequence `1` | committed |
| Proposal/refused/conflicted/no-effect | typed Receipt, null Author Action sequence | complete Draft retained; journal not GC'd |
| long session | 240 intents, one bounded-idle command | zero lost, duplicated, or reordered characters |
| lifecycle audit | 2,100 ordered records | persist-before-network and settlement/convergence-before-GC verified |

## Real desktop input evidence

### Automated Chrome evidence

The automated run used Google Chrome `150.0.7871.182` on macOS arm64 and
exercised real Tiptap DOM transactions, native English keyboard input,
selection replacement, clipboard shortcuts, drag/drop, and author undo/redo.

The `synthetic-composition-boundary-only` scenario deliberately emits four
Tiptap transactions inside one synthetic composition boundary. It proves the
segmentation rule and byte-exact Core path for `中文输入`, but it is explicitly
tagged `real_os_ime_evidence: false` and is not accepted as operating-system IME
evidence.

### macOS Chinese Pinyin checkpoint

The repository owner used the macOS Simplified Chinese Pinyin candidate UI in
the visible Codex In-app Browser to enter `中文输入验证`. The operating system
produced three completed composition commits: `中文`, `输入`, and `验证`.
The harness observed each through `os-composition-events`, persisted each to
IndexedDB before its network submission, and retained them as three commands
and Author Actions `1`, `2`, and `3`.

The final editor, pending projection, Core authority, and converged projection
were byte-exact: 18 UTF-8 bytes with SHA-256
`9a4f24d2e492af0d00eaa1a745c6437963252423643a167106b1e4b3ffb2ae9c`.
Each command received one Receipt and one Activity position; GC followed
durable settlement and projection convergence.

One native `Meta+Z` then reversed the uninterrupted author typing burst to the
empty pre-input text. The harness journaled that inverse as a distinct
`explicit_command`, and Core committed Author Action `4`. Reload preserved its
Head, empty authority, and Activity position `4` exactly, with no pending
submission. This observation does not authorize submission coalescing across
the three composition boundaries: Issue #46 must map the author-perceived undo
burst separately from the completed input units.

## Measurements

Measurements are probe observations, not production budgets or SLAs.

| Probe | Observation |
| --- | --- |
| journal durability, representative scenarios | mean `0.7–4.7 ms` |
| HTTP-first settlement | mean `4.5 ms` |
| Event-first settlement with injected response delay | mean `94.5 ms` |
| gap Snapshot/resync work | mean `1.2 ms` |
| replay-floor Snapshot/resync work | mean `1.7 ms` |
| real IME composition journal durability | `2.7–4.6 ms` |
| real IME composition Receipt settlement | `3.4–10.4 ms` |
| 240 continuous one-character intents | one Core command, `6,079` command-payload bytes |
| unresolved journal at 240 intents | `240` records, `276,265` serialized bytes |
| long-session settlement | mean `103.8 ms` |
| long-session convergence plus serial per-entry collection | mean `1,404.7 ms` |

The long-session result rejects copying the harness's full materialized
before/after text per journal entry or serial per-entry GC into production.
Issue #70 must choose a bounded patch/materialization and collection strategy;
Issue #76 owns the resulting storage, payload, latency, and compaction envelope.

## Precise normative handoff

### Issue #46 — manuscript authority and Author Action semantics

Apply this evidence to these sections of
`docs/foundation/manuscript-revision-proposal-state-machine.md`:

- **3.2 Author Action order:** allocate exactly one sequence only for one
  successful author-owned transition; duplicates retain the same Receipt and
  sequence; Proposal/refused/conflicted/no-effect retain `null`.
- **7.2 ApplyAuthorEdit:** accept one completed semantic unit; bounded idle may
  combine only equal bindings and must remain one indivisible command.
- **9. Core Transition atomicity:** the pre-commit crash produced no authority;
  the post-commit/pre-response crash recovered one Head, Receipt, Author Action,
  and Activity position.
- **10. Undo and reapplication:** preserve author-perceived semantic grouping;
  the real IME checkpoint observed one native undo reverse three completed
  composition commits from one uninterrupted typing burst. Browser history
  supplies a local inverse candidate, not a durable generic redo contract, and
  its grouping must not be confused with submission coalescing.
- **12. Recovery:** reconcile `OutcomeUnknown` by idempotency key, Receipt,
  digest, Head, writer generation, and admission before any reauthorization.

This evidence does not modify those normative sections.

### Issue #70 — Editor Session, journal, projection, and recovery

Apply this evidence to these sections of
`docs/foundation/web-editor-session-synchronization-and-recovery-semantics.md`:

- **2. Editor Session and writer ownership:** one project writer generation,
  secondary read-only tabs, explicit takeover, and stale-writer refusal.
- **3. Local Edit Journal:** real IndexedDB, atomic cross-tab local order,
  durable intent before network, exact bindings and digest; the real IME path
  preserved all three candidate commits independently. Do not copy the
  harness's full-text-per-entry growth or serial collection.
- **4. Immediate pending projection:** pending text remains visibly and
  structurally distinct from Core authority.
- **5. Submission and convergence:** HTTP and Activity are independently
  ordered, duplicates are idempotent, and GC waits for both settlement and
  convergence.
- **6. Reload, crash, resync, and chapter switching:** Receipt-first
  reconciliation, binding-equal recovery only, Recovery Draft on mismatch,
  Snapshot for gaps/replay-floor misses, and persist-before-switch.
- **7. Proposal and refusal recovery:** preserve the complete Draft and retain
  its journal record for every non-authoritative outcome.
- **8. Deterministic verification:** retain named fault points before/after
  journal durability, submission, atomic Core transition, acknowledgement,
  Activity, projection convergence, and GC.

Also align Issue #70 verification with
`deterministic-verification-and-failure-recovery-gates.md` sections **2.3,
2.4, 2.5, 3, 7, and 9**.

### Issue #76 — performance and storage envelope

Own thresholds and production measurement for:

- idle maximum, maximum intents/operations, and maximum payload per coalesced
  command;
- IndexedDB patch representation, materialization cadence, quota behavior, and
  compaction;
- batched/transactional GC after settlement and convergence;
- input-to-pending, journal durability, settlement, convergence, reload,
  resync, and long-session p95/p99 latency;
- Activity/Snapshot replay bytes and offline journal growth.

No threshold in this document is normative.

### Issue #45 — refusal and conflict UX

The harness only proves typed and inspectable states. It makes no visual,
interaction, copy, layout, or final editor UI recommendation.

## Evidence files

- [`automated-scenario-results.json`](evidence/issue-69/automated-scenario-results.json)
  contains every automated acceptance result.
- [`automated-measurements.csv`](evidence/issue-69/automated-measurements.csv)
  contains the bounded per-scenario measurements.
- [`automated-environment.json`](evidence/issue-69/automated-environment.json)
  binds the source commit, browser, operating system, and expected fault
  signals.
- [`representative-trace.jsonl`](evidence/issue-69/representative-trace.jsonl)
  is a bounded extract of lifecycle and fault-window records. The harness
  branch retains the complete trace.
- [`manual-real-ime-checkpoint.json`](evidence/issue-69/manual-real-ime-checkpoint.json)
  binds the human input, exact states, Core snapshots, assertions, and
  normative handoffs.
- [`manual-real-ime-trace.jsonl`](evidence/issue-69/manual-real-ime-trace.jsonl)
  contains the 52-record real composition, settlement, undo, GC, and reload
  sequence.

## Limits

- The fake Core is contract-faithful evidence, not a Core implementation.
- The harness uses whole materialized text to make exact recovery observable;
  it is not a recommended wire or journal representation.
- Headless measurements do not predict production latency.
- Automated composition is not real IME evidence.
- The apparatus does not establish final UI behavior, visual design, refusal
  copy, product dependencies, configuration, migration, or release thresholds.
- No StoryOS product Rust or TypeScript code, implementation issue, or
  normative foundation contract is created by this experiment.
