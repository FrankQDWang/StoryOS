# Manuscript Revision and Proposal State Machine

- Status: accepted
- Wayfinder resolution: [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46)
- Canonical glossary: [`CONTEXT.md`](../../CONTEXT.md)
- Parent domain model: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Ownership and deployment decision: [ADR 0004: Adopt a PostgreSQL Service and Project Isolation Boundary](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)
- Research input: [Manuscript revision and Proposal state-machine source audit](../research/manuscript-revision-proposal-state-machine-source-audit.md)
- Editor evidence: [Tiptap / ProseMirror durable Proposal mechanics](../research/tiptap-prosemirror-proposal-mechanics.md)

## 1. Scope and authority

This specification defines the logical StoryOS Core contract for manuscript
identities, immutable revisions, Proposal state, author and Agent edits,
validation, Acceptance, rejection, replanning, undo, idempotency, conflict, and
crash recovery. It refines the parent domain model without changing its primary
authority rule: only a Direct Author Action, Acceptance, or safe author-approved
compensation changes Authoritative State.

The following are deliberately outside this contract and are implemented by
the [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md):

- PostgreSQL tables, indexes, constraints, transaction isolation, durability, and locking policy;
- separately stored payload layout if any, compression, physical deduplication, and encryption;
- backup, restore, migration execution, and portability;
- concrete cache eviction and retention periods.

Tiptap and ProseMirror are reconstructible editor adapters. Their DOM,
Selection, plugin state, Decorations, NodeViews, history branches, and
document-wide integer positions are never Core truth.

## 2. Primitive contracts

### 2.1 Durable identities

Every durable entity and record identity is a distinct UUIDv7 newtype. At
minimum this includes:

```text
UserId                       ProjectId
ManuscriptObjectId           ManuscriptBlockId
AuthoritativeRevisionId      AuthoritativeCommitId
ProposalId                   ProposalRevisionId
ProposalOperationId          ProposalGenerationId
DomainReceiptId              ValidationReceiptId
AcceptanceReceiptId          UndoAcceptanceReceiptId
AuthorUndoReceiptId          AuthorCommandAdmissionId
EditorSessionId              ProjectionCheckpointId
EditorInputFenceId
DraftId                      IdempotencyKey
DomainEventId
```

Core uses an RFC 9562-conforming CSPRNG implementation and enforces uniqueness.
Types are not interchangeable even though their wire representation is the
same. Code must never parse or sort the UUID timestamp to decide freshness,
causality, authority, conflict, project order, or authorization. Knowing an ID
grants no capability.

### 2.2 Project Scope

Every entity, record, command, idempotency binding, sequence, Head, reference,
query, and recovery projection in this state machine belongs to one trusted
Project Scope:

```text
ProjectScope {
  owner_user_id: UserId
  project_id: ProjectId
}
```

The Project has one owning User acting as its Project Author. Project Scope is
resolved from trusted Host state and must match every referenced object; a
caller field, ProjectId alone, globally unique object ID, or editor session
cannot establish ownership. Any missing or mismatched member fails before a
domain attempt and cannot produce or retrieve a Receipt. Ownership transfer,
shared ownership, and multi-author editing are outside the current Foundation.

### 2.3 Ordered integers

The logical integer types are:

```text
AuthoritativeCommitSequence = u64
AuthorActionSequence        = u64
ProposalStreamSequence      = u64
SchemaVersion               = u32
CoordinateVersion           = u32
BlockTokenOffset            = u32
```

All `u64` values use canonical unsigned decimal strings in JCS inputs and JSON
wire contracts so Rust and TypeScript do not cross JavaScript's exact-integer
limit. A sequence never wraps or resets. Exhaustion fails closed and requires a
versioned migration before further writes of that kind.

`BlockTokenOffset` is zero-based and block-relative. An anchor satisfies
`from <= to`; equality represents insertion at a point. Offset overflow, an
out-of-bounds range, or unsupported coordinate version is a typed conflict.

### 2.4 Audit time

Every durable record carries an audit timestamp supplied by the trusted Host.
Time is presentation and forensic evidence only. It is not a concurrency
precondition, sequence, or source of causal order.

## 3. Independent Project Scope order

### 3.1 Authoritative Commit order

Each Project Scope has one `AuthoritativeCommitSequence`, beginning at 1. A Core
Transition that successfully changes Authoritative State allocates exactly the
next value and appends one `AuthoritativeCommit`. Failed, refused, conflicted,
invalid, and no-effect attempts allocate none. Transaction rollback leaves no
gap in the committed sequence.

The Commit binds:

```text
AuthoritativeCommit {
  project_scope
  commit_id
  sequence
  author_action_sequence
  actor
  cause
  prior_and_resulting_revision_pairs
  created_at
}
```

### 3.2 Author Action order

Each Project Scope separately has one `AuthorActionSequence`, beginning at 1. A
successful author-owned Core Transition allocates one value even when it
creates several Revisions, Receipts, or a Commit. Exact idempotent retry returns
the existing value. Refused, conflicted, invalid, no-effect, recovery-Draft, and
pure Agent-generation transitions allocate none.

The canonical result record binds:

```text
AuthorActionRef {
  sequence
  kind
  domain_record_ref
  disposition:
    Forward {
      undo:
        Reversible { undo_handler_kind, source_refs }
        | Barrier { reason }
    }
    | Compensation {
        source_author_action_sequence
        domain_receipt_ref
      }
}
```

This is a logical order coordinate, not a mutable action ledger. A storage
implementation may maintain a derived lookup index, but the canonical Revision,
Receipt, or Commit retains the binding.

The **Author Undo Frontier** is the latest `Forward` entry not named by a
successfully committed `Compensation`. A Compensation remains in sequence for
audit but is never an undo candidate. A Reversal Proposal is a new Forward
action and does not compensate the unsafe source action merely by being
created. At most one committed Compensation may name a given Forward action;
concurrent attempts serialize on the expected Frontier.

## 4. Immutable revisions

### 4.1 Common Revision Envelope

Every Authoritative Revision and Proposal Revision uses the same logical
envelope:

```text
RevisionEnvelope<ObjectId, RevisionId, Payload> {
  project_scope: ProjectScope
  object_id: ObjectId
  revision_id: RevisionId
  parent_revision_id: RevisionId | None
  revision_kind
  schema_version: SchemaVersion
  creator
  cause
  created_at
  payload: Payload
  payload_digest: DigestValue
}
```

The first revision alone has no parent. Every append supplies an exact expected
current Revision; a successful append records that matched Revision as its
parent. A stale append writes no Revision. One object identity has one
single-parent linear history: there is no last-write-wins, automatic rebase, or
side branch. Alternative work receives a new Proposal or Artifact identity with
explicit Provenance.

Physical inline-versus-blob placement is not part of this envelope.

### 4.2 Authoritative object identity

Stable top-level manuscript block identity follows the canonical split, join,
transfer, retype, and restoration rules in `CONTEXT.md`. Equality of text never
proves object identity. Authoritative Revision identity and payload digest never
replace the stable manuscript object or block identity.

## 5. Canonical digest profiles

### 5.1 Digest value

The closed manuscript, Proposal, anchor, and Core-command boundaries use
SHA-256 over UTF-8 RFC 8785 JCS:

```text
DigestValue {
  algorithm: "sha256"
  profile
  value_hex_lowercase
}
```

Each JCS input contains its profile name, schema version, coordinate version
when applicable, and the exact typed fields covered by that purpose. Required
initial profiles are:

```text
storyos.manuscript-payload.jcs.v1
storyos.proposal-revision-payload.jcs.v1
storyos.proposal-anchor-base-slice.jcs.v1
storyos.command.<command-kind>.jcs.v1
```

The schemas enforce I-JSON, duplicate-free names, valid Unicode scalar data,
and exact schema fields. Unknown fields, lone surrogates, non-finite numbers,
and non-conforming values fail before hashing. Wide integers are canonical
decimal strings. The initial closed contracts contain no floating-point domain
values.

Authoritative prose preserves its exact accepted Unicode sequence. No digest or
revision path applies implicit NFC or NFKC normalization. Search indexes may
derive normalized text without changing authoritative bytes or offsets.

Rust and TypeScript implementations must share committed golden vectors for
every profile. Equal bytes under different profiles do not assert equal
semantics.

## 6. Proposal payload and state

### 6.1 Stable Operations

A Proposal Revision contains a nonempty ordered collection of stable
`ProposalOperationId` values. Domain semantics, not visual diff hunks, define
the order. Each incarnation records its exact target identities, base
Revisions, preconditions, typed candidate payload, Anchors when applicable, and
candidate/base digests.

Applied and rejected incarnations are frozen. A new Revision may edit only
pending Operations. Reopening retains an Operation ID only when target and
semantic identity are unchanged; otherwise it creates a new ID with Provenance.

### 6.2 Inline Proposal Anchors

Each `InlineEditProposal` Operation contains one or more ordered anchors:

```text
ProposalAnchor {
  manuscript_block_id: ManuscriptBlockId
  base_authoritative_revision_id: AuthoritativeRevisionId
  manuscript_schema_version: SchemaVersion
  coordinate_profile: "prosemirror-token-utf16.v1"
  from: BlockTokenOffset
  to: BlockTokenOffset
  boundary_profile: "exclusive-authoritative-edges.v1"
  base_slice_digest: DigestValue
}
```

The base-slice digest covers the structural block identity and type, schema and
coordinate versions, offsets, and exact base slice; it is not a visible-text
hash. Both Proposal edges belong to adjacent Authoritative State, and only
input strictly inside the range edits the Proposal. Multi-block Operations use
ordered block-relative anchors, never one document-wide range.

### 6.3 Four orthogonal state axes

```text
generation: generating | ready_partial | ready
validation: pending | valid | invalid | conflicted
closure: open | withdrawn | superseded
operation resolution: pending | applied | rejected
```

Retention remains separate. `partially_resolved`, `resolved`, completion,
partial application, and Acceptance Eligibility are derived projections. There
is no Proposal-level `accepted`, `rejected`, or `stale` state.

### 6.4 Exhaustive transitions

Unlisted transitions are invalid.

| Axis | From | To | Required command or fact |
| --- | --- | --- | --- |
| generation | `generating` | `ready` | current Generation completes |
| generation | `generating` | `ready_partial` | author pause, interruption, or recoverable termination |
| generation | `ready_partial` | `generating` | explicit continuation with new Generation ID |
| generation | `ready_partial` | `ready` | author explicitly completes current content |
| generation | `ready` | `generating` | explicit continue/regenerate with new Generation ID |
| validation | `pending` | `valid` | current Core validation succeeds |
| validation | `pending` | `invalid` | current content violates its own contract |
| validation | `pending`, `valid`, or `invalid` | `conflicted` | exact target proof no longer holds |
| validation | `valid` | `pending` | author edits pending content in a new Proposal Revision |
| validation | `invalid` | `pending` | corrected new Proposal Revision |
| validation | `conflicted` | `pending` | explicitly replanned new Proposal Revision |
| closure | `open` | `withdrawn` | `WithdrawProposal` |
| closure | `withdrawn` | `open` | explicit reopen with new Revision |
| closure | `open` or `withdrawn` | `superseded` | `SupersedeProposal` |
| resolution | `pending` | `applied` | successful Acceptance |
| resolution | `pending` | `rejected` | `RejectProposalOperations` |
| resolution | `rejected` | `pending` | explicit reopen in a new Revision |
| resolution | `applied` | `pending` | successful Undo Acceptance in a new Revision |

`superseded` is terminal. Content correction, replan, rejected-operation
reopen, and Undo Acceptance append new Proposal Revisions. A content Revision
starts validation at `pending`; Core may record an immediately proven
structural conflict in the same Transition. Closure never rewrites Operation
resolution, and resolution never rewrites Retention.

### 6.5 Validation and conflict

Proposal Invalidity means an exact Proposal Revision violates its own schema,
Operation contract, or domain invariants against its declared base. Proposal
Conflict means an internally well-formed Revision can no longer prove its exact
target, base, Anchor, or preconditions against current Authoritative State.

Any of the following conflicts:

- a missing or duplicate target Block ID;
- any change to a referenced target Authoritative Revision, including a change
  outside the proposed range in the same Block;
- unsupported schema, coordinate, boundary, or digest profile;
- a deleted, ambiguous, inverted, or out-of-bounds Anchor;
- a base-slice digest mismatch;
- split, join, move, retype, or structural range reshaping;
- another unresolved ownership reservation on the Block.

Core never silently maps, rebases, merges, or revalidates the old Revision
against a changed Head. `ReplanProposal` appends a new Revision with current
base Revisions, Anchors, digests, and preconditions, retaining an Operation ID
only when semantic identity is proven unchanged.

## 7. Command contracts

### 7.1 Common command rules

Every schema-valid domain command has a version, exact Project Scope, typed
cause, and idempotency key. Author commands additionally carry an immutable
`AuthorCommandAdmissionId` governed by
[Author Command Admission](author-command-admission.md).

The trusted requester User and command Project Scope must match every input,
Head, Revision, Receipt, checkpoint, idempotency record, and producer cause.
Scope validation precedes lookup and mutation so an opaque ID cannot become a
cross-project existence oracle.

The v1 producer cause is exhaustive:

```text
ProducerCause =
  AuthorCommandAdmission { author_command_admission_id }
  | EditorInputFence { editor_input_fence_id }
  | AgentRunStep { run_step_id }
  | ToolCall { tool_call_id }
```

MCP servers and extensions produce through a ToolCall cause.

Core computes the command digest; a caller-provided digest is never trusted.
Within one Core Transition:

- same idempotency key and same digest returns the original Receipt;
- same key with another digest returns `IdempotencyConflict` and creates no
  second Receipt or effect;
- exact retry is a Receipt lookup, not another attempt.

Malformed schemas and unauthorized access fail before entry into a domain
attempt and produce no domain Receipt. Once a valid author command enters its
first domain attempt, success, refusal, invalidity, conflict, and no effect each
produce one immutable Receipt.

### 7.2 ApplyAuthorEdit

Every submitted semantic editor unit uses this command kind; the client cannot
choose an authoritative or Proposal write endpoint. The Web Editor Session
contract owns how completed local intents are grouped into bounded submissions.

```text
ApplyAuthorEdit {
  command_schema_version
  project_scope
  editor_session_id
  writer_generation
  normalized_edit_intent
  selection_snapshot
  expected_authoritative_heads
  expected_proposal_heads
  author_command_admission_id
  idempotency_key
}
```

The v1 normalized intent is an ordered nonempty list of these typed primitives:

```text
ReplaceSelection { exact structured slice }
SplitBlock
JoinBlocks
MoveBlock
RetypeBlock
```

Typing, deletion, cut, paste, drop, and IME output normalize to
`ReplaceSelection`; an empty replacement deletes. The Local Edit Journal
preserves completed browser intents in order. The production editor-session
prototype selects the bounded submission cadence that preserves complete IME
input, author-visible undo, crash recovery, and the performance envelope.

Core recomputes ownership from current Heads, Anchors, and reservations. Raw
ProseMirror Steps and browser events may be retained as bounded diagnostics but
are not command semantics. The exhaustive result is:

```text
AuthoritativeApplied { revisions, commit, author_action_sequence }
ProposalRevised { proposal_revision, author_action_sequence }
RefusedToDraft { refused_edit_draft }
Conflicted { current_heads, reasons }
NoEffect { reason }
```

One command is never split. A mixed Authoritative/Proposal edit changes neither
target and creates a `RefusedEditDraft`. A Proposal edit appends a Revision and
resets validation. An authoritative edit appends Revisions and exactly one
Commit.

### 7.3 Proposal generation

One Proposal has at most one active writer. Each Agent production attempt has a
new `ProposalGenerationId` and owns a strictly continuous stream sequence
starting at 1.

```text
AppendProposalGenerationBatch {
  command_schema_version
  project_scope
  proposal_id
  generation_id
  stream_seq
  expected_previous_stream_seq
  expected_proposal_revision_id
  expected_candidate_digest
  operation_reservation
  batch_payload
  batch_digest
  producer_cause
  idempotency_key
}
```

`(generation_id, stream_seq)` is the batch identity. An exact duplicate returns
the existing outcome; a different digest is a protocol conflict. Gaps wait and
cannot be applied out of order. An admitted batch may touch only its own
reservation and appends one durable Proposal Revision before it becomes a
recoverable review state. Provider token events are Model Stream evidence, not
Proposal batches.

At the first author input signal, the editor adapter synchronously closes its
runtime Agent-write gate before another Agent dispatch may touch ProseMirror.
The Host then issues:

```text
PauseProposalGeneration {
  command_schema_version
  project_scope
  proposal_id
  generation_id
  expected_proposal_revision_id
  expected_candidate_digest
  last_applied_stream_seq
  projection_checkpoint_id
  editor_input_fence_id
  editor_session_id
  writer_generation
  local_intent_range
  idempotency_key
}
```

One Core Transition binds the exact current durable candidate Head, records
generation `ready_partial`, and persists the admitted-through Pause Fence and
Receipt/event. It never promotes provisional, unadmitted editor bytes into a
Revision. Batches above the fence remain Run evidence and are permanently
ineligible for Proposal or editor replay. `compositionend` never resumes
generation. Explicit continuation creates a new Generation ID. This automatic
safety Transition uses an `EditorInputFence` cause and does not allocate an
Author Action Sequence or consume an Author Command Admission. The eventual
successful `ApplyAuthorEdit` has its own admission and is the gesture's one
Forward action. A crash after the fence but before that edit leaves the safety
fact intact without inventing an author action.

### 7.4 Validate and replan

```text
ValidateProposal {
  command_schema_version
  project_scope
  proposal_id
  proposal_revision_id
  expected_target_revisions
  validator_contract_version
  cause
  idempotency_key
}

ReplanProposal {
  command_schema_version
  project_scope
  proposal_id
  conflicted_proposal_revision_id
  expected_current_proposal_head
  expected_current_target_revisions
  replacement_operations
  producer_cause
  idempotency_key
}
```

Only Core produces a Validation Receipt. It binds the exact Proposal Revision,
target Revisions, schema checks, domain invariants, preconditions, validator
version, and result. Replan appends a new pending Revision; it never edits the
old conflict or Receipt.

### 7.5 AcceptProposal

```text
AcceptProposal {
  command_schema_version
  project_scope
  proposal_id
  proposal_revision_id
  validation_receipt_id
  selected_operation_ids
  expected_target_revisions
  author_command_admission_id
  idempotency_key
}
```

Selections are nonempty, duplicate-free sets. Their canonical digest order is
by `ProposalOperationId`; execution order comes from domain semantics or Bundle
dependencies, never client array order. Expected target Revisions are complete,
duplicate-free, and canonically ordered by typed target identity.

After current access validation, Acceptance executes in this order:

1. Compute the canonical command digest and resolve idempotency.
2. Validate the exact Author Command Admission for a first attempt.
3. Require the named Proposal Revision to be current, retained, `ready`, and
   `open`.
4. Require the named Validation Receipt to be `valid` for that exact Revision
   and exact target set.
5. Require every selected Operation to be pending and all dependencies met.
6. Compare every expected target Head and revalidate Anchors, base-slice
   digests, permissions, and preconditions.
7. Dry-run the entire selection through the closed StoryOS domain handler and
   validate resulting domain invariants.
8. Commit all selected effects atomically.

Acceptance never automatically replans or replaces a Validation Receipt.

### 7.6 Other Proposal decisions

The remaining Proposal decisions are exact-head, idempotent commands:

```text
RejectProposalOperations
WithdrawProposal
ReopenWithdrawnProposal
SupersedeProposal
ReopenRejectedOperations
CompleteReadyPartialProposal
ContinueProposalGeneration
```

Each carries exact Project Scope, exact Proposal ID and Revision, complete selected
Operation IDs when applicable, current target expectations, a typed producer
cause, and an idempotency key. Reject, author reopen, supersede, explicit
completion, and continuation require Author Command Admission. Withdrawal accepts
either Author Command Admission or the exact current producer cause. Rejection preserves a typed
author reason. Supersession binds the replacing Proposal and is terminal for
the replaced Proposal. Reopen and continuation append a Revision or lifecycle
event under the exhaustive state table and require validation again where
specified.

## 8. Acceptance and Receipts

### 8.1 Acceptance result

The exhaustive result for an ordinary domain Proposal is:

```text
AcceptanceResult =
  Applied {
    authoritative_commit_id
    applied_operation_ids
    prior_authoritative_revision_ids
    resulting_authoritative_revision_ids
  }
  | Invalid { violations }
  | Conflicted { conflicts, current_target_revisions, replan_required }
  | Refused { reason }
  | NoEffect { reason }
```

Only `Applied` changes Authoritative State and resolves the selected Operations
as applied. `Invalid` projects validation invalid; `Conflicted` projects it
conflicted. `Refused` covers state or selection ineligibility without changing
content validity. `NoEffect` creates no Commit and leaves Operations pending.
All reasons are versioned exhaustive types; free text is presentation only.

Bundle Receipts compose exact child Receipts under the existing atomic or
ordered-independent Bundle policy.

### 8.2 Acceptance Receipt

Every first Acceptance attempt produces:

```text
AcceptanceReceipt {
  receipt_id
  project_scope
  command_digest
  idempotency_key
  author_command_admission_id
  proposal_id
  proposal_revision_id
  validation_receipt_id
  selected_operation_ids
  expected_target_revisions
  prior_authoritative_revisions
  resulting_authoritative_revisions
  authoritative_commit_ids
  author_action_sequence | None
  child_receipts
  result
  created_at
}
```

An exact retry returns this object unchanged. Infrastructure failure or lost
acknowledgement is not an Acceptance Result.

## 9. Core Transition atomicity

One logical Core Transition performs the complete write set:

1. Resolve idempotency and validate exact Heads, Revisions, command digest, and
   preconditions.
2. Append immutable Authoritative and Proposal Revisions as applicable.
3. Append an Authoritative Commit and allocate its sequence when authority
   changes.
4. Allocate an Author Action Sequence for a successful author-owned change.
5. Append Operation-resolution and lifecycle events.
6. Advance normalized current Heads and derived projections.
7. Append the immutable Domain Receipt.
8. Persist required outbox or wakeup intent.
9. Commit once and publish success only afterward.

All effects become visible together. A validation refusal or conflict may
commit only its no-change Receipt and conflict projection. A failure before
commit exposes no partial domain effect. External notification is delivered
from durable outbox intent and is never transaction truth.

## 10. Undo and reapplication

### 10.1 Unified newest-first routing

The public coordinator command is:

```text
UndoLatestAuthorAction {
  command_schema_version
  project_scope
  expected_author_undo_frontier_sequence
  author_command_admission_id
  idempotency_key
}
```

Core requires the exact current Author Undo Frontier. A mismatch is a conflict.
A Barrier returns unavailable and cannot be skipped. A Reversible Forward
action routes to its registered typed handler:

- Direct Author Action -> compensating Authoritative Revision and Commit;
- author Proposal edit -> restoration in a new Proposal Revision;
- Acceptance -> `UndoAcceptance`;
- rejection -> rejected-operation reopen;
- withdrawal -> typed Proposal reopen.

When a typed handler directly reverses its source, the same Transition records
an immutable `AuthorUndoReceipt` and appends a `Compensation` Author Action
entry naming the source sequence and exact domain Receipt. That Compensation
is never a future undo candidate. If the handler instead returns
`ReversalRequired`, creation of the Reversal Proposal is a new `Forward`
action and the source remains uncompensated. ProseMirror history may supply a
session-local inverse candidate only for the exact Frontier; Core still
verifies durable history. A handlerless action is a Barrier.

### 10.2 UndoAcceptance

```text
UndoAcceptance {
  command_schema_version
  project_scope
  acceptance_receipt_id
  expected_current_target_revisions
  expected_current_proposal_head
  author_command_admission_id
  idempotency_key
}
```

Direct compensation requires all of these:

- the source Acceptance Result is `Applied`;
- no distinct successful Undo already compensated it;
- the source Acceptance is the current Author Undo Frontier;
- every affected target's current Head is exactly the resulting Revision in the
  source Receipt;
- every current payload digest matches that resulting Revision;
- prior Revisions, payloads, digests, schema, and compensation handler remain
  usable.

Success appends new compensating Revisions whose parents are the current Heads
and whose payloads equal the source prior authoritative payloads. It assigns
new Revision IDs and one new Commit; it never deletes the source Commit,
reactivates an old Revision ID, or mutates a Receipt.

Safe Proposal lineage appends a reopening Revision. Proposal lineage drift may
derive a new Proposal without blocking otherwise safe authoritative
compensation. Any non-exact authoritative target Head forbids direct
compensation even when a range appears non-overlapping: Core creates a Reversal
Proposal when a safe inverse can be expressed, otherwise returns unavailable.

```text
UndoAcceptanceResult =
  Compensated { authoritative_commit_id, proposal_ref | None }
  | ReversalRequired { reversal_proposal_ref }
  | Unavailable { reason }
```

There is no durable generic redo and no `RedoAcceptance`. Reapplication is a
new Author Edit, reopen, or Acceptance against current state with a new Author
Command Admission, idempotency key, Commit, and Receipts.

## 11. Editor ownership and support boundary

Production Proposal editing is admitted only for the current Editor Support
Profile: desktop Chrome with Chinese and English input, exact compatible editor
contract versions, prior evidence, and passing live non-destructive capability
checks. Unknown or violated evidence selects Proposal Safe Mode.

The adapter closes its in-memory Agent-write gate synchronously on the earliest
author input signal. This runtime mechanism protects active browser composition
but is not durable truth. Core Heads, Proposal Pause Fences, command Receipts,
and sequence checks provide correctness.

Native undo/redo, UniqueID repair, paste, cut, drop, drag, deletion, and IME
output all re-enter the same ownership classifier. Agent batches remain outside
ProseMirror history. Immediate browser display, IndexedDB journal continuity,
writer generation, acknowledgement/Event convergence, and resynchronization
are owned by
[Web Editor Session, Synchronization, and Recovery Semantics](web-editor-session-synchronization-and-recovery-semantics.md).

## 12. Recovery

Recovery trusts only validated durable Core facts:

- current Authoritative Heads and Commit sequence;
- current Proposal Heads, state axes, and Operation resolutions;
- Domain, Validation, Acceptance, Undo, and Author Undo Receipts;
- Proposal Generations, stream events, and Pause Fences;
- Anchors, digests, schema/coordinate profiles, and durable outbox intent.

An editor projection checkpoint is a cache keyed by every exact Head, digest,
and contract version used to build it. A full match permits reuse; any mismatch
discards the whole checkpoint and rebuilds from Core. The Local Edit Journal
separately owns unsettled browser intents. DOM, Selection, Decorations,
NodeViews, plugin state, and ProseMirror history are not restored as domain
truth.

For a command idempotency key:

- a Receipt proves the Transition committed and the UI replays that result;
- with validated storage and no Receipt, the Transition did not commit;
- an uncommitted direct author edit preserved in the Local Edit Journal may be
  retried idempotently; stale or unprovable intent becomes a non-authoritative
  Recovery Draft requiring explicit retry or discard;
- Acceptance and Undo without a Receipt are shown as not committed and are
  settled as requiring reconfirmation and never automatically executed after
  restart.

For a streaming Proposal:

- `ready_partial` or a Pause Fence permanently excludes later batches;
- `generating` may rebuild only from an exact Head, digest, contiguous sequence,
  compatible checkpoint, and no ambiguous runtime-gate window;
- ambiguity between a closed runtime gate and a durable fence creates a
  Proposal Recovery Conflict;
- missing sequences, digest mismatch, invalid Anchors, duplicate Block IDs, or
  unsupported contracts select Proposal Safe Mode and disable Acceptance.

Recovery never infers success from a network process, model stream, editor
cache, timestamp, or absent error. Physical corruption detection and store
repair belong to the [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md);
until repaired, Core remains fail-closed.

## 13. Normative commands, events, and records

Storage and protocol adapters may namespace serialization, but must preserve
these one-to-one semantics:

| Command or decision | Durable event or record |
| --- | --- |
| apply an Author Edit to authority | `AuthoritativeAuthorEditApplied` |
| apply an Author Edit to a Proposal | `ProposalAuthorEditApplied` |
| refuse mixed ownership | `RefusedEditDraftCreated` |
| start or continue generation | `ProposalGenerationStarted` |
| admit a generation batch | `ProposalGenerationBatchAdmitted` |
| pause generation | `ProposalGenerationPaused`, `ProposalPauseFenceRecorded` |
| finish generation | `ProposalGenerationCompleted` |
| validate Proposal | `ProposalValidationSettled` |
| detect target drift | `ProposalConflictDetected` |
| replan Proposal | `ProposalReplanned` |
| accept selected Operations | `AcceptanceAttemptSettled`, `ProposalOperationsApplied` |
| reject selected Operations | `ProposalOperationsRejected` |
| reopen rejected Operations | `ProposalOperationsReopened` |
| withdraw or reopen | `ProposalWithdrawn`, `ProposalReopened` |
| supersede | `ProposalSuperseded` |
| undo Acceptance | `UndoAcceptanceSettled` |
| route unified undo | `AuthorUndoRouted`, `AuthorUndoSettled` |
| allocate author-owned order | `AuthorActionRecorded` |
| preserve an uncommitted edit | `RecoveryDraftCreated` |
| expose ambiguous Proposal recovery | `ProposalRecoveryConflictDetected` |

Every event binds its exact Project Scope, owning identity, expected prior Revision or state,
idempotency key, cause, correlation references, author-action sequence when
applicable, audit time, and controlled payload references or digests. Unknown
event variants fail closed until a versioned migration or compatible reader is
available.

## 14. Normative invariants

1. Durable identity never implies authority, order, causality, or capability.
2. One object identity has one immutable linear Revision history.
3. Authoritative Commit order and Author Action order are independent explicit
   Project Scope-local sequences.
4. A digest has meaning only under one exact versioned Digest Profile.
5. One Core command result is atomic with its Revisions, Heads, Receipt, events,
   sequences, resolutions, and outbox intent.
6. Proposal generation, validation, closure, and per-Operation resolution never
   collapse into one status.
7. Any referenced target Revision change conflicts the old Proposal Revision.
8. Tiptap state and document-wide positions are projections, never durable
   Proposal identity or authority.
9. One Author Edit is classified and committed as a whole.
10. Author input permanently fences the current Agent generation before the
    candidate is edited.
11. Acceptance binds exact Author Command Admission, Proposal Revision,
    Validation Receipt, pending selection, target Revisions, and idempotency
    input.
12. Direct Undo Acceptance requires exact resulting target Heads; otherwise it
    creates a Reversal Proposal or returns unavailable.
13. Unified undo acts only on the exact Author Undo Frontier; Compensation
    actions are unique per source and are not candidates, and a Barrier is
    never skipped.
14. Recovery reconstructs from Core facts and exposes ambiguity instead of
    replaying or guessing.
15. Every entity, command, reference, Head, Receipt, event, sequence,
    idempotency record, and recovery projection validates one exact Project
    Scope; no opaque ID, digest, or client assertion permits cross-scope access.
