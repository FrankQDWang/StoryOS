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

Every submitted Author Edit uses this command kind; the client cannot choose an
authoritative or Proposal write endpoint. The Core contract owns the semantic
submission boundary. The Web Editor Session contract owns journal persistence,
dispatch, projection, acknowledgement ordering, and takeover, but may not alter
this boundary.

```text
ApplyAuthorEdit {
  command_schema_version
  project_scope
  editor_session_id
  writer_generation
  chapter_object_id
  author_edit_units
  submission_bindings {
    target_refs
    observed_ownership_partition
    expected_authoritative_heads
    expected_proposal_heads
    editor_contract_revision
    undo_group_binding
  }
  retry_source:
    FreshEditorIntent
    | DraftRetry { draft_id, draft_revision_id, selected_payload_range }
  author_command_admission_id
  idempotency_key
}

AuthorEditUnit {
  normalized_primitives
  selection_snapshot
}
```

`author_edit_units` is an ordered nonempty list of completed semantic editor
intents. Each unit's `normalized_primitives` is an ordered nonempty list of:

```text
ReplaceSelection { exact structured slice }
SplitBlock
JoinBlocks
MoveBlock
RetypeBlock
```

Typing, deletion, cut, paste, drop, and completed IME output normalize to
`ReplaceSelection`; an empty replacement deletes. Raw ProseMirror transactions,
Steps, and browser events may be retained as bounded diagnostics but are not
command semantics.

One command per completed semantic intent is the conservative correctness
boundary. Bounded idle coalescing may place consecutive completed intents in one
command only while Project Scope, chapter, target, observed ownership, every
expected Head, writer generation, prospective admission binding,
admission-expiry boundary, editor-contract revision, and author-visible undo
group are exactly equal. Composition, paste, cut, drop, every structural
primitive, an explicit command, or any unequal binding flushes the current
unit. A current Protocol Limit Profile supplies the maximum idle time, intent
count, operation count, and payload; absence of a proven bound falls back to one
intent per command. Coalescing occurs before admission issuance. The final
combined command receives one Admission over its exact digest; no post-admission
merge may change that digest. Browser history grouping is independent and never
changes this commit boundary.

Core treats `observed_ownership_partition` as a stale-detection precondition,
not a grant. At the first domain attempt it recomputes ownership from current
Heads, Anchors, unresolved reservations, and the closed Direct Author Action
rules. The exhaustive result is:

```text
ApplyAuthorEditResult =
  AuthoritativeApplied {
    authoritative_revisions
    authoritative_commit_id
    author_action_sequence
  }
  | ProposalRevised {
      proposal_revision_id
      author_action_sequence
    }
  | RefusedToDraft {
      refused_edit_draft_id
      refused_edit_draft_revision_id
    }
  | Conflicted {
      current_authoritative_heads
      current_proposal_heads
      reasons
      proposal_conflict_ref | None
    }
  | NoEffect { reason }
```

One command is never split or partially applied. All primitives classified as
Direct Author Action append the required Authoritative Revisions and exactly
one Authoritative Commit. All primitives classified inside one exact pending
Proposal ownership region append one Proposal Revision and reset validation to
`pending`; Core may project `conflicted` in the same Transition when the exact
target proof has already failed. A mixed authoritative/Proposal edit changes
neither target and creates one `RefusedEditDraft` containing the complete
attempted structured payload. A stale Head, Anchor, reservation, writer
generation, editor contract, or ownership binding conflicts without creating a
Draft. A well-formed edit whose normalized result equals current durable content
is `NoEffect`.

Narrowing a Refused Edit Draft or retrying a Recovery Draft always constructs a
new `ApplyAuthorEdit` with `DraftRetry`; it uses the same classifier and never
preselects the authoritative route demonstrated by the prototype. A successful
`AuthoritativeApplied` or `ProposalRevised` result may close the exact source
Draft as `superseded` in the same Transition. `RefusedToDraft` may close it only
when the newly created Draft preserves the complete retried payload.
`Conflicted` and `NoEffect` leave it open. Copying Draft text is a read action,
not a Core command.

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
  producer_cause
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
old conflict or Receipt. The named source may carry either Proposal Conflict or
Proposal Recovery Conflict; Replan must resolve the exact preserved surface and
current targets without collapsing those condition kinds.

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

The remaining Proposal and editor-flow Draft decisions are exact-head,
idempotent commands:

```text
RejectProposalOperations
WithdrawProposal
ReopenWithdrawnProposal
SupersedeProposal
ReopenRejectedOperations
CompleteReadyPartialProposal
ContinueProposalGeneration
ExpandRefusedEditDraftToProposal
CloseEditorFlowDraft
```

Each Proposal command carries exact Project Scope, Proposal ID and Revision,
complete selected Operation IDs when applicable, current target expectations,
one exact producer cause, and an idempotency key. Each Draft command binds one
exact retained open Draft Revision. An author-caused first attempt requires an
Author Command Admission over the entire command digest. `WithdrawProposal` and
`ReplanProposal` may instead use the exact current `AgentRunStep` or `ToolCall`
cause; that producer-owned transition allocates no Author Action. No other
command in this list accepts a non-author cause.

All first attempts produce one `DomainReceipt`. The exhaustive common outcome is
closed as:

```text
ProposalOrDraftDecisionResult =
  TransitionApplied {
    prior_and_resulting_proposal_heads
    proposal_or_draft_lifecycle_event_refs
    created_proposal_ref | None
    author_action_sequence | None
  }
  | Refused { reason }
  | Conflicted { current_heads, reasons, proposal_conflict_ref | None }
  | NoEffect { reason }
```

Command-specific applied payloads below determine which references are present.
Refusal, conflict, and no effect append only the Receipt and applicable Proposal
condition projection, allocate no Authoritative Commit or Author Action, and do
not close a Draft.

| Command | Exact applied effect | Authoritative Commit | Author Action on author cause |
| --- | --- | ---: | ---: |
| `RejectProposalOperations` | selected pending Operations become `rejected`; typed author reason is preserved | 0 | 1 Forward |
| `WithdrawProposal` | closure becomes `withdrawn` without changing Operation resolution | 0 | 1 Forward; 0 for current producer |
| `ReopenWithdrawnProposal` | new open Proposal Revision against the exact withdrawn Head | 0 | 1 Forward |
| `SupersedeProposal` | closure becomes terminal `superseded` and binds the replacing Proposal Revision | 0 | 1 Forward |
| `ReopenRejectedOperations` | new Proposal Revision reopens the exact rejected Operations as `pending` and validation as `pending` | 0 | 1 Forward |
| `CompleteReadyPartialProposal` | generation becomes `ready` for the exact current content; validation remains or becomes `pending` as required | 0 | 1 Forward |
| `ContinueProposalGeneration` | a new Generation ID starts from the exact `ready_partial` or `ready` Head | 0 | 1 Forward |
| `ExpandRefusedEditDraftToProposal` | new Proposal and first Proposal Revision derive from the complete Refused Edit Draft Revision; the Draft stays open until a separate close | 0 | 1 Forward |
| `CloseEditorFlowDraft` | exact `RefusedEditDraft` or `RecoveryDraft` becomes `closed` with `dismissed`, `superseded`, or `abandoned` reason | 0 | 1 Forward |

`ReplanProposal` follows the same result and Receipt contract: applied appends
one new pending Proposal Revision against current targets, with one Forward
Author Action for an author cause and none for an AgentRunStep or ToolCall cause.
Rejection never means withdrawal. Supersession never rewrites Operation
resolution. Reopen, replan, and continuation never reuse a prior Validation
Receipt or Generation ID.

Narrowed retry and Recovery Draft retry are not additional command kinds: they
are new `ApplyAuthorEdit` commands as specified in section 7.2. Copy is a read.
An explicit Draft discard is `CloseEditorFlowDraft { reason: abandoned }`.
These mappings let the complete author-facing text and actions remain
inspectable without making UI labels or prototype state a second Core contract.

The accepted prototype's transitions map one-to-one as follows:

| Author-facing transition | Core command and positive semantic result |
| --- | --- |
| narrow Refused Edit Draft; retry Recovery Draft | new `ApplyAuthorEdit`; any of its five results remains possible |
| expand Refused Edit Draft | `ExpandRefusedEditDraftToProposal.TransitionApplied` |
| discard either Draft | `CloseEditorFlowDraft.TransitionApplied { reason: abandoned }` |
| replan Proposal Conflict or Proposal Recovery Conflict | `ReplanProposal.TransitionApplied` |
| reject conflicted or ordinary Proposal Operations | `RejectProposalOperations.TransitionApplied` |
| withdraw a Proposal Recovery Conflict surface | `WithdrawProposal.TransitionApplied` |
| accept ordinary Proposal Operations | `AcceptProposal.Applied` |
| copy any preserved text | read-only projection; no Core command or Receipt |

The prototype label `no-effect` after reject, withdraw, or Draft discard means
no manuscript authority was changed and no candidate remains active on that
surface. It does not replace the positive Core lifecycle/result Receipt with
`NoEffect`. Complete Draft payloads and preserved Proposal content remain
recoverable from their Core Artifact Revisions; UI character or line counts are
evidence, not schema fields.

### 7.7 Non-author production and validation settlement

`AppendProposalGenerationBatch`, `PauseProposalGeneration`, and
`ValidateProposal` are not author commands. Their first domain attempts produce
a `DomainReceipt`, `DomainReceipt`, and `ValidationReceipt` respectively.
Generation batch admission uses the exact `AgentRunStep` or `ToolCall` cause;
pause uses `EditorInputFence`; validation retains the exact cause of the command
or committed transition that requested validation. None allocates an
Authoritative Commit or Author Action. Exact duplicate batches and exact command
retries return their existing Receipt; sequence gaps and digest mismatch settle
as typed conflict without another Proposal Revision.

Automatic target-drift projection is caused by the exact authority-changing
Transition that made a Proposal's proof non-current. It appends condition/event
evidence but no extra Author Action. Producer cause never defaults to browser,
network, model, extension, Artifact, or generic `System`.

## 8. Outcomes, Acceptance, and Receipts

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

### 8.3 Domain Receipt and allocation matrix

Every first domain attempt whose command contract does not name a more specific
Receipt produces:

```text
DomainReceipt {
  receipt_id
  project_scope
  command_kind
  command_digest
  idempotency_key
  producer_cause
  author_command_admission_id | None
  expected_heads
  prior_heads
  resulting_heads
  authoritative_revision_ids
  proposal_revision_ids
  authoritative_commit_ids
  author_action_sequence | None
  draft_artifact_refs
  condition_refs
  result
  created_at
}
```

The result variant, not a nullable allocation, explains why each collection is
empty. The closed allocation matrix is:

| First-attempt result | Receipt | Authoritative Commit | Author Action | Draft Artifact | Proposal condition |
| --- | --- | ---: | ---: | ---: | ---: |
| `ApplyAuthorEdit.AuthoritativeApplied` | `DomainReceipt` | exactly 1 | exactly 1 Forward | 0 | 0 |
| `ApplyAuthorEdit.ProposalRevised` | `DomainReceipt` | 0 | exactly 1 Forward | 0 | 0 or the same Revision's immediately proven `ProposalConflict` |
| `ApplyAuthorEdit.RefusedToDraft` | `DomainReceipt` | 0 | 0 | exactly 1 `RefusedEditDraft` | 0 |
| `ApplyAuthorEdit.Conflicted` | `DomainReceipt` | 0 | 0 | 0 | exactly 1 `ProposalConflict` only when an exact Proposal Revision is the affected surface; otherwise 0 |
| `ApplyAuthorEdit.NoEffect` | `DomainReceipt` | 0 | 0 | 0 | 0 |
| applied author Proposal decision, replan, expand, or Draft close | `DomainReceipt` | 0 | exactly 1 Forward | 0 new Draft | only the command-specific validation projection |
| applied producer withdrawal or replan | `DomainReceipt` | 0 | 0 | 0 | only the command-specific validation projection |
| refused, conflicted, or no-effect Proposal/Draft decision | `DomainReceipt` | 0 | 0 | 0 | conflict condition only when returned |
| generation batch or pause attempt | `DomainReceipt` | 0 | 0 | 0 | 0 |
| validation attempt | `ValidationReceipt` | 0 | 0 | 0 | `ProposalConflict` exactly when result is `conflicted` |
| `AcceptProposal.Applied` for an ordinary domain Proposal | `AcceptanceReceipt` | exactly 1 | exactly 1 Forward | 0 | 0 |
| `AcceptProposal.Invalid` | `AcceptanceReceipt` | 0 | 0 | 0 | validation becomes `invalid` |
| `AcceptProposal.Conflicted` | `AcceptanceReceipt` | 0 | 0 | 0 | exactly 1 `ProposalConflict` |
| `AcceptProposal.Refused` or `.NoEffect` | `AcceptanceReceipt` | 0 | 0 | 0 | 0 |

Bundle Acceptance retains its declared atomic or ordered-independent policy:
the parent Acceptance Receipt records the exact child Receipts and committed
child Commit references. It does not weaken the one-Commit rule for any
ordinary domain Proposal child, and an exact retry never creates another child
Receipt, Commit, Author Action, Draft, or condition.

No author-facing outcome is inferred from allocation presence. The immutable
typed Receipt is the settlement authority and carries every created identity.

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
7. Append the immutable typed Receipt selected by the owning command contract.
8. For an author command, atomically link that Receipt through the owning
   Admission's terminal `ReceiptSettled` record.
9. Persist required outbox or wakeup intent.
10. Commit once and publish success only afterward.

All effects become visible together. A validation refusal or conflict may
commit only its no-change Receipt and conflict projection. A failure before
commit exposes no partial domain effect. External notification is delivered
from durable outbox intent and is never transaction truth.

`RequiresReconfirmation` is not a Core result and has no Receipt. It is the
Admission boundary's terminal proof that the admitted command will create no
Core effect. `outcome_unknown` is nonterminal recovery evidence and cannot be
linked as a command result, Receipt, Commit, Author Action, Draft, or Proposal
condition.

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
- withdrawal -> typed Proposal reopen;
- Draft close -> typed Draft reopen while retained;
- any action without a registered exact handler -> Barrier.

Every first attempt records:

```text
AuthorUndoReceipt {
  receipt_id
  project_scope
  command_digest
  idempotency_key
  author_command_admission_id
  expected_author_undo_frontier_sequence
  source_author_action_ref
  handler_receipt_ref | None
  authoritative_commit_ids
  author_action_sequence | None
  result:
    Compensated
    | ReversalRequired { reversal_proposal_ref }
    | Unavailable { reason }
    | Conflicted { current_author_undo_frontier_sequence }
  created_at
}
```

When a typed handler directly reverses its source, the same Transition records
the handler's typed Receipt, the `AuthorUndoReceipt`, and one `Compensation`
Author Action entry naming the source sequence and handler Receipt. That
Compensation is never a future undo candidate. If the handler instead returns
`ReversalRequired`, creation of the Reversal Proposal and its first Revision is
one new `Forward` action and the source remains uncompensated. ProseMirror
history may supply a session-local inverse candidate only for the exact
Frontier; Core still verifies durable history.

### 10.2 UndoAcceptance

```text
UndoAcceptanceHandlerInput {
  project_scope
  acceptance_receipt_id
  expected_current_target_revisions
  expected_current_proposal_head
  source_author_undo_command_ref
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

`UndoAcceptance` is the typed handler reached through the exact
`UndoLatestAuthorAction` admission, not a second public author command. Its
input is covered by the root command's digest and exact Frontier. It produces
one `UndoAcceptanceReceipt` referenced by the root `AuthorUndoReceipt` and
cannot receive a separate Admission, idempotency key, or retry. The closed
undo-allocation matrix is:

| Undo result | Handler Receipt | Authoritative Commit | Author Action | Proposal/Draft effect |
| --- | --- | ---: | ---: | --- |
| compensate Direct Author Action | `DomainReceipt` | exactly 1 | exactly 1 Compensation | none |
| compensate author Proposal edit, rejection, withdrawal, or Draft close | `DomainReceipt` | 0 | exactly 1 Compensation | exact new Proposal Revision, resolution/lifecycle event, or Draft reopen |
| `UndoAcceptance.Compensated` | `UndoAcceptanceReceipt` | exactly 1 | exactly 1 Compensation | safe Proposal reopening Revision when available |
| `UndoAcceptance.ReversalRequired` or another handler's `ReversalRequired` | owning typed Receipt | 0 | exactly 1 Forward | exactly 1 Reversal Proposal; source remains uncompensated |
| `Unavailable` | owning typed Receipt or no child when the root is a Barrier | 0 | 0 | none |
| root Frontier `Conflicted` | no child; `AuthorUndoReceipt` only | 0 | 0 | none |

The root `AuthorUndoReceipt` is always the public settlement. Its exact retry
returns the same root and child Receipt references. A crash before the atomic
Transition exposes neither compensation nor Reversal Proposal; a crash after
commit reconciles to those same Receipts and never creates a second Author
Action.

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

Missing response is never evidence that a Core Transition did not commit.
Recovery follows the Author Command Admission lifecycle and the following
closed matrix:

| Last proven boundary | Authoritative evidence | Required settlement or action | New Core allocation |
| --- | --- | --- | --- |
| pre-admission validation refused | immutable `PreAdmissionRefusalRecord` | display the typed refusal; a changed request starts a fresh challenge | no Admission, command, Receipt, Commit, Author Action, Draft, or condition |
| admission issuance transaction did not commit | validated absence of the Admission and nonce consumption | the client may begin a fresh admission flow | none |
| Admission is `pending` and authoritative storage cannot be validated | no safe positive or negative Receipt proof | append or retain `outcome_unknown`; perform read-only reconciliation and block invocation | none |
| exact typed Receipt is found | Receipt, idempotency record, and command digest match the Admission | atomically settle or observe `ReceiptSettled`; replay the immutable acknowledgement | only the allocations already named by that Receipt |
| validated storage proves no Receipt; action is the same unexpired `direct_editor_action`; complete intent, Project Scope, chapter, targets, ownership, Heads, writer generation, Admission, editor contract, and undo binding all match | exact Admission, idempotency record, current Core facts, and complete journal intent | invoke the already-admitted command once under the same Command, Admission, nonce, and idempotency key | exactly the owning command's eventual Receipt matrix; never a second Author Action |
| validated storage proves no Receipt; action is explicit, Admission expired, any binding changed, or complete intent is not recoverable | exact negative Receipt proof plus the mismatching or unrecoverable fact | terminal `RequiresReconfirmation`; this Admission can never later settle to a Receipt | no Core command effect; create one `RecoveryDraft` only when complete author-edit payload can be preserved |
| Core Transition committed before acknowledgement | Receipt and `ReceiptSettled` exist atomically with all effects | replay the same Receipt and converge projection | no new allocation |

Acceptance, rejection, withdrawal, Draft closure, Author Undo, and every other
explicit command in the reconfirmation row are never automatically invoked
after restart. A later confirmation creates a new idempotency record, nonce,
Command, Admission, and eventual Receipt. A direct edit's new explicit Retry
does the same and re-enters `ApplyAuthorEdit`; the old Admission remains
terminal.

A `RecoveryDraft` is created only by Host-assigned `EditorRecovery`, contains
the complete recoverable author-edit payload and exact evidence reference, and
is the only new Draft Artifact in this recovery flow. It is not a Receipt,
condition, or proof that the edit was attempted or committed. A later Retry
uses a new Admission unless the automatic same-Admission row above still
applies. Journal mechanics, Snapshot transport, writer takeover, local
projection, and garbage collection remain owned by the Web Editor Session
contract.

For a streaming Proposal:

- `ready_partial` or a Pause Fence permanently excludes later batches;
- `generating` may rebuild only from an exact Head, digest, contiguous sequence,
  compatible checkpoint, and no ambiguous runtime-gate window;
- ambiguity between a closed runtime gate and a durable fence creates a
  Proposal Recovery Conflict condition on the preserved Proposal surface;
- missing sequences, digest mismatch, invalid Anchors, duplicate Block IDs, or
  unsupported contracts select Proposal Safe Mode and disable Acceptance.

Proposal Conflict and Proposal Recovery Conflict are conditions on a preserved
Proposal surface. Neither is a Draft Artifact. When recovery can preserve
complete author text but cannot prove a Proposal projection, it may create one
Recovery Draft for that text and independently project Proposal Recovery
Conflict; the two facts remain orthogonal.

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
| expand a Refused Edit Draft | `ProposalCreatedFromRefusedEditDraft` |
| close or reopen an editor-flow Draft | `EditorFlowDraftClosed`, `EditorFlowDraftReopened` |
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
16. The conservative Author Edit commit boundary is one completed semantic
    intent; bounded idle coalescing is valid only while every frozen Scope,
    chapter, target, ownership, Head, writer, Admission, editor-contract, and
    undo binding remains equal.
17. Every first domain attempt settles through its owning typed Receipt; only
    the exhaustive result variant allocates the Commit, Author Action, Draft, or
    Proposal condition named by its matrix.
18. A missing response or temporarily unavailable store never proves
    non-commit. Post-admission uncertainty remains `outcome_unknown` until
    authoritative reconciliation reaches exactly one terminal settlement.
19. Refused Edit Draft and Recovery Draft are the only Draft Artifact kinds in
    this editor flow. Proposal Conflict and Proposal Recovery Conflict remain
    conditions on preserved Proposal surfaces.
