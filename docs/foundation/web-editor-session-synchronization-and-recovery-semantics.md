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
submission queueing, branch-shaped HTTP settlement and Project Activity
convergence, Snapshot resynchronization, writer takeover, browser recovery, and
Local Edit Journal payload collection.

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
idempotency key, and final body/digest. Its exact-retry material is held only
in the protected short-lived capsule described in section 3.5. A valid
admitted attempt receives an Author Command Admission. After that Admission
exists, an uncertain explicit takeover is never automatically invoked by
recovery.

The Server compares the observed generation and performs one closed transition:

```text
TakeOverProjectWriterResult =
  TakeoverApplied {
    prior_editor_session_id
    prior_writer_generation
    resulting_editor_session_id
    resulting_writer_generation
    resulting_snapshot_id
    resulting_snapshot_activity_position
    resulting_heads
  }
  | TakeoverCompareFailed {
      observed_writer_generation
      current_writer_generation
      current_writer_projection
      current_snapshot_id
      current_snapshot_activity_position
      current_heads
      reason:
        writer_generation_advanced_after_admission
        | requester_became_current_after_admission
    }
```

The result is carried only by the existing `DomainReceipt` /
`DomainReceiptRef` selected by the protocol's closed `ReceiptRef` union; #70
creates no takeover-specific Receipt identity or reference.

Before Admission issuance, the Server validates that the takeover's observed
writer generation is still current. A stale or already-changed observation
fails as a pre-admission Problem with no Admission or Receipt.
`TakeoverApplied` atomically advances the generation, grants it to the
requester, fences every older generation before another author-command
Admission can be issued, and returns the canonical Snapshot from which the
writer reconciles. Only a takeover that passed that issuance check but loses
the later Core/domain compare—because another already-admitted takeover
advanced the generation or made this requester current—returns
`TakeoverCompareFailed`. It is the typed Receipt-backed no-change result of the
admitted compare-and-set and advances no generation.
Both results settle through their existing `OtherEditorReceiptSettled {
receipt_ref: DomainReceiptRef, project_activity_position, settled_at }`; their
transaction commits the required Project Activity row even when no writer
state changes.
The returned Snapshot must project at or beyond that settlement position and
never replaces it. Pre-admission validation failure has no Admission or
Receipt.
`RequiresReconfirmation` has an Admission but no Receipt or takeover effect.
Missing acknowledgement first permits only exact transport replay from the
protected capsule; a settlement query exists only when the received
`outcome_unknown` variant supplies it. An unexpected takeover `Accepted` is
retained only as protocol-incompatibility evidence. Neither takeover result
creates a manuscript Core effect or Author Action.

After `TakeoverApplied`, the prior session becomes read-only. The winner opens
a new Local Edit Journal partition bound to the resulting generation; neither
the winner's nor the prior session's older partition is rebound. Their complete
local payload remains non-authoritative and must not be deleted or resubmitted
merely because takeover occurred. If StoryOS must preserve that payload outside
the local journal, it remains local-only until a future wire owner adds an
explicit ingress; takeover carries no hidden recovery-payload channel.

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
    | Closed {
        reason:
          editor_session_closed
          | client_session_ended
          | project_unavailable
      }
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
submission. It is never coalesced. Before Admission, its protected transport
attempts may only finish that same already-confirmed submission; after
Admission, uncertainty can never automatically invoke it.

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

### 3.5 Frozen groups, transport attempts, and browser evidence

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

  frozen_request_body_ref
  frozen_request_digest_input_ref
  frozen_payload_coverage_digest
  frozen_at
}

GroupReconciliation =
  NoReconciliationNeeded {
    basis:
      frozen_not_submitted
      | active_attempt {
          transport_attempt_id
          prior_no_admission_proof_observation_id | null
        }
  }
  | TransportOrAdmissionUnknown {
      transport_attempt_id
      exact_transport_retry_capsule_id
    }
  | KnownSettlementQuery {
      source_observation_id
      settlement_query
    }
  | ProtocolIncompatibleAccepted {
      accepted_observation_id
      settlement_query
    }
  | OutcomeQueryUnresolved {
      original_delivery_unknown_attempt_id
      strongest_valid_observation:
        NoOutcomeObserved
        | ChallengeIssued { expires_at }
        | AdmissionCommitted {
            command_id
            author_command_admission_id
            reconciliation_required: true
          }
      latest_outcome_query_attempt_id
      latest_outcome_query_observation_id | null
    }
  | ProvenNoAdmission {
      proof_observation_id
      disposition:
        terminal_pre_admission_refusal
        | fresh_challenge_permitted
    }
  | TerminalResolved {
      terminal_observation:
        PreAdmissionProblem { protocol_observation_id }
        | Committed {
            observation:
              CommandResponse { protocol_observation_id }
              | OutcomeQueryResponse { outcome_query_observation_id }
          }
        | OutcomeQueryRejected { outcome_query_observation_id }
        | RequiresReconfirmation { protocol_observation_id }
    }

GroupSettlement =
  Unsettled
  | PreAdmissionRefused {
      pre_admission_problem_observation_id
    }
  | OutcomeQueryRejectedNoAdmission {
      outcome_query_observation_id
      reason: challenge_expired_unconsumed
      observed_at
    }
  | AppliedReceiptSettled {
      apply_author_edit_applied_observation_id
      receipt_ref: DomainReceiptRef
      project_activity_position
      committed_at
    }
  | ZeroAuthorityReceiptSettled {
      apply_author_edit_zero_authority_observation_id
      receipt_ref: DomainReceiptRef
      result: NoEffect | Conflicted | Refused
      committed_at
    }
  | OtherEditorReceiptSettled {
      other_editor_committed_observation_id
      receipt_ref: ReceiptRef
      project_activity_position
      committed_at
    }
  | RequiresReconfirmation {
      requires_reconfirmation_observation_id
      author_command_admission_id
      reconfirmation_reason
      recovery_draft_ref | null
      recorded_at
    }

AuthorSurfaceConvergence =
  Pending
  | AppliedReceiptConverged {
      apply_author_edit_applied_observation_id
      receipt_ref: DomainReceiptRef
      project_activity_position
      projection_proof:
        ProcessedProjectActivity {
          processed_through_project_activity_position
        }
        | SnapshotProjection {
            snapshot_id
            snapshot_activity_position
          }
      resulting_heads
      resulting_surface_refs
    }
  | ZeroAuthorityReceiptVisible {
      apply_author_edit_zero_authority_observation_id
      receipt_ref: DomainReceiptRef
      result: NoEffect | Conflicted | Refused
      unchanged_installed_base_proof | attention_surface_proof
    }
  | OtherEditorReceiptConverged {
      other_editor_committed_observation_id
      receipt_ref: ReceiptRef
      project_activity_position
      projection_proof:
        ProcessedProjectActivity {
          processed_through_project_activity_position
        }
        | SnapshotProjection {
            snapshot_id
            snapshot_activity_position
          }
      resulting_heads
      resulting_surface_refs
    }
  | PreAdmissionRefusalConverged {
      pre_admission_problem_observation_id
      local_refusal_surface_id
      preserved_local_payload_ref
    }
  | OutcomeQueryRejectedVisible {
      outcome_query_observation_id
      local_rejected_surface_id
      preserved_local_payload_ref
    }
  | ReconfirmationConverged {
      requires_reconfirmation_observation_id
      author_command_admission_id
      preserved_local_payload_ref
      recovery_draft_ref | null
      local_reconfirmation_surface_id
    }

AuthorAttention =
  None
  | Required { reason, local_author_surface_id }
  | Resolved { decision_receipt_ref | local_resolution_observation_id }

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
the action class, request contract, idempotency key, final body and digest
coverage, transport, reconciliation, Admission, settlement, convergence, and
collection lifecycle. Its transport, reconciliation, settlement, convergence,
attention, and retention axes advance only through the positive transitions
below.

Before each physical send, the Protected Web Client durably commits one
short-lived exact-retry capsule in a protected IndexedDB store:

```text
ProtectedExactTransportRetryCapsule {
  exact_transport_retry_capsule_id
  journal_submission_group_id
  project_scope
  client_session_binding_ref
  client_session_generation
  request_identity {
    api_major
    method
    route_template
    command_schema
    command_kind
  }
  exact_request_body_ref
  canonical_command_digest
  digest_profile
  exact_client_controlled_headers {
    "Idempotency-Key": idempotency_key
    "X-StoryOS-Anti-Forgery": anti_forgery_nonce
    "Content-Type": content_type
  }
  challenge_expires_at
  committed_before_send_at
  disposition:
    Available
    | Collected {
        reason:
          replayable_response_evidence_durable
          | original_attempt_proven_no_admission
      }
}
```

The capsule contains only the exact request and client-controlled header
material required by #58 to replay the same Scope, method, route, kind, key,
body, and digest, including the returned nonce. It is never exposed to the
editor DOM, diagnostics, logs, export, clipboard, extension, Tool, or MCP
surface and is never authority. The protected Client Session credential
remains owned by its normal binding; the capsule cannot replace or renew it.
`challenge_expires_at` is the challenge response's public expiry, not an
Admission issuance time or lifetime.
Exact replay revalidates the same protected Host, Origin, and Client Session
binding at send time. If any is invalid or expired, the original delivery
remains unknown and the client never sends blindly.

For an `ApplyAuthorEdit` initial attempt whose acknowledgement is wholly absent,
`getApplyAuthorEditOutcome` is the first reconciliation action. The browser uses
the original `idempotency_key` in the route and the original challenge nonce
only in the `X-StoryOS-Anti-Forgery` header. It revalidates the exact protected
Host, Project Scope, current Client Session binding and generation, and complete
stored challenge binding. It does not perform `ExactTransportReplay`, obtain a
new challenge, or send the command again before this read.
The read uses `SensitiveSafeReadWithRefererFallback`. Every success and Problem
response is `Cache-Control: no-store`; a cache is never outcome evidence.

Physical delivery is an append-only attempt log:

```text
PhysicalTransportAttempt {
  transport_attempt_id
  journal_submission_group_id
  attempt_ordinal
  attempt_kind:
    Initial
    | ExactTransportReplay {
        replay_of_transport_attempt_id
      }
    | FreshChallengeAfterProvenNoAdmission {
        proof_observation_id
      }
  exact_transport_retry_capsule_id
  started_at
  outcome:
    InFlight
    | ResponseObserved { protocol_observation_id }
    | DeliveryUnknown {
        observed_at
        evidence:
          connection_lost
          | process_crashed
          | response_unreadable
      }
}
```

Every send names a capsule already committed before `started_at`. An initial
or fresh-challenge attempt owns a new capsule; an exact replay reuses the
original capsule byte-for-byte. A fresh-challenge attempt is legal only after
positive no-Admission proof and commits a new capsule with the new nonce but
the same frozen Scope, method, route, kind, key, body, and digest. Each group
has at most one `FreshChallengeAfterProvenNoAdmission` attempt; another
no-Admission refusal terminates that original logical submission rather than
opening a challenge loop. The latest attempt's `InFlight` record is durably
appended before the network send and derives the current transport view; the
append-only log makes a second `InFlight` reachable without rewriting an
earlier attempt.

The Web Client stores no universal Admission attachment. It appends only
locally identified observations whose protocol fields are copied from the
exact received #58 variant:

```text
BrowserProtocolObservation =
  PreAdmissionProblemObservation {
    protocol_observation_id
    local_problem_payload_ref
    exact_problem_payload_digest
    type
    status
    code
    retryability
    correlation_id
    project_scope | null
    command_id: null
    author_command_admission_id: null
    settlement_query: null
    recovery_disposition: null
    safe_conflict | null
    resync | null
    limit_profile_revision
    observed_at
  }
  | UnresolvedTransportProblemObservation {
      protocol_observation_id
      local_problem_payload_ref
      exact_problem_payload_digest
      type
      status
      code
      retryability
      correlation_id
      project_scope | null
      command_id | null
      author_command_admission_id: null
      settlement_query: null
      recovery_disposition: null
      safe_conflict | null
      resync | null
      limit_profile_revision
      observed_at
    }
  | AcceptedObservation {
      protocol_observation_id
      acknowledgement_kind: "accepted"
      envelope_version
      command_id
      project_scope
      correlation_id
      operation_ref
      settlement_query
      admitted_activity_position
      accepted_at
      limit_profile_revision
    }
  | ApplyAuthorEditAppliedObservation {
      protocol_observation_id
      schema_id: "storyos.command.apply-author-edit.response.v2"
      command_id
      author_command_admission_id
      project_scope
      correlation_id
      receipt: DomainReceipt { result: AuthoritativeApplied }
      effect: AuthoritativeApplied {
        authoritative_revision
        authoritative_commit_id
        author_action_sequence
        project_activity_position
      }
      completed_intent_record_id
      local_intent_sequence
      observed_at
    }
  | ApplyAuthorEditZeroAuthorityObservation {
      protocol_observation_id
      schema_id: "storyos.command.apply-author-edit.response.v2"
      command_id
      author_command_admission_id
      project_scope
      correlation_id
      receipt: DomainReceipt { result: NoEffect | Conflicted | Refused }
      effect: NoEffect | Conflicted | Refused
      completed_intent_record_id
      local_intent_sequence
      observed_at
    }
  | OtherEditorCommittedObservation {
      protocol_observation_id
      acknowledgement_kind: "committed"
      envelope_version
      command_id
      project_scope
      correlation_id
      receipt_ref: ReceiptRef
      project_activity_position
      committed_at
      limit_profile_revision
    }
  | RequiresReconfirmationObservation {
      protocol_observation_id
      acknowledgement_kind: "requires_reconfirmation"
      envelope_version
      command_id
      project_scope
      correlation_id
      author_command_admission_id
      reconfirmation_reason
      recovery_draft_ref | null
      recorded_at
      limit_profile_revision
    }
  | OutcomeUnknownProblemObservation {
      protocol_observation_id
      local_problem_payload_ref
      exact_problem_payload_digest
      type
      status
      code
      retryability: outcome_unknown
      correlation_id
      project_scope
      command_id
      author_command_admission_id
      settlement_query
      recovery_disposition: reconciliation_required
      safe_conflict | null
      resync | null
      limit_profile_revision
      observed_at
    }
```

`protocol_observation_id`, `local_problem_payload_ref`,
`exact_problem_payload_digest`, and `observed_at` are browser-local envelope
facts. Every other field in an observation is copied only when its named #58
variant supplies it.

`PreAdmissionProblemObservation` is classified only when the exact #58 Problem
semantics positively prove that the observed attempt created no Admission or
Receipt. Its payload reference is local storage for the received sanitized
Problem representation, not a Server refusal-record reference.
`UnresolvedTransportProblemObservation` preserves an exact safe Problem such
as a concurrent `command_in_progress` or a replay attempted after Client
Session invalidation; it makes no Admission/Receipt/settlement claim and leaves
the group `TransportOrAdmissionUnknown`.
`AcceptedObservation` proves only the fields in `Accepted`; it does not prove
an Author Command Admission identity or terminal state.
`ApplyAuthorEditAppliedObservation` maps to `AppliedReceiptSettled`.
`ApplyAuthorEditZeroAuthorityObservation` maps to
`ZeroAuthorityReceiptSettled`. `OtherEditorCommittedObservation` retains the
pre-correction Receipt/Activity shape and maps to
`OtherEditorReceiptSettled`.
`RequiresReconfirmationObservation` has no Receipt or settlement query.
`OutcomeUnknownProblemObservation` records a separate `StoryOSProblem`, makes
no Receipt-presence claim, and carries the exact non-null settlement query
supplied by #58. No observation fabricates issuance/expiry, a
client-addressable Admission/refusal/settlement record, or a field absent from
its protocol variant.

The missing-first-acknowledgement read has a separate append-only attempt and
observation identity. Each physical GET appends exactly one attempt; each valid
200 response appends exactly one immutable observation. A repeated GET may have
a new query correlation and observation while the reducer still creates only
one lifecycle effect. It does not reinterpret a command response observation:

```text
ApplyAuthorEditOutcomeQueryAttempt =
  {
    outcome_query_attempt_id
    journal_submission_group_id
    exact_transport_retry_capsule_id
    request_identity {
      method: GET
      route_template: /api/v1/projects/{project_id}/manuscript/author-edit-outcomes/{idempotency_key}
      request_schema: storyos.query.apply-author-edit-outcome.request.v1
      idempotency_key
    }
    started_at
    outcome:
      InFlight
      | QueryUnavailable {
          observed_at
          evidence: delivery_unknown | safe_problem | invalid_envelope
          local_response_payload_ref | null
          exact_response_payload_digest | null
        }
      | ResponseObserved { outcome_query_observation_id }
  }

ApplyAuthorEditOutcomeQueryObservation =
  {
    outcome_query_observation_id
    outcome_query_attempt_id
    journal_submission_group_id
    exact_transport_retry_capsule_id
    local_response_payload_ref
    exact_response_payload_digest
    schema_id: storyos.query.apply-author-edit-outcome.response.v1
    query_correlation_id
    project_scope
    observed_at
    outcome:
      Committed { exact_apply_author_edit_response_v2 }
      | RejectedNoAdmission { reason: challenge_expired_unconsumed }
      | StillUnknown {
          observation:
            ChallengeIssued { expires_at }
            | AdmissionCommitted {
                command_id
                author_command_admission_id
                reconciliation_required: true
              }
        }
      | RequiresReconfirmation {
          command_id
          author_command_admission_id
          reconfirmation_reason:
            admission_expired
            | binding_changed
            | direct_edit_intent_unrecoverable
          recovery_draft_ref | null
        }
  }

OutcomeQueryReducer =
  NoOutcomeObserved -> ChallengeIssued | AdmissionCommitted | Rejected | RequiresReconfirmation | Committed
  ChallengeIssued(exact same expires_at) -> ChallengeIssued | AdmissionCommitted | Rejected | RequiresReconfirmation | Committed
  AdmissionCommitted(same Command and Admission) -> AdmissionCommitted | RequiresReconfirmation | Committed(same identities)
  QueryUnavailable -> preserve strongest_valid_observation
  Rejected | Committed | RequiresReconfirmation -> terminal immutable
  late original POST acknowledgement + GET Committed -> one exact settlement
```

The complete query attempt and observation bind the frozen group, protected
capsule, query request, public response envelope, local response bytes, and
local observation time in one Journal transaction. `query_correlation_id` is
the query envelope identity. It is distinct from the nested original command
correlation inside a `Committed` response. The state reducer exact-deduplicates
the derived lifecycle effect but never discards either immutable channel
observation. The editor does not display saved, rejected, or settled and does
not release a dependent group until that complete Journal transaction commits.
A first durable terminal command-response or outcome-query channel supplies the
tagged `Committed.observation`; a later exact matching channel appends evidence
without replacing that terminal identity or creating another settlement.
A changed Project Scope, idempotency key, Command, Admission, Receipt, command
correlation, digest, or result relation fails closed as protocol/corruption
evidence and preserves the prior valid state.

`RejectedNoAdmission` enters
`TerminalResolved { OutcomeQueryRejected { outcome_query_observation_id } }`,
`OutcomeQueryRejectedNoAdmission { outcome_query_observation_id, reason,
observed_at }`, and
`OutcomeQueryRejectedVisible { outcome_query_observation_id,
local_rejected_surface_id, preserved_local_payload_ref }`. All three records
name the same exact query observation. This branch is not a Problem observation,
Receipt, Activity, or saved result. It keeps `AuthorAttention.Required` until
the preserved local work has an explicit author disposition.

The safe outcome Query does not consume the nonce, append Server lifecycle
state, invoke Core, create a Receipt or Activity, or change authority. A query
transport failure, malformed response, or canonical non-200 Problem remains
unresolved and records only the exact local evidence that is available. No
`Rejected` or `StillUnknown` branch releases the dependent queue, collects the
capsule or payload, invokes or replays the command, or silently succeeds.
This client-first bootstrap applies only to `ApplyAuthorEdit`. It does not add
the outcome route to takeover or another command lifecycle.

The client-observable versus Server-only audit is closed:

| Authoritative Server fact | Permitted browser-local evidence |
| --- | --- |
| `PreAdmissionRefusalRecord` identity and audit fields | no record reference; only the exact sanitized Problem observation and local correlation |
| Author Command Admission record, issuance time, and expiry | no universal local copy; Admission ID only when the exact received variant supplies it through `ApplyAuthorEditAppliedObservation`, `ApplyAuthorEditZeroAuthorityObservation`, `RequiresReconfirmationObservation`, `OutcomeUnknownProblemObservation`, or the outcome Query `AdmissionCommitted` observation; only `ChallengeIssued` supplies challenge expiry |
| Admission settlement record/reference | no local record reference; only the exact applied, zero-authority, other-editor committed, or `RequiresReconfirmationObservation` branch |
| outcome bootstrap and settlement Query | the named `getApplyAuthorEditOutcome` route uses only the original capsule key and nonce; any other settlement Query exists only when `Accepted` or `outcome_unknown` supplies it |
| idempotency and nonce record identities | no Server-record references; only frozen local key/digest plus the protected exact-retry capsule |
| typed Receipt | the exact Receipt supplied by a committed response |
| Project Activity position | only an owning Activity-backed tagged result; for `ApplyAuthorEdit` this is only `AuthoritativeApplied` |

The Server-side Admission, refusal, idempotency, nonce, and settlement records
remain authoritative and need not be client-addressable.

The legal positive state paths are:

```text
frozen group
  -> NoReconciliationNeeded { frozen_not_submitted }
  -> attempt Initial / NoReconciliationNeeded { active_attempt }

ApplyAuthorEdit initial attempt DeliveryUnknown
  -> OutcomeQueryUnresolved { NoOutcomeObserved }
  -> attempt getApplyAuthorEditOutcome using the same available capsule

outcome QueryUnavailable
  -> OutcomeQueryUnresolved { preserve strongest_valid_observation }

outcome StillUnknown ChallengeIssued | AdmissionCommitted
  -> OutcomeQueryUnresolved { advance only to the strongest valid observation }

outcome Rejected | Committed
  -> TerminalResolved through the exact outcome Query observation

other editor-command attempt DeliveryUnknown
  -> TransportOrAdmissionUnknown
  -> attempt ExactTransportReplay using the same available capsule

unresolved transport Problem
  -> TransportOrAdmissionUnknown

exact replay or initial response Accepted for a Release-1 editor command
  -> ProtocolIncompatibleAccepted {
       exact Accepted observation and settlement_query
     }

response OutcomeUnknown Problem
  -> KnownSettlementQuery { exact StoryOSProblem.settlement_query }

KnownSettlementQuery
  -> KnownSettlementQuery { repeated outcome_unknown }
     | ProtocolIncompatibleAccepted { unexpected Accepted }
     | TerminalResolved { Committed | RequiresReconfirmation }

positive no-Admission Problem proof for the original attempt
  -> ProvenNoAdmission
  -> TerminalResolved { PreAdmissionProblem }
     | attempt FreshChallengeAfterProvenNoAdmission /
       NoReconciliationNeeded {
         active_attempt { prior_no_admission_proof_observation_id }
       }

fresh-challenge attempt
  -> DeliveryUnknown | PreAdmissionProblem
     | ProtocolIncompatibleAccepted | Committed
     | RequiresReconfirmation

response Committed
  -> TerminalResolved { Committed }

response RequiresReconfirmation
  -> TerminalResolved { RequiresReconfirmation }
```

Each reconciliation transition is appended to the group's local state log;
the current state is the latest valid transition and earlier proof is never
rewritten or discarded.

`KnownSettlementQuery` always contains the query from its named
`OutcomeUnknownProblemObservation`; there is no nullable query state. Query
observation may later append an `ApplyAuthorEditAppliedObservation`,
`ApplyAuthorEditZeroAuthorityObservation`,
`OtherEditorCommittedObservation`, `RequiresReconfirmationObservation`, or
another `OutcomeUnknownProblemObservation`. A locally known `AcceptedObservation`
still makes no claim about Admission existence. For the Release-1 editor
commands owned here it instead enters `ProtocolIncompatibleAccepted`: the
browser preserves its exact operation/query evidence and all payload, pauses
the queue, and requires protocol resync or a compatible client/Server pair; it
does not assume any unspecified asynchronous terminal or follow that query as
an editor-command lifecycle. Any repeated query-bearing observation for the
same group must carry the same query; mismatch fails closed into resync rather
than selecting one.

The attempt-to-GC closure matrix is:

| Durable response observation | Reconciliation / settlement | Capsule and journal consequence |
| --- | --- | --- |
| unresolved transport Problem | `TransportOrAdmissionUnknown` / `Unsettled` | keep capsule and all payload; no GC |
| unexpected `Accepted` for a Release-1 editor command | `ProtocolIncompatibleAccepted` / `Unsettled` | response makes exact replay unnecessary, but the queue fails closed and payload remains GC-ineligible |
| `outcome_unknown` Problem | `KnownSettlementQuery` / `Unsettled` | no Receipt claim; payload remains GC-ineligible |
| positive no-Admission Problem, fresh challenge still permitted | `ProvenNoAdmission` / `Unsettled` | collect the old capsule, retain all payload, and commit the new capsule before resend |
| terminal pre-admission Problem, including after fresh-challenge resend | `TerminalResolved { PreAdmissionProblem }` / `PreAdmissionRefused` | collect the attempt capsule; journal GC still requires refusal-surface convergence and an exact complete successor |
| outcome Query unavailable or `StillUnknown` | `OutcomeQueryUnresolved` / `Unsettled` | keep capsule and all payload; preserve the strongest valid observation; no GC |
| outcome Query `RejectedNoAdmission` | `TerminalResolved { OutcomeQueryRejected }` / `OutcomeQueryRejectedNoAdmission` | collect the attempt capsule only after the exact query observation is durable; journal GC waits for `OutcomeQueryRejectedVisible`, an exact successor, and dependency closure |
| applied `ApplyAuthorEdit` v2 response, including from the outcome Query, exact replay, or fresh-challenge resend | `TerminalResolved { Committed }` / `AppliedReceiptSettled` | collect the attempt capsule; journal GC waits for applied Activity/Snapshot convergence and successor proof |
| zero-authority `ApplyAuthorEdit` v2 response, including from the outcome Query, exact replay, or fresh-challenge resend | `TerminalResolved { Committed }` / `ZeroAuthorityReceiptSettled` | collect the attempt capsule; never wait for or fabricate Activity; journal GC still requires the result-visible branch and an exact successor proof |
| other editor `Committed` response | `TerminalResolved { Committed }` / `OtherEditorReceiptSettled` | retain its Receipt/Activity convergence and successor rules |
| `RequiresReconfirmation`, including after a supplied settlement query | `TerminalResolved { RequiresReconfirmation }` / matching terminal settlement | collect the attempt capsule; journal GC waits for the visible terminal surface and section 9's exact successor rule |

The original capsule is collected only after a replayable response observation,
an outcome Query `Committed` or `RejectedNoAdmission` observation, or positive
no-Admission proof for the original attempt is durable. A crash
before capsule commit sends nothing. A crash after capsule commit but before
send leaves an unused replayable capsule. A crash after send but before
response durability leaves the preceding durable state. After reload, crash,
process restart, or continued current-process recovery, an `ApplyAuthorEdit`
`DeliveryUnknown` state with an available exact capsule and still-valid Client
Session permits only the protected outcome Query before any command replay.
Section 10 consumes this same rule; it does not select `ExactTransportReplay`
for `ApplyAuthorEdit`. Other command classes retain their exact replay rule.
Server or PostgreSQL process interruption does not create a different first
browser action. Durable Server facts remain owned by the PostgreSQL Project
Storage, Isolation, and Migration Contract. The browser follows the same
DeliveryUnknown or durable-observation matrix.
Missing/corrupt capsule, invalid or expired Client Session binding, or
unverifiable request equality remains in the applicable inspectable unknown
state; it never permits a new challenge, changed request, or blind repeat.

Here, replayable response evidence means an exact durable `Accepted`,
`Committed`, `RequiresReconfirmation`, `outcome_unknown` Problem with its
query, terminal pre-admission Problem observation, or outcome Query
`Committed` or `RejectedNoAdmission`. An outcome Query `StillUnknown` or
`UnresolvedTransportProblemObservation` is not sufficient and keeps the
capsule `Available`.

An exact replay is the same logical submission, not a new command invocation.
If the nonce was already consumed, #58 permits it only to resolve the same
record, key, and digest to its in-progress or immutable prior response and it
can never create a second Admission. If the original send never consumed the
nonce, normal admission may process that one original submission once. If the
protocol instead positively proves that the original attempt created no
Admission, the same still-unadmitted frozen group may obtain a fresh
challenge. That is a new physical attempt, not recovery invocation of an
admitted command.
For a capsule whose attempt was never appended or started, the journal's
atomic persist-before-attempt order itself is positive proof that nothing was
sent; this is the only local no-Admission proof that does not require a Server
response.

Every typed Core result reaches one explicit Receipt settlement branch.
`AppliedReceiptSettled` is the Activity-backed `ApplyAuthorEdit` branch.
`ZeroAuthorityReceiptSettled` covers `NoEffect`, `Conflicted`, and `Refused`
without a Project Activity member. Settlement, result visibility, applied
convergence, attention, and payload retention remain separate axes and never
override one another.

This correction is a contract-only hard cut, not an IndexedDB migration. At the
locked baseline, the Web journal writes `receipt_settled` only after it validates
`AuthoritativeApplied` and its Activity position; it cannot persist zero authority
or reinterpret an old record. A later runtime must version or migrate that record
and map it to `AppliedReceiptSettled` only when exact applied Receipt and Activity
fields prove the branch; every other old shape fails closed. This correction changes no IndexedDB version or persisted runtime value.

## 4. Completed intent and pre-Admission coalescing

One `CompletedIntentRecord` is the conservative direct-edit unit. Distinct
records retain distinct `AuthorEditUnit.normalized_primitives` and
`selection_snapshot` values and enter a group in local-intent order; their
semantic payloads are not required to be equal.

Composition completion, paste, cut, drop, each of `SplitBlock`, `JoinBlocks`,
`MoveBlock`, and `RetypeBlock`, and every explicit editor command are
unconditional **group flush boundaries**.
The boundary record is isolated from records on both sides: the earlier group
freezes before it, and it freezes no later than its own completion. Every
explicit editor command is one `ExplicitEditorCommandRecord` and one
single-record submission group.
Ordinary typing, deletion, and selection replacement encoded as
`ReplaceSelection` may coalesce under the shared-binding rules below; a
`ReplaceSelection` produced by composition completion, paste, cut, or drop
still honors that input-origin flush boundary.

Before Admission issuance, adjacent completed `direct_editor_action` intents
may be coalesced only while:

- every admission-preparation input named by Author Command Admission
  sections 3 and 5 is present and equal;
- every shared `ApplyAuthorEdit` binding—Project Scope, chapter, retry source,
  target, ownership observation, Authoritative and Proposal Head, Anchor,
  reservation, editor-contract revision, and author-visible undo-group
  binding—is equal;
- the current writer generation remains current;
- none of the candidate records belongs to a frozen group; and
- the applicable idle, intent, operation, and payload bounds remain proven.

Any difference or unverifiable fact freezes the earlier direct-edit group.
Coalescing chooses the ordered coverage of one `JournalSubmissionGroup`; it
never mutates or combines the immutable source intent records. One group-level
idempotency key applies only to the final combined request, and its physical
attempt commits the matching short-lived protected retry capsule, so no
per-intent idempotency identity can conflict. Coalescing never
alters local undo grouping, crosses a partition, includes an explicit command,
or applies to `explicit_editor_command`.

The #70-owned [structured prerelease policy](author-edit-batch-release-1-policy.json)
selects the replaceable Release 1 window and ceilings and records the candidate
set, evidence, and future anonymous calibration gate. Its revision maps exactly
to `storyos.editor-contract.release-1.v2`, which the existing request digest
already binds. The selection is conservative prerelease policy, not a permanent
or real-user-validated product default.

The positive long-session case is therefore reachable: 240 consecutive typing
intents with distinct ordered `AuthorEditUnit` payloads/selections but equal
shared bindings may form one bounded group under the selected prerelease policy.
Changing any shared binding or
encountering composition completion, paste, cut, drop, `SplitBlock`,
`JoinBlocks`, `MoveBlock`, `RetypeBlock`, or an explicit command freezes the
group immediately.

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
| `saved` | `AuthoritativeApplied` has converged through its exact Activity/Snapshot position, or `NoEffect` has resolved against the already-installed exact durable base without advancing that base; the resulting current surface requires no attention |
| `needs_attention` | a visible zero-authority `Refused` or `Conflicted` result, outcome Query rejection with preserved local work, a converged reconfirmation, Draft, or other typed result requires an author decision, or current evidence cannot safely reconstruct the surface |

`Accepted` HTTP acknowledgement, an exact replay that remains in progress, a
Receipt without its applicable result visibility or convergence proof, and an
Event without matching settlement remain `saving` or `needs_attention`; none
is `saved`. A zero-authority Receipt never waits for a nonexistent Activity.

Selection, Decorations, NodeViews, editor history, and cursor position are
presentation state. The exact `AuthorEditUnit.selection_snapshot` and
undo-group binding retained by an intent record are command/recovery evidence,
but restoring them does not make DOM state durable truth. A
`PreAdmissionRefusalConverged`, `OutcomeQueryRejectedVisible`, or
`ReconfirmationConverged` group can be fully converged while `AuthorAttention`
remains `Required`; it is never relabelled `saved` merely to express
convergence.

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
`OutcomeQueryUnresolved`, `outcome_unknown`, gap, stale writer, or required
resync pauses this queue.

A read-only observer submits its one-record takeover group through the
Project-local takeover coordination lane, not through the queue it is asking
to fence. Same-origin takeover requests retain local-sequence order, but the
Server's observed-generation checks and admitted Core/domain compare-and-set
choose the winner. On
`TakeoverApplied`, the old writer queue is fenced immediately: already-admitted
groups reconcile independently, frozen/unsubmitted groups and their payloads
remain retained, and a new writer queue opens only after the returned Snapshot
is installed. A takeover whose observed generation is stale before Admission
receives a pre-admission Problem and no Receipt. A concurrently admitted
takeover that passed issuance bindings but loses the later domain compare
receives `TakeoverCompareFailed`. A terminal pre-admission refusal releases no
dependent group until its current bindings are explicitly rebuilt.

This serial browser admission order prevents dependent edits from overtaking
one another but is not Project authority order. An `ApplyAuthorEdit`
`Refused`, `Conflicted`, or `NoEffect` result allocates no Author Action or
Project Activity. Project authority remains ordered only by Core Heads, Author
Action Sequence, and Project Activity that an owning result actually creates.

## 6. Admission, acknowledgement, and reconciliation

### 6.1 Closed phase matrix

| Boundary | Proven fact | Client projection and next action |
| --- | --- | --- |
| capsule durable, before physical send | exact replay material exists; no delivery or Server fact | append the initial attempt and send once |
| `ApplyAuthorEdit` attempt `DeliveryUnknown`, capsule available, Client Session binding still exact | no claim whether Admission or Receipt exists | enter `OutcomeQueryUnresolved { NoOutcomeObserved }`; call the protected outcome Query first with the stored key and nonce; do not replay the command |
| `ApplyAuthorEdit` attempt `DeliveryUnknown`, capsule missing/corrupt or Client Session invalid/expired | no safe outcome read and no claim whether Admission or Receipt exists | remain `OutcomeQueryUnresolved { NoOutcomeObserved }`; block replay, fresh challenge, and dependent submission |
| another editor-command attempt `DeliveryUnknown`, exact capsule and Client Session still valid | no claim whether Admission or Receipt exists | retain `TransportOrAdmissionUnknown` and that command class's byte-identical exact-replay rule |
| outcome Query transport failure, malformed response, or canonical non-200 Problem | no new command-outcome evidence | append `QueryUnavailable`; preserve the strongest valid observation and remain `Unsettled` |
| outcome Query `StillUnknown { ChallengeIssued }` | exact original challenge remains durable at the returned expiry; no Admission or rejection proof | retain `OutcomeQueryUnresolved`; accept only the same `expires_at`; keep queue, capsule, and payload blocked |
| outcome Query `StillUnknown { AdmissionCommitted }` | exact Command and Admission exist and reconciliation remains required; no Receipt or Activity is named | advance the strongest observation to the exact admitted identity; keep queue, capsule, and payload blocked; never invoke or replay; repeat only the protected GET |
| outcome Query `RequiresReconfirmation` | terminal Admission settlement with no Receipt or Core effect | enter the exact no-Receipt terminal settlement; show the closed reason; never invoke the old command |
| outcome Query `Rejected { challenge_expired_unconsumed }` | positive proof that this identity created no Admission, Receipt, Activity, or authority | enter the exact query-rejected terminal reconciliation, settlement, and visible-author branch; do not fabricate a Problem or resend permission |
| outcome Query `Committed` | exact nested `ApplyAuthorEditResponse` v2 | use the existing applied Receipt-plus-Activity or zero-authority Receipt-only settlement; exact-deduplicate any late POST acknowledgement |
| pre-admission Problem | exact safe `PreAdmissionProblemObservation`; no Admission or Receipt for the proven attempt | retain complete payload; either terminally show the typed refusal or, only when the exact Problem permits and the frozen request remains equal, enter `ProvenNoAdmission` before a fresh challenge |
| unexpected HTTP `Accepted` for a Release-1 editor command | exact asynchronous operation reference and settlement query; no browser-visible Admission identity or terminal fact | enter `ProtocolIncompatibleAccepted`; preserve payload, pause the queue, and fail closed for protocol resync/compatible deployment without claiming query convergence |
| HTTP `applyAuthorEdit` v2 `AuthoritativeApplied` | terminal `AppliedReceiptSettled`, exact typed Receipt, and applied-only Activity position | append `ApplyAuthorEditAppliedObservation`; wait for applied author-facing Activity/Snapshot convergence if not already observed |
| HTTP `applyAuthorEdit` v2 `NoEffect`, `Conflicted`, or `Refused` | terminal `ZeroAuthorityReceiptSettled` and exact typed Receipt; no Activity exists for this result | append `ApplyAuthorEditZeroAuthorityObservation`; make the exact result visible without advancing checkpoint, Activity watermark, authoritative projection, or active base |
| other editor HTTP `Committed` | terminal `OtherEditorReceiptSettled`, exact typed Receipt, and its existing Activity position | retain its Activity/Snapshot convergence contract |
| HTTP `RequiresReconfirmation` | terminal Admission settlement with no Receipt or Core effect | show the exact reconfirmation reason and applicable preserved payload/Draft; a later author confirmation creates a new command and Admission |
| post-admission `outcome_unknown` Problem | exact Admission identity and settlement query from that Problem; no claim about Receipt presence | enter `KnownSettlementQuery`; forbid blind retry or a new command derived from the uncertain one |
| Project Activity before an Activity-backed HTTP result | committed Event position, not necessarily the client's matching acknowledgement observation | retain/deduplicate the Event; for missing `ApplyAuthorEdit` acknowledgement use the protected outcome Query, otherwise use an actually supplied settlement query or the owning command's permitted recovery action; converge only after the exact Receipt/result is known |
| Activity-backed HTTP result before projection reaches `project_activity_position` | applied Receipt settlement known, author-facing projection not yet converged | retain payload and wait/replay/query until Event processing or a Snapshot proves a position at or beyond the settlement |

`Accepted` never carries or implies a typed Receipt and is not an Admission
terminal state; it also does not prove an Admission identity. A qualifying
pre-admission Problem proves Receipt and Admission absence only for the
attempt it positively classifies. Post-admission uncertainty never proves
Receipt absence.

Release 1 closes the acknowledgement route for every editor command consumed
by this contract:

| Editor command class | Owning execution shape | Permitted first terminal/nonterminal response |
| --- | --- | --- |
| `ApplyAuthorEdit` | complete owning short transaction | `Committed`, `RequiresReconfirmation`, or pre/post-admission `StoryOSProblem`; never `Accepted` |
| `TakeOverProjectWriter` | complete owning short transaction | `Committed`, `RequiresReconfirmation`, or pre/post-admission `StoryOSProblem`; never `Accepted` |
| explicit editor decisions, including Proposal, Draft, undo, and reversal controls | complete owning short transaction | `Committed`, `RequiresReconfirmation`, or pre/post-admission `StoryOSProblem`; never `Accepted` |

This is the protocol's existing rule that work able to settle in its owning
short transaction uses `Committed`; long work is modeled as an asynchronous
operation before Author Command Admission. The general #58 `Accepted` variant
remains nonterminal for command kinds that legitimately own asynchronous work,
but receiving it for one of these Release-1 editor commands is a protocol
incompatibility, not a promise that the editor can obtain a defined terminal
state from its query.

### 6.2 Reconciliation matrix

When the first `ApplyAuthorEdit` acknowledgement is wholly absent, section 3.5's
protected `getApplyAuthorEditOutcome` read runs first from the original capsule;
the browser never sends the command again before that read. This contract
consumes the existing #58 route and adds no public route. For other command
classes, the existing exact-replay and supplied-settlement-query rules remain
unchanged. When an `outcome_unknown` Problem actually supplies a settlement
query, reconciliation uses that exact query. An unexpected
`Accepted` preserves its exact query only as protocol-incompatibility evidence;
the Release-1 editor lifecycle does not follow it. Outside the named
`ApplyAuthorEdit` outcome route and an exactly supplied settlement query, the
browser has no query and never invents one. Journal presence, cache, process
state, missing HTTP, or Event arrival is never an effect oracle.

| Authoritative finding | Required client behavior |
| --- | --- |
| exact replay for another command returns `PreAdmissionProblemObservation` that positively proves the original attempt created no Admission | enter `ProvenNoAdmission`; either terminally resolve that refusal or, only when its retry semantics and every frozen local fact allow, obtain a fresh challenge and append a fresh physical attempt |
| outcome Query returns `applyAuthorEdit` v2 `AuthoritativeApplied` | append the exact query observation and one `ApplyAuthorEditAppliedObservation` lifecycle effect, enter terminal `AppliedReceiptSettled`, wait for its exact Activity/Snapshot convergence, and never invoke again |
| outcome Query returns `applyAuthorEdit` v2 `NoEffect`, `Conflicted`, or `Refused` | append the exact query observation and one `ApplyAuthorEditZeroAuthorityObservation` lifecycle effect, enter terminal `ZeroAuthorityReceiptSettled`, make only that Receipt result visible with no Activity/checkpoint/projection/base advance, and never invoke again |
| outcome Query returns `Rejected { challenge_expired_unconsumed }` | append `RejectedNoAdmission`; enter `OutcomeQueryRejectedNoAdmission` and `OutcomeQueryRejectedVisible` through the same query observation identity; retain the author payload and never resend automatically |
| outcome Query returns `StillUnknown { ChallengeIssued }` | preserve `OutcomeQueryUnresolved`, exact same expiry, `Unsettled`, queue block, capsule, and payload; no success, rejection, or retry permission |
| outcome Query returns `StillUnknown { AdmissionCommitted }` | preserve the exact Command and Admission as the strongest valid observation, remain `Unsettled`, and never invoke or replay the admitted command |
| outcome Query returns `RequiresReconfirmation` | append that exact observation, enter the terminal no-Receipt settlement, and never invoke the old command |
| outcome Query transport or Problem handling is unavailable | append only `QueryUnavailable`; preserve the strongest valid observation and remain unresolved |
| late original POST acknowledgement and outcome Query `Committed` both arrive | require the same Scope, key, digest, Command, Admission, Receipt, nested command correlation, and result; append both channel observations but create one settlement and one author-facing effect |
| exact replay or settlement Query returns another editor `Committed` result | append `OtherEditorCommittedObservation`, enter `OtherEditorReceiptSettled`, retain its existing Activity/Snapshot convergence contract, and never invoke again |
| settlement Query returns `RequiresReconfirmation` | append that exact observation, enter the terminal no-Receipt settlement, and never invoke the old command |
| another command's exact replay or supplied Query remains `outcome_unknown` | retain `TransportOrAdmissionUnknown` or `KnownSettlementQuery` as applicable; block blind invocation and dependent submissions |
| exact replay or Query unexpectedly returns `Accepted` for a Release-1 editor command | enter `ProtocolIncompatibleAccepted`; retain all evidence/payload and fail closed without assuming or polling an unspecified terminal |
| authoritative Server recovery finds validated no Receipt for an already admitted command | the Server applies #68's same-Admission rule; the browser neither reconstructs Server records nor supplies an Admission lifetime, and observes only the eventual #58 response |

The explicit-command original-submission/recovery boundary is:

| Boundary | Permitted action | Logical identity |
| --- | --- | --- |
| before the initial send | send the already-confirmed frozen command once after capsule durability | original explicit submission |
| `ApplyAuthorEdit` delivery unknown with exact capsule | call `getApplyAuthorEditOutcome` with the original key and nonce before any command replay | read-only observation of the original identity; not a command or Admission |
| another editor command delivery unknown with exact capsule | exact transport replay through the same route | same original key/body/digest/decision; not a second command or Admission |
| positive proof that the attempt created no Admission | at most one fresh-challenge physical attempt when every frozen fact remains equal | same original key/body/digest/decision; completion of the original submission |
| Admission exists and no Receipt is yet proven | no automatic Core invocation for an explicit command; settle or observe `RequiresReconfirmation` | original Admission closes without Receipt |
| author visibly reconfirms after `RequiresReconfirmation` | form and submit a new command | new key, nonce, Command, and Admission |

The Server-side automatic branch requires equality of Project Scope, protected Client
Session binding/generation, client/security contracts, Editor Session, writer
generation, action class, request contract, final digest/profile and covered
fields, targets/Heads/Revisions, idempotency and nonce records, complete
`ApplyAuthorEdit` intent/selections/retry source/ownership/Anchors/reservations,
editor contract, undo group, and durable journal reconstruction. Equality of a
subset is failure.

A visible reconfirmation creates a new idempotency key, anti-forgery challenge,
Command, Admission, and eventual Receipt. Once an Admission exists,
Acceptance, rejection, withdrawal, Draft closure, Author Undo, takeover, and
other explicit commands are never automatically invoked by no-Receipt
recovery; they settle `RequiresReconfirmation`. Before Admission, exact
transport replay, or one fresh-challenge attempt after positive no-Admission
proof, may only complete the same already-confirmed logical submission. It
retains the same frozen body, digest, key, action and decision and cannot
become a second explicit command or Admission.

## 7. Branch-shaped result visibility, Activity convergence, Snapshot, and resync

The client consumes the one Project Activity Stream for results that actually
create Project Activity. It durably deduplicates by `event_id`, validates replay
generation and contiguous `stream_sequence` within that generation, and uses
the Event's Project Activity position, Receipt reference, resulting Heads, and
typed cause. HTTP and SSE may arrive in either order; arrival time changes no
meaning. A zero-authority `ApplyAuthorEdit` Receipt has no matching Event and
never enters this Activity convergence path.

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
4. replays only an available exact-transport capsule or follows a
   `KnownSettlementQuery` actually supplied by an `outcome_unknown` Problem;
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
| `AppliedReceiptSettled` | the exact `ApplyAuthorEditAppliedObservation`, `DomainReceiptRef`, and applied-only canonical `project_activity_position`; processed Project Activity or a Snapshot must prove projection at or beyond that same position; the resulting authoritative Head and Commit are reflected before `AppliedReceiptConverged` |
| `ZeroAuthorityReceiptSettled` | the exact `ApplyAuthorEditZeroAuthorityObservation`, `DomainReceiptRef`, and `NoEffect | Conflicted | Refused` effect are visible as `ZeroAuthorityReceiptVisible`; no Project Activity position, checkpoint advance, authoritative projection advance, or active-base roll-forward exists or is fabricated |
| `OtherEditorReceiptSettled` | the exact `OtherEditorCommittedObservation`, `ReceiptRef`, and canonical `project_activity_position`; processed Project Activity or a Snapshot proves projection at or beyond it, preserving the pre-correction convergence rule |
| `PreAdmissionRefused` | the exact `PreAdmissionProblemObservation`, typed refusal surface, and complete preserved local payload are visible; no Activity position or resulting Head is required or fabricated |
| `RequiresReconfirmation` | the exact `RequiresReconfirmationObservation`, reason, and applicable retained payload or returned `recovery_draft_ref` plus reconfirmation controls are visible; no Receipt, Core effect, Activity position, or resulting Head is required or fabricated |

Applied manuscript convergence additionally requires no unresolved earlier
Activity gap. Zero-authority result visibility neither clears nor creates an
unrelated Activity gap. A no-Receipt refusal/reconfirmation surface may
converge while an unrelated manuscript projection remains in resync, but it
cannot label that manuscript surface `saved`. Every branch rejects an
incompatible writer generation for the surface being shown. `AuthorAttention`
is evaluated separately: a visible zero-authority conflict or refusal, or a
converged Draft or reconfirmation, normally remains `Required`.

## 8. Core result and control projection

### 8.1 `ApplyAuthorEdit` result matrix

The client projects the exact Receipt result; it never infers a Proposal
Acceptance from an authoritative result or from the absence of a candidate.

| Core result | Author-facing surface after settlement | Controls and journal consequence |
| --- | --- | --- |
| `AuthoritativeApplied` | resulting authoritative Head and Commit become `saved` only after `AppliedReceiptConverged` and complete active-base installation | no recovery controls; retain journal payload until GC successor proof |
| `NoEffect` | `ZeroAuthorityReceiptVisible`; the already-installed durable base and exact current surface may remain `saved` only when they independently prove the no-op | no Activity, checkpoint, projection, or base advance; retain until the unchanged durable Revision is an exact digest-equal successor |
| `Conflicted` | `ZeroAuthorityReceiptVisible`; complete local intent remains beside the independently current authoritative projection; `needs_attention` | no Activity, checkpoint, projection, or base advance; retain payload and revalidate Heads before dependent submission |
| `Refused` | `ZeroAuthorityReceiptVisible`; exact refusal reason and complete local intent remain visible; `needs_attention` | no Activity, checkpoint, projection, or base advance; retain payload until an explicit safe successor exists |

This response-v2 table does not redefine Proposal, Draft, or recovery command
results. `ProposalConflict` and `ProposalRecoveryConflict` remain conditions on
a preserved Proposal surface. Neither is a Draft Artifact. A healthy local
journal retaining complete conflicted text is not automatically converted into
a `RecoveryDraft`.

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

Release 1 has exactly one reachable browser-to-Host ingress for recovered
author-edit bytes: the existing
`POST /api/v1/projects/{project_id}/manuscript/author-edits` route carrying the
complete `ApplyAuthorEdit.author_edit_units` and submission bindings. Once that
request is admitted, the Host already retains the exact command body and can
create a `RecoveryDraft` while settling `RequiresReconfirmation`; the exact
#58 response returns `recovery_draft_ref` when creation occurred, and the
existing `GET /api/v1/projects/{project_id}/artifacts/{artifact_id}` projection
exposes its `EditorRecovery.recovery_evidence_ref`. #70 adds no route, request
field, upload, or hidden browser-to-Host channel.

`EditorRecoveryEvidenceId` identifies the immutable Host-owned Operational
Record for only that reachable path:

```text
EditorRecoveryEvidenceRecord {
  editor_recovery_evidence_id
  project_scope
  editor_session_id
  writer_generation

  admitted_apply_author_edit {
    route_template:
      /api/v1/projects/{project_id}/manuscript/author-edits
    command_id
    author_command_admission_id
    canonical_command_digest
    digest_profile
    application_wire_record_ref
  }
  complete_author_edit_units_ref
  complete_payload_digest
  editor_contract_revision
  requires_reconfirmation {
    reconfirmation_reason
    recorded_at
  }
  host_created_at
  resulting_recovery_draft_ref
}
```

The Host writes the evidence record and resulting `RecoveryDraft` revision in
one durable operation, so the record never points to a missing result and the
Draft Creator's `recovery_evidence_ref` resolves to this exact identity. The
`application_wire_record_ref` is #58's existing identity for the exact
schema-valid, authorized command bytes retained at the application ingress; the
evidence additionally binds those bytes to the complete Author Edit Units and
canonical digest/profile. The evidence binds only Server-retained fields from
the admitted `ApplyAuthorEdit` body and its real Admission settlement; it does
not claim that browser journal, checkpoint, in-memory, or takeover bytes
crossed an interface.

A `RecoveryDraft` exists only when the StoryOS Host durably creates one Core
Draft Artifact with:

- Host-assigned `EditorRecovery` Creator;
- exact Project Scope, Editor Session, and writer generation;
- the complete structured author-edit payload and digest already retained from
  the admitted `ApplyAuthorEdit` request;
- exact `EditorRecoveryEvidenceId` and `RequiresReconfirmation` settlement; and
- one immutable Artifact Revision and provenance closure.

Creation causes no Core edit invocation and no Author Action. A browser-local
record/group, in-memory text, copyable text, clipboard content, checkpoint, or
pending projection is never a `RecoveryDraft` or another Artifact.

The ingress matrix is closed:

| Recovery source | Release-1 Host ingress and result |
| --- | --- |
| admitted `ApplyAuthorEdit` body reaches terminal `RequiresReconfirmation` and the Host retains its complete verified payload | the existing author-edits command/settlement may atomically create the evidence record and `RecoveryDraft`; #58 returns non-null `recovery_draft_ref` |
| the same settlement cannot verify a complete retained payload | `recovery_draft_ref: null`; preserve any complete browser copy locally |
| frozen/unsubmitted journal group, pre-admission refusal, or delivery/Admission uncertainty | local journal/capsule only; no Host Artifact ingress and no `RecoveryDraft` claim |
| takeover, fenced-writer journal payload, journal schema/persistence recovery, or explicit local preservation | local-only until a future wire owner adds an explicit semantic command and schema; takeover does not carry those bytes |
| DOM, clipboard, or surviving in-memory text | copyable local material only; no Artifact or recovery-evidence record |

This contract therefore does not make journal, in-memory, or takeover evidence
Host-creatable in Release 1. The broader Artifact classification remains
reserved, but reachability requires a future public-protocol owner to add a
real ingress before any such source may allocate `EditorRecovery`.
A complete local journal group may still enter the ordinary
author-confirmed/current-binding `ApplyAuthorEdit` flow; that is a normal edit
attempt, not a RecoveryDraft-preservation channel.

A `RecoveryDraft` Retry is a new `ApplyAuthorEdit`/`DraftRetry` with an
explicit author confirmation and new Admission unless section 6's exact
same-Admission automatic branch still applies before Draft creation. Its
result and closure follow section 8.2.

## 10. Reload, crash, and takeover recovery matrix

| Last durable boundary | Reload/recovery result |
| --- | --- |
| before intent-record transaction commit | no durable record; any surviving in-memory text is copyable local payload, not saved or a Draft |
| intent record committed, before group freeze/Admission | rebuild complete intent; current writer may freeze a new group after all bindings revalidate |
| group frozen, before capsule transaction commit | no send was permitted; obtain a challenge, commit the capsule, and append the initial attempt |
| capsule committed, no attempt appended or send started | local atomic order positively proves no delivery/Admission; if the challenge remains valid append/send the initial attempt, otherwise collect the unused capsule and obtain a fresh challenge for the same frozen group |
| send may have occurred, crash before response observation commits, `ApplyAuthorEdit`, capsule available, Client Session binding still exact | reopen the exact capsule; enter `OutcomeQueryUnresolved { NoOutcomeObserved }` if not already there; call `getApplyAuthorEditOutcome` first with the stored key and nonce; do not replay the command, obtain a new challenge, or send again |
| send may have occurred, crash before response observation commits, another editor command, exact capsule and Client Session still valid | reopen the exact capsule and append only `ExactTransportReplay` through the original command route |
| `ApplyAuthorEdit` delivery unknown and Client Session binding is invalid/expired or capsule/request equality is unverifiable | remain `OutcomeQueryUnresolved { NoOutcomeObserved }`; block replay, fresh challenge, and dependent submission |
| another editor-command delivery unknown and Client Session binding is invalid/expired or capsule/request equality is unverifiable | remain `TransportOrAdmissionUnknown`; no exact replay, fresh challenge, changed request, or blind repeat |
| exact replay positively proves no Admission | enter `ProvenNoAdmission`; either converge the pre-admission refusal or, only when its exact retry semantics permit, commit a fresh-challenge capsule and append a new physical attempt |
| unexpected `Accepted` observation recorded for a Release-1 editor command | enter `ProtocolIncompatibleAccepted`; retain its exact operation/query evidence and payload, pause the queue, and require protocol resync/compatible deployment without inferring Admission or query convergence |
| `outcome_unknown` Problem recorded | `KnownSettlementQuery` from the exact Problem; no Receipt-presence claim and no blind invocation |
| validated no-Receipt after Admission | follow only section 6.2's direct-edit equality or `RequiresReconfirmation` branch |
| applied Core result committed before HTTP or Activity observation | exact Receipt, applied effect, Activity relation, and Heads survive; replay them and converge without another invocation |
| zero-authority Core result committed before HTTP observation | exact Receipt and zero-authority effect survive with no Activity relation; replay them into `ZeroAuthorityReceiptVisible` without another invocation or any checkpoint/projection/base advance |
| Activity-backed Receipt observed before matching Activity | retain payload and replay/query from the required Activity position |
| Activity observed before its Activity-backed HTTP result | retain/deduplicate Event; for missing `ApplyAuthorEdit` acknowledgement use the protected outcome Query; otherwise use an actually supplied `outcome_unknown` query, wait for HTTP, or use the available exact replay capsule; Event arrival alone does not settle the local group |
| pre-admission refusal or `RequiresReconfirmation` observed | converge through the applicable no-Receipt branch without requiring Activity/Heads; retain author attention and payload |
| applied convergence or zero-authority result visibility proven before payload collection | group becomes GC-eligible only if section 11's exact successor proof also holds |
| payload collected before reload | reconstruct from the recorded durable successor and retained collection fence; never from missing bytes |
| reload in stale writer generation | read-only; reconcile admitted groups, preserve unsubmitted payload, and require explicit takeover for new writing |
| `TakeoverApplied` during unsettled work | open the exact new-generation partition; older partitions remain read-only; every group follows its own observed response/query/capsule state; unsubmitted or browser-only payload remains local |
| stale/changed takeover generation detected before Admission | retain the current read-only projection and exact pre-admission Problem; no Admission, Receipt, takeover Activity, or generation change exists |
| `TakeoverCompareFailed` after issuance bindings passed | retain the current read-only projection, `DomainReceiptRef`, canonical settlement Activity position, and returned Snapshot at or beyond it; do not advance generation, rebind a partition, or retry automatically |
| takeover delivery unknown | exact transport replay is allowed only with its available capsule and current binding; never obtain a fresh challenge until positive no-Admission proof |
| takeover `outcome_unknown` or `RequiresReconfirmation` | use only the actually supplied query or visible reconfirmation fields; after Admission exists, never invoke the explicit takeover automatically |
| takeover unexpectedly returns `Accepted` | enter `ProtocolIncompatibleAccepted`; preserve evidence and fail closed rather than assuming an asynchronous takeover terminal |

Server or PostgreSQL process interruption uses this same section 10 matrix. It
does not create a different first browser action.

## 11. Deterministic journal garbage collection

### 11.1 Eligibility

Secret capsule collection is separate from journal-payload collection. An
`Available` capsule is retained while exact replay may still be required and
is collected only under section 3.5's closed positive reasons. Its attempt
identity, frozen request digest, and collection reason remain; its nonce and
exact header bytes do not.

Journal intent payload bytes, group payload coverage, and patch/checkpoint
dependencies are eligible for collection only when all of these are proven:

1. the group has terminal `AppliedReceiptSettled`,
   `ZeroAuthorityReceiptSettled`, `OtherEditorReceiptSettled`,
   `RequiresReconfirmation`, `PreAdmissionRefused`, or
   `OutcomeQueryRejectedNoAdmission` evidence as applicable and
   `GroupReconciliation.TerminalResolved` names the exact matching protocol
   or query observation; `OutcomeQueryUnresolved`, `TransportOrAdmissionUnknown`, `KnownSettlementQuery`,
   `ProtocolIncompatibleAccepted`, `ProvenNoAdmission`, and `Unsettled` are
   ineligible;
2. the author-facing surface has resolved through the applicable branch:
   applied results include their required Activity/Snapshot position;
   zero-authority results include `ZeroAuthorityReceiptVisible` and no Activity,
   checkpoint, projection, or base advance; other editor results follow their
   owner; and pre-admission refusal, outcome Query rejection, and
   `RequiresReconfirmation` include their exact no-Receipt surface and impose
   no fabricated Activity/Head requirement;
3. the complete payload has an exact durable successor that is independently
   readable and digest-verified, or another retained complete journal
   materialization still covers it; and
4. no unsettled group, intent record, checkpoint, retry, undo candidate,
   reconciliation record, or visible recovery surface depends on the bytes;
   and
5. the collector reads the current Project writer generation and revalidates
   the exact owning partition disposition. `CurrentWriterOpen` must still match
   the current generation; `ReadOnlyFenced` may collect under its immutable old
   generation only when its recorded resulting generation and all other
   successor/dependency proofs match; `ReadOnlyObserver` must still be
   non-writer under the current Server projection; and `Closed` must retain one
   exact closed reason plus the corresponding closed Client/Editor/Project
   binding evidence. An old partition is never required to become current
   again.

Exact durable successors include the resulting Authoritative or Proposal
Revision, a `RefusedEditDraft` or `RecoveryDraft` Artifact Revision, or another
typed retained Core payload that contains the complete intent. A
zero-authority Receipt alone is not a successor. `NoEffect` may use the exact
digest-equal current durable Revision. A conflict, refusal, or
`RequiresReconfirmation` with only one local complete copy is never eligible.
Only section 9's admitted-`ApplyAuthorEdit` ingress can make a
`RecoveryDraft` successor in Release 1; local-only takeover, journal, or
in-memory payload does not.
Clipboard copy, DOM text, cached response, Event payload, and an author-visible
label are not durable successors.

### 11.2 Collection transaction and retained fence

Collection is a batched IndexedDB transaction. It:

1. revalidates terminal settlement, reconciliation, convergence, successor
   digest, current Project writer generation, exact partition disposition, and
   dependency closure;
2. marks the exact groups, intent records, and patch/checkpoint ranges
   `Eligible`;
3. writes one immutable local `collection_fence` containing group/record/range
   identities, payload digests, successor references, exact protocol
   observation identities and fields, the branch-shaped convergence evidence,
   and reason;
4. deletes only the covered payload bytes and now-unreferenced checkpoint or
   patch material; and
5. marks those ranges `Collected`.

The compact group and intent identities, command digest/idempotency binding,
attempt log, typed Receipt or refusal/reconfirmation observations,
reconciliation states, successor references, projection convergence, and
collection fence remain inspectable under their owning retention contracts.
Browser evidence retains an Admission identity only when the exact received
variant supplied it through `ApplyAuthorEditAppliedObservation`,
`ApplyAuthorEditZeroAuthorityObservation`, `RequiresReconfirmationObservation`,
`OutcomeUnknownProblemObservation`, or the outcome Query `AdmissionCommitted`
observation. GC never
silently erases evidence needed to reject an exact duplicate, explain a
result, or reconstruct why payload collection was safe.

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
- non-overlapping complete group coverage; group-level idempotency/body/digest;
  protected short-lived exact-retry capsule handling; outcome Query key-in-path
  and nonce-header-only proof; exactly one record per explicit-command group;
  and no per-intent request identity;
- 240 distinct ordered typing intents coalescing under equal shared bindings;
  coalescible ordinary typing/deletion/selection `ReplaceSelection`; every
  composition/paste/cut/drop/`SplitBlock`/`JoinBlocks`/`MoveBlock`/
  `RetypeBlock`/explicit-command group flush boundary; and no post-Admission
  merge;
- exact Protected Web Client, Client Session, Editor Session, writer
  generation, Project Scope, request, digest, Head/Anchor, nonce/idempotency,
  capsule, and challenge-expiry substitutions;
- pre-admission refusal, unexpected `Accepted`, `Committed`,
  `RequiresReconfirmation`, post-admission `outcome_unknown`, validated
  no-Receipt recovery, and exact retry;
- the closed `PreAdmissionProblemObservation`,
  `UnresolvedTransportProblemObservation`, `AcceptedObservation`,
  `ApplyAuthorEditAppliedObservation`,
  `ApplyAuthorEditZeroAuthorityObservation`,
  `OtherEditorCommittedObservation`,
  `RequiresReconfirmationObservation`, and
  `OutcomeUnknownProblemObservation` field matrix, including no invented
  Admission identity/query/lifetime and no Receipt on Accepted,
  reconfirmation, or outcome-unknown;
- the closed `ApplyAuthorEditOutcomeQueryAttempt`,
  `ApplyAuthorEditOutcomeQueryObservation`, `OutcomeQueryUnresolved`,
  `OutcomeQueryRejectedNoAdmission`, and `OutcomeQueryRejectedVisible` shapes;
  complete group/capsule/request/envelope/response-byte identity; all three
  public outcomes; `QueryUnavailable` strongest-state preservation; same-expiry
  and same-Admission progress; terminal immutability; and cross-channel exact
  deduplication;
- the Release-1 editor acknowledgement-route matrix: `ApplyAuthorEdit`,
  `TakeOverProjectWriter`, and explicit editor decisions settle in their
  owning short transaction, while unexpected `Accepted` enters
  `ProtocolIncompatibleAccepted` without assumed query convergence;
- applied HTTP-before-Activity, Activity-before-applied-HTTP, zero-authority
  Receipt-only settlement, acknowledgement loss, duplicates, older-cursor
  overlap, gaps, replay-floor misses, Snapshot/resync, and branch-shaped result
  visibility or projection convergence;
- crash cuts before/after journal commit, capsule commit, physical send,
  response-observation durability, capsule collection, first Core invocation,
  Core commit, Activity, convergence, GC eligibility, and collection;
- initial, outcome-Query, exact-replay, and proven-no-Admission fresh-challenge
  physical attempts; the at-most-one fresh-challenge bound; invalid/expired
  Client Session fail-closed behavior; the `ApplyAuthorEdit` GET-first path from
  delivery unknown to committed, rejected, or inspectable unknown, including
  after reload, crash, or process restart; and no second command invocation;
- chapter switching with pending records/groups, one current-writer queue, and
  the separately fenced takeover coordination lane;
- secondary read-only sessions; exact `TakeoverApplied`,
  post-issuance `TakeoverCompareFailed`, pre-admission stale-generation
  refusal, acknowledgement loss, and `RequiresReconfirmation`; stale-writer
  fencing; new-generation partition creation; closed `DomainReceiptRef`;
  canonical Project Activity allocation only when the owning tagged result
  permits it—`ApplyAuthorEdit` permits it only for `AuthoritativeApplied`, while
  takeover retains its separate owner-defined applied/compare-failed rule—and
  preservation without automatic Draft
  fabrication;
- every `ApplyAuthorEdit`, source-Draft, explicit decision, exact retry, undo,
  reversal, and stale-control row in section 8;
- positive classification of Admission/refusal/reconciliation evidence,
  Receipts, Draft Artifacts, Proposal conditions, local payload, projections,
  and authority;
- the Release-1 `RecoveryDraft` ingress matrix: only an admitted
  `ApplyAuthorEdit` body's terminal `RequiresReconfirmation` may return a
  Host-created Draft, with exact authorized bytes named by
  `application_wire_record_ref`; journal, takeover, and in-memory sources
  remain local-only; and
- applied Activity-backed convergence, zero-authority Receipt-only result
  visibility, and no-Receipt refusal/reconfirmation convergence, including
  proof that `ApplyAuthorEdit` `NoEffect`, `Conflicted`, and `Refused` have no
  fabricated Activity/checkpoint/projection/base advance;
- GC refusal for unknown, unsettled, unconverged, dependency-bearing, or
  only-complete-copy payloads, plus atomic batched collection and retained
  evidence fences; and
  `OutcomeQueryUnresolved/TransportOrAdmissionUnknown/KnownSettlementQuery/
  ProvenNoAdmission -> TerminalResolved -> eligible` reconciliation across both
  current and correctly fenced old partitions, while unresolved states and
  `ProtocolIncompatibleAccepted` remain fail-closed and ineligible.

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
   coverage plus one action class, request contract, idempotency key, final
   body/digest, transport-attempt, settlement, and reconciliation lifecycle.
   The nonce exists durably only inside its short-lived protected exact-retry
   capsule and is never browser authority or general journal evidence.
5. The Web Client never generates or supplies an
   `AuthorCommandAdmissionId`; the Server issues and attaches it only after
   the final request and digest are fixed.
6. Coalescing is pre-Admission, bounded, shared-binding-equality-only, and
   direct-edit-only. It preserves distinct ordered Author Edit Units and never
   crosses composition, paste, cut, drop, `SplitBlock`, `JoinBlocks`,
   `MoveBlock`, `RetypeBlock`, partition, or explicit-command group flush
   boundaries; ordinary typing/deletion/selection `ReplaceSelection` remains
   coalescible when every shared binding is equal.
7. HTTP `Accepted` is nonterminal. Only `Committed` with a typed Receipt or
   `RequiresReconfirmation` closes an Admission, and `Accepted` itself proves
   no browser-visible Admission identity. Release-1 editor commands settle in
   an owning short transaction, so their unexpected `Accepted` response fails
   closed as `ProtocolIncompatibleAccepted`.
8. Post-admission uncertainty claims neither Receipt presence nor absence,
   forbids blind retry, and uses the named protected `getApplyAuthorEditOutcome`
   route only for the original `ApplyAuthorEdit` key and nonce. Any other
   settlement query exists only when the exact received `outcome_unknown`
   Problem supplied it. An unexpected editor
   `Accepted` query is retained as incompatibility evidence, not followed as a
   defined editor terminal path.
9. Automatic recovery invocation requires validated no-Receipt evidence, the
   same unexpired direct edit, and equality of every Admission, Core, and
   journal binding.
10. Pending projection, Snapshot, journal, cache, DOM, and network state never
   become authority or a settlement oracle.
11. `ApplyAuthorEdit` convergence is tagged by result. Only
    `AuthoritativeApplied` retains a canonical `project_activity_position` and
    uses processed Activity or a Snapshot at or beyond it.
    `NoEffect`, `Conflicted`, and `Refused` use
    `ZeroAuthorityReceiptVisible` with no Activity, checkpoint, authoritative
    projection, or active-base advance. Pre-admission refusal and
    `RequiresReconfirmation` converge through their exact no-Receipt surfaces
    without fabricated Activity or resulting Heads.
12. Every Core result, source-Draft disposition, explicit decision, undo, and
    reversal projects
    only its exact permitted controls; a closed source has no stale retry or
    expansion control and no result fabricates Proposal Acceptance.
13. Takeover is one non-auto-recoverable explicit-command group whose applied
    or compare-failed result is carried only by the existing
    `DomainReceiptRef`; both results settle with canonical Project Activity,
    and an applied result opens a new generation partition without rebinding an
    old one.
14. A `RecoveryDraft` is a durable Core Draft Artifact created only by
    Host-assigned `EditorRecovery` from a complete Server-retained admitted
    `ApplyAuthorEdit` body and one exact `EditorRecoveryEvidenceRecord`.
    Browser-only journal, takeover, and in-memory bytes have no Release-1 Host
    ingress.
15. Journal payload collection requires terminal settlement, the applicable
    applied convergence or zero-authority result-visible proof, an exact durable
    complete successor, and no remaining dependency; a zero-authority Receipt
    alone is not a successor, and unknown, unsettled, or only-complete-copy
    payloads remain retained.
16. `ApplyAuthorEdit` delivery uncertainty permits only the protected outcome
    Query before any command replay, including after reload, crash, or process
    restart. Other editor commands retain exact
    transport replay through their original command route. A missing capsule or
    invalid/expired Client Session remains in the applicable inspectable unknown
    state; it never creates a second Admission or blindly re-invokes an admitted
    explicit command.
17. Journal GC requires `TerminalResolved` with its exact
    pre-admission, Committed, outcome Query rejected, or reconfirmation
    observation. A fenced old partition may collect after exact successor and
    disposition proof without pretending its generation is current.
