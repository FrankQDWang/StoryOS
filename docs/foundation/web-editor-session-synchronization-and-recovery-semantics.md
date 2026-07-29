# Web Editor Session, Synchronization, and Recovery Semantics

- Status: current
- Canonical issue: [Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70)
- Evidence owner: [Validate Production Editor Session, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/69)
- Retained UX evidence: [Validate Production Proposal Refusal, Conflict, and Recovery UX](https://github.com/FrankQDWang/StoryOS/issues/45)
- Release baseline: [Define AI-Independent Editor First-Release Baseline and Handoff Criteria](ai-independent-editor-first-release-baseline-and-handoff-criteria.md)
- Author admission: [Author Command Admission](author-command-admission.md)
- Core state machine: [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md)
- Artifact classification: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Public protocol: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)
- Verification: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md)

## 1. Purpose, authority, and owner boundary

This specification is the sole owner of Web Editor Session identity and
lifecycle, browser-local edit continuity, immediate pending projection,
submission queueing, HTTP acknowledgement and Project Activity convergence,
Snapshot resynchronization, writer takeover, browser recovery, and Local Edit
Journal payload collection.

StoryOS Core and PostgreSQL remain authoritative for manuscript and Proposal
state, Heads, Author Actions, typed Receipts, Artifacts, and Project Activity.
The Server owns Author Command Admission issuance and settlement. The public
protocol owns routes, headers, wire serialization, acknowledgement DTOs,
Problems, Snapshot descriptors, and SSE envelopes. This contract consumes
those facts and defines how one browser session preserves and projects them;
it does not create another authority, Receipt, Admission, wire contract,
retention profile, performance threshold, or Agent-assistance workflow.

A Local Edit Journal intent record/group, local sequence, browser checkpoint,
Pending Edit Projection, DOM state, clipboard content, network response, or
cached Event is never evidence that a Core effect committed or did not commit.

## 2. Web Editor Session identity and writer lifecycle

### 2.1 Editor Session identity

`EditorSessionId` is a Server-assigned opaque identity for one browser editing
session and binds:

```text
EditorSessionBinding {
  editor_session_id
  project_scope { owner_user_id, project_id }
  client_session_binding_ref
  client_session_generation
  client_contract_revision
  security_policy_revision
  opened_at
}
```

All fields are immutable for that Editor Session. A reload may recover the
same session only while the Server still validates the exact binding; a new
or invalid Client Session Binding creates a new `EditorSessionId`. An Editor
Session is not a User, Project, browser tab authority, Admission, or writer
lease.

The session disposition is the closed lifecycle:

```text
EditorSessionDisposition =
  Open
  | Closed { reason: author_closed | client_session_ended | project_unavailable }
```

Connection loss, backgrounding, reload, and process death do not close or
reopen the durable identity. They are observations resolved against the
Server-held binding.

### 2.2 One current writer generation per Project

Each Project has one monotonically increasing `writer_generation` and at most
one open Editor Session holding that generation as current writer. Every other
open Editor Session is a read-only observer. A local sequence or earlier
writer grant never establishes current writer status.

```text
EditorWriterProjection =
  CurrentWriter { writer_generation }
  | ReadOnly {
      observed_writer_generation
      reason: secondary_session | superseded_by_takeover | binding_invalid
    }
```

The Server derives this projection from the exact Project Scope, Editor
Session, protected Client Session generation, and current Project writer
generation. It is not a client-selected role.

Writer takeover is the explicit browser/session command
`TakeOverProjectWriter`. It is durably journaled and submitted as one
`explicit_editor_command` group under sections 3 and 4. Its final request
binds the requesting Editor Session, protected Client Session generation,
observed current writer generation, current Project Scope, request contract,
idempotency key, and challenge. A valid admitted attempt receives an Author
Command Admission and is never automatically invoked or recovered after
uncertainty.

The Server compares the observed generation and performs one closed transition:

```text
WriterTakeoverReceipt =
  TakeoverApplied {
    writer_takeover_receipt_ref
    command_ref
    author_command_admission_id
    prior_editor_session_id
    prior_writer_generation
    resulting_editor_session_id
    resulting_writer_generation
    takeover_activity_position
    resulting_snapshot_id
    resulting_snapshot_activity_position
    resulting_heads
  }
  | TakeoverCompareFailed {
      writer_takeover_receipt_ref
      command_ref
      author_command_admission_id
      observed_writer_generation
      current_writer_generation
      current_writer_projection
      current_snapshot_id
      current_snapshot_activity_position
      current_heads
      reason: generation_changed | requester_already_current
    }
```

`TakeoverApplied` atomically advances the generation, grants it to the
requester, fences every older generation before another author-command
Admission can be issued, records one Project Activity position for the
takeover, and returns the canonical Snapshot from which the writer reconciles.
`TakeoverCompareFailed` is a typed Receipt-backed no-change result of the
admitted compare-and-set: it advances no generation and creates no takeover
Activity; its Snapshot position is a current read boundary, not a fabricated
effect. Pre-admission validation failure has no Admission or Receipt.
`RequiresReconfirmation` has an Admission but no Receipt or takeover effect.
Missing acknowledgement follows the same settlement query as every other
admitted group. Neither takeover result creates a manuscript Core effect or
Author Action.

After `TakeoverApplied`, the prior session becomes read-only. The winner opens
a new Local Edit Journal partition bound to the resulting generation; neither
the winner's nor the prior session's older partition is rebound. Their complete
local payload remains non-authoritative and must not be deleted or resubmitted
merely because takeover occurred. If StoryOS must preserve that payload outside
the local journal, it follows the exact `RecoveryDraft` creation rule in
section 9.

A stale writer fails closed before Admission issuance. If an Admission was
already issued, the client follows the Admission reconciliation matrix in
section 6; it never treats a takeover response as proof that the earlier
command did not commit.

### 2.3 Ordering domains remain separate

The following orders are not interchangeable:

| Order | Scope and owner | Permitted use |
| --- | --- | --- |
| `local_intent_sequence` | one Project-scoped IndexedDB allocator shared by exact Editor Session/writer-generation journal partitions | preserve browser-local intent and deterministic reconciliation order |
| `writer_generation` | one Project, Server-owned | fence stale writers and identify the current writer |
| `AuthorActionSequence` | one Project Scope, Core-owned | order successful author-owned Core Transitions and undo |
| Project Activity position | one Project Scope, protocol/Core-owned | order public committed activity and resume projection |
| Authoritative and Proposal Heads | exact durable objects, Core-owned | prove current state and command preconditions |

Recorder sequence, IndexedDB order, timestamps, UUID order, HTTP arrival, and
SSE arrival never establish Project authority order.

## 3. IndexedDB Local Edit Journal

### 3.1 Database, partition, and local order

The Protected Web Client maintains one Project Scope-bound IndexedDB database.
It owns the one atomic `local_intent_sequence` allocator shared by same-origin
tabs, but it does not turn all tabs or generations into one Local Edit Journal.

One canonical Local Edit Journal is an immutable-identity partition:

```text
LocalEditJournalPartition {
  journal_partition_id
  project_scope
  editor_session_id
  writer_generation
  client_session_binding_ref
  client_session_generation
  client_contract_revision
  security_policy_revision
  created_at
  disposition:
    CurrentWriterOpen
    | ReadOnlyObserver { observed_current_writer_generation }
    | ReadOnlyFenced { resulting_writer_generation }
    | Closed { reason }
}
```

Every intent and submission group binds exactly one partition. Takeover creates
a new partition for the new generation and fences older partitions; it never
rewrites their Editor Session or writer generation. This preserves the
ubiquitous-language definition of Local Edit Journal as one Project Scope,
Editor Session, and writer generation while retaining one Project-wide local
order for deterministic cross-tab reconciliation. Only
`CurrentWriterOpen` admits a `CompletedIntentRecord`; a read-only observer may
record only the explicit takeover decision needed to request writer status.

### 3.2 Immutable intent records

The journal has two closed intent kinds. Each receives one unique, never-reused
`local_intent_sequence` in the same transaction that makes its complete payload
durable.

```text
CompletedIntentRecord {
  completed_intent_record_id
  local_intent_sequence
  journal_partition_id
  project_scope
  editor_session_id
  writer_generation

  chapter_object_id
  base_snapshot_id
  base_activity_position
  target_refs
  expected_authoritative_heads
  expected_proposal_heads
  proposal_anchors
  observed_ownership_partition

  author_edit_unit {
    normalized_primitives
    selection_snapshot
  }
  retry_source
  editor_contract_revision
  undo_group_binding

  payload_chain_ref
  payload_digest
  created_at
}

ExplicitEditorCommandRecord {
  explicit_command_record_id
  local_intent_sequence
  journal_partition_id
  project_scope
  editor_session_id
  writer_generation

  command_kind
  exact_semantic_payload_ref
  semantic_payload_digest
  exact_target_head_anchor_bindings
  editor_contract_revision
  author_visible_decision_ref
  created_at
}
```

`CompletedIntentRecord` mirrors the Core `AuthorEditUnit` exactly: its
`selection_snapshot` is nested inside that unit and there is no parallel
selection list. It contains no action class, request contract, idempotency key,
challenge, Command, or Admission. The journal cannot choose authoritative
versus Proposal routing.

An `ExplicitEditorCommandRecord` durably represents takeover, Acceptance,
rejection, withdrawal, replan, reopen, supersede, continuation, completion,
Draft closure, Author Undo, or another explicit editor command before network
submission. It is never coalesced and never automatically invoked after
uncertainty.

### 3.3 Bounded payload representation and checkpoints

Direct-edit intent records store completed intent as a bounded chain of typed
structural patches over one exact materialized checkpoint. The journal does not
copy the whole chapter before and after every intent.

```text
JournalPayloadChain {
  checkpoint_ref {
    chapter_object_id
    materialized_payload
    materialized_payload_digest
    source_snapshot_id
    source_heads
  }
  ordered_patch_refs {
    patch_id
    completed_intent_record_id
    local_intent_sequence
    normalized_primitives
    resulting_payload_digest
  }
}
```

Every retained direct-edit record must be reconstructable to its exact
`AuthorEditUnit` and complete structured author-edit payload without a network,
cache, DOM, ProseMirror history, or collected predecessor. Every retained
explicit-command record must likewise reconstruct its complete semantic
payload. Checkpoint replacement is atomic: the new
materialization and all still-required patch coverage become durable before
the old materialization or patches are eligible for collection. Patch count,
materialization cadence, byte ceilings, and performance targets are supplied
by the applicable Protocol Limit Profile and the measurement owner; this
contract does not invent numerical thresholds.

### 3.4 Atomic local durability

One IndexedDB transaction atomically:

1. validates the Project Scope store and schema;
2. allocates `local_intent_sequence`;
3. writes the immutable intent record, complete payload or patch bytes,
   digests, and referenced checkpoint;
4. advances the journal's durable high-watermark; and
5. records the projection dependency needed to reproduce the pending view.

Network submission cannot start before this transaction commits. The author
may see the browser's native editing response during the interaction, but the
Web Client may label the edit `saving` only after it can rebuild that completed
intent from durable journal bytes.

Before accepting more editing input, the client proves that the current
journal schema, transaction, checkpoint, and complete-payload reconstruction
are usable. Quota, corruption, schema incompatibility, or persistence failure
stops new editing and submission and enters read-only local recovery.
In-memory or copyable text remains plainly labelled local payload. It is not a
`RecoveryDraft`, Artifact, Snapshot, or successful save. StoryOS may create a
durable `RecoveryDraft` only through section 9.

### 3.5 Frozen submission groups and reconciliation attachment

Network submission is owned by a separate group identity:

```text
JournalSubmissionGroup {
  journal_submission_group_id
  journal_partition_id
  project_scope
  editor_session_id
  writer_generation

  ordered_coverage: NonEmptyList {
    local_intent_sequence
    intent_record_ref
    payload_digest
  }
  covered_sequence_range { first, last }

  action_class: direct_editor_action | explicit_editor_command
  api_major
  method
  route_template
  command_schema
  command_kind
  digest_profile
  idempotency_key
  anti_forgery_challenge_ref

  frozen_request_body_ref
  frozen_request_digest_input_ref
  frozen_payload_coverage_digest
  frozen_at
}

GroupSubmission =
  FrozenNotSubmitted
  | InFlight
  | ReconciliationRequired

GroupSettlement =
  Unsettled
  | PreAdmissionRefused { refusal_record_ref }
  | ReceiptSettled { receipt_ref, receipt_kind }
  | RequiresReconfirmation {
      author_command_admission_id
      terminal_settlement_ref
      reason
    }

AuthorSurfaceConvergence =
  Pending
  | ReceiptBackedConverged {
      receipt_ref
      authority_position:
        ProjectActivity { project_activity_position }
        | SnapshotRead { snapshot_id, snapshot_activity_position }
      resulting_heads
      resulting_surface_refs
    }
  | PreAdmissionRefusalConverged {
      refusal_record_ref
      refusal_surface_ref
      preserved_payload_ref
    }
  | ReconfirmationConverged {
      author_command_admission_id
      terminal_settlement_ref
      preserved_local_payload_ref
      recovery_draft_ref | None
      reconfirmation_surface_ref
    }

AuthorAttention =
  None
  | Required { reason, author_surface_ref }
  | Resolved { decision_receipt_ref | read_only_resolution_ref }

GroupPayloadRetention =
  Retained
  | Eligible { eligibility_evidence }
  | Collected { collection_fence }
```

Coverage is strictly ordered by `local_intent_sequence`, contains no duplicate
or skipped record inside its `covered_sequence_range`, and is complete for the
frozen body. Once grouped, one intent record belongs to exactly one
not-yet-collected group; groups never overlap. A direct-edit group contains one
or more `CompletedIntentRecord` values. An explicit-command group contains
exactly one `ExplicitEditorCommandRecord`. The group—not an intent record—owns
the action class, request contract, idempotency/challenge inputs, final body and
digest coverage, submission, Admission, acknowledgement, settlement,
convergence, and collection lifecycle. Its submission, settlement,
convergence, attention, and retention axes advance append-only.

The Web Client appends only this browser-safe Server attachment:

```text
AdmissionReconciliationAttachment {
  journal_submission_group_id
  command_ref
  author_command_admission_id
  settlement_query
  issued_at
  expires_at
  idempotency_binding {
    idempotency_key
    idempotency_record_ref
    canonical_command_digest
    digest_profile
  }
  acknowledgement_observations: AppendOnlyList {
    observed_at
    kind: Accepted | Committed | RequiresReconfirmation | outcome_unknown
    correlation_ref
    receipt_ref | None
    terminal_settlement_ref | None
    reconfirmation_reason | None
  }
}
```

The attachment is append-only observation, not a client-owned Server record.
One group has no attachment before Admission and exactly one thereafter; that
attachment is never replaced.
It never contains a nonce value or secret, never creates a Command or
Admission identity, and never substitutes a browser timestamp for Server
issuance, expiry, Receipt, or settlement. Only the Server creates and returns
the Command/Admission references after the final request and canonical digest
exist. Its `settlement_query` is immutable, and every uncertainty or
acknowledgement observation for the group reconciles through that same query.

`PreAdmissionRefused` proves there is no Admission or Receipt. `InFlight` is
not settlement. `ReconciliationRequired` is used after post-admission
uncertainty and authorizes only the same read-only settlement query.
`ReceiptSettled` covers every typed Core result, including refusal, conflict,
and no effect. Convergence, attention, and payload retention remain separate
axes and never override settlement.

## 4. Completed intent and pre-Admission coalescing

One `CompletedIntentRecord` is the conservative direct-edit unit. Composition
completion, paste, cut, drop, and every structural primitive are unconditional
record boundaries. Every explicit editor command is instead one
`ExplicitEditorCommandRecord` and one single-record submission group.

Before Admission issuance, adjacent completed `direct_editor_action` intents
may be coalesced only while:

- every admission-preparation input named by Author Command Admission
  sections 3 and 5 is present and equal;
- every `ApplyAuthorEdit` semantic field, chapter, selection, retry source,
  target, ownership observation, Authoritative and Proposal Head, Anchor,
  editor-contract revision, and author-visible undo-group binding is equal;
- the current writer generation remains current;
- none of the candidate records belongs to a frozen group; and
- the applicable idle, intent, operation, and payload bounds remain proven.

Any difference or unverifiable fact freezes the earlier direct-edit group.
Coalescing chooses the ordered coverage of one `JournalSubmissionGroup`; it
never mutates or combines the immutable source intent records. One
group-level idempotency key and challenge apply only to the final combined
request, so no per-intent idempotency identity can conflict. Coalescing never
alters local undo grouping, crosses a partition, includes an explicit command,
or applies to `explicit_editor_command`.

After the final body is frozen, the Server recomputes its canonical digest,
validates all bindings, claims idempotency, consumes the nonce record, and
issues and attaches one Admission. The group body, digest, ordered coverage,
and payload are immutable after that point. Later typing creates another
intent record and can never enter the admitted group.

## 5. Pending projection, chapter switching, and queueing

### 5.1 Pending Edit Projection

The Pending Edit Projection is rebuilt from one authorized durable Server
Snapshot plus retained journal payloads for the selected chapter. It is
immediate and non-authoritative. It never changes an Authoritative or Proposal
Head and never hides which bytes remain local.

The author-facing save state is derived:

| State | Exact meaning |
| --- | --- |
| `saving` | complete intent/command is durably journaled, but settlement or its applicable convergence branch remains pending |
| `saved` | a Receipt-backed terminal result has converged through its exact Activity/Snapshot position and the resulting current surface requires no attention |
| `needs_attention` | a converged refusal, reconfirmation, conflict, Draft, or other typed result requires an author decision, or current evidence cannot safely reconstruct/converge the surface |

`Accepted` HTTP acknowledgement, an idempotency record without terminal
settlement, a Receipt without author-facing convergence, and an Event without
matching settlement remain `saving` or `needs_attention`; none is `saved`.

Selection, Decorations, NodeViews, editor history, and cursor position are
presentation state. The exact `AuthorEditUnit.selection_snapshot` and
undo-group binding retained by an intent record are command/recovery evidence,
but restoring them does not make DOM state durable truth. A
`PreAdmissionRefusalConverged` or `ReconfirmationConverged` group can be fully
converged while `AuthorAttention` remains `Required`; it is never relabelled
`saved` merely to express convergence.

### 5.2 Chapter switching

Before changing the selected chapter, the client atomically completes the
current semantic intent and its journal transaction or refuses the switch with
an explicit local-recovery state. It never “commits pending browser state” to
authority.

Each chapter projection is rebuilt from its own Snapshot/Head facts plus the
applicable partition's intent records and groups targeting that chapter. The
Project retains one writer generation and one Project-local sequence allocator;
switching chapters does not create another writer, reorder the queue, merge
groups across chapters, or imply Core settlement. Returning to a chapter
restores its exact pending or attention surfaces from journal and durable
Server facts.

### 5.3 Submission scheduler

The current-writer partition owns one serialized author-command queue ordered
by each frozen group's first covered `local_intent_sequence`. It contains
direct-edit groups and every single-record explicit-command group except
takeover. Release 1 begins submission of a later queued command only after the
earlier command reaches a terminal Admission settlement and the current Heads
needed by the later command have been revalidated. An earlier
`outcome_unknown`, gap, stale writer, or required resync pauses this queue.

A read-only observer submits its one-record takeover group through the
Project-local takeover coordination lane, not through the queue it is asking
to fence. Same-origin takeover requests retain local-sequence order, but the
Server's observed-generation compare-and-set alone chooses the winner. On
`TakeoverApplied`, the old writer queue is fenced immediately: already-admitted
groups reconcile independently, frozen/unsubmitted groups and their payloads
remain retained, and a new writer queue opens only after the returned Snapshot
is installed. Concurrent or stale takeover groups receive
`TakeoverCompareFailed`. A terminal pre-admission refusal releases no dependent
group until its current bindings are explicitly rebuilt.

This serial browser admission order prevents dependent edits from overtaking
one another but is not Project authority order. Core may allocate no Author
Action for a refused, conflicted, or no-effect result; Project authority remains
ordered only by Core Heads, Author Action Sequence, and Project Activity.

## 6. Admission, acknowledgement, and reconciliation

### 6.1 Closed phase matrix

| Boundary | Proven fact | Client projection and next action |
| --- | --- | --- |
| pre-admission Problem | sanitized `PreAdmissionRefusalRecord`; no Admission, nonce consumption, Command, Receipt, or Core effect | retain complete payload; show typed refusal; a changed request starts a fresh challenge |
| Admission issued, no response proving settlement | one pending Admission | pause dependent submission; use only its `settlement_query` |
| HTTP `Accepted` | durable asynchronous operation reference only | nonterminal `saving`; observe the named settlement query and applicable Project Activity/Snapshot result |
| HTTP `Committed` | terminal `ReceiptSettled` and exact typed Receipt | project the Receipt result; wait for author-facing projection convergence if not already observed |
| HTTP `RequiresReconfirmation` | terminal Admission settlement with no Receipt or Core effect | show the exact reconfirmation reason and applicable preserved payload/Draft; a later author confirmation creates a new command and Admission |
| post-admission `outcome_unknown` Problem | no claim about Receipt presence; same Admission and settlement query | set `ReconciliationRequired`; forbid blind retry or a new command derived from the uncertain one |
| Project Activity before HTTP `Committed` | committed Event position, not necessarily the client's matching acknowledgement observation | retain/deduplicate the Event, query settlement, and converge only after the exact Receipt/result is known |
| HTTP `Committed` before its required Activity/Snapshot position | Receipt settlement known, author-facing projection not yet converged | retain payload and wait/replay/query from the position kind named by that Receipt result |

`Accepted` never carries or implies a typed Receipt and is not an Admission
terminal state. A pre-admission Problem alone proves Receipt absence.
Post-admission uncertainty never does.

### 6.2 Reconciliation matrix

Reconciliation uses the exact Admission's `settlement_query` and validated
authoritative storage. It never consults journal presence, cache, process
state, missing HTTP, or Event arrival as an effect oracle.

| Authoritative finding | Required client behavior |
| --- | --- |
| exact typed Receipt and matching idempotency/digest | observe or replay `ReceiptSettled`, project only its immutable result, and never invoke again |
| storage is unavailable or Receipt presence cannot be proven | retain `outcome_unknown`, block invocation and dependent submissions, and repeat only bounded read-only reconciliation |
| validated no-Receipt proof; same unexpired `direct_editor_action`; every Admission section 3/5 binding and every complete Core/journal field equal | the Server may invoke once under the same Command, Admission, nonce-consumption record, and idempotency key |
| validated no-Receipt proof; explicit command, expired Admission, changed/missing/unverifiable binding, or incomplete intent | settle `RequiresReconfirmation`; the old Admission can never later receive a Receipt |

The automatic branch requires equality of Project Scope, protected Client
Session binding/generation, client/security contracts, Editor Session, writer
generation, action class, request contract, final digest/profile and covered
fields, targets/Heads/Revisions, idempotency and nonce records, complete
`ApplyAuthorEdit` intent/selections/retry source/ownership/Anchors/reservations,
editor contract, undo group, and durable journal reconstruction. Equality of a
subset is failure.

A visible reconfirmation creates a new idempotency key, anti-forgery challenge,
Command, Admission, and eventual Receipt. Acceptance, rejection, withdrawal,
Draft closure, Author Undo, takeover, and other explicit commands are never
automatically invoked after reload or crash.

## 7. Activity convergence, Snapshot, and resync

The client consumes the one Project Activity Stream. It durably deduplicates
by `event_id`, validates replay generation and contiguous `stream_sequence`
within that generation, and uses the Event's Project Activity position,
Receipt reference, resulting Heads, and typed cause. HTTP and SSE may arrive
in either order; arrival time changes no meaning.

A duplicate Event changes nothing. An older retained cursor may replay
overlap, which is deduplicated. A sequence gap, cursor ahead, cursor below the
replay floor, incompatible Snapshot, changed authorization, mismatched Scope,
or irreconcilable Head pauses submission and enters resync.

Resync:

1. preserves every retained journal payload and current reconciliation record;
2. requests an authorized bounded canonical Snapshot with its Project Activity
   position and replay generation;
3. discards browser checkpoints whose complete Head/digest/contract key does
   not match;
4. reconciles every admitted group by its
   `AdmissionReconciliationAttachment`;
5. rebuilds each visible chapter from the Snapshot plus still-valid local
   intent records/groups; and
6. resumes strictly after the Snapshot position.

A Snapshot is a durable Server reading boundary, not a browser materialization
or authority copy. Resync never silently skips a gap, translates a cursor by
guessing, discards an unsettled payload, or turns local order into Project
order.

Convergence is positive and branches by settlement kind:

| Settlement | Required convergence proof |
| --- | --- |
| `ReceiptSettled` | the exact Receipt plus its required authority position: processed Activity at or beyond a Receipt-backed effect, or the exact Snapshot read boundary for a typed no-change result such as `TakeoverCompareFailed`; exact resulting Heads and every Receipt-allocated Draft, Proposal, lifecycle, condition, Author Action, and control are reflected |
| `PreAdmissionRefused` | the exact `PreAdmissionRefusalRecord`, typed refusal surface, and complete preserved local payload are visible; no Activity position or resulting Head is required or fabricated |
| `RequiresReconfirmation` | the exact Admission terminal-settlement reference, reason, and applicable retained payload or resulting `RecoveryDraft` plus reconfirmation controls are visible; no Receipt, Core effect, Activity position, or resulting Head is required or fabricated |

Receipt-backed manuscript convergence additionally requires no unresolved
earlier Activity gap. A no-Receipt refusal/reconfirmation surface may converge
while an unrelated manuscript projection remains in resync, but it cannot
label that manuscript surface `saved`. Every branch rejects an incompatible
writer generation for the surface being shown. `AuthorAttention` is evaluated
separately: a converged conflict, refusal, Draft, or reconfirmation normally
remains `Required`.

## 8. Core result and control projection

### 8.1 `ApplyAuthorEdit` result matrix

The client projects the exact Receipt result; it never infers a Proposal
Acceptance from an authoritative result or from the absence of a candidate.

| Core result | Author-facing projection after convergence | Controls and journal consequence |
| --- | --- | --- |
| `AuthoritativeApplied` | resulting authoritative Head and Commit are `saved`; no pending Proposal candidate | no recovery controls; retain until GC successor proof |
| `ProposalRevised.Pending` | resulting Proposal Head is `saved` on the editable Proposal surface | accept/reject as permitted by current Proposal state; applicable author undo remains Core-ordered |
| `ProposalRevised.StructuralReshapeConflict` | complete new Proposal Revision plus `ProposalConflict`; `needs_attention` | replan, copy, reject; Acceptance disabled |
| `RefusedToDraft` | complete Core `RefusedEditDraft`; `needs_attention` | narrow, copy, expand with `WholeDraftPayload`, discard |
| `Conflicted` on a Proposal surface | current Heads and exact `ProposalConflict` when allocated; `needs_attention` | replan, copy, reject; no fabricated Draft or Acceptance |
| `Conflicted` without a Proposal condition | complete local intent remains retained beside current authoritative projection; `needs_attention` | explicit author review may form a new edit; local payload is not a Draft |
| `NoEffect` | exact current durable surface and no candidate; `saved` after Receipt and convergence | no stale retry, expansion, Acceptance, or recovery control |

`ProposalConflict` and `ProposalRecoveryConflict` remain conditions on a
preserved Proposal surface. Neither is a Draft Artifact. A healthy local
journal retaining complete conflicted text is not automatically converted
into a `RecoveryDraft`.

### 8.2 Source-Draft and editor decision matrix

| Command/result | Source Draft projection | Controls after convergence |
| --- | --- | --- |
| `DraftRetry.AuthoritativeApplied` or `.ProposalRevised` | exact source `ClosedSuperseded`; immutable history remains inspectable | remove source retry/expand/discard; show only controls of the resulting authoritative or Proposal surface |
| `DraftRetry.RefusedToDraft` replacement | old source `ClosedSuperseded`; exactly one new open `RefusedEditDraft` | controls appear only on the new Draft |
| `DraftRetry.Conflicted` or `.NoEffect` with matched source | exact source remains open with no lifecycle event | keep only controls permitted by that open source and current result |
| `DraftRetry` source Revision/digest/closure mismatch | project exact observed current source disposition | if closed, remove all stale retry/expand controls; if open, use only its current Revision |
| `ExpandRefusedEditDraftToProposal` applied | complete `WholeDraftPayload` becomes one fresh Proposal; exact source `ClosedSuperseded` | Proposal controls only; no second expansion from the closed source |
| expansion source mismatch or not open | no Proposal, no Author Action, no lifecycle event | project exact current source; closed source has no retry/expand control |
| explicit Draft close | exact `DraftClosureChanged` and Receipt | no stale Draft controls; applicable Core undo only |
| exact retry of any settled command | same Receipt, Heads, Draft/Proposal, lifecycle events, and Author Action identities | never duplicate a surface or control |

Copy is read-only and creates no Receipt. A clipboard copy is not a durable
successor for journal GC.

### 8.3 Proposal, condition, and Draft control consumer matrix

This matrix consumes the current Proposal/Draft state and retained production
UX contract. It does not create a Core transition or infer eligibility from a
label.

| Current exact surface | Controls projected by the Web Client |
| --- | --- |
| ordinary Proposal Revision, `open`, `ready`, exact valid Validation Receipt, selected pending Operations | accept selected Operations; reject selected Operations; copy |
| ordinary Proposal without current Acceptance Eligibility | reject only currently pending Operations when allowed; copy; no accept |
| `ProposalConflict` on a preserved Proposal | replan, copy, reject pending Operations; no accept |
| `ProposalRecoveryConflict` on a preserved Proposal | replan, copy, withdraw; no accept |
| open `RefusedEditDraft` | narrow/retry through `DraftRetry`, copy, expand only with `WholeDraftPayload`, discard |
| open `RecoveryDraft` | retry through `DraftRetry`, copy, discard; no expansion |
| closed Draft or `superseded` Proposal | immutable history/copy only; no retry, expand, accept, reject, replan, withdraw, or discard |

All state-changing controls create one durable
`ExplicitEditorCommandRecord`/group except narrow/retry, which creates the
new direct-edit intent/group owned by `ApplyAuthorEdit`. Copy remains read-only.

The result consumer is likewise closed:

| Settled explicit command/result | Exact projected surface and stale-control removal |
| --- | --- |
| `AcceptProposal.Applied` | project the exact Commit, applied Operation resolutions, Heads, and remaining Proposal axes; remove controls for applied Operations and derive any remaining controls only from the returned current state |
| `AcceptProposal.Invalid` | project current Proposal with validation `invalid`; remove accept until a new current Revision validates |
| `AcceptProposal.Conflicted` | project its exact `ProposalConflict`; expose replan/copy/reject and remove accept |
| `AcceptProposal.Refused` or `.NoEffect` | project the exact returned current target; expose only controls permitted by that current state and never imply Acceptance |
| `RejectProposalOperations` applied | selected Operations become `rejected`; remove their accept/reject controls and expose reopen only against the exact rejection events |
| `WithdrawProposal` applied | closure becomes `withdrawn`; remove accept/reject/replan/continue/complete and expose only exact reopen or supersede when current state permits |
| `ReplanProposal` applied | project the new pending Revision and remove controls bound to the consumed Conflict condition; Acceptance waits for a new valid Validation Receipt |
| `ReopenWithdrawnProposal` applied | project the new open, validation-pending Revision; remove the stale reopen control |
| `SupersedeProposal` applied | project terminal `superseded`; remove every active Proposal control |
| `ReopenRejectedOperations` applied | selected Operations become pending in the new validation-pending Revision; remove stale rejection-event controls |
| `ContinueProposalGeneration` applied | project the new Generation ID and `generating`; remove controls bound to the prior Generation ID and disable Acceptance until a later exact ready/valid state |
| `CompleteReadyPartialProposal` applied | project `ready` for the same current Revision with every other returned axis preserved; remove the stale complete control and derive Acceptance only from current validation/closure/resolution |
| `CloseEditorFlowDraft` applied | project exact closed reason/event; remove retry, expand, and discard from that Draft |
| any explicit decision `Refused`, `Conflicted`, or `NoEffect` | project the returned `DecisionCurrentTarget` and condition exactly; allocate no lifecycle change locally and derive controls only from that returned current target |
| exact retry of any settled explicit command | replay the same Receipt, event identities, Heads, and controls; create no duplicate transition or surface |

No old control survives merely because it was rendered before the Receipt,
Snapshot, takeover, or resync. In particular, a Proposal Recovery Conflict
retains `withdraw`, while a Proposal Conflict does not acquire it unless the
current Core state independently permits withdrawal.

### 8.4 Undo and reversal

The browser submits only the exact current `AuthorUndoFrontier` through
`UndoLatestAuthorAction`. It never uses local history order as Core truth and
never skips a Barrier.

| Core undo settlement | Projection |
| --- | --- |
| compensation of direct edit or Proposal edit | exact new Heads/Proposal Revision and Compensation Author Action |
| compensation of source-consuming retry or expansion | content/Proposal effect and exact source-Draft reopen appear together or neither appears |
| `ReversalRequired` | one new inspectable Reversal Proposal; source remains uncompensated |
| `Unavailable` or `Conflicted` | no partial local inverse and `needs_attention` |

Native browser undo may form a new direct-edit candidate only against the exact
visible Frontier. There is no durable generic redo. Reapplication is a new
command with a new Admission.

## 9. RecoveryDraft and local recovery

`EditorRecoveryEvidenceId` identifies one immutable Host-owned Operational
Record created only when the Host can positively bind a complete recovery
payload:

```text
EditorRecoveryEvidenceRecord {
  editor_recovery_evidence_id
  project_scope
  editor_session_id
  writer_generation
  journal_partition_id | None

  source_boundary:
    Journal {
      ordered_intent_record_refs
      covered_sequence_range
      journal_submission_group_id | None
      checkpoint_ref
    }
    | ExplicitInMemoryBoundary {
        in_memory_boundary_id
        capture_reason
        captured_at
      }

  complete_structured_payload_ref
  complete_payload_digest
  editor_contract_revision

  admission_evidence:
    NotApplicable
    | PreAdmissionRefusal { refusal_record_ref }
    | AdmissionSettlement {
        command_ref
        author_command_admission_id
        terminal_settlement_ref
      }
  takeover_evidence:
    NotApplicable
    | TakeoverApplied { writer_takeover_receipt_ref }
    | TakeoverCompareFailed { writer_takeover_receipt_ref }

  creation_reason:
    RequiresReconfirmationPreservation
    | writer_takeover_preservation
    | journal_schema_or_persistence_recovery
    | explicit_durable_preservation
  host_created_at
  resulting_recovery_draft_ref
}
```

The Host writes the evidence record and resulting `RecoveryDraft` revision in
one durable operation, so the record never points to a missing result and the
Draft Creator's `recovery_evidence_ref` resolves to this exact identity. A
journal source must name an ordered, non-overlapping, complete intent range and
its checkpoint. An explicit in-memory source must name the one positively
captured boundary and complete payload; merely finding DOM or clipboard text
does not create such evidence. Admission and takeover variants are populated
only when applicable and never claim a Receipt or effect absent from their
named records.

A `RecoveryDraft` exists only when the StoryOS Host durably creates one Core
Draft Artifact with:

- Host-assigned `EditorRecovery` Creator;
- exact Project Scope, Editor Session, and writer generation;
- the complete structured author-edit payload and digest;
- exact journal record/group/range and checkpoint evidence when available;
- exact `EditorRecoveryEvidenceId`, including its Admission settlement,
  takeover, journal, or in-memory boundary as applicable; and
- one immutable Artifact Revision and provenance closure.

Creation causes no Core edit invocation and no Author Action. A browser-local
record/group, in-memory text, copyable text, clipboard content, checkpoint, or pending
projection is never a `RecoveryDraft` or another Artifact.

StoryOS creates a `RecoveryDraft` only when the complete payload is available
and durable preservation is required because the healthy local continuation
cannot safely remain the sole recovery surface, such as terminal
`RequiresReconfirmation` with recoverable author-edit payload, takeover or
journal/schema recovery that must cross the local session boundary, or
explicit durable preservation before local collection. It does not create one
for every expired, stale, conflicted, no-effect, or unverifiable group.
Incomplete or unverifiable text remains visibly incomplete local recovery
material and cannot be promoted into an Artifact.

A `RecoveryDraft` Retry is a new `ApplyAuthorEdit`/`DraftRetry` with an
explicit author confirmation and new Admission unless section 6's exact
same-Admission automatic branch still applies before Draft creation. Its
result and closure follow section 8.2.

## 10. Reload, crash, and takeover recovery matrix

| Last durable boundary | Reload/recovery result |
| --- | --- |
| before intent-record transaction commit | no durable record; any surviving in-memory text is copyable local payload, not saved or a Draft |
| intent record committed, before group freeze/Admission | rebuild complete intent; current writer may freeze a new group after all bindings revalidate |
| group frozen, Admission issuance did not commit | validated absence of Admission/nonce consumption permits a fresh challenge for that same frozen group |
| Admission attachment recorded, settlement unverifiable | `outcome_unknown`; read-only reconciliation through the same `settlement_query`; no blind invocation |
| validated no-Receipt after Admission | follow only section 6.2's direct-edit equality or `RequiresReconfirmation` branch |
| Core committed before HTTP or Activity observation | exact Receipt/settlement/Heads survive; replay them and converge without another invocation |
| Receipt observed before matching Activity | retain payload and replay/query from the required Activity position |
| Activity observed before HTTP | retain/deduplicate Event and query the exact settlement; Event arrival alone does not settle the local group |
| pre-admission refusal or `RequiresReconfirmation` observed | converge through the applicable no-Receipt branch without requiring Activity/Heads; retain author attention and payload |
| convergence proven before payload collection | group becomes GC-eligible only if section 11's successor proof also holds |
| payload collected before reload | reconstruct from the recorded durable successor and retained collection fence; never from missing bytes |
| reload in stale writer generation | read-only; reconcile admitted groups, preserve unsubmitted payload, and require explicit takeover for new writing |
| `TakeoverApplied` during unsettled work | open the exact new-generation partition; older partitions remain read-only; every admitted group reconciles by its own Admission; unsubmitted payload remains local or becomes a `RecoveryDraft` only under section 9 |
| `TakeoverCompareFailed` | retain the current read-only projection and returned Snapshot; do not advance generation, create Activity, rebind a partition, or retry automatically |
| takeover Admission uncertain or `RequiresReconfirmation` | reconcile only through its attachment or expose visible reconfirmation; never repeat the explicit takeover automatically |

## 11. Deterministic journal garbage collection

### 11.1 Eligibility

Journal intent payload bytes, group payload coverage, and patch/checkpoint
dependencies are eligible for collection only when all of these are proven:

1. the group has terminal `ReceiptSettled`, `RequiresReconfirmation`, or
   `PreAdmissionRefused` evidence as applicable; it is neither `Unsettled` nor
   `ReconciliationRequired`;
2. the author-facing surface has converged through the applicable branch:
   Receipt-backed results include their required Activity/Snapshot position,
   while pre-admission refusal and `RequiresReconfirmation` include their exact
   no-Receipt surface and impose no fabricated Activity/Head requirement;
3. the complete payload has an exact durable successor that is independently
   readable and digest-verified, or another retained complete journal
   materialization still covers it; and
4. no unsettled group, intent record, checkpoint, retry, undo candidate, reconciliation
   record, or visible recovery surface depends on the bytes.

Exact durable successors include the resulting Authoritative or Proposal
Revision, a `RefusedEditDraft` or `RecoveryDraft` Artifact Revision, or another
typed retained Core payload that contains the complete intent. `NoEffect` may
use the exact digest-equal current durable Revision. A conflict or
`RequiresReconfirmation` with only one local complete copy is never eligible.
Clipboard copy, DOM text, cached response, Event payload, and an author-visible
label are not durable successors.

### 11.2 Collection transaction and retained fence

Collection is a batched IndexedDB transaction. It:

1. revalidates terminal settlement, convergence, successor digest, current
   writer generation, and dependency closure;
2. marks the exact groups, intent records, and patch/checkpoint ranges
   `Eligible`;
3. writes one immutable local `collection_fence` containing group/record/range
   identities, payload digests, successor references, Admission/Receipt or
   refusal/reconfirmation references, the exact branch-shaped convergence
   evidence, and reason;
4. deletes only the covered payload bytes and now-unreferenced checkpoint or
   patch material; and
5. marks those ranges `Collected`.

The compact group and intent identities, command digest/idempotency binding, Admission
identity and lifecycle, typed Receipt or refusal/reconfirmation evidence,
reconciliation observations, successor references, projection convergence,
and collection fence remain inspectable under their owning retention
contracts. GC never silently erases evidence needed to reject an exact
duplicate, explain a result, or reconstruct why payload collection was safe.

A crash before the collection transaction commits leaves all payloads
retained. A crash after commit exposes the complete fence and no partial
dependency deletion. Collection cadence and storage/latency envelopes remain
owned by the measurement and retention contracts.

## 12. Deterministic verification obligations

Browser integration and the deterministic oracle cover:

- supported Chinese and English IME, native paste/cut/drop, deletion,
  selection replacement, structural edits, and author-visible local undo;
- atomic Project-local sequence, partition, intent-record, group, and
  checkpoint durability before submission and complete reconstruction after
  every local fault cut;
- non-overlapping complete group coverage; group-level
  idempotency/challenge/digest; exactly one record per explicit-command group;
  and no per-intent request identity;
- every coalescing equality and unconditional flush boundary, including no
  post-Admission merge;
- exact Protected Web Client, Client Session, Editor Session, writer
  generation, Project Scope, request, digest, Head/Anchor, nonce/idempotency,
  and lifetime substitutions;
- pre-admission refusal, `Accepted`, `Committed`,
  `RequiresReconfirmation`, post-admission `outcome_unknown`, validated
  no-Receipt recovery, and exact retry;
- HTTP-before-Activity, Activity-before-HTTP, acknowledgement loss,
  duplicates, older-cursor overlap, gaps, replay-floor misses, Snapshot/resync,
  and projection convergence;
- crash cuts before/after journal commit, Admission issuance, first Core
  invocation, Core commit, acknowledgement, Activity, convergence, GC
  eligibility, and collection;
- chapter switching with pending records/groups, one current-writer queue, and
  the separately fenced takeover coordination lane;
- secondary read-only sessions; exact `TakeoverApplied`,
  `TakeoverCompareFailed`, pre-admission refusal, acknowledgement loss, and
  `RequiresReconfirmation`; stale-writer fencing; new-generation partition
  creation; and preservation without automatic Draft fabrication;
- every `ApplyAuthorEdit`, source-Draft, explicit decision, exact retry, undo,
  reversal, and stale-control row in section 8;
- positive classification of Admission/refusal/reconciliation evidence,
  Receipts, Draft Artifacts, Proposal conditions, local payload, projections,
  and authority;
- `RecoveryDraft` creation only from complete payload plus one exact
  `EditorRecoveryEvidenceRecord`, including journal and explicit in-memory
  boundaries; and
- Receipt-backed versus no-Receipt convergence, including proof that refusal
  and reconfirmation require no fabricated Activity position or resulting
  Heads;
- GC refusal for unknown, unsettled, unconverged, dependency-bearing, or
  only-complete-copy payloads, plus atomic batched collection and retained
  evidence fences.

Passing evidence uses the virtual clock, explicit interleaving schedule,
contract fault points, Multi-Scope adversarial world, and replayable
Verification Evidence Bundle owned by the deterministic verification
contract. Empirical latency, throughput, and storage observations remain
advisory until their numerical values are accepted by the appropriate owner.

## 13. Normative invariants

1. One exact Project has one current writer generation and at most one current
   writer; every stale session is read-only and cannot obtain a new Admission.
2. Every completed direct-edit intent and explicit editor command has one
   immutable partition-bound record before submission and remains exactly
   reconstructable until safe collection.
3. Local order, Project Activity order, Author Action order, Heads, and writer
   generation are distinct typed orders and never substitute for one another.
4. One frozen submission group owns ordered non-overlapping complete record
   coverage plus one action class, request contract, idempotency/challenge,
   final digest, Admission, settlement, and reconciliation lifecycle.
5. The Web Client never generates or supplies an
   `AuthorCommandAdmissionId`; the Server issues and attaches it only after
   the final request and digest are fixed.
6. Coalescing is pre-Admission, bounded, equal-binding-only, direct-edit-only,
   and never crosses a semantic, partition, or explicit-command boundary.
7. HTTP `Accepted` is nonterminal. Only `Committed` with a typed Receipt or
   `RequiresReconfirmation` closes an Admission.
8. Post-admission uncertainty claims neither Receipt presence nor absence,
   forbids blind retry, and reconciles through the same settlement query.
9. Automatic recovery invocation requires validated no-Receipt evidence, the
   same unexpired direct edit, and equality of every Admission, Core, and
   journal binding.
10. Pending projection, Snapshot, journal, cache, DOM, and network state never
   become authority or a settlement oracle.
11. Receipt-backed convergence requires its Activity/Snapshot position;
    pre-admission refusal and `RequiresReconfirmation` converge through their
    exact no-Receipt surfaces without fabricated Activity or resulting Heads.
12. Every Core result, source-Draft disposition, explicit decision, undo, and
    reversal projects
    only its exact permitted controls; a closed source has no stale retry or
    expansion control and no result fabricates Proposal Acceptance.
13. Takeover is one non-auto-recoverable explicit-command group with one typed
    applied or compare-failed Receipt result; an applied result opens a new
    generation partition and never rebinds an old one.
14. A `RecoveryDraft` is a durable Core Draft Artifact created only by
    Host-assigned `EditorRecovery` from complete payload and one exact
    `EditorRecoveryEvidenceRecord`.
15. Journal payload collection requires terminal settlement, author-facing
    convergence, an exact durable complete successor, and no remaining
    dependency; unknown, unsettled, or only-complete-copy payloads remain
    retained.
