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

A Local Edit Journal entry, local sequence, browser checkpoint, Pending Edit
Projection, DOM state, clipboard content, network response, or cached Event is
never evidence that a Core effect committed or did not commit.

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

An explicit takeover:

1. reauthorizes the requester, Project Scope, protected Client Session, and
   observed current writer generation;
2. atomically advances the Project writer generation;
3. grants the new generation to the requesting Editor Session;
4. fences every older generation before another author-command Admission can
   be issued; and
5. returns the authoritative Snapshot and Project Activity position from
   which the new writer must reconcile.

The prior session becomes read-only. Its complete local payload remains
non-authoritative and must not be deleted or resubmitted merely because
takeover occurred. If StoryOS must preserve that payload outside the local
journal, it follows the exact `RecoveryDraft` creation rule in section 9.

A stale writer fails closed before Admission issuance. If an Admission was
already issued, the client follows the Admission reconciliation matrix in
section 6; it never treats a takeover response as proof that the earlier
command did not commit.

### 2.3 Ordering domains remain separate

The following orders are not interchangeable:

| Order | Scope and owner | Permitted use |
| --- | --- | --- |
| `local_intent_sequence` | one Project Scope Local Edit Journal, allocated atomically across tabs | preserve browser-local intent and deterministic reconciliation order |
| `writer_generation` | one Project, Server-owned | fence stale writers and identify the current writer |
| `AuthorActionSequence` | one Project Scope, Core-owned | order successful author-owned Core Transitions and undo |
| Project Activity position | one Project Scope, protocol/Core-owned | order public committed activity and resume projection |
| Authoritative and Proposal Heads | exact durable objects, Core-owned | prove current state and command preconditions |

Recorder sequence, IndexedDB order, timestamps, UUID order, HTTP arrival, and
SSE arrival never establish Project authority order.

## 3. IndexedDB Local Edit Journal

### 3.1 Store and entry identity

The Protected Web Client maintains one Project Scope-bound IndexedDB Local
Edit Journal. A single IndexedDB transaction allocates the next
`local_intent_sequence` across same-origin tabs and creates one opaque
`LocalJournalEntryId`; neither value is reused after deletion or takeover.

Each immutable completed-intent entry binds:

```text
LocalEditJournalEntry {
  local_journal_entry_id
  local_intent_sequence
  project_scope

  editor_session_id
  writer_generation
  client_session_binding_ref
  client_session_generation
  client_contract_revision
  security_policy_revision

  chapter_object_id
  target_refs
  base_snapshot_id
  base_activity_position
  expected_authoritative_heads
  expected_proposal_heads
  proposal_anchors
  observed_ownership_partition

  author_edit_units
  selection_snapshots
  retry_source
  editor_contract_revision
  undo_group_binding

  action_class: direct_editor_action
  api_major
  method
  route_template
  command_schema
  command_kind
  digest_profile
  idempotency_key
  anti_forgery_challenge_ref

  payload_chain
  payload_digest
  created_at
}
```

The entry records the browser-held admission-preparation inputs needed for
Server validation. It does not contain a nonce value. It never contains or
generates an `AuthorCommandAdmissionId`; only the Server may issue that
identity after the final request body and canonical digest exist. A later
Server response may attach an issued Admission reference to the entry's
append-only reconciliation record.

`author_edit_units`, selection, retry source, target/Head/Anchor facts,
ownership observation, editor contract, and undo grouping use the exact
semantics owned by the Manuscript state machine. The journal cannot choose
authoritative versus Proposal routing.

### 3.2 Bounded payload representation and checkpoints

The journal stores completed intent as a bounded chain of typed structural
patches over one exact materialized checkpoint. It does not copy the whole
chapter before and after every intent.

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
    covered_local_sequence
    normalized_primitives
    resulting_payload_digest
  }
}
```

Every retained entry must be reconstructable to the complete exact
author-edit payload without a network, cache, DOM, ProseMirror history, or
collected predecessor. Checkpoint replacement is atomic: the new
materialization and all still-required patch coverage become durable before
the old materialization or patches are eligible for collection. Patch count,
materialization cadence, byte ceilings, and performance targets are supplied
by the applicable Protocol Limit Profile and the measurement owner; this
contract does not invent numerical thresholds.

### 3.3 Atomic local durability

One IndexedDB transaction atomically:

1. validates the Project Scope store and schema;
2. allocates `local_intent_sequence`;
3. writes the completed entry, complete patch bytes, digests, and referenced
   checkpoint;
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

### 3.4 Closed entry axes

An entry uses separate append-only axes rather than an open-ended “local
disposition” field:

```text
Preparation =
  CollectingCompletedIntents
  | FinalRequestFrozen { request_digest_input_ref }
  | AdmissionIssued { author_command_admission_id }

Submission =
  NotSubmitted
  | InFlight
  | ReconciliationRequired

Settlement =
  Unsettled
  | PreAdmissionRefused { refusal_record_ref }
  | ReceiptSettled { receipt_ref, project_activity_position }
  | RequiresReconfirmation {
      author_command_admission_id
      reason
      recovery_draft_ref | None
    }

Projection =
  Pending
  | Converged { snapshot_id, activity_position, resulting_heads }
  | NeedsAttention { reason, author_surface_ref | None }

PayloadRetention =
  Retained
  | Eligible { eligibility_evidence }
  | Collected { collection_fence }
```

`PreAdmissionRefused` proves there is no Admission or Receipt. `InFlight` is
not settlement. `ReconciliationRequired` is used after post-admission
uncertainty and authorizes only the same read-only settlement query.
`ReceiptSettled` covers every typed Core result, including refusal, conflict,
and no effect. Projection and payload retention never override settlement.

## 4. Completed intent and pre-Admission coalescing

One completed semantic editor intent is the conservative journal and command
unit. Composition completion, paste, cut, drop, every structural primitive,
and every explicit command are unconditional boundaries.

Before Admission issuance, adjacent completed `direct_editor_action` intents
may be coalesced only while:

- every admission-preparation input named by Author Command Admission
  sections 3 and 5 is present and equal;
- every `ApplyAuthorEdit` semantic field, chapter, selection, retry source,
  target, ownership observation, Authoritative and Proposal Head, Anchor,
  editor-contract revision, and author-visible undo-group binding is equal;
- the current writer generation remains current;
- no entry is already in `AdmissionIssued`; and
- the applicable idle, intent, operation, and payload bounds remain proven.

Any difference or unverifiable fact freezes the earlier request. The combined
entry retains exact coverage of every source `local_intent_sequence`; coverage
is ordered, non-overlapping, and complete. Coalescing never alters local undo
grouping and never crosses an explicit command.

After the final body is frozen, the Server recomputes its canonical digest,
validates all bindings, claims idempotency, consumes the nonce record, and
issues and attaches one Admission. The final body, digest, sequence coverage,
and payload are immutable after that point. Later typing starts another entry
and can never merge into the admitted command.

## 5. Pending projection, chapter switching, and queueing

### 5.1 Pending Edit Projection

The Pending Edit Projection is rebuilt from one authorized durable Server
Snapshot plus retained journal payloads for the selected chapter. It is
immediate and non-authoritative. It never changes an Authoritative or Proposal
Head and never hides which bytes remain local.

The author-facing save state is derived:

| State | Exact meaning |
| --- | --- |
| `saving` | complete intent is durably journaled, but terminal settlement and author-facing projection convergence are not both proven |
| `saved` | terminal settlement is proven and the resulting non-attention state is visible from a Snapshot/Event position at or beyond that settlement |
| `needs_attention` | a typed terminal or recovery result requires an author decision, or current evidence cannot safely reconstruct/converge the projection |

`Accepted` HTTP acknowledgement, an idempotency record without terminal
settlement, a Receipt without author-facing convergence, and an Event without
matching settlement remain `saving` or `needs_attention`; none is `saved`.

Selection, Decorations, NodeViews, editor history, and cursor position are
presentation state. Exact selections and undo-group bindings retained by an
entry are command/recovery evidence, but restoring them does not make DOM state
durable truth.

### 5.2 Chapter switching

Before changing the selected chapter, the client atomically completes the
current semantic intent and its journal transaction or refuses the switch with
an explicit local-recovery state. It never “commits pending browser state” to
authority.

Each chapter projection is rebuilt from its own Snapshot/Head facts plus the
Project journal entries targeting that chapter. The Project retains one writer
generation and one `local_intent_sequence`; switching chapters does not create
another writer, reorder the queue, merge entries across chapters, or imply
Core settlement. Returning to a chapter restores its exact pending or
attention surfaces from journal and durable Server facts.

### 5.3 Submission queue

The current writer owns one Project-scoped queue ordered by the frozen entry's
lowest covered `local_intent_sequence`. Release 1 begins submission of a later
author command only after the earlier command reaches a terminal Admission
settlement and the current Heads needed by the later command have been
revalidated. An earlier `outcome_unknown`, gap, stale writer, or required
resync pauses the queue.

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
| HTTP `Accepted` | durable asynchronous operation reference only | nonterminal `saving`; observe the named settlement query and Project Activity |
| HTTP `Committed` | terminal `ReceiptSettled` and exact typed Receipt | project the Receipt result; wait for author-facing projection convergence if not already observed |
| HTTP `RequiresReconfirmation` | terminal Admission settlement with no Receipt or Core effect | show the exact reconfirmation reason and applicable preserved payload/Draft; a later author confirmation creates a new command and Admission |
| post-admission `outcome_unknown` Problem | no claim about Receipt presence; same Admission and settlement query | set `ReconciliationRequired`; forbid blind retry or a new command derived from the uncertain one |
| Project Activity before HTTP | committed Event position, not necessarily the client's matching acknowledgement observation | retain/deduplicate the Event, query settlement, and converge only after the exact Receipt/result is known |
| HTTP before Project Activity | settlement known, author-facing projection not yet converged | retain payload and wait/replay/query from the required activity position |

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
4. reconciles every admitted entry by Admission/idempotency/Receipt facts;
5. rebuilds each visible chapter from the Snapshot plus still-valid local
   entries; and
6. resumes strictly after the Snapshot position.

A Snapshot is a durable Server reading boundary, not a browser materialization
or authority copy. Resync never silently skips a gap, translates a cursor by
guessing, discards an unsettled payload, or turns local order into Project
order.

Projection convergence for one entry requires:

- terminal settlement;
- a Snapshot or processed Activity position at or beyond the settlement's
  `project_activity_position`;
- exact resulting Heads and every Receipt-allocated Draft, Proposal,
  lifecycle, condition, and Author Action reference reflected on the
  author-facing surface; and
- no unresolved earlier gap or incompatible writer generation.

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

### 8.3 Undo and reversal

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

A `RecoveryDraft` exists only when the StoryOS Host durably creates one Core
Draft Artifact with:

- Host-assigned `EditorRecovery` Creator;
- exact Project Scope, Editor Session, and writer generation;
- the complete structured author-edit payload and digest;
- exact journal entry/range and checkpoint evidence when available;
- exact Admission settlement, takeover, or in-memory recovery evidence; and
- one immutable Artifact Revision and provenance closure.

Creation causes no Core edit invocation and no Author Action. A browser-local
entry, in-memory text, copyable text, clipboard content, checkpoint, or pending
projection is never a `RecoveryDraft` or another Artifact.

StoryOS creates a `RecoveryDraft` only when the complete payload is available
and durable preservation is required because the healthy local continuation
cannot safely remain the sole recovery surface, such as terminal
`RequiresReconfirmation` with recoverable author-edit payload, takeover or
journal/schema recovery that must cross the local session boundary, or
explicit durable preservation before local collection. It does not create one
for every expired, stale, conflicted, no-effect, or unverifiable entry.
Incomplete or unverifiable text remains visibly incomplete local recovery
material and cannot be promoted into an Artifact.

A `RecoveryDraft` Retry is a new `ApplyAuthorEdit`/`DraftRetry` with an
explicit author confirmation and new Admission unless section 6's exact
same-Admission automatic branch still applies before Draft creation. Its
result and closure follow section 8.2.

## 10. Reload, crash, and takeover recovery matrix

| Last durable boundary | Reload/recovery result |
| --- | --- |
| before journal transaction commit | no durable entry; any surviving in-memory text is copyable local payload, not saved or a Draft |
| journal committed, before final request/Admission | rebuild complete intent; current writer may freeze and start a fresh Admission flow after all bindings revalidate |
| final request frozen, Admission issuance did not commit | validated absence of Admission/nonce consumption permits a fresh admission challenge for the same visible intent |
| Admission issued, settlement unverifiable | `outcome_unknown`; read-only reconciliation; no blind invocation |
| validated no-Receipt after Admission | follow only section 6.2's direct-edit equality or `RequiresReconfirmation` branch |
| Core committed before HTTP or Activity observation | exact Receipt/settlement/Heads survive; replay them and converge without another invocation |
| Receipt observed before matching Activity | retain payload and replay/query from the required Activity position |
| Activity observed before HTTP | retain/deduplicate Event and query the exact settlement; Event arrival alone does not settle the local entry |
| convergence proven before payload collection | entry becomes GC-eligible only if section 11's successor proof also holds |
| payload collected before reload | reconstruct from the recorded durable successor and retained collection fence; never from missing bytes |
| reload in stale writer generation | read-only; reconcile admitted entries, preserve unsubmitted payload, and require explicit takeover for new writing |
| takeover during unsettled work | old writer remains read-only; every admitted entry reconciles by its own Admission; unsubmitted complete payload remains local or becomes a `RecoveryDraft` only under section 9 |

## 11. Deterministic journal garbage collection

### 11.1 Eligibility

Journal payload bytes and patch/checkpoint dependencies are eligible for
collection only when all of these are proven:

1. the entry has terminal `ReceiptSettled`, `RequiresReconfirmation`, or
   `PreAdmissionRefused` evidence as applicable; it is neither `Unsettled` nor
   `ReconciliationRequired`;
2. the author-facing projection has converged to that terminal fact, including
   every resulting Head, Draft, Proposal condition, lifecycle event, control,
   and required Activity position;
3. the complete payload has an exact durable successor that is independently
   readable and digest-verified, or another retained complete journal
   materialization still covers it; and
4. no unsettled entry, checkpoint, retry, undo candidate, reconciliation
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
2. marks the exact entries and patch/checkpoint ranges `Eligible`;
3. writes one immutable local `collection_fence` containing entry/range
   identities, payload digests, successor references, Admission/Receipt or
   refusal/reconfirmation references, convergence position, and reason;
4. deletes only the covered payload bytes and now-unreferenced checkpoint or
   patch material; and
5. marks those ranges `Collected`.

The compact entry identity, command digest/idempotency binding, Admission
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
- atomic journal sequence/entry/checkpoint durability before submission and
  complete reconstruction after every local fault cut;
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
- chapter switching with pending entries and one Project queue;
- secondary read-only sessions, explicit takeover, stale-writer fencing, and
  preservation without automatic Draft fabrication;
- every `ApplyAuthorEdit`, source-Draft, explicit decision, exact retry, undo,
  reversal, and stale-control row in section 8;
- positive classification of Admission/refusal/reconciliation evidence,
  Receipts, Draft Artifacts, Proposal conditions, local payload, projections,
  and authority;
- `RecoveryDraft` creation only from complete payload plus exact
  `EditorRecovery` evidence; and
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
2. Every completed intent is durably journaled before submission and remains
   exactly reconstructable until safe collection.
3. Local order, Project Activity order, Author Action order, Heads, and writer
   generation are distinct typed orders and never substitute for one another.
4. The Web Client never generates or supplies an
   `AuthorCommandAdmissionId`; the Server issues and attaches it only after
   the final request and digest are fixed.
5. Coalescing is pre-Admission, bounded, equal-binding-only, and never crosses
   a semantic or explicit-command boundary.
6. HTTP `Accepted` is nonterminal. Only `Committed` with a typed Receipt or
   `RequiresReconfirmation` closes an Admission.
7. Post-admission uncertainty claims neither Receipt presence nor absence,
   forbids blind retry, and reconciles through the same settlement query.
8. Automatic recovery invocation requires validated no-Receipt evidence, the
   same unexpired direct edit, and equality of every Admission, Core, and
   journal binding.
9. Pending projection, Snapshot, journal, cache, DOM, and network state never
   become authority or a settlement oracle.
10. Every Core result, source-Draft disposition, undo, and reversal projects
    only its exact permitted controls; a closed source has no stale retry or
    expansion control and no result fabricates Proposal Acceptance.
11. A `RecoveryDraft` is a durable Core Draft Artifact created only by
    Host-assigned `EditorRecovery` from complete payload and exact evidence.
12. Journal payload collection requires terminal settlement, author-facing
    convergence, an exact durable complete successor, and no remaining
    dependency; unknown, unsettled, or only-complete-copy payloads remain
    retained.
