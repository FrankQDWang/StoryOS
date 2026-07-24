# Frozen experiment contract

## Question

What is the smallest editor-to-Core commit unit that preserves completed
semantic editor intent, author-perceived undo, exact IME output, durable
recovery, writer fencing, and deterministic Core settlement while keeping
command count, latency, payload, and Local Edit Journal growth bounded?

This experiment chooses evidence for the normative owners. It does not create
new editor/session semantics.

## Immutable correctness invariants

1. A completed browser intent is distinct from a durable journal record, a
   local pending projection, Core authority, Receipt settlement, projection
   convergence, and journal garbage collection. Every trace names the exact
   stage.
2. The Local Edit Journal is durably written before the first network byte of a
   submission is attempted.
3. No candidate crosses a composition, clipboard, drag/drop, structural
   operation, explicit command, chapter, target, ownership class, expected
   Head, writer generation, admission binding, or undo-group boundary.
4. A Core command is one indivisible semantic unit. Core either commits the
   whole unit or preserves the whole Draft; the browser never splits a command
   after admission.
5. Only one successful author-owned Core transition allocates one Author Action
   sequence. Refused, conflicted, proposal-routed, no-effect, invalid, and
   unresolved commands allocate none.
6. `OutcomeUnknown` is never blindly retried. Recovery first reconciles the
   idempotency key, Receipt, Activity position, current Head, writer generation,
   and admission binding.
7. Automatic recovery is allowed only for the same unsettled, unexpired direct
   edit whose target, expected Head, ownership, writer generation, admission
   token, editor contract, digest, and idempotency key still match.
8. Journal garbage collection occurs only after durable Receipt settlement and
   a projection at or beyond the settlement position.
9. A sequence gap or replay-floor miss pauses submission and requires a bounded
   Snapshot/resynchronization path. Recovery never infers missing authority.
10. A stale writer cannot commit. Takeover begins from a Core Snapshot plus
    reconciled journal state; text from the fenced tab remains a copyable
    Recovery Draft.
11. Across every accepted scenario there are zero lost, duplicated, or
    reordered characters; zero silent duplicate Author Actions; byte-exact
    committed IME output; author-perceived undo grouping; and no visual or
    persisted conflation of authority with pending projection.

Correctness gates candidate selection. Performance is compared only after these
invariants pass.

## Candidate policies

| ID | Commit unit | Intended role | Expected risk |
| --- | --- | --- | --- |
| `transaction` | Every normalized ProseMirror transaction | Negative baseline | Selection/bookkeeping churn leaks into durable command boundaries; IME and undo fragment |
| `semantic-intent` | Every completed semantic editor intent | Correctness baseline | Correct but can create excessive commands during continuous typing |
| `bounded-idle` | Completed intents coalesced during a short idle period only when every binding and undo group is equal | Selection candidate | Must flush at every hard boundary and remain crash-safe |
| `fixed-window` | All edits arriving in a fixed time window | Negative/performance control | Time alone can cross semantic and contract boundaries |

The experiment may eliminate candidates. It may not weaken an invariant to keep
one alive.

## Semantic input boundaries

The browser classifier records these intent kinds:

- `typing`
- `delete_backward`
- `delete_forward`
- `selection_replace`
- `composition`
- `paste`
- `cut`
- `drop`
- `structural`
- `explicit_command`

Composition flushes only after `compositionend` and the final Tiptap transaction.
Paste, cut, drop, structure, explicit command, chapter switch, ownership change,
Head change, writer-generation change, admission change, and undo-group change
are unconditional hard boundaries.

## Trace schema

Every JSONL record has:

| Field | Meaning |
| --- | --- |
| `trace_schema` | Frozen schema identifier `storyos.issue69.trace.v1` |
| `run_id` / `scenario_id` | Stable run and scenario identity |
| `seq` / `at_ms` | Monotonic trace order and elapsed time |
| `actor` | `browser`, `journal`, `projection`, `transport`, `core`, `activity`, or `recovery` |
| `stage` | Exact lifecycle stage, never an overloaded “saved” flag |
| `chapter_id` / `target_id` | Scope of the observation |
| `editor_session_id` / `writer_generation` | Session fence |
| `intent_id` / `intent_kind` / `undo_group` | Browser-semantic identity |
| `journal_id` / `local_order` | Local durability identity |
| `command_id` / `idempotency_key` / `payload_digest` | Submission identity |
| `expected_head` / `resulting_head` | Authority precondition/result |
| `admission_id` / `admission_expires_at` | Direct-edit admission binding |
| `receipt_id` / `outcome` / `author_action_seq` | Durable Core settlement |
| `activity_position` / `projection_position` | Convergence and gap detection |
| `text_sha256` / `utf8_bytes` | Byte-exact content observation |
| `details` | Bounded stage-specific typed fields |

Lifecycle stage values used by the gate:

1. `intent_completed`
2. `journal_persist_started`
3. `journal_durable`
4. `pending_projection_applied`
5. `submission_started`
6. `core_committed`
7. `receipt_settled`
8. `activity_observed`
9. `projection_converged`
10. `journal_gc_eligible`
11. `journal_gc_completed`

Recovery adds `outcome_unknown`, `submission_paused`, `snapshot_loaded`,
`journal_reconciled`, `recovery_draft_preserved`, and
`submission_reauthorized`.

## Fault matrix

| Area | Fault window | Required observation |
| --- | --- | --- |
| Browser durability | before journal write; after journal durability; after pending projection | no network before durability; reload reconstructs exact text or preserves Recovery Draft |
| Transport order | HTTP before Activity; Activity before HTTP | one settlement, one converged projection |
| Delivery | duplicate HTTP; duplicate Activity; lost HTTP acknowledgement | idempotent Receipt; no duplicate Author Action; lost ack becomes `OutcomeUnknown` then reconcile |
| Activity stream | sequence gap; replay-floor miss | submission pauses; bounded Snapshot/resync; no inferred missing event |
| Core transition | before commit; after Receipt/Head/Action atomic commit but before reply | no partial authority; reconciliation distinguishes not-committed from committed |
| Session fence | secondary tab; explicit takeover; stale writer submit | secondary read-only; old generation refused; old text preserved |
| Bindings | Head, admission, ownership, chapter, target, undo group change | hard flush or refusal; never coalesced across boundary |
| Outcome | authoritative, Proposal, refused, conflicted, no-effect | typed complete result; Draft preserved for every non-authoritative outcome |
| Local lifecycle | chapter switch; reload after every local boundary | per-chapter pending state; deterministic replay/reconciliation |
| Core lifecycle | process crash after every Core durability boundary | restart from independent persisted Core state |
| Scale | long session and journal growth | bounded latency/output; settled converged records GC; unresolved records remain |
| Input | Chinese/English IME, typing, delete, selection replace, native paste/cut/drop, undo/redo | exact text and author-perceived intent/undo boundaries |

## Measurements

Correctness fields are binary gates. For passing candidates the lab records:

- input-to-pending-projection latency;
- journal durability latency;
- submission-to-Receipt settlement latency;
- settlement-to-projection-convergence latency;
- reload recovery and Snapshot/resync latency;
- normalized transactions, semantic intents, commands, Core commits, and Author
  Actions;
- command payload bytes;
- peak/current journal record and byte counts;
- Snapshot and Activity replay bytes.

Raw output is bounded per scenario. High-volume observations are aggregated and
only the first/last relevant lifecycle records are retained.

## Ownership handoff

- Issue #46 receives evidence about Author Action allocation/order,
  `ApplyAuthorEdit` whole-command atomicity, undo, typed outcomes, and
  `OutcomeUnknown` reconciliation.
- Issue #70 receives evidence about IndexedDB journal shape, pending projection,
  submission/convergence, reload/crash/takeover, writer fencing,
  Snapshot/resynchronization, and deterministic verification points.
- Issue #76 receives performance and storage envelope measurements. This
  experiment sets no normative production threshold.
- Issue #45 owns later refusal/conflict UX. This lab only makes those typed
  states inspectable.
