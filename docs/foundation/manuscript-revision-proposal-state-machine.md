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
pure Agent-generation transitions allocate none. A successful author-authored
Proposal Revision is a successful author-owned Transition and allocates one
Forward action even though it allocates no Authoritative Commit.

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

Author-command schema fragments below show the internal Core invocation shape.
The Web Client forms a digest-covered command body without an
`AuthorCommandAdmissionId`. After that final body and digest exist, the Server
performs the admission flow and attaches
`AuthorCommandAdmission { author_command_admission_id }` as the Core producer
cause. Any `author_command_admission_id` shown on an author command is that
Server-added cause and settlement link; it is excluded from the command's own
digest and cannot recursively participate in the inputs from which it is
issued. The Receipt repeats the exact producer cause and atomically links it to
`ReceiptSettled`.

These internal fragments colocate semantic fields and arbitration metadata for
readability; they do not define public-body or header placement. Under the
[Versioned Command, Query, Artifact, and Event Protocol section 7.2](versioned-command-query-artifact-event-protocol.md#72-shared-command-metadata)
and
[section 7.3](versioned-command-query-artifact-event-protocol.md#73-idempotency),
the canonical digest covers the exact semantic command inputs under that
contract's digest profile, while `idempotency_key` remains the separate
idempotency-arbitration and anti-forgery fence input. The key is compared with
the digest but is not redefined here as a member of its own digest.

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
DraftRetryPayloadRange =
  WholeDraftPayload
  | ExactStructuredRange {
      coordinate_profile
      from
      to
      slice_digest
    }

SourceDraftDisposition =
  NotApplicable
  | Unchanged {
      source_draft_kind: RefusedEditDraft | RecoveryDraft
      source_draft_id
      requested_source_draft_revision_id
      current_source_draft_revision_id
      current_source_draft_payload_digest
      current_closure:
        open
        | closed { close_reason, closure_event_ref }
    }
  | ClosedSuperseded {
      source_draft_kind: RefusedEditDraft | RecoveryDraft
      source_draft_id
      source_draft_revision_id
      source_draft_payload_digest
      prior_closure: open
      resulting_closure: closed
      close_reason: superseded
      closure_event_ref
    }

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
    | DraftRetry {
        source_draft_kind: RefusedEditDraft | RecoveryDraft
        source_draft_id
        source_current_draft_revision_id
        source_draft_payload_digest
        expected_source_draft_closure: open
        selected_payload_range: DraftRetryPayloadRange
      }
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
boundary. Bounded idle coalescing occurs before Admission issuance, when no
Command ID, Author Command Admission ID, issuance time, or expiry exists and
the canonical command digest is not final. It therefore cannot compare a
prospective Admission identity or lifetime.

The coalescing boundary must freeze and compare every pre-issuance input that
[Author Command Admission section 3](author-command-admission.md#3-exact-admission-bindings)
will resolve, validate, allocate, or bind: authenticated requester User; Project
Scope and scope kind; Client Session Binding record and generation; accepted
client-contract and security-policy revisions; applicable Editor Session and
writer generation or explicit not-applicable values; the
`direct_editor_action` class; exact API major, method, route template, command
schema, and command kind; digest profile; every target and expected Head or
Revision; the pre-domain idempotency record and key resolved from the submitted
input; and the one-use anti-forgery nonce record resolved from its challenge.
It must also compare the #46 semantic fields: chapter, ordered completed
intents and selections, retry source, target refs, observed ownership,
authoritative and Proposal Heads, editor-contract revision, and author-visible
undo group. Any difference or unverifiable input flushes the unit. Composition,
paste, cut, drop, every structural primitive, and an explicit command are
unconditional flush boundaries.

Release 1 uses the operation-specific policy
`storyos.author-edit-batch.release-1.v1`. Its inclusive ceilings are:

| Bound | Ceiling |
| --- | ---: |
| idle gap between adjacent completed intents | 250 ms |
| ordered `AuthorEditUnit` values in one command | 240 |
| normalized primitives across the complete command | 240 |
| encoded public JSON command body | 1 MiB under `storyos.foundation.absolute.v1` |

These values are safety ceilings, not a batching target. The client may freeze
earlier. It freezes immediately when a hard input boundary occurs, any shared
binding differs, any comparison is unavailable, or adding the next intent
would exceed a ceiling. The 240-unit value keeps the accepted sustained-input
case reachable without requiring every command to contain 240 units. The 250 ms
value affects only optional pre-Admission grouping and never delays a semantic
hard boundary. Absence or mismatch of the policy falls back to one intent per
command.

For the Release 1 manual-input surface, each `AuthorEditUnit` contains exactly
one `ReplaceSelection`. Unit order is the frozen group's contiguous
`local_intent_sequence` order. Core evaluates the list from first to last
against one transient working body. Each unit's UTF-16 coordinates are relative
to the body produced by all preceding units in the same command. An invalid
coordinate, unsupported unit, duplicate, omission, reorder, gap, or exceeded
bound invalidates the complete command. Core never commits a valid prefix.

Only after the final ordered request is frozen does the Server compute its
canonical digest, resolve every Admission section 3 binding, claim the
idempotency record, consume the nonce record, and issue one Admission with
`issued_at` and `expires_at`. The digest covers the exact ordered unit list and
all existing request fields. One challenge, nonce, idempotency record, and
Admission bind the complete list. Any unit, order, selection, text, binding, or
policy change creates a different command and cannot reuse the consumed
challenge or immutable settlement. No post-Admission merge may change the body
or digest. Browser history grouping is independent and never changes this
commit boundary.

Core validates the complete command boundary, current ownership facts,
expected Heads, target, unit shapes, bounds, and ordered selections before it
settles the result. It derives the final body in memory. One command receives
one exhaustive result. An authoritative result writes exactly one resulting
Authoritative Revision, Authoritative Commit, Forward Author Action, Domain
Receipt, Project Activity event, and resulting Head transition for the final
body in one transaction. No intermediate unit body becomes authority or a
durable settlement. A final body equal to the initial body is one whole-command
`NoEffect`. Refusal, conflict, validation failure, or transaction failure
creates no partial unit settlement. Exact retry returns the same whole-command
result and never reapplies a unit.

The Web Local Edit Journal retains the complete one-to-one mapping between the
group's contiguous ordered coverage and these units. The existing wire
`completed_intent_record_id` and `local_intent_sequence` name the first covered
record as the group anchor; they do not replace the complete local coverage.
One Receipt settles the complete group. Each covered record refers to this same
immutable settlement, and no record receives a partial Receipt. Pending Edit
Projection applies every unsettled unit in frozen order from its durable
checkpoint. It converges a committed group only after both the final canonical
result and its Receipt-backed Project Activity position are observed.
Per-record checkpoints and projection dependencies remain immutable evidence;
settlement does not rewrite their source Snapshot, source Heads, payload
digest, or local order. Normal-path active-base roll-forward uses only the
final canonical result and never an intermediate unit body. A later group
cannot be created until the current serialized group settles and the complete
authorized `EditorBaseSnapshot` tuple is installed.

The [Complete Bounded Manual Input and IME Semantics](https://github.com/FrankQDWang/StoryOS/issues/108)
implementation may consume this batch policy only for Direct Author Action
`ReplaceSelection` units from its accepted typing, deletion, selection,
clipboard, and supported IME surface. It may implement the hard-flush rules,
immutable group coverage, same-session second edit, and complete durable base
roll-forward through the existing `getEditorSession`,
`createProjectCommandChallenge`, and `applyAuthorEdit` operations. This policy
does not authorize Proposal or mixed-ownership settlement,
`RequiresReconfirmation`, acknowledgement-loss handling, reload, restart,
recovery, writer or replay-generation change, Snapshot resync, late-result
handling, or Local Edit Journal garbage collection. Those successor behaviors
remain with [Reconcile Acknowledgement Loss without Duplicate Authority](https://github.com/FrankQDWang/StoryOS/issues/109),
[Recover Settled and Unsettled Edits across Reload and Restart](https://github.com/FrankQDWang/StoryOS/issues/110),
and [Fence Stale Writers and Resync across Replay Generations](https://github.com/FrankQDWang/StoryOS/issues/111).

Core treats `observed_ownership_partition` as a stale-detection precondition,
not a grant. At the first domain attempt it recomputes ownership from current
Heads, Anchors, unresolved reservations, and the closed Direct Author Action
rules. The exhaustive result is:

```text
ApplyAuthorEditResult {
  effect:
    AuthoritativeApplied {
      authoritative_revisions
      authoritative_commit_id
      author_action_sequence
    }
    | ProposalRevised {
        proposal_revision_id
        author_action_sequence
        resulting_validation:
          Pending
          | StructuralReshapeConflict { proposal_conflict_ref }
      }
    | RefusedToDraft {
        refused_edit_draft_id
        refused_edit_draft_revision_id
        refusal_origin:
          FreshEditorIntent
          | DraftRetryReplacement { replacement_provenance_edge_ref }
      }
    | Conflicted {
        current_authoritative_heads
        current_proposal_heads
        reasons
        proposal_conflict_ref | None
      }
    | NoEffect { reason }
  source_draft_disposition: SourceDraftDisposition
}
```

One command is never split or partially applied. Core selects exactly one result
in this order:

1. Validate the command's exact expected Authoritative and Proposal Heads,
   writer generation, editor contract, current retained/open Proposal
   eligibility, `DraftRetry` source Revision/digest/closure when present, target
   Revisions, Anchors, reservations, and observed ownership precondition against
   current facts. Any stale, mismatched, missing, or failed exact proof returns
   `Conflicted`; it appends no Authoritative or Proposal Revision and allocates
   no Author Action.
2. With all preconditions proven, recompute ownership from those current facts.
   Mixed authoritative/Proposal ownership returns `RefusedToDraft`, changes
   neither target, and preserves the complete attempted structured payload in
   one `RefusedEditDraft`.
3. A single-owner command whose normalized result equals current durable
   content returns `NoEffect`.
4. An all-authoritative command returns `AuthoritativeApplied`, appends the
   required Authoritative Revisions, and creates exactly one Authoritative
   Commit and one Forward Author Action.
5. A command strictly inside one exact pending Proposal ownership region
   returns `ProposalRevised`, appends exactly one Proposal Revision, and creates
   exactly one Forward Author Action.

Ordinary `ProposalRevised` resets validation to `Pending` and creates no
condition. The sole append-and-immediate-conflict case is a command whose
pre-command proofs all passed but whose own applied structural primitive
creates Proposal Structural Reshaping. That result is
`ProposalRevised.StructuralReshapeConflict`: the new Revision preserves the
author edit and the same Transition deterministically records its
`ProposalConflict`. This fact is caused by the new candidate shape and cannot
be selected for a stale command. Once a Proposal is already conflicted, step 1
returns `Conflicted`; direct Proposal editing cannot append another Revision
until explicit replan.

The production editor-session evidence is authoritative for completed-intent
segmentation, durability, acknowledgement ordering, and recovery mechanics, but
not for #46's Author Action allocation. Its merged
`Proposal/refused/conflicted/no-effect` row uses a harness-level nullable field
and does not distinguish a successful author-authored Proposal Revision from
no-change outcomes. Under the canonical Author Action and unified undo
contracts, `ProposalRevised` therefore allocates exactly one Forward action;
`RefusedToDraft`, `Conflicted`, and `NoEffect` allocate none. This owner boundary
is required so a successful Proposal edit has a durable Author Undo Frontier.

Narrowing a Refused Edit Draft or retrying a Recovery Draft always constructs a
new `ApplyAuthorEdit` with `DraftRetry`; it uses the same classifier and never
preselects the authoritative route demonstrated by the prototype. The command
binds the exact current source Draft Revision, payload digest, and `open`
closure. Its source disposition is closed and exhaustive:

| Retry source | Effect | Required source-Draft disposition | Active recovery surface after settlement |
| --- | --- | --- | --- |
| `FreshEditorIntent` | any of the five effects | `NotApplicable` | only a newly allocated Refused Edit Draft when the effect is `RefusedToDraft` |
| `DraftRetry` | `AuthoritativeApplied` or `ProposalRevised` | `ClosedSuperseded`; same Transition records prior `open`, resulting `closed`, reason `superseded`, and one closure event | none on the source Draft |
| `DraftRetry` | `RefusedToDraft` | `ClosedSuperseded`; the new Refused Edit Draft preserves the complete selected retry payload, records `DraftRetryReplacement` provenance, and is the sole open recovery surface | exactly the new Refused Edit Draft |
| `DraftRetry` | `NoEffect` | `Unchanged` with exact current `Open` closure and no new event | the exact source Draft |
| `DraftRetry` | `Conflicted` after the source precondition matched | `Unchanged` with exact current `Open` closure and no new event | the exact source Draft |
| `DraftRetry` | `Conflicted` because the source Revision, digest, or closure no longer matches | `Unchanged` with the exact observed current source Revision, digest, and open-or-closed closure; no new event | no new recovery surface |

The original source Draft remains immutable and inspectable after
`ClosedSuperseded`; closure consumes only its active controls. An exact retry
returns the same Receipt, closure-event identity, replacement Draft or Proposal,
and Author Action without closing or creating anything again. Copying Draft
text is a read action, not a Core command.

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

Every command has the common command fields `command_schema_version`,
`project_scope`, one semantic payload discriminant, and the separate
`idempotency_key` arbitration field. The command schema, resolved Scope, and
semantic payload participate in the canonical digest exactly as owned by the
protocol digest profile; the idempotency key participates in its independent
arbitration and anti-forgery fence. Its Core invocation carries the producer
cause defined in section 7.1. Payload schemas are closed: every field in the
selected row below is required, and every field named only by another row is
prohibited. Selected Operation sets are nonempty and duplicate-free; expected
target sets are complete and canonically ordered.

```text
BoundedAuthorNote =
  Omitted
  | Present { text: bounded by the current Protocol Limit Profile }

ProposalRejectionReason =
  AuthorDeclined { note: BoundedAuthorNote }

ProposalWithdrawalReason =
  AuthorWithdrew { note: BoundedAuthorNote }
  | CurrentProducerWithdrew
```

| Command discriminant | Required protocol-digest-covered semantic payload |
| --- | --- |
| `RejectProposalOperations` | `proposal_id`, `source_current_proposal_revision_id`, `selected_pending_operation_ids`, complete `expected_target_revisions`, `rejection_reason: ProposalRejectionReason` |
| `WithdrawProposal` | `proposal_id`, `source_current_proposal_revision_id`, `expected_closure: open`, complete `expected_target_revisions`, `withdrawal_reason: ProposalWithdrawalReason` |
| `ReopenWithdrawnProposal` | `proposal_id`, `source_current_proposal_revision_id`, exact `withdrawal_event_ref`, `expected_closure: withdrawn`, complete `expected_target_revisions` |
| `SupersedeProposal` | `proposal_id`, `source_current_proposal_revision_id`, `expected_closure: open \| withdrawn`, `replacing_proposal_id`, `replacing_proposal_revision_id`, `expected_replacing_proposal_head`, complete `expected_target_revisions` |
| `ReopenRejectedOperations` | `proposal_id`, `source_current_proposal_revision_id`, `selected_rejected_operation_ids`, matching `rejection_event_refs`, complete `expected_target_revisions` |
| `CompleteReadyPartialProposal` | `proposal_id`, `source_current_proposal_revision_id`, `generation_id`, `expected_generation_state: ready_partial`, `expected_candidate_digest`, `last_applied_stream_seq`, complete `expected_target_revisions` |
| `ContinueProposalGeneration` | `proposal_id`, `source_current_proposal_revision_id`, `prior_generation_id`, `expected_generation_state: ready_partial \| ready`, `expected_candidate_digest`, `selected_pending_operation_ids`, complete `expected_target_revisions` |
| `ExpandRefusedEditDraftToProposal` | `source_draft_id`, `source_current_draft_revision_id`, `source_draft_payload_digest`, `expected_source_draft_closure: open`, `selected_payload_range: WholeDraftPayload`, `proposal_kind`, complete `target_refs`, complete `expected_target_revisions` |
| `CloseEditorFlowDraft` | `draft_kind: RefusedEditDraft \| RecoveryDraft`, `draft_id`, `source_current_draft_revision_id`, `source_draft_payload_digest`, `expected_closure: open`, `close_reason: dismissed \| superseded \| abandoned` |

All commands except `WithdrawProposal` require
`AuthorCommandAdmission { action_class: explicit_editor_command }`.
`WithdrawProposal` permits either that author cause or the exact
`AgentRunStep`/`ToolCall` recorded as the current Proposal Revision's producer;
`CurrentProducerWithdrew` is required for the latter and forbidden for the
author cause. `ReplanProposal` likewise permits an explicit author cause or the
exact current producer. No other non-author cause is eligible.

All first domain attempts produce one `DomainReceipt`. Pre-domain schema,
scope, authorization, Admission, idempotency, or cause failures follow section
7.1 and produce no domain Receipt. Applied results are a closed discriminated
union rather than one Proposal-shaped record with implicit empty fields:

```text
AuthorForwardAllocation =
  Forward { author_action_sequence }

AuthorDecisionAllocation =
  AuthorForwardAllocation
  | CurrentProducerOwned

DecisionCurrentTarget =
  Proposal {
    proposal_id
    current_proposal_revision_id
    state_axes
    current_target_revisions
  }
  | Draft {
      draft_kind
      draft_id
      current_draft_revision_id
      current_payload_digest
      closure
    }

ProposalOrDraftDecisionResult =
  ProposalOperationsResolved {
    proposal_id
    source_current_proposal_revision_id
    operation_ids
    prior_resolution: pending
    resulting_resolution: rejected
    rejection_reason: ProposalRejectionReason
    preserved_generation
    preserved_validation
    preserved_closure
    resolution_event_refs
    action: AuthorForwardAllocation
  }
  | ProposalClosureChanged {
      proposal_id
      source_current_proposal_revision_id
      prior_closure: open
      resulting_closure: withdrawn
      preserved_generation
      preserved_validation
      preserved_operation_resolutions
      withdrawal:
        AuthorWithdrew {
          note: BoundedAuthorNote
          action: AuthorForwardAllocation
        }
        | CurrentProducerWithdrew {
            action: CurrentProducerOwned
          }
      closure_event_ref
    }
  | ProposalRevisionAppended {
      proposal_id
      prior_proposal_revision_id
      resulting_proposal_revision_id
      transition:
        ReopenWithdrawn {
          withdrawal_event_ref
          prior_closure: withdrawn
          resulting_closure: open
          resulting_validation: pending
          preserved_generation
          preserved_operation_resolutions
          action: AuthorForwardAllocation
        }
        | ReopenRejected {
            operation_ids
            rejection_event_refs
            prior_resolution: rejected
            resulting_resolution: pending
            resulting_validation: pending
            preserved_generation
            preserved_closure
            action: AuthorForwardAllocation
          }
        | Replan {
            source_condition:
              ProposalConflict { proposal_conflict_ref }
              | ProposalRecoveryConflict { proposal_recovery_conflict_ref }
            resulting_validation: pending
            preserved_generation
            preserved_closure
            preserved_operation_resolutions
            action: AuthorDecisionAllocation
          }
      state_event_refs
    }
  | ProposalSuperseded {
      proposal_id
      source_current_proposal_revision_id
      prior_closure: open | withdrawn
      resulting_closure: superseded
      preserved_generation
      preserved_validation
      preserved_operation_resolutions
      replacing_proposal_id
      replacing_proposal_revision_id
      supersession_event_ref
      action: AuthorForwardAllocation
    }
  | ProposalGenerationCompleted {
      proposal_id
      source_current_proposal_revision_id
      generation_id
      prior_generation_state: ready_partial
      resulting_generation_state: ready
      preserved_validation
      preserved_closure
      preserved_operation_resolutions
      generation_event_ref
      action: AuthorForwardAllocation
    }
  | ProposalGenerationStarted {
      proposal_id
      source_current_proposal_revision_id
      prior_generation_id
      prior_generation_state: ready_partial | ready
      new_generation_id
      resulting_generation_state: generating
      selected_operation_ids
      preserved_validation
      preserved_closure
      preserved_operation_resolutions
      generation_event_ref
      action: AuthorForwardAllocation
    }
  | ProposalCreatedFromDraft {
      source_draft_id
      source_current_draft_revision_id
      source_draft_payload_digest
      selected_payload_range: WholeDraftPayload
      proposal_id
      proposal_revision_id
      resulting_generation: ready
      resulting_validation: pending
      resulting_closure: open
      resulting_operation_resolutions: all pending
      provenance_edge_ref
      source_draft_disposition: ClosedSuperseded
      action: AuthorForwardAllocation
    }
  | DraftClosureChanged {
      draft_kind
      draft_id
      source_current_draft_revision_id
      prior_closure: open
      resulting_closure: closed
      close_reason: dismissed | superseded | abandoned
      closure_event_ref
      action: AuthorForwardAllocation
    }
  | Refused { current_target: DecisionCurrentTarget, reason }
  | Conflicted {
      current_target: DecisionCurrentTarget
      reasons
      condition:
        NoCondition
        | ProposalConflict { proposal_conflict_ref }
        | ProposalRecoveryConflict { proposal_recovery_conflict_ref }
    }
  | NoEffect { current_target: DecisionCurrentTarget, reason }
```

`preserved_*` fields carry the exact unchanged value, not a boolean. No applied
variant may omit or null a listed state, reason, event, identity, or allocation;
the enclosing Receipt also binds the command payload and prior/resulting Heads.

| Command | Applied result variant | Authoritative Commit | Author Action |
| --- | --- | ---: | ---: |
| `RejectProposalOperations` | `ProposalOperationsResolved` with selected Operations `rejected` and exact rejection events | 0 | 1 Forward |
| author `WithdrawProposal` | `ProposalClosureChanged` to `withdrawn`; Operation resolution unchanged | 0 | 1 Forward |
| current-producer `WithdrawProposal` | same `ProposalClosureChanged` with `CurrentProducerOwned` | 0 | 0 |
| `ReopenWithdrawnProposal` | `ProposalRevisionAppended.ReopenWithdrawn`, closure `open` | 0 | 1 Forward |
| `SupersedeProposal` | `ProposalSuperseded`, closure terminal `superseded` | 0 | 1 Forward |
| `ReopenRejectedOperations` | `ProposalRevisionAppended.ReopenRejected`, selected Operations `pending`, validation `pending` | 0 | 1 Forward |
| `CompleteReadyPartialProposal` | `ProposalGenerationCompleted`, generation `ready`; current Proposal Revision and every other axis are preserved | 0 | 1 Forward |
| `ContinueProposalGeneration` | `ProposalGenerationStarted` with a newly allocated Generation ID; current Proposal Revision and every other axis are preserved until a content batch appends a Revision | 0 | 1 Forward |
| `ExpandRefusedEditDraftToProposal` | `ProposalCreatedFromDraft` from `WholeDraftPayload`; exact source Draft becomes `closed/superseded` with the returned closure event | 0 | 1 Forward |
| `CloseEditorFlowDraft` | `DraftClosureChanged` with the exact close reason | 0 | 1 Forward |

`ReplanProposal` uses the exact fields in section 7.4 and returns
`ProposalRevisionAppended.Replan`: one Forward action for an author cause and
`CurrentProducerOwned` for an AgentRunStep or ToolCall cause. Rejection never
means withdrawal. Supersession never rewrites Operation resolution. Reopen,
replan, and continuation never reuse a prior Validation Receipt or Generation
ID. `CompleteReadyPartialProposal` and the initial
`ContinueProposalGeneration` Transition append no content Revision and change
only the generation axis. Refused, conflicted, and no-effect results allocate no
Authoritative Commit or Author Action and do not change Proposal or Draft
lifecycle.

Narrowed retry and Recovery Draft retry are not additional command kinds: they
are new `ApplyAuthorEdit` commands as specified in section 7.2, and only that
`DraftRetry` payload may select an `ExactStructuredRange`.
`ExpandRefusedEditDraftToProposal` always carries every preserved line through
`WholeDraftPayload`, requires the exact source Refused Edit Draft to remain
`open`, and atomically creates the fresh Proposal plus
`ClosedSuperseded`. Its exact retry returns the same Proposal, closure event,
and Forward action; a distinct command against the now-closed source cannot
create another Proposal. After idempotency resolution, a mismatched source
Revision or digest returns `Conflicted`, while an exact current source whose
closure is not `open` returns
`Refused { reason: SourceDraftNotOpen }`; both return the current Draft target
and allocate no Proposal, Author Action, or lifecycle event. Copy is a read.
An explicit Draft discard is `CloseEditorFlowDraft { reason: abandoned }`.
These mappings let the complete author-facing text and actions remain
inspectable without making UI labels or prototype state a second Core contract.

The accepted prototype's transitions map one-to-one as follows:

| Author-facing transition | Core command and positive semantic result |
| --- | --- |
| narrow Refused Edit Draft; retry Recovery Draft | new `ApplyAuthorEdit`; any of its five results remains possible |
| expand Refused Edit Draft | `ProposalCreatedFromDraft` |
| discard either Draft | `DraftClosureChanged { close_reason: abandoned }` |
| replan Proposal Conflict or Proposal Recovery Conflict | `ProposalRevisionAppended.Replan` |
| reject conflicted or ordinary Proposal Operations | `ProposalOperationsResolved` |
| withdraw a Proposal Recovery Conflict surface | `ProposalClosureChanged` |
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

Every first Acceptance domain attempt produces:

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
  artifact_lifecycle_event_refs
  condition_refs
  result
  created_at
}
```

The result variant, not a nullable allocation, explains why each collection is
empty. The closed allocation matrix is:

| First-attempt result | Receipt | Authoritative Commit | Author Action | New Draft Artifact | Source Draft lifecycle | Proposal condition |
| --- | --- | ---: | ---: | ---: | --- | ---: |
| `ApplyAuthorEdit.AuthoritativeApplied` | `DomainReceipt` | exactly 1 | exactly 1 Forward | 0 | Fresh N/A; `DraftRetry` exact source closes `superseded` | 0 |
| `ApplyAuthorEdit.ProposalRevised.Pending` | `DomainReceipt` | 0 | exactly 1 Forward | 0 | Fresh N/A; `DraftRetry` exact source closes `superseded` | 0 |
| `ApplyAuthorEdit.ProposalRevised.StructuralReshapeConflict` | `DomainReceipt` | 0 | exactly 1 Forward | 0 | Fresh N/A; `DraftRetry` exact source closes `superseded` | exactly 1 `ProposalConflict` caused by the newly appended Revision's structural shape |
| `ApplyAuthorEdit.RefusedToDraft` | `DomainReceipt` | 0 | 0 | exactly 1 `RefusedEditDraft` | Fresh N/A; `DraftRetry` exact source closes `superseded` and the new Draft replaces it as the sole open surface | 0 |
| `ApplyAuthorEdit.Conflicted` | `DomainReceipt` | 0 | 0 | 0 | Fresh N/A; `DraftRetry` source is unchanged and creates no lifecycle event | exactly 1 `ProposalConflict` only when an exact Proposal Revision is the affected surface; otherwise 0 |
| `ApplyAuthorEdit.NoEffect` | `DomainReceipt` | 0 | 0 | 0 | Fresh N/A; `DraftRetry` source remains `open` with no lifecycle event | 0 |
| applied author Proposal decision, replan, or Draft close | `DomainReceipt` | 0 | exactly 1 Forward | 0 new Draft | N/A | only the condition explicitly named by its closed result variant |
| `ExpandRefusedEditDraftToProposal` applied | `DomainReceipt` | 0 | exactly 1 Forward | 0 | exact source closes `superseded` | 0 |
| applied producer withdrawal or replan | `DomainReceipt` | 0 | 0 | 0 | N/A | only the condition explicitly named by its closed result variant |
| refused, conflicted, or no-effect Proposal/Draft decision | `DomainReceipt` | 0 | 0 | 0 | unchanged when one is named | conflict condition only when returned |
| generation batch or pause attempt | `DomainReceipt` | 0 | 0 | 0 | N/A | 0 |
| validation attempt | `ValidationReceipt` | 0 | 0 | 0 | N/A | `ProposalConflict` exactly when result is `conflicted` |
| `AcceptProposal.Applied` for an ordinary domain Proposal | `AcceptanceReceipt` | exactly 1 | exactly 1 Forward | 0 | N/A | 0 |
| `AcceptProposal.Invalid` | `AcceptanceReceipt` | 0 | 0 | 0 | N/A | validation becomes `invalid` |
| `AcceptProposal.Conflicted` | `AcceptanceReceipt` | 0 | 0 | 0 | N/A | exactly 1 `ProposalConflict` |
| `AcceptProposal.Refused` or `.NoEffect` | `AcceptanceReceipt` | 0 | 0 | 0 | N/A | 0 |

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

For a source-consuming `DraftRetry` or
`ExpandRefusedEditDraftToProposal`, source closure, replacement provenance,
new Draft or Proposal allocation, lifecycle event, result, Receipt, and Author
Action when applicable are one write set. None may become visible alone.

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
- DraftRetry-backed Direct Author Action or Proposal edit -> the same typed
  content compensation plus exact source-Draft reopen;
- Refused Edit Draft expansion -> exact derived-Proposal withdrawal plus exact
  source-Draft reopen;
- Acceptance -> `UndoAcceptance`;
- rejection -> rejected-operation reopen;
- withdrawal -> typed Proposal reopen;
- Draft close -> typed Draft reopen while retained;
- any action without a registered exact handler -> Barrier.

A handler that reopens a consumed source Draft requires the exact retained
`ClosedSuperseded` event from its source Receipt and the exact current
Draft/Proposal or Authoritative Heads produced by that Forward action. It
reverses all of those effects atomically or returns `Conflicted`/`Unavailable`
without a partial reopen or content compensation. `DraftRetry.RefusedToDraft`
has no Forward Author Action: its old closed Draft and new sole open
replacement remain the positive refusal settlement.

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
| compensate ordinary Direct Author Action | `DomainReceipt` | exactly 1 | exactly 1 Compensation | none |
| compensate `DraftRetry.AuthoritativeApplied` | `DomainReceipt` | exactly 1 | exactly 1 Compensation | exact source Draft reopen |
| compensate ordinary author Proposal edit | `DomainReceipt` | 0 | exactly 1 Compensation | exact new Proposal Revision |
| compensate `DraftRetry.ProposalRevised` | `DomainReceipt` | 0 | exactly 1 Compensation | exact new Proposal Revision plus exact source Draft reopen |
| compensate rejection, withdrawal, or explicit Draft close | `DomainReceipt` | 0 | exactly 1 Compensation | exact resolution/lifecycle event or Draft reopen |
| compensate Refused Edit Draft expansion | `DomainReceipt` | 0 | exactly 1 Compensation | withdraw exact derived Proposal and reopen exact source Draft |
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
| validated storage proves no Receipt; action is the same unexpired `direct_editor_action`; every binding required by Author Command Admission sections 3 and 5 is equal; and the complete #46 intent, chapter, selections, retry source, targets, recomputed ownership, Authoritative and Proposal Heads, Anchors, reservations, editor-contract revision, author-visible undo group, and durable journal facts all match | exact Admission and idempotency/nonce records, current Core facts, and complete journal intent | invoke the already-admitted command once under the same Command, Admission, nonce, and idempotency key | exactly the owning command's eventual Receipt matrix; never a second Author Action |
| validated storage proves no Receipt; action is explicit, Admission expired, any section 3 or 5 binding or #46 fact changed, or complete intent is not recoverable | exact negative Receipt proof plus the mismatching or unrecoverable fact | terminal `RequiresReconfirmation`; this Admission can never later settle to a Receipt | no Core command effect; create one `RecoveryDraft` only when complete author-edit payload can be preserved |
| Core Transition committed before acknowledgement | Receipt and `ReceiptSettled` exist atomically with all effects | replay the same Receipt and converge projection | no new allocation |

Acceptance, rejection, withdrawal, Draft closure, Author Undo, and every other
explicit command in the reconfirmation row are never automatically invoked
after restart. A later confirmation creates a new idempotency record, nonce,
Command, Admission, and eventual Receipt. A direct edit's new explicit Retry
does the same and re-enters `ApplyAuthorEdit`; the old Admission remains
terminal.

“Every binding” above normatively means the complete sets owned by
[Author Command Admission section 3](author-command-admission.md#3-exact-admission-bindings)
and
[section 5](author-command-admission.md#5-first-invocation-and-recovery-rules),
not the illustrative #46 facts that follow it. This state machine consumes
that equality decision and does not redefine Admission issuance, expiry,
settlement, or recovery lifecycle.

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
| apply an Author Edit to authority | `AuthoritativeAuthorEditApplied`; also `EditorFlowDraftClosed` for `DraftRetry` |
| apply an Author Edit to a Proposal | `ProposalAuthorEditApplied`; also `EditorFlowDraftClosed` for `DraftRetry` |
| refuse mixed ownership | `RefusedEditDraftCreated`; also `EditorFlowDraftClosed` when replacing a `DraftRetry` source |
| expand a Refused Edit Draft | `ProposalCreatedFromRefusedEditDraft`, `EditorFlowDraftClosed` |
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
    intent. `storyos.author-edit-batch.release-1.v1` permits at most 240 ordered
    units, 240 normalized primitives, a 250 ms adjacent-intent idle gap, and the
    active 1 MiB public command-body ceiling before Admission. Every shared
    Admission and Author Edit semantic field must remain equal. Core evaluates
    the immutable units in order against one transient body and settles the
    complete list atomically as one result. The final body and digest receive
    one Admission; no intermediate unit or later merge can create a partial effect.
17. Every first domain attempt settles through its owning typed Receipt; only
    the exhaustive result variant allocates the Commit, Author Action, Draft, or
    Proposal condition named by its matrix.
18. A missing response or temporarily unavailable store never proves
    non-commit. Post-admission uncertainty remains `outcome_unknown` until
    authoritative reconciliation reaches exactly one terminal settlement.
19. Refused Edit Draft and Recovery Draft are the only Draft Artifact kinds in
    this editor flow. Proposal Conflict and Proposal Recovery Conflict remain
    conditions on preserved Proposal surfaces.
20. A source-consuming Draft retry or expansion closes its exact source once as
    `superseded` in the same Transition as its replacement result. Exact retry
    replays those identities; conflict or no effect creates no source lifecycle
    event.
