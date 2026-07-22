# Versioned Command, Query, Artifact, and Event Protocol

Status: accepted Foundation protocol specification

Decision owner: [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58)

Primary-source evidence: [Versioned HTTP, SSE, schema, digest, and MCP protocol primary sources](../research/versioned-http-sse-protocol-primary-sources.md)

Security input: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)

## 1. Purpose and authority

This specification fixes the Foundation wire contract shared by the StoryOS
Client, StoryOS Server, Core boundary, Workers and Adapters, and external
Provider, Tool, MCP, and MCP App integrations. It turns already accepted
StoryOS domain semantics into versioned, bounded, replayable, generated
contracts. It does not redefine those semantics and does not authorize
production implementation.

The protocol preserves these fixed product boundaries:

- StoryOS supports page-by-page or passage-by-passage Discovery Writing. No
  command, query, event, Artifact, or compatibility mechanism creates an
  Agent-authored outline, an Author Plan, or preplanned story structure.
- The default author experience remains a zero-configuration conversational
  Agent plus direct editor operations. Protocol versions, cursors, manifests,
  limits, and compatibility records are inspectable system contracts, not
  routine author settings.
- The Foundation Validation Deployment bootstraps one local User without an
  account or login product. The same DTOs and authority rules support a later
  controlled cloud deployment with multiple isolated Users.
- Every Project has one Project Author. Shared ownership, ownership transfer,
  teams, billing, and multi-author collaboration are absent.
- PostgreSQL is authoritative. A Project is never a directory, database, file,
  SQLite database, vector store, Neo4j graph, microservice, broker topic, or
  event-sourced aggregate.
- Models and embeddings use external APIs. Provider contracts remain neutral;
  Bailian is only a current test Provider.
- Tool, MCP, MCP App, Provider, research, import, and author-provided content is
  untrusted and non-authoritative. Only an admitted StoryOS Host/Core command,
  and where required Proposal plus explicit Acceptance, may change
  Authoritative State.

Normative domain meaning remains in [CONTEXT.md](../../CONTEXT.md), the
accepted ADRs, and the existing Foundation specifications. Where a wire example
and a domain contract appear to disagree, the domain contract wins and the
wire schema must be corrected before generation.

## 2. Ownership and exclusions

This ticket owns:

- stable public Client-to-Server HTTP and SSE contracts;
- shared Core command, query, Artifact, event, Receipt, error, Attempt, and
  compatibility DTO boundaries;
- internal Worker/Adapter handoff contracts only to the extent required to
  preserve idempotency, fencing, OutcomeUnknown, and external compatibility;
- exact Host bindings around Provider, Tool, MCP, MCP App, Credential
  Reference, disclosure, import, and export messages;
- Rust-source generation requirements, canonical fixtures, wire evidence,
  schema drift rules, and absolute protocol ceilings.

This ticket does not own:

- production Rust, TypeScript, browser, database, generator, or test code;
- crate, module, or repository placement;
- database migrations or a complete identity/account system;
- retention duration, compaction, archival, tombstone payload policy, backup
  retention, or the materialization policy for Snapshots;
- executable verification suites, Eval evidence requirements, or the first
  production vertical slice.

## 3. Protocol surface inventory

StoryOS does not use one universal envelope for every boundary. Stable fields
are composed into surface-specific contracts.

| Surface | Stability owner | Contract shape | Must not contain |
| --- | --- | --- | --- |
| Client to StoryOS Server | public API N/N-1 | resource-specific HTTP Commands and Queries, Project Activity SSE, RFC 9457 Problem Details | database rows, lease claim tokens, credentials, raw Provider messages |
| StoryOS Server to Core | Core contract revision | typed domain commands, exact preconditions, trusted Request Context, typed Receipts | cookies, Origin strings as authority, public error prose |
| Core to Worker | internal contract revision | immutable work intent, Attempt identity, current fence generation, payload references, budgets | reusable author authority, mutable latest contract, raw secret |
| Worker to Adapter | exact Adapter contract revision | semantic request plus wire projection and destination admission references | independent Project selection, SDK retry ownership, hidden fallback |
| Tool/MCP/Provider result to Host | exact Registration and Adapter revision | untrusted bounded result/evidence mapped to a typed Attempt | Author Intent, Acceptance, automatic instruction authority |
| MCP App View to Host bridge | MCP Apps profile plus StoryOS bridge revision | JSON-RPC transport inside an Instance-bound StoryOS bridge envelope | ambient Project context, credential, originating Run authority, direct Tool client |
| Project archive import/export | archive profile revision | self-describing manifest plus allowlisted entries and digests | SQL dump, executable entry, Credential value, new ownership claim |

Public API contracts are stable product surfaces. Core contracts may be shared
between Server and Core without becoming public DTOs. Worker and Adapter
contracts are not exposed to the browser. External protocol messages are
captured behind a Host mapping and never become canonical StoryOS domain types.

## 4. Common wire primitives

### 4.1 Serialization

- Public JSON uses UTF-8, media type `application/json`, duplicate-free member
  names, valid Unicode scalar values, and I-JSON-compatible values.
- RFC 9457 errors use `application/problem+json`.
- SSE uses UTF-8 `text/event-stream`.
- Project archives use
  `application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"`.
- Every schema-bearing object names an exact schema URI or closed schema
  identifier. An unversioned `json` or `metadata` catch-all is forbidden.
- Domain `u64` values, including Project Activity, Run, Mailbox, and Commit
  sequences, are unsigned canonical decimal strings on the JSON wire.
- Audit time is UTC RFC 3339 with at least millisecond precision. It is never a
  concurrency precondition or causal order.

### 4.2 Identity

Every durable identity is an opaque strongly typed UUIDv7 string. A schema
uses a distinct named type even where the JSON representation is identical.
UUID timestamp bits, lexical order, content digests, URLs, Provider IDs, MCP
JSON-RPC IDs, and cursors never establish authority, freshness, or causality.

The minimum cross-surface identities are:

```text
UserId                         ProjectId
CommandId                      QueryId
SnapshotId                     CursorId
ActivityEventId                ArtifactId
ArtifactRevisionId             PayloadId
ApplicationWireRecordId        ProposalId
AcceptanceReceiptId            UndoAcceptanceReceiptId
ToolCallId                     DestinationAttemptAdmissionDecisionId
AgentRunId                     RunStepId
ExecutionAttemptId             DestinationAttemptId
DomainReceiptId                CorrelationId
AuthorIntentId                 ApprovalId
CapabilityGrantId              AppViewInstanceId
AppActionRequestId             CredentialReferenceId
RegistrationId                 RegistrationRevisionId
ExternalCompatibilityDecisionId AppViewInstanceNegotiationId
IdempotencyKey
```

### 4.3 Project Scope and requester

The trusted scope is always:

```text
ProjectScope {
  owner_user_id: UserId,
  project_id: ProjectId
}
```

The public route contains `project_id` as a target selector. The Server derives
the requester User from the current Client Session Binding, resolves the
selected Project, constructs the full Project Scope, and independently checks
that every referenced object belongs to it. A public command body does not
accept `owner_user_id`, role, Capability, or Project Scope as authority.

Successful project-bearing responses, Receipts, Events, Snapshots, payload
descriptors, and inspectable security evidence include the complete Project
Scope. Internal commands carry the complete Scope explicitly. A mismatch or
missing member fails before object lookup can reveal existence.

### 4.4 Cause, correlation, and audit fields

Every durable command, Receipt, Attempt, Event, Artifact Revision, and
Application Wire Record binds:

```text
AuditContext {
  requester_user_id
  project_scope
  actor_kind
  actor_id
  correlation_id
  causation: { kind, id } | null
  command_id | null
  agent_run_id | null
  run_step_id | null
  created_at
}
```

`actor_kind` is a closed control enum. External names, Provider identities,
Tool identities, and extension kinds use validated namespaced identifiers and
cannot extend control meaning.

Every public response also carries or references one `CorrelationId`. A
Command supplies it in typed metadata; the Server assigns a Query correlation
when one is absent and echoes it in the response. Correlation, tracing, and
causation fields never grant authority or determine idempotency.

### 4.5 Digests

Digest values use:

```text
DigestValue {
  algorithm: "sha256",
  profile: string,
  value_hex_lowercase: string
}
```

Semantic JSON digests use UTF-8 RFC 8785 JCS under one exact Digest Profile.
The profile fixes all covered fields, schema versions, wide-integer encoding,
and Unicode handling. Authoritative prose is never implicitly normalized.

An Application Wire Record separately retains exact accepted message-content
bytes. A JCS digest cannot be presented as proof of original property order,
whitespace, escape spelling, or transport bytes. HTTP headers, cookies,
authorization, anti-forgery material, Credential values, credential-value
digests, TLS, compression, and chunk framing are never Application Wire
Records.

## 5. Client Session and request-admission contract

### 5.1 Client Session Binding

The Server owns an opaque Client Session Binding created either by trusted
local bootstrap or a future identity flow. It binds one server-derived User,
allowed first-party Origin, allowed Host, session generation, issued and expiry
times, and a browser-held opaque session handle.

The browser handle is carried in an `HttpOnly`, `SameSite=Strict` cookie;
controlled-cloud transport additionally requires TLS and `Secure`. The local
profile binds only the configured loopback listener and exact first-party
Origin. No bearer token appears in a URL, SSE query, log, Artifact, or export.

Every request:

1. validates Host before routing;
2. validates the exact allowed Origin when a browser Origin is expected;
3. resolves the Client Session Binding and its current generation;
4. derives the requester User;
5. resolves and validates the Project Scope;
6. applies method, content-type, size, and schema gates;
7. performs operation-specific authorization.

CORS is deny-by-default and permits credentials only for the exact first-party
Origin. CORS is not anti-forgery or authorization.

### 5.2 State-changing requests

CSRF resistance is the compound admission of exact Host, exact first-party
Origin, current Client Session Binding generation, non-simple content type,
and the command-bound anti-forgery challenge below. No one input substitutes
for the others, and CORS remains only a browser response-sharing policy.

The first-party client obtains an anti-forgery challenge through:

```text
POST /api/v1/projects/{project_id}/anti-forgery-challenges

AntiForgeryChallengeRequest {
  method
  route_template
  command_schema
  canonical_command_digest
  idempotency_key
}

AntiForgeryChallenge {
  nonce
  expires_at
  limit_profile_revision
}
```

This preflight validates exact Host, first-party Origin, current Client Session
generation, resolved Project Scope, non-simple content type, closed schema,
digest syntax, and admission rate. It resolves one pre-domain idempotency record
under the exact arbitration tuple from section 7.3 and binds the challenge to
that record identity, its `idempotency_key`, and its canonical command digest.
It creates no Command, Receipt, domain Attempt, or authority and exposes no
target-object existence. The returned nonce is sent only in the command header,
never a URL or log. The client performs this automatically; it is not an author
setting or confirmation step.

Every state-changing request:

- uses a non-safe HTTP method and a non-simple JSON content type;
- carries `Idempotency-Key` and `X-StoryOS-Anti-Forgery` headers;
- has a closed command schema and canonical command digest;
- consumes one unguessable anti-forgery nonce bound server-side to the current
  Client Session Binding generation, Project Scope, method, route template,
  command kind, `idempotency_key`, canonical command digest, and exact
  pre-domain idempotency record;
- validates any required Author Intent, Approval, Acceptance, Capability, and
  expected Revision independently.

Nonce consumption is recorded atomically against that same idempotency record.
The anti-forgery nonce grants no reusable authority: the same digest under a
new `idempotency_key` is a different record and cannot reuse the nonce. An exact
transport retry may present an already consumed nonce only to resolve the same
record, key, and digest to its in-progress or immutable prior result; it cannot
start another admission. Another method, target, body, scope, command kind, key,
or record conflicts. Author Intent remains Host-attested evidence for the exact
author gesture and is never created by accepting a cookie or nonce.

### 5.3 Reads and SSE

Sensitive reads and SSE require the same current Client Session Binding,
Host/Origin checks, Project Scope resolution, and authorization. Each SSE
connect and reconnect is reauthorized. A cursor is never a bearer capability.

## 6. Public version model

The initial profile is:

```text
api_major: 1
envelope_version: 1
problem_profile: storyos.problem.v1
activity_profile: storyos.project-activity.v1
limit_profile: storyos.foundation.absolute.v1
compatibility_profile: storyos.public.n-n-minus-1.v1
```

Public HTTP routes are rooted at `/api/v1`. API-major changes are breaking and
use another route root. Envelope versions are integers encoded as JSON numbers
because they remain within the `u32` range. Payload and Event schemas have
independent identifiers such as `storyos.command.accept-proposal.v1` or
`storyos.event.proposal-conflict-detected.v1`.

The Server declares its current public release `N`, supported previous release
`N-1`, supported API majors, public envelope profiles, payload schemas,
activity profiles, and supported Protocol Limit Profile revisions through:

```text
GET /api/v1/protocol
```

This query contains no project data and grants no session, Project, Tool,
Provider, or Capability access.

## 7. Command protocol

### 7.1 Resource-specific Commands

Public Commands use resource-specific routes and schemas. StoryOS does not
accept a generic `{ command_type, payload }` mutation endpoint. Representative
families are:

```text
POST /api/v1/projects/{project_id}/manuscript/author-edits
POST /api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances
POST /api/v1/projects/{project_id}/proposals/{proposal_id}/rejections
POST /api/v1/projects/{project_id}/agent-runs
POST /api/v1/projects/{project_id}/agent-runs/{run_id}/pause
POST /api/v1/projects/{project_id}/agent-runs/{run_id}/resume
POST /api/v1/projects/{project_id}/agent-runs/{run_id}/cancel
POST /api/v1/projects/{project_id}/waits/{wait_id}/resolutions
POST /api/v1/projects/{project_id}/approvals/{approval_id}/decisions
POST /api/v1/projects/{project_id}/context-controls
POST /api/v1/projects/{project_id}/imports
POST /api/v1/projects/{project_id}/exports
```

Core, Tool, Model, Subrun, Memory, Skill, App, and disclosure domains retain
their own exact command families. A new family is a contract addition and must
name its authority and transaction owner.

### 7.2 Shared command metadata

Every public command body composes, rather than wraps, this metadata:

```text
CommandMeta {
  command_schema: string
  expected: command-specific closed preconditions
  author_intent_id: AuthorIntentId | null
  correlation_id: CorrelationId
}
```

The route and body determine the command kind. The Server assigns `CommandId`
on the first committed admission. The `Idempotency-Key` header is a UUIDv7
newtype generated before first submission. The canonical command digest covers:

- API major, route template, method, command schema, and command kind;
- resolved Project Scope;
- all target identities and expected Revisions/Heads;
- the exact typed body after closed-schema validation;
- Author Intent, Approval, Acceptance Receipt, or Undo Acceptance Receipt
  references where applicable.

It excludes session cookies, anti-forgery material, trace headers, audit time,
and transport framing.

### 7.3 Idempotency

The idempotency arbitration key is:

```text
(owner_user_id, project_id, command_kind, idempotency_key)
```

Its record stores the command digest/profile, Command ID, admission state,
immutable acknowledgement, Receipt/result reference, and commit time.

- Same scope, kind, key, and digest returns the original acknowledgement and
  references byte-for-byte under the same negotiated representation.
- Same key with another scope or kind never locates or conflicts with the other
  record. Normal admission decides the requested target without revealing the
  other namespace; an authorized command in another Scope may therefore own an
  independent record under the same opaque key value.
- Same scope and kind with another digest returns `idempotency_conflict` and
  changes nothing.
- A concurrent duplicate may wait only within the synchronous response budget;
  if the first transaction has not committed, it returns
  `command_in_progress` without starting a second attempt.
- A crash before the admission transaction commits leaves no effect or
  acknowledgement. A crash after commit resolves from the immutable record.
- A physical external resend always creates another Attempt and never reuses
  an earlier Destination Attempt merely because the logical command is the
  same.

Idempotency evidence cannot be reusable while its command, Receipt, effect,
Attempt uncertainty, or replay reference remains meaningful. Exact expiry and
tombstone duration belong to the retention contract; expiry never permits a
known old key to execute again without a new command identity.

### 7.4 Preconditions and concurrency

Domain expected Heads and Revisions are explicit closed fields in the command
schema. They are authoritative concurrency inputs. `If-Match` may additionally
guard a single HTTP representation only when its strong ETag denotes the same
state; it never replaces a multi-object Proposal, Run, or Acceptance
precondition.

- A missing mandatory expected field returns `428 precondition_required`.
- A failed HTTP representation condition returns `412 precondition_failed`.
- A stale or inconsistent domain Head, Revision, Run sequence, Proposal state,
  or author undo frontier encountered by an admitted domain command settles a
  no-effect Receipt as described below. `409 domain_conflict` is reserved for
  non-command or pre-attempt conflicts where no domain Receipt can exist.
- StoryOS never performs last-write-wins, implicit rebase, inferred non-overlap,
  or blind retry.

### 7.5 Command Acknowledgement

Every successful submission produces exactly one of:

```text
CommandAcknowledgement =
  Committed {
    acknowledgement_kind: "committed"
    envelope_version
    command_id
    project_scope
    correlation_id
    receipt_ref
    project_activity_position
    committed_at
    limit_profile_revision
  }
  | Accepted {
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
```

`Committed` is returned only after the complete Core Transition, including a
no-change transition, its Receipt, required Events and sequence facts,
invalidation, and outbox intent commit. It means that settlement evidence
committed, not that the requested domain effect was accepted or changed state.
The `receipt_ref` is mandatory and names the stable typed result: for an
Acceptance command it is an `AcceptanceReceiptId`, for undo an
`UndoAcceptanceReceiptId`, and otherwise the owning typed `DomainReceiptId`.
The Acceptance command is the Acceptance Attempt and its `CommandId` identifies
that submitted attempt; there is no separate `AcceptanceId`.

An applied action on an existing resource, a domain refusal, invalidity,
conflict, or no-effect settlement returns HTTP `200` with `Committed`; creation
of a durable resource may return `201`. The Receipt carries the closed domain
disposition and only scope-safe conflict or refusal evidence. Exact idempotent
replay returns the same acknowledgement and Receipt reference. A committed
domain outcome is never replaced by Problem Details, because that would hide
its stable Receipt path; Problem Details represents a pre-attempt admission,
transport, query, or infrastructure failure for which no domain Receipt was
committed.

`Accepted` uses HTTP `202`. It means only that asynchronous work and its
recovery identity are durable. Completion may later succeed, partially
succeed, fail, cancel, or remain OutcomeUnknown. Settlement is observed through
the named Canonical Query and Project Activity Stream, never by holding the
HTTP response open or treating process state as truth.

An operation that can settle entirely in its owning short transaction uses
`Committed`; it is not arbitrarily downgraded to `Accepted` after a timeout.
Long work is modeled as an asynchronous operation before admission.

### 7.6 Domain attempt boundary

Malformed JSON, duplicate keys, unsupported schemas, invalid session,
anti-forgery failure, cross-scope access, and authorization failure occur before
a domain attempt and create no Domain Receipt. Safe Problem Details and
security diagnostics may be retained without the rejected body.

Once a schema-valid authorized command enters its first domain attempt, every
success, refusal, invalidity, conflict, and no-effect settlement creates one
immutable typed Receipt. Infrastructure uncertainty is never fabricated as a
domain result.

## 8. Query protocol

### 8.1 Resource-specific Queries

Queries use resource-specific GET or bounded POST-search routes. A POST Query
is safe by contract, creates no domain effect, and never shares an idempotency
namespace with Commands. Representative families are:

```text
GET  /api/v1/projects/{project_id}/manuscript/...
GET  /api/v1/projects/{project_id}/proposals/{proposal_id}
GET  /api/v1/projects/{project_id}/artifacts/{artifact_id}
GET  /api/v1/projects/{project_id}/agent-runs/{run_id}
GET  /api/v1/projects/{project_id}/commands/{command_id}
GET  /api/v1/projects/{project_id}/receipts/{receipt_id}
GET  /api/v1/projects/{project_id}/snapshots/{snapshot_id}
POST /api/v1/projects/{project_id}/queries/retrieval
POST /api/v1/projects/{project_id}/queries/history
```

The Server assigns one `QueryId` when the first qualified evaluation begins.
Every page cursor binds and reuses that Query identity, normalized request, and
correlation identity. Retrying without the cursor creates a new Query identity
even when it reaches the same Snapshot.

### 8.2 Canonical Query

A Canonical Query reads exact canonical facts at one committed boundary. Its
response includes:

```text
CanonicalQueryPage<T> {
  envelope_version
  query_schema
  query_id
  correlation_id
  project_scope
  snapshot
  required_activity_position | null
  items: [T]
  next_cursor | null
  page_count
  page_bytes
  redaction_profile
  limit_profile_revision
}
```

`required_activity_position` implements read-your-acknowledgement. The Server
must return a Snapshot containing at least that Project Activity position,
wait within the bounded Query budget, or return `snapshot_not_ready`. It cannot
answer from an older cache.

### 8.3 Projection Query

A Projection Query identifies its rebuildable projection and lag:

```text
ProjectionQueryPage<T> {
  envelope_version
  query_schema
  query_id
  correlation_id
  project_scope
  source_snapshot
  projection_kind
  projection_generation
  projection_watermark
  required_watermark | null
  completeness
  lag
  items: [T]
  next_cursor | null
  page_count
  page_bytes
  redaction_profile
  limit_profile_revision
}
```

An unmet required watermark returns `projection_not_ready`; it is not an empty
result. Index score, embedding identity, cache identity, and projection
generation never replace domain identity or current qualification.

### 8.4 Snapshot and pagination

```text
SnapshotDescriptor {
  snapshot_id
  project_scope
  snapshot_kind
  project_activity_position
  source_watermarks
  projection_generations
  redaction_profile
  schema_profile
  replay_generation
  created_at
  expires_at | null
}
```

Each `snapshot_kind` selects closed typed structures for `source_watermarks`
and `projection_generations`; they are not arbitrary JSON extension maps.

A Snapshot token is opaque, integrity protected, and bound to the requester,
Project Scope, query kind, normalized filters, stable order, schema profile,
redaction profile, projection generation, and Protocol Limit Profile. It is
not a bearer capability and is reauthorized on every page.

All pages use the same Snapshot and deterministic typed order with a stable
identity tie-breaker. A page cursor contains or references the last ordered
key plus the exact Snapshot binding. Another Scope, filter, order, schema,
redaction profile, or generation returns `cursor_scope_mismatch` through a
non-oracular error. An unavailable Snapshot returns `snapshot_expired` with a
safe resync link; the Server never mixes pages from different boundaries.

The retention ticket decides Snapshot materialization and lifetime. This
protocol fixes its identity and failure semantics.

### 8.5 Query redaction and non-oracle behavior

Every Query names or derives a redaction profile. Secret material, Credential
values or digests, raw authorization evidence, hidden chain-of-thought,
database diagnostics, and another Scope's identifiers are never returned.

Unknown object, wrong Project, and object hidden from the requester use the
same `resource_not_found` problem type, status, safe detail, and response-size
class. `403 operation_denied` is used only when the requester is allowed to
know the resource exists but lacks the named action.

Query response bytes are rebuildable projections and are not retained as
Application Wire Records. The Snapshot, query definition/digest when required
for audit, redaction profile, and projection watermarks are the durable
evidence.

## 9. Artifact and payload protocol

### 9.1 Artifact identity and revision

The public Artifact representation preserves the domain split between a stable
logical Artifact and its immutable linear Revisions:

```text
ArtifactRevisionDescriptor {
  envelope_version
  project_scope
  artifact_id
  artifact_revision_id
  parent_revision_id | null
  artifact_kind
  payload_schema
  media_type
  content_digest
  payload
  creator
  audit_context
  provenance_edges
  created_at
  limit_profile_revision
}
```

`artifact_kind`, `payload_schema`, and `media_type` are separate. The kind
selects domain meaning, the schema selects the typed payload contract, and the
media type selects byte interpretation. Unknown extension kinds stay
namespaced and non-authoritative. An `ArtifactId`, mutable `latest` alias, URL,
or digest never substitutes for an exact `ArtifactRevisionId` in a Command,
Receipt, Proposal, Acceptance, provenance edge, disclosure manifest, or Event.

Every new Revision carries `expected_revision_id`. A stale expected Revision
conflicts; revisions never branch. An alternative interpretation, merge, or
semantic type change creates a new Artifact identity with exact provenance.

### 9.2 Inline payload and Payload Reference

`payload` is exactly one of:

```text
InlinePayload {
  disposition: "inline"
  media_type
  payload_schema
  byte_length
  content_digest
  value
}

PayloadReference {
  disposition: "reference"
  payload_id
  project_scope
  media_type
  payload_schema
  byte_length
  content_digest
  fetch_path
}
```

Inline use is allowed only below the effective inline ceiling and only where
the schema permits JSON or UTF-8 text. All other content uses a Payload
Reference. `fetch_path` is a same-Server path selector, not a bearer URL; each
fetch revalidates Client Session, requester, exact Project Scope, payload
lifecycle, redaction, and byte range. Redirects to an external object store,
if a future deployment adds them, require a separate short-lived scoped access
contract owned by that deployment.

The Server verifies declared length, media type, schema, and digest while
streaming under the effective ceiling. It rejects partial, over-limit, digest-
mismatched, decompression-expanded, or schema-invalid payloads before creating
an Artifact Revision. Credential values and credential-bearing transport bytes
are never Artifact payloads.

### 9.3 Immutability and projection

Accepted payload bytes are immutable. Physical deduplication by digest may
share storage, but Project Scope, Artifact identity, provenance, lifecycle,
authorization, and audit evidence remain independent. The immutable payload is
canonical evidence; thumbnails, rendered views, indexes, embeddings, excerpts,
and App presentations are bounded rebuildable projections with their own
schema, source Revision, generation, and watermark.

Unknown schemas are preserved and exportable within their safe lifecycle, but
are never executed, used to validate Acceptance, rendered with active content,
or allowed to create authority. A compatible schema migration appends a new
Revision; it never rewrites stored historical bytes.

### 9.4 Upload admission

A large upload uses a durable upload admission bound to Client Session,
Project Scope, command kind, expected byte length, media type, payload schema,
digest, expiry, and Protocol Limit Profile. Chunks are offset-checked and
non-authoritative. Finalization is one state-changing Command with the normal
idempotency, anti-forgery, precondition, scan, digest, and Receipt boundaries.
An abandoned or invalid upload creates no Artifact Revision. Retention owns
staging cleanup time; the protocol owns the fail-closed finalization contract.

## 10. Project Activity Events and SSE

### 10.1 One canonical Project Activity Stream

Each exact Project Scope owns one canonical durable Project Activity Stream.
It is the public event chronology for inspectable committed project activity;
it is not the database source of truth and does not turn StoryOS into whole-
system Event Sourcing.

Run, Artifact, Proposal, command, and other public subscriptions are derived
views over this stream. They use the same Project Activity cursor and may
filter only after authorization. Internal Worker queues, lease traffic,
Mailbox messages, Adapter frames, Provider tokens, and progress noise are not
public Project Activity Events.

### 10.2 Durable Event envelope

```text
ProjectActivityEvent {
  envelope_version
  activity_profile
  event_id
  event_schema
  event_kind
  project_scope
  requester_user_id
  actor
  project_sequence
  stream_sequence
  agent_run_id | null
  run_step_id | null
  run_sequence | null
  aggregate_ref | null
  correlation_id
  causation
  command_id | null
  receipt_ref | null
  occurred_at
  recorded_at
  payload
  payload_digest
  application_wire_record_ref
  limit_profile_revision
}
```

`event_id` is globally opaque and stable. `project_sequence` is the monotonic
position allocated at the canonical Project commit. `stream_sequence` is the
position in the current replay generation and is contiguous only within that
generation. `run_sequence` is present only for an Event that advances the
named Agent Run and is contiguous within that Run. Consumers order by the
typed sequence for their boundary, never by UUID or timestamp.

One canonical transaction commits the domain transition or no-change Receipt,
Project Activity rows, Project/Run sequence advancement, projection
invalidation, and outbox intent. An Event is visible only after that commit.
The SSE delivery process reads committed Events and may deliver duplicates; it
never allocates a new Event or changes settlement.

### 10.3 SSE representation

The endpoint is:

```text
GET /api/v1/projects/{project_id}/activity?snapshot_id={snapshot_id}&protocol_release={release}
```

The SSE frame uses:

```text
id: <opaque scoped Project Activity cursor>
event: storyos.project-activity
data: <one compact JSON ProjectActivityEvent>
```

The `id` is an opaque, integrity-protected cursor bound to requester, exact
Project Scope, activity profile, replay generation, sequence position,
redaction profile, filter digest, and compatibility profile. The Event's
durable `event_id` remains inside `data`; SSE `id` is not the Event identity.
The client reconnects with the standard `Last-Event-ID` header. StoryOS does
not place a cursor or session token in the URL.

Filters use closed query parameters naming public view kinds and exact
in-scope identities. The Server normalizes them into the cursor binding.
Changing the filter starts from an explicitly supplied Snapshot or current
authorized position; a cursor from another filter, Run, requester, or Project
returns the same non-oracular cursor failure.

### 10.4 Snapshot, replay, and reconnect

Before an initial connection, the client loads a bounded Query and supplies its
non-secret, non-authorizing `snapshot_id`. The Server reauthorizes that Snapshot
and begins strictly after its `project_activity_position`; reconnects instead
use `Last-Event-ID`. The client therefore loads one Snapshot, replays Events
after it, and then follows live Events without a snapshot-to-live gap. A direct
current-only view requests a newly established Snapshot first; it never guesses
the current event position.

Every initial connect and reconnect reruns Client Session, Host, Origin,
Project Scope, filter, lifecycle, and operation authorization. Revocation ends
the stream and prevents replay even when the cursor is otherwise valid.
Heartbeat comments carry no domain state and do not extend authorization.

Within the advertised replay floor, the Server resumes strictly after the
cursor position. The cursor in an SSE `id` denotes the Event carried by that
frame, so a reconnect presenting it through `Last-Event-ID` uses an exclusive
resume and the Server does not intentionally include that Event again.
Delivery remains at-least-once: a frame may be observed again when the client
did not durably retain the newest cursor, reconnects from an older cursor, or a
transport duplicates bytes before the newer cursor is accepted. Clients
therefore deduplicate by durable `event_id` and validate sequence, including
overlap after an older resume point. A cursor ahead of the current
authorized stream fails closed. A valid cursor older than the replay floor
returns `409 activity_cursor_too_old` as Problem Details before an SSE stream
starts, with a safe resync query. It never silently skips missing Events.

### 10.5 Branch, compaction, and retention handoff

The canonical Project Activity chronology does not branch. Domain branches or
alternative Artifact derivations are represented as typed payload references,
not as event-log forks.

Compaction or archival creates a new `replay_generation`, publishes an
authorized Snapshot and replay floor, and preserves the last old-generation
position in the compaction evidence. Old cursors either map through an exact
verified handoff or return `activity_cursor_too_old`; they are never guessed.
The retention/archival ticket owns durations, storage tiers, compaction
mechanics, and historical payload removal. This protocol owns the generation,
Snapshot, cursor-too-old, reauthorization, and no-silent-gap semantics.

SSE reconnection time is not a retention promise. If the Server intentionally
ceases reconnect for a stream, standard HTTP `204` may be used only after
current authorization and a terminal protocol state make that behavior safe.

## 11. Problem Details and stable errors

### 11.1 Error envelope

Public errors use RFC 9457 Problem Details with a StoryOS extension profile:

```text
StoryOSProblem {
  type
  title
  status
  detail
  instance
  code
  retryability
  correlation_id
  project_scope | null
  command_id | null
  safe_conflict | null
  resync | null
  limit_profile_revision
}
```

`type` is a stable HTTPS URI documented by StoryOS. `code` is a stable closed
machine identifier within the API major. `title` and `detail` are localized or
human-safe display text and are never parsed. `instance` is an opaque safe
diagnostic reference, not a database key. `retryability` is one of
`never`, `same_request`, `after_condition`, or `outcome_unknown`; it is advice,
not authorization to bypass idempotency, expected Revision, or current grants.

### 11.2 Required error semantics

| Code | HTTP | Retryability | Meaning |
| --- | ---: | --- | --- |
| `invalid_request` | 400 | never | malformed or closed-schema-invalid request |
| `invalid_session` | 401 | after_condition | no current Client Session Binding |
| `operation_denied` | 403 | after_condition | caller may know the resource but the action is denied |
| `resource_not_found` | 404 | never | unknown, cross-Scope, or existence-hidden resource |
| `method_not_allowed` | 405 | never | route exists but method is not admitted |
| `domain_conflict` | 409 | after_condition | current in-Scope state prevents a non-command or pre-domain-attempt operation |
| `idempotency_conflict` | 409 | never | same arbitration key has another command digest |
| `command_in_progress` | 409 | same_request | identical admission has not settled its first transaction |
| `activity_cursor_too_old` | 409 | after_condition | authorized replay position precedes the current replay floor |
| `cursor_scope_mismatch` | 409 | never | cursor binding does not match the requested view |
| `snapshot_expired` | 409 | after_condition | the exact stable Snapshot is no longer available |
| `precondition_failed` | 412 | after_condition | HTTP representation precondition failed |
| `payload_too_large` | 413 | never | a byte or expansion ceiling was exceeded |
| `unsupported_media_type` | 415 | never | media type is not admitted for this schema |
| `unprocessable_content` | 422 | never | well-formed content violates typed semantic validation |
| `archive_integrity_failed` | 422 | never | archive root coverage or trusted integrity protection did not verify |
| `precondition_required` | 428 | never | a mandatory expected Revision/Head is absent |
| `rate_limited` | 429 | after_condition | a bounded admission budget is temporarily unavailable |
| `outcome_unknown` | 503 | outcome_unknown | an admitted effect cannot safely be classified or repeated |
| `snapshot_not_ready` | 503 | after_condition | required canonical position is not yet readable within budget |
| `projection_not_ready` | 503 | after_condition | required projection watermark is unavailable |
| `upgrade_required` | 409 | after_condition | no safe compatible protocol projection exists |
| `limit_exceeded` | 422 | never | a named content, complexity, or non-byte work bound was exceeded |

An implementation may add more specific codes without changing these
meanings. A conflict exposes only typed in-Scope revisions or settlement links
that the requester may already inspect. It never returns another User,
Project, hidden Artifact, Capability, Credential Reference, or registration
identity.

Problem Details never stands in for or creates a domain Receipt. A domain
command's committed refusal, conflict, invalidity, or no-effect outcome uses
the `Committed` acknowledgement and its mandatory `receipt_ref`; a Problem
means the current request has no committed domain Receipt path.

### 11.3 Non-oracular failures and OutcomeUnknown

Unknown object, wrong owner, wrong Project, cross-Scope cursor, and object
hidden by policy are normalized before public detail and audit-size selection.
They do not differ in status, code, timing class, body shape, or safe detail in
a way intended to reveal existence. Security logs may keep a sanitized reason
code and correlation reference, never the rejected secret or full body.

`outcome_unknown` is used only after durable admission when the system cannot
prove whether an external or authoritative effect occurred. It includes the
logical command/operation and exact Attempt references the caller is allowed
to inspect, consumes conservative budgets, and forbids blind resubmission.
The caller must use the settlement Query or an explicitly authorized fenced
recovery Command.

## 12. Concurrency, Attempts, and recovery

### 12.1 Attempt identity and state

A logical Command, ToolCall, model operation, research fetch, import, export,
or projection job may own multiple physical Attempts. Every physical execution
has a new typed identity and immutable binding:

```text
ExecutionAttemptRecord {
  attempt_id
  attempt_kind
  attempt_number
  project_scope
  operation_ref
  semantic_request_digest
  registration_revision | null
  adapter_revision | null
  external_compatibility_decision | null
  destination_attempt_admission_decision | null
  limit_profile_revision
  effective_bounds
  lease_generation
  fence_generation
  state
  admitted_at
  terminal_at | null
  outcome_ref | null
}
```

The closed common states are `prepared`, `awaiting_approval`, `admitted`,
`claimed`, `outcome_unknown`, `succeeded`, `failed`, `refused`, and `cancelled`.
Owning domains may refine them, but cannot make a terminal state reopen or
collapse `outcome_unknown` into failure. State transitions append immutable
evidence; they do not rewrite prior claims or disclosure records.

A retry, resend, redispatch, repair, fallback, changed destination, or changed
Registration always creates a new Attempt. An SDK retry is disabled unless the
Adapter exposes each physical send as a StoryOS Attempt. Logical idempotency
does not erase physical effects.

### 12.2 Lease and fence

Claiming retryable work occurs in a transaction that verifies the exact Project
Scope, current Attempt state, current lease generation, current fence
generation, admission record, effective budget, and pinned contract revisions.
It then assigns a bounded lease and an unforgeable claim reference. Lease time
controls liveness only; the monotonic fence generation controls authority.

Every progress write, result, external dispatch claim, Artifact creation,
Receipt settlement, outbox row, and follow-up work submission carries the
current Attempt identity and fence generation. The authoritative write checks
both under forced RLS. A late, duplicate, expired, cancelled, or superseded
worker may append bounded diagnostic evidence to a quarantine path, but cannot
settle work, publish an Artifact, advance a Run, or consume new authority.

Recovery first fences the prior claim, records its last provable boundary, and
then decides whether a successor Attempt is permitted. An Attempt at or beyond
an uncertain external dispatch boundary remains `outcome_unknown`; recovery
cannot infer failure from lease expiry, lost connection, process death, or an
absent Provider response.

### 12.3 Commit and outbox boundary

For an internal authoritative transition, one PostgreSQL transaction commits:

- the domain transition or no-change Receipt;
- immutable command and Attempt settlement evidence;
- Project Activity Events and sequence allocation;
- projection invalidation and affected generation facts;
- idempotency acknowledgement/result references; and
- outbox intents for post-commit workers or delivery.

If it rolls back, none of those facts exists. An outbox worker may redeliver,
but consumers deduplicate the stable outbox item and validate the current
fence. Deleting or acknowledging an outbox row is never the authoritative
business settlement.

For external egress, the Host persists the semantic request, destination and
disclosure manifests, Wire Payload Projection, Destination Attempt, final
Destination Attempt Admission Decision identity, its frozen effective bounds,
and fence before I/O. The short dispatch transaction revalidates and binds that
exact admitted Decision to the Attempt, wire projection, manifests, credential
binding generation, claim, and Outbound Disclosure Event. It then changes the
Attempt to conservative `outcome_unknown`; only after that commit may bytes
leave. A crash before this claim proves no dispatch authority was committed. A
crash after it preserves the exact Decision and wire evidence but remains
OutcomeUnknown until stronger destination evidence settles it. Later provider
or transport evidence may append `confirmed_submitted` or a typed terminal
result without rewriting the claim.

## 13. Security-sensitive protocol contracts

### 13.1 Requester and Project Scope envelopes

Every security-sensitive internal request composes one surface-specific
envelope containing the server-derived requester, exact Project Scope,
operation identity, Audit Context, current policy/grant revisions, and a typed
request body. There is no shared ambient `current_project`, default User,
caller-selected role, or caller-created Capability. A referenced Run, Step,
Artifact, ToolCall, Approval, Registration, Credential Reference, destination,
Snapshot, cursor, and payload is independently rejoined on both members of the
Scope before use.

Public errors and timing do not reveal which join failed. Internal diagnostics
retain a sanitized reason code and correlation identity. They do not retain
session handles, anti-forgery nonces, credential material, raw rejected bodies,
or cross-Scope identifiers.

### 13.2 Capability and Approval references

```text
OperationAuthorityEvidence {
  project_scope
  requester_user_id
  operation_ref
  capability_grant_id | null
  capability_revision | null
  approval_id | null
  approval_kind | null
  approval_request_digest | null
  author_intent_id | null
  evaluated_policy_revision
  effective_bounds
}
```

Capability is a bounded authorization input, not an identity or bearer token.
The Host computes effective authority as the non-escalating intersection of
project policy, current Run Capability Grant, caller route, requested operation,
resource Scope, destination, data categories, budget, and time. Approval is an
immutable Author decision over one exact typed operational request and digest.
A changed input, effect, destination, disclosure, Registration, Credential
Reference, or wider bound requires a new decision. Approval never substitutes
for Proposal Acceptance or changes Authoritative State.

### 13.3 Tool and MCP registration

Tool and MCP discovery is untrusted observation. Reusable contract and Adapter
mapping is a Host-owned immutable revision that contains no Project data,
Credential Reference, grant, or runtime use state:

```text
GlobalExternalContractRegistrationRevision {
  registration_id
  revision_id
  registration_kind
  source_identity
  trusted_contract_digest
  protocol_revision
  adapter_revision
  destination_identity
  effect_ceiling
  intake_contract
  lifecycle
  created_at
}

ProjectExternalUseBindingRevision {
  project_scope
  binding_revision_id
  registration_revision
  project_use_authorization_revision
  credential_binding:
    NoCredentialBinding { binding_kind: "none" }
    | ProjectCredentialBinding {
        binding_kind: "project_credential"
        project_scope
        credential_binding_revision
        credential_reference_id
        credential_binding_generation
      }
  allowed_purposes
  allowed_destination_identity
  effect_ceiling
  lifecycle
  created_at
}
```

`registration_kind` distinguishes built-in Tool, MCP Tool, MCP Resource, MCP
Prompt, MCP App UI Resource, Model Provider, embedding Provider, and research
Adapter contracts. The global Registration is reusable Host mapping evidence
only while it is free of project-derived content and authority. The separate
use binding is exact Project Scope-bound enablement plus any Credential
Reference binding; a Credential Reference never occupies a global namespace.
`project_use_authorization_revision` is a closed surface-specific reference,
such as a Project Tool Enablement or Project Destination Grant, rather than a
universal authority record. A credential-bearing variant repeats the complete
Scope and must join both members to the outer binding; paired nullable
credential fields are forbidden.
Neither discovery, enablement, model-visible exposure, a handshake, a name, a
semver range, nor Credential configuration grants invocation authority.

Before each new use the Host verifies the exact active Registration revision,
trusted contract digest, Adapter revision, external compatibility decision,
destination, effect ceiling, Credential Reference availability and binding
generation, project enablement, caller route, Capability, Approval when
required, and limits. Every use, Attempt, and External Contract Compatibility
Decision repeats and validates both members of the Project Scope and pins the
exact `ProjectExternalUseBindingRevision`; no global Registration, credential
identifier, or compatibility cache may satisfy that join. Drift quarantines
the Registration for new work and clears derived exposure. Historical work
remains inspectable against its pinned revisions.

### 13.4 Tool and MCP invocation

```text
ToolInvocationRequest {
  project_scope
  requester_user_id
  tool_call_id
  caller_route
  registration_revision
  project_external_use_binding_revision
  credential_binding:
    NoCredentialBinding { binding_kind: "none" }
    | ProjectCredentialBindingRef {
        binding_kind: "project_credential"
        project_scope
        credential_binding_revision
        credential_reference_id
        credential_binding_generation
      }
  external_compatibility_decision
  validated_arguments
  argument_digest
  resolved_target_refs
  requested_effect
  authority_evidence
  context_assembly_manifest_ref
  destination_context_manifest_ref
  destination_attempt_id
  destination_attempt_admission_decision_ref
  processing_boundary:
    ControlledProcessing {
      boundary_kind: "controlled"
    }
    | ExternalProcessing {
        boundary_kind: "external"
        outbound_disclosure_manifest_ref
        wire_payload_projection_ref
      }
  fence_generation
  limit_profile_revision
  effective_bounds
}
```

The Context Assembly Manifest, Destination Context Manifest, Destination
Attempt, and final Destination Attempt Admission Decision are mandatory for
every Tool or MCP execution, including a controlled built-in destination. The
external processing variant, its Outbound Disclosure Manifest, and its Wire
Payload Projection are additionally mandatory for an External Processing
Destination; a controlled destination
uses the closed controlled variant rather than nullable fields. A missing,
cross-Scope, stale, refused, or differently bounded reference fails closed;
`null` cannot bypass any of the seven Context Assembly gates.

Only the Tool Gateway may turn this request into execution. It revalidates the
composite Scope, project use and Credential bindings, all authority and
manifests, and the exact admitted Decision; resolves secrets internally;
invokes the pinned Adapter; validates bounded output against the exact
contract; and records the outcome. A model, generated program, MCP App,
Provider-hosted tool, raw MCP client, or Adapter cannot call through this
boundary directly.

MCP JSON-RPC request IDs are transport correlation only. They do not replace
`ToolCallId`, Attempt identity, idempotency, Project Scope, Capability, or
fence. External output stays untrusted and non-authoritative; an effectful or
bulk result changes Authoritative State only through a typed Proposal and
explicit Acceptance.

A Tool or MCP result is retained as a bounded typed result and provenance
source. If any part later enters model, Tool, App, research, or other
destination context, it becomes a new Context Candidate and crosses the full
seven gates again, producing new Projection and manifest evidence for that
new use. The earlier ToolCall, result validation, disclosure, or admitted
Decision is never reusable context eligibility or outbound authority.

### 13.5 MCP App bridge

An MCP App message is admitted only inside a Host-created bridge envelope:

```text
AppBridgeMessage {
  bridge_revision
  app_view_instance_id
  app_view_artifact_revision_id
  instance_negotiation_id
  sandbox_profile_revision
  instance_sequence
  direction
  bridge_request_id: string | null
  jsonrpc_message
  payload_digest
  limit_profile_revision
}
```

The bridge binds one exact App View Instance and Artifact Revision. Initialization
fixes the supported protocol subset and effective sandbox; it grants no Project,
Run, Tool, Credential, model, transcript, or network authority. Every incoming
message validates source window, origin expectations, Instance lifecycle,
monotonic bounded sequence, JSON-RPC shape, declared method, payload digest,
message size, rate, and current resource eligibility.

The bridge profile permits one JSON-RPC 2.0 object per message; batches are
forbidden. A request or response ID is a 1-to-128-byte ASCII string and JSON
numbers and `null` are forbidden IDs. For a request, `bridge_request_id` is
present and byte-for-byte equal to `jsonrpc_message.id`; there is no case
folding, Unicode normalization, numeric coercion, or alternate mapping. A
response uses the same string as its request. Under Digest Profile
`storyos.app-bridge-binding.jcs.v1`, the payload digest is JCS SHA-256 over the
closed object `{ bridge_request_id, jsonrpc_message }`, so the duplicated
binding cannot diverge. Allowed request, response, error, and notification
shapes and method names come from the exact closed Instance negotiation
profile.

A JSON-RPC notification has no `id` and sets `bridge_request_id` to `null`. It
is admitted only for explicitly allowlisted presentation/lifecycle methods
that cannot read data or produce semantic, context, transcript, disclosure, or
authoritative effects. A semantic notification is rejected before routing and
cannot create anonymous or unrepeatable work.

Presentation-only signals remain ephemeral and bounded. Any message with
semantic meaning, data access, disclosure, model-context impact, transcript
impact, or effect creates one durable `AppActionRequest` before routing. Its
idempotency key is `(app_view_instance_id, bridge_request_id)` bound to method
and payload digest: an exact duplicate resolves to the same Request, another
payload is a protocol violation, and another Instance creates another Request.
The Host derives a new operation scope and routes through the normal Query,
Tool, Model, or Command boundary; the bridge never forwards an effect directly.

An App response is dispatched only to the same live Instance through a durable
delivery obligation. Termination abandons an undelivered response without
cancelling or retrying the routed work or transferring the JSON-RPC promise to
a replacement Instance. Durable results remain available through normal
StoryOS Artifact, Message, Query, and Event contracts.

### 13.6 Credential Reference

```text
CredentialReferenceDescriptor {
  credential_reference_id
  project_scope
  backend_kind
  non_secret_locator
  generation
  binding_state
  allowed_registration_revisions
  created_at
  updated_at
}
```

The descriptor never contains a credential value, value digest, ciphertext,
preview, environment expansion, or portable secret. Public representations
omit `non_secret_locator` unless the deployment proves it safe. Only the exact
execution boundary resolves the value from the deployment secret backend after
fresh Scope, Registration, operation, Capability, Approval, and generation
checks. The value is injected ephemerally and cannot enter model context,
arguments, output, logs, Wire Payload Projection, Artifact, transcript,
Application Wire Record, backup, import, or export.

### 13.7 Project import and export

The Project Export Archive has a self-describing top-level manifest:

```text
ProjectArchiveManifest {
  archive_profile
  export_id
  project_scope
  source_snapshot
  source_schema_compatibility
  schema_catalog
  table_family_counts
  serialization_profile
  digest_profile
  limit_profile_revision
  entries: [{ path, media_type, payload_schema, byte_length, digest }]
  provenance_closure
  known_purged_gaps
  root_digest
  integrity_protection
  created_at
}
```

Export reads one transactionally consistent boundary and includes every
non-secret canonical record, immutable retained payload, schema/profile ID,
provenance fact, and required Application Wire Record. It excludes cookies,
nonces, Credential values, disposable projections, caches, embeddings,
Provider-held state, logs, and executable runtime state. It preserves exact
User, Project, object, Revision, and event identities and grants no destination
or ownership authority.

`digest_profile` is exactly `storyos.project-archive-root.jcs.v1`. Entries are
sorted by their normalized UTF-8 path bytes, paths are unique, and each entry
digest covers the exact uncompressed entry bytes under its declared profile.
The exact root input is:

```text
ProjectArchiveRootInput {
  archive_profile
  export_id
  project_scope
  source_snapshot
  source_schema_compatibility
  schema_catalog
  table_family_counts
  serialization_profile
  digest_profile
  limit_profile_revision
  entries
  provenance_closure
  known_purged_gaps
  created_at
}
```

JCS UTF-8 bytes of that closed object are hashed with SHA-256. Excluding
`root_digest` and `integrity_protection` prevents self-reference while the
named input covers the schema catalog, counts, profiles, every entry descriptor
and digest, provenance closure, known gaps, source Snapshot, Scope, and time.

`integrity_protection` names an exact signature or deployment integrity
profile, a trusted out-of-archive anchor ID, the canonical protected-input
profile, and its proof bytes. Profile
`storyos.project-archive-protected-input.jcs.v1` signs exactly the JCS UTF-8
bytes of `{ archive_profile, export_id, project_scope, root_digest }` after all
digests recompute.
An embedded public key or digest with no independently trusted anchor is not
integrity protection. Export completes only after the proof is durable. Import
recomputes every entry and root digest, resolves the trusted anchor, and
verifies the proof before referential validation or visibility. Any missing,
unknown, malformed, mismatched, or untrusted proof returns
`archive_integrity_failed`, leaves live state unchanged, and quarantines or
discards staging under the retention-owned cleanup policy. This contract fixes
verification, not key lifetime or archive retention duration.

Import streams into non-authoritative staging and validates media type, archive
profile, total and expanded bytes, entry count, nesting, path normalization,
duplicate/case-colliding names, symlink/hardlink/device rejection, compression
ratio, digests, duplicate JSON keys, schema support, referential closure, exact
Project Scope, and object identities before any visibility. Active content is
never rendered during inspection. Restore is allowed only for the same durable
User and Project identity into a target where that Scope is absent. It becomes
visible atomically; no partial merge, overwrite, ID remap, copy, fork, or
ownership transfer is implied. Unresolvable Credential References become
explicitly `unbound`.

### 13.8 Provider, embedding, and research disclosure

Every external operation first commits a provider-neutral semantic request and
minimum-necessary context selection, then the applicable immutable manifests:

```text
DestinationManifestBinding {
  project_scope
  requester_user_id
  purpose
  processing_destination_identity
  registration_revision
  project_external_use_binding_revision
  adapter_revision
  external_compatibility_decision
  context_assembly_manifest_ref
  destination_context_manifest_ref
  outbound_disclosure_manifest_ref
  disclosed_data_categories
  exact_source_revisions
  wire_payload_projection_ref
  authority_evidence
  hard_bounds
  destination_attempt_id
  destination_attempt_admission_decision_ref
}
```

The Host admits the destination, source Revisions, lifecycle, Memory
Suppression, Context Exclude, Purpose, project grant, exact disclosure Approval
when required, Credential Reference, policy, and budgets immediately before
dispatch. The final Admission Decision freezes the actual effective bounds and
is referenced by the manifest binding, Destination Attempt, dispatch claim,
Outbound Disclosure Event, and crash-cut evidence. The manifests and Decision
commit before egress and record only minimum-necessary logical and non-secret
wire evidence. Redirects, DNS resolution changes,
private/link-local targets, proxy behavior, authentication challenges,
fallbacks, cache references, and Provider aliases cannot silently change the
Processing Destination Identity or expand disclosure.

Research fetches use a dedicated egress Adapter with allowed schemes, resolved-
address checks at connection time, bounded redirects, content-type and byte
limits, and no ambient cookies, cloud metadata credentials, proxy credentials,
or internal network access. Downloaded content is untrusted source evidence,
not instruction or authority. Embedding operations cross the same disclosure
and Attempt boundary as model calls; projection rebuild grants no exception.

## 14. Versioning and compatibility

### 14.1 Independent version axes

StoryOS versions these axes independently:

| Axis | Identifies | Breaking change response |
| --- | --- | --- |
| API major | HTTP route and method semantics | new `/api/v{major}` root |
| public protocol release | one generated public contract set | publish `N+1`, retain `N` while it is N-1 |
| envelope version | shared structural fields for one surface | new envelope schema and projection |
| payload schema | typed Command, Query item, Artifact payload, error extension, or bridge body | new schema identifier and migration/projection rule |
| Event schema | durable Event meaning and payload | new schema identifier; never rewrite history |
| activity profile | SSE cursor and replay rules | new profile and explicit cursor handoff |
| Core contract revision | Server-to-Core typed semantics | coordinated internal migration |
| Worker/Adapter revision | execution handoff and wire mapping | fence old work and pin or migrate explicitly |
| archive profile | export manifest and portable representation | reader compatibility or explicit import refusal |
| Protocol Limit Profile | public validity ceilings, counting rules, and interpretation | every numeric or semantic change creates a new profile revision; no silent narrowing or widening |
| external Registration revision | pinned third-party observation and Host mapping | new External Contract Compatibility Decision |

An API major is not reused as a payload, Event, Core, Adapter, or external
version. A single release catalog records the exact compatible combination.

### 14.2 Public N/N-1 contract

A Server release `N` supports exactly its current generated public release and
the immediately preceding supported release `N-1` for the same API major. The
client sends `StoryOS-Protocol-Release` on ordinary public requests; the Server
echoes the selected value. Because the browser `EventSource` API cannot set an
arbitrary request header, the SSE endpoint uses the closed, non-secret
`protocol_release` query parameter instead. No session handle, cursor, or
Capability enters that URL. Omission selects the current release only for the
first-party client shipped with that Server and is never used by an unknown
external client. `/api/v1/protocol` advertises the exact support window.

For both N and N-1:

- Commands, command preconditions, control messages, import manifests, and App
  bridge inputs are closed. Unknown members, duplicate names, unknown
  discriminators, and unknown control enums are rejected.
- Query, Artifact, Problem Details extension, and Event outputs may add fields
  whose omission preserves meaning. Clients ignore unknown presentation-safe
  fields while retaining the enclosing typed identity.
- Unknown values that may change authority, lifecycle, settlement, Approval,
  Acceptance, replay, security, or side-effect meaning are not generic
  fallbacks. The Server produces a known compatible projection or returns
  `upgrade_required` before the client consumes or advances past them.
- Removing a field, changing its type or meaning, narrowing a previously valid
  input, adding a required input, changing digest coverage, changing default
  authority, reusing an enum value, or changing cursor interpretation is
  breaking.

The Server generates public Events for every supported release. Before an SSE
stream starts or resumes it verifies that the requested replay interval and
live projection can be represented in the selected release. If not, it returns
`upgrade_required` before opening the stream. A later projection failure closes
at the last safely delivered cursor and records a compatibility diagnostic; the
client must not advance past an unrecognized control event.

Supporting N/N-1 does not require one binary to understand arbitrary older
clients, old API majors, or every historical payload as a current mutable
input. Removing N-1 support is a declared release event and cannot occur in a
patch to N.

The release catalog maps each supported public release to an exact Protocol
Limit Profile revision. A client selecting release N-1 thereby selects its
advertised profile; while N-1 is supported, the Server must continue to admit
an N-1 input that was valid under that profile unless an independent current
authorization or temporary admission condition refuses it. A narrower profile
requires a newly negotiated release/profile and cannot be imposed under the
same revision. Every response echoes the selected profile revision.

### 14.3 Additive and breaking examples

Additive within a public release family:

- an optional output field with presentation-only meaning;
- a new resource-specific Query route;
- a new Artifact kind that an old client can preserve and show through the
  bounded generic read-only renderer;
- a new Event with an N-1 projection to already understood lifecycle meaning;
- a temporary `429 rate_limited` response that changes no content-validity
  rule or supported Profile revision.

Breaking or upgrade-gated:

- accepting an unknown Command field and guessing its intent;
- treating an unknown Approval or Attempt state as success;
- changing an Artifact digest profile without a new schema/profile;
- reinterpreting an SSE cursor across compaction without a verified handoff;
- widening a Capability, destination, Credential binding, Tool effect, or
  disclosure because an external server changed version;
- making a previously inline payload become silently truncated.

### 14.4 Historical records after upgrade

The Application Wire Record retains the exact accepted message-content bytes
and their API major, public release, envelope, payload/Event schema, digest
profile, redaction class, and Protocol Limit Profile. It is append-only. An
upgrade may add a separately identified parsed or compatibility projection,
but cannot normalize, reserialize, migrate in place, or delete the original
wire bytes while they remain retained evidence.

Historical Command input bytes are never replayed as fresh authority. A repair
or re-execution constructs a new current typed Command with a new idempotency
key, current admission, exact provenance to the historical record, and any
required new Author evidence.

Historical Event bytes remain inspectable even when their schema is no longer
current. Replay to a supported client uses a generated compatibility projection
whose provenance pins the original Event and mapping revision. If no safe
projection exists, replay stops with `upgrade_required`; it never drops the
Event or invents a default enum.

### 14.5 External Provider, Tool, MCP, and MCP App compatibility

External contracts receive no StoryOS N/N-1 promise. Each use pins one exact
Registration and Adapter revision plus an immutable External Contract
Compatibility Decision. The decision records the observed protocol/capability
exchange, trusted schema or ToolSpec digest, wire mapping, actual Processing
Destination Identity, effect ceiling, disclosure categories, Credential
binding generation, admitted StoryOS contract releases, decision result, and
reason.

Semver, Provider aliases, MCP protocol negotiation, JSON-RPC method discovery,
Tool names, resource URIs, SDK types, and successful handshakes are evidence,
not compatibility or authority. Contract drift quarantines new work. A Host-
reviewed Adapter update may declare compatibility only after mapping every
input, output, error, effect, cancellation, retry, and streaming semantic.
Changing actual processor, endpoint/account boundary, effect, destination,
data category, Credential binding, or capability ceiling requires new
authorization in addition to a new compatibility decision.

Every compatibility decision used for project work is bound to the exact
Project Scope and `ProjectExternalUseBindingRevision`. A global contract or
Adapter compatibility observation may be reused only as non-authorizing input;
it cannot carry a Credential Reference, satisfy project enablement, or admit an
Attempt without the scope-bound decision.

This rule resolves the parent-map fog item about Provider/MCP version
compatibility. No separate follow-up ticket is needed.

## 15. Protocol Limit Profile

### 15.1 Rule

Every admission resolves one immutable Protocol Limit Profile revision. The
Foundation absolute profile below is a safety ceiling, not a target default.
Every numerical or semantic change to a protocol content, shape, byte,
complexity, token-counting, replay-batch, time, attempt, expansion, rate, or
concurrency limit creates a new Profile revision. A supported revision is
immutable; the Server cannot use deployment or current resource pressure to
make an input that is valid under that revision permanently invalid.

An admission computes `effective_bounds` as the non-widening intersection of
the selected Profile and exact revisioned operation policy, Project/Run grants,
Capability, Approval, destination intake contract, Registration, and
provider/model mapping. Those narrower authorization and destination bounds
are not a silent rewrite of public protocol validity. The admission freezes
their actual values and all source revision references in its Receipt,
Attempt, Snapshot, Application Wire Record, limit Problem, or external
manifest. A later policy or capacity change cannot reinterpret that admission.

Clients may use release-negotiated limits to batch or choose referenced
payloads, but cannot request a wider limit. The default author experience does
not expose routine limit controls.

### 15.2 Foundation absolute profile

`storyos.foundation.absolute.v1` fixes these inclusive maxima:

| Boundary | Absolute ceiling |
| --- | ---: |
| HTTP request target | 8 KiB |
| complete HTTP request header section | 32 KiB |
| one HTTP header field | 8 KiB |
| ordinary public JSON Command or POST Query body | 1 MiB |
| JSON nesting depth | 64 |
| members in one JSON object | 4,096 |
| items in one JSON array | 10,000 |
| one JSON string after UTF-8 encoding | 1 MiB |
| inline Artifact/Event payload | 64 KiB |
| one immutable referenced payload | 64 MiB |
| one upload chunk | 8 MiB |
| Query page items | 500 |
| Query page encoded bytes | 4 MiB |
| normalized Query filter clauses | 128 |
| Query sort keys | 8 |
| one encoded Project Activity Event | 128 KiB |
| one SSE reconnect replay response | 1,000 Events or 8 MiB, whichever comes first |
| queued unsent SSE bytes per connection | 4 MiB |
| concurrent SSE connections per Client Session | 16 |
| concurrent SSE connections for one Project Scope | 32 |
| one MCP App bridge message | 256 KiB |
| outstanding App Action Requests per App View Instance | 64 |
| incoming App bridge messages per Instance | 120 in any rolling 10 seconds |
| concurrent App View Instances per Client Session | 32 |
| one non-secret external semantic request or validated Tool input | 8 MiB |
| one external result before referenced-payload handling | 64 MiB |
| one model-visible Context item | 10,000 tokens under the admitted Token Counting Profile |
| items selected into one Effective Destination Context | 4,096 |
| total model input for one Destination Attempt | 1,000,000 tokens under the admitted Token Counting Profile |
| requested model output for one Destination Attempt | 128,000 tokens under the admitted Token Counting Profile |
| physical Attempts for one logical operation | 32 |
| HTTP redirects for one admitted research fetch | 10 |
| one external network Attempt wall time | 10 minutes |
| one asynchronous operation wall-clock budget before explicit continuation | 24 hours |
| compressed Project archive | 2 GiB |
| expanded Project archive | 16 GiB |
| archive entries | 100,000 |
| archive expansion ratio | 100:1 |
| archive path after normalization | 512 UTF-8 bytes |
| archive directory depth | 16 |
| complete archive validation wall time | 15 minutes |

Provider or Adapter limits, current model context, author-granted budget, and
deployment capacity usually make effective values materially smaller. The
10,000-token per-item ceiling is also a repository invariant; introducing a
new context item kind that can exceed 1,000 tokens requires explicit design
review even though the absolute ceiling is higher.

Token ceilings are not evaluated by an unspecified tokenizer or character
estimate. Every token-limited admission pins:

```text
TokenCountingProfile {
  profile_revision
  tokenizer_family
  tokenizer_revision_or_digest
  input_normalization
  special_token_rules
  message_and_tool_framing_rules
  count_algorithm_revision
}

ProviderTokenMappingEvidence {
  processing_destination_identity
  registration_revision
  adapter_revision
  provider_model_identity
  token_counting_profile_revision
  evidence_revision
  verified_at
}
```

The external compatibility decision and Destination Attempt bind that mapping
evidence. Counts cover the exact admitted semantic projection plus all wire
framing and fixed overhead defined by the profile. A missing, drifted, or
unverifiable Provider mapping fails admission; the Host does not guess from
characters, another model, a Provider alias, or a mutable SDK helper.

No partial success is inferred when a hard ceiling is hit. An over-limit input
is rejected before domain admission. A page or replay batch ends only at a
typed item boundary and supplies a continuation cursor. A streamed external
result that crosses its admitted bound settles the Attempt with a typed limit
outcome and cannot be silently truncated into a valid result. An archive that
crosses any bound remains non-authoritative staging and is discarded under the
retention-owned cleanup rule.

### 15.3 Effective values and exhaustion

`GET /api/v1/protocol` publishes every supported immutable public Profile and
its numeric values. Project- or operation-specific admissions return only
values the requester is allowed to know. Rate/concurrency exhaustion returns
`rate_limited` with a bounded
condition or safe `Retry-After`; content/complexity exhaustion returns
`payload_too_large` or `limit_exceeded`. Retry never bypasses the original
idempotency, Snapshot, Capability, Attempt, or fence binding.

Dynamic resource pressure may only refuse or defer a new admission through a
temporary rate/concurrency outcome such as `429 rate_limited` with
`after_condition`. It cannot, under the same Profile revision, convert an
otherwise valid content shape, byte length, complexity, replay batch, or token
count into `payload_too_large`, `limit_exceeded`, or another permanent-invalid
classification. Once admitted, the frozen effective bounds govern settlement
even if current capacity changes.

Timeout stops waiting, not truth. After durable admission, timeout reports the
known asynchronous operation or `outcome_unknown` as appropriate. It does not
convert an uncertain external effect to failure or authorize another Attempt.

Effective defaults, replay retention windows, UI behavior near limits,
backpressure tuning, model context allocation, Subrun/Mailbox quotas, and
first-slice operating values remain empirical items in
[EXPERIMENTAL-TUNING-REGISTER.md](../../EXPERIMENTAL-TUNING-REGISTER.md). They
must fit inside this profile and cannot weaken its semantics.

## 16. Rust source, generation, and verification contract

### 16.1 One source of truth

The StoryOS Rust contracts crate is the sole editable source for external
contract shapes. It owns the typed Rust public API DTOs, schema identifiers,
field requirements, discriminators, control enums, validation annotations,
digest coverage descriptors, compatibility projections, and Protocol Limit
Profiles. Generated artifacts are reviewed and checked in, but never edited as
independent truth.

The generator emits, from the same contract graph:

- OpenAPI 3.1 for public HTTP routes, headers, media types, status codes,
  Problem Details, and examples;
- JSON Schema 2020-12 for every public payload, Event, Artifact, archive,
  bridge, and exposed compatibility profile;
- TypeScript client operations and types for every public release N and N-1;
- a schema catalog mapping every schema ID to its Rust owner and generated
  artifact;
- canonical positive and negative fixtures; and
- the golden Application Wire Record and SSE frame corpus.

Core, Worker, Adapter, Provider, Tool, MCP, and App bridge Rust contracts are
typed in the same source-of-truth crate where external or persisted, but are
generated into separate catalogs and packages. Internal contracts are not
accidentally published in OpenAPI or the browser TypeScript client.

Untyped JSON maps are allowed only for schema-declared opaque, untrusted data
whose byte, depth, and interpretation limits are explicit. They cannot carry
identity, authority, lifecycle, settlement, preconditions, error control,
Capability, Approval, Acceptance, Attempt, or fence meaning.

### 16.2 Application Wire Record corpus

The selective Application Wire Record contains exactly:

- the exact message-content bytes of every schema-valid, authorized Command or
  durable asynchronous-admission body accepted by the Server, stored once with
  its route, method, selected release, schema, content type, digest profile,
  idempotency record, and resulting Command identity; and
- the exact compact JSON bytes of each public Event representation when first
  materialized for a public release, stored once with the durable Event,
  activity profile, schema, redaction profile, and representation digest.

It does not contain full HTTP requests/responses, TLS or compression framing,
headers, cookies, session handles, anti-forgery nonces, Credential values or
digests, malformed or unauthorized bodies, database rows, query response
archives, SSE heartbeat/retry frames, or repeated Event deliveries. Security
diagnostics may retain bounded sanitized reason metadata without the rejected
body. Retention owns how long eligible records remain; while retained they are
immutable and exportable as historical evidence.

The golden corpus is distinct from production records. It contains synthetic,
non-secret fixtures for current N and N-1 and fixes exact bytes, including UTF-8,
number/string form, field order chosen by the serializer, escaping, media type,
digest, SSE line framing, and expected parse result. JCS semantic-digest
fixtures are paired with differently serialized but semantically equal JSON to
prove that the digest does not claim original bytes.

### 16.3 Required drift gates

The eventual repository verification command must fail when:

- regenerating OpenAPI, JSON Schema, TypeScript, schema catalog, examples, or
  golden corpus changes the worktree;
- a public route, response, status, header, schema ID, discriminator, enum,
  limit, or error code lacks a Rust owner or generated representation;
- OpenAPI and JSON Schema disagree on required, nullable, closed, or bounded
  fields;
- an input schema accidentally permits unknown members or duplicate-name
  parsing differs between public entry points;
- an output change is classified additive but changes control meaning;
- the N/N-1 corpus cannot be parsed and projected by its advertised client;
- a stored historical wire fixture is reserialized after upgrade;
- a digest coverage fixture changes without a new Digest Profile;
- any Profile numeric value changes without a new Protocol Limit Profile
  revision, or N/N-1 enforcement rejects an input valid under its selected
  supported revision without an independent temporary or authorization cause;
- a token-limited fixture lacks an exact Token Counting Profile and verified
  Provider mapping evidence;
- a Tool/MCP invocation omits a mandatory context or Destination Attempt
  admission reference, or an external invocation omits disclosure/wire evidence;
- an archive root, protected-input digest, trusted-anchor proof, or exact entry
  digest does not recompute;
- an Artifact/Event example references a mutable alias rather than an exact
  Revision;
- a secret-bearing or cross-Scope field enters any generated output, log
  fixture, archive, or wire record; or
- an internal Worker/Adapter contract appears in the public package.

The contracts crate must expose one machine-readable compatibility report that
classifies every generated diff as additive, breaking, representation-only,
or security-sensitive. A human review is mandatory for a breaking or security-
sensitive classification. Deterministic Verification owns the executable CI
implementation; this specification owns what it must prove.

### 16.4 Canonical adversarial fixture families

The generated positive corpus includes applied, refused, invalid, conflicted,
and no-effect committed domain outcomes with their stable typed Receipt
references, identical idempotent replays, exclusive SSE resume, bounded
at-least-once duplicate handling, all required Tool/MCP seven-gate evidence,
and a verifiable signed or integrity-protected archive. The negative corpus
includes at least:

1. duplicate JSON names at the top level and inside nested authority fields;
2. unknown Command fields, discriminators, Approval states, and Attempt states;
3. valid unknown presentation-only output fields;
4. wrong `owner_user_id`, Project, Run, Artifact Revision, cursor, Snapshot,
   Capability, Approval, and Credential Reference substitutions;
5. caller-supplied owner, role, or Capability claims;
6. reused idempotency key with a changed body, route, command kind, or scope,
   and the same digest under a new key attempting to reuse a consumed nonce;
7. stale expected Revision/Head and concurrent identical submission;
8. expired, cross-route, cross-body, cross-record, replayed, and stolen
   anti-forgery nonce, plus exact retry against the wrong idempotency record;
9. disallowed Host, Origin, content type, and credentialed cross-origin request;
10. valid exclusive SSE reconnect, client resume from an older cursor with
    duplicate delivery and Event-ID dedupe, cross-filter cursor, cursor ahead,
    cursor below replay floor, revocation between reconnects, and compaction
    generation change;
11. payload length/digest/media/schema mismatch and payload-reference swapping;
12. JSON, archive, decompression, Query, SSE, Tool, Provider, and bridge inputs
    at, below, and one unit above every Profile ceiling; same-revision dynamic
    content narrowing; frozen-bound drift; and missing or wrong tokenizer
    mapping evidence;
13. archive path traversal, absolute path, Unicode/case collision, symlink,
    hardlink, device entry, recursive archive, expansion bomb, root
    self-reference, reordered/omitted coverage, bad entry/root digest, and
    missing, embedded-only, unknown, or invalid integrity anchor/proof;
14. stale lease, old fence, late result, duplicate outbox delivery, missing or
    mismatched final Admission Decision, crash before dispatch claim, crash
    after claim, and uncertain Provider outcome;
15. Tool/MCP discovery drift, same-name replacement, unpinned schema, global
    Credential Reference or compatibility cache, cross-Scope enablement/use,
    missing seven-gate manifest, widened effect, hidden SDK retry, result reuse
    without reassembly, and incompatible cancellation/error mapping;
16. spoofed App window, wrong Instance or Artifact Revision, absent/mismatched,
    numeric, null, overlength, or non-ASCII bridge request ID, semantic
    notification, repeated bridge ID with another digest, an attempt to reuse
    one Instance's Request in another Instance, out-of-order sequence, rate
    exhaustion, stale sandbox, and response after Instance termination;
17. Credential value, value digest, locator, or secret-bearing header attempts
    through arguments, output, logs, wire records, archives, and App messages;
18. Provider redirect, DNS rebinding, private/link-local address, ambient proxy
    or metadata credential, destination fallback, and disclosure widening;
19. unknown historical Event control semantics with and without a compatible
    N-1 projection; and
20. query/cache/projection rows that are stale, partially rebuilt, cross-Scope,
    below a required watermark, or falsely empty.

## 17. Illustrative wire examples

These examples use one synthetic Project Scope. Generated canonical fixtures,
not Markdown formatting, own exact production bytes.

### 17.1 Committed Command acknowledgement

```json
{
  "acknowledgement_kind": "committed",
  "envelope_version": 1,
  "command_id": "018f0000-0000-7001-8000-000000000011",
  "project_scope": {
    "owner_user_id": "018f0000-0000-7001-8000-000000000001",
    "project_id": "018f0000-0000-7001-8000-000000000002"
  },
  "correlation_id": "018f0000-0000-7001-8000-000000000013",
  "receipt_ref": {
    "kind": "domain_receipt",
    "id": "018f0000-0000-7001-8000-000000000012"
  },
  "project_activity_position": "42",
  "committed_at": "2026-07-21T10:15:30.123Z",
  "limit_profile_revision": "storyos.foundation.absolute.v1"
}
```

The Server returns this only after the owning PostgreSQL transaction commits.
An identical submission under the same key and digest returns the same selected-
release representation; it does not create activity position `43`.

### 17.2 Accepted asynchronous acknowledgement

```json
{
  "acknowledgement_kind": "accepted",
  "envelope_version": 1,
  "command_id": "018f0000-0000-7001-8000-000000000021",
  "project_scope": {
    "owner_user_id": "018f0000-0000-7001-8000-000000000001",
    "project_id": "018f0000-0000-7001-8000-000000000002"
  },
  "correlation_id": "018f0000-0000-7001-8000-000000000023",
  "operation_ref": {
    "kind": "agent_run",
    "id": "018f0000-0000-7001-8000-000000000022"
  },
  "settlement_query": "/api/v1/projects/018f0000-0000-7001-8000-000000000002/agent-runs/018f0000-0000-7001-8000-000000000022",
  "admitted_activity_position": "43",
  "accepted_at": "2026-07-21T10:16:00.000Z",
  "limit_profile_revision": "storyos.foundation.absolute.v1"
}
```

This says the Run and recovery identity are durable, not that model or Tool
work succeeded.

### 17.3 Canonical Query page

```json
{
  "envelope_version": 1,
  "query_schema": "storyos.query.proposal.v1",
  "query_id": "018f0000-0000-7001-8000-000000000031",
  "correlation_id": "018f0000-0000-7001-8000-000000000033",
  "project_scope": {
    "owner_user_id": "018f0000-0000-7001-8000-000000000001",
    "project_id": "018f0000-0000-7001-8000-000000000002"
  },
  "snapshot": {
    "snapshot_id": "018f0000-0000-7001-8000-000000000032",
    "project_scope": {
      "owner_user_id": "018f0000-0000-7001-8000-000000000001",
      "project_id": "018f0000-0000-7001-8000-000000000002"
    },
    "snapshot_kind": "canonical",
    "project_activity_position": "43",
    "source_watermarks": {},
    "projection_generations": {},
    "redaction_profile": "storyos.author.v1",
    "schema_profile": "storyos.public.release.1",
    "replay_generation": "1",
    "created_at": "2026-07-21T10:16:30.000Z",
    "expires_at": null
  },
  "required_activity_position": "43",
  "items": [],
  "next_cursor": null,
  "page_count": 0,
  "page_bytes": 0,
  "redaction_profile": "storyos.author.v1",
  "limit_profile_revision": "storyos.foundation.absolute.v1"
}
```

An empty `items` array is meaningful only because this is a qualified Canonical
Snapshot, not a lagging Projection Query.

### 17.4 Project Activity SSE frame

```text
id: eyJwcm9maWxlIjoic3Rvcnlvcy1zY29wZWQtY3Vyc29yIn0
event: storyos.project-activity
data: {"envelope_version":1,"activity_profile":"storyos.project-activity.v1","event_id":"018f0000-0000-7001-8000-000000000041","event_schema":"storyos.event.proposal-conflict-detected.v1","event_kind":"proposal_conflict_detected","project_scope":{"owner_user_id":"018f0000-0000-7001-8000-000000000001","project_id":"018f0000-0000-7001-8000-000000000002"},"requester_user_id":"018f0000-0000-7001-8000-000000000001","actor":{"kind":"author","id":"018f0000-0000-7001-8000-000000000001"},"project_sequence":"44","stream_sequence":"44","agent_run_id":null,"run_step_id":null,"run_sequence":null,"aggregate_ref":{"kind":"proposal","id":"018f0000-0000-7001-8000-000000000042"},"correlation_id":"018f0000-0000-7001-8000-000000000043","causation":{"kind":"command","id":"018f0000-0000-7001-8000-000000000011"},"command_id":"018f0000-0000-7001-8000-000000000011","receipt_ref":{"kind":"domain_receipt","id":"018f0000-0000-7001-8000-000000000044"},"occurred_at":"2026-07-21T10:17:00.000Z","recorded_at":"2026-07-21T10:17:00.005Z","payload":{"proposal_id":"018f0000-0000-7001-8000-000000000042","observed_head_revision_id":"018f0000-0000-7001-8000-000000000045"},"payload_digest":{"algorithm":"sha256","profile":"storyos.event-payload.jcs.v1","value_hex_lowercase":"073c643d461f8ce5d6a865fded918411ba1888f20cfd9a6a6418b1480cd9c62b"},"application_wire_record_ref":"018f0000-0000-7001-8000-000000000046","limit_profile_revision":"storyos.foundation.absolute.v1"}

```

The SSE `id` is the scoped cursor; the durable Event identity is the `event_id`
inside `data`. Reconnecting with this exact `id` resumes exclusively at the
next authorized position; reconnecting from an older retained cursor may
redeliver this Event, which the client discards by `event_id`.

### 17.5 Non-oracular not found

```json
{
  "type": "https://storyos.dev/problems/resource-not-found",
  "title": "Resource not found",
  "status": 404,
  "detail": "The requested resource is unavailable.",
  "instance": "urn:storyos:problem:018f0000-0000-7001-8000-000000000051",
  "code": "resource_not_found",
  "retryability": "never",
  "correlation_id": "018f0000-0000-7001-8000-000000000052",
  "project_scope": null,
  "command_id": null,
  "safe_conflict": null,
  "resync": null,
  "limit_profile_revision": "storyos.foundation.absolute.v1"
}
```

The same public shape covers an unknown object and a cross-Scope substitution;
the latter may have a distinct sanitized internal security reason.

### 17.6 App bridge action binding

```json
{
  "bridge_revision": "storyos.mcp-app-bridge.v1",
  "app_view_instance_id": "018f0000-0000-7001-8000-000000000061",
  "app_view_artifact_revision_id": "018f0000-0000-7001-8000-000000000062",
  "instance_negotiation_id": "018f0000-0000-7001-8000-000000000063",
  "sandbox_profile_revision": "storyos.app-sandbox.v1",
  "instance_sequence": "7",
  "direction": "app_to_host",
  "bridge_request_id": "app-request-7",
  "jsonrpc_message": {
    "jsonrpc": "2.0",
    "id": "app-request-7",
    "method": "storyos.query_artifact",
    "params": {
      "artifact_revision_id": "018f0000-0000-7001-8000-000000000064"
    }
  },
  "payload_digest": {
    "algorithm": "sha256",
    "profile": "storyos.app-bridge-binding.jcs.v1",
    "value_hex_lowercase": "51d58aaeb339232414589911534cdf613486469bd8475716c1ab53143ab782ba"
  },
  "limit_profile_revision": "storyos.foundation.absolute.v1"
}
```

The bridge Request does not contain Project Scope or Capability supplied by the
App. The Host derives both from the Instance binding and current operation,
then creates the durable App Action Request before querying.

### 17.7 Project archive root and integrity binding

The synthetic archive contains one entry whose exact uncompressed bytes,
without a trailing newline, are:

```text
{"project_id":"018f0000-0000-7001-8000-000000000002","schema":"storyos.project-record.v1"}
```

```json
{
  "archive_profile": "storyos.project-export.v1",
  "export_id": "018f0000-0000-7001-8000-000000000071",
  "project_scope": {
    "owner_user_id": "018f0000-0000-7001-8000-000000000001",
    "project_id": "018f0000-0000-7001-8000-000000000002"
  },
  "source_snapshot": {
    "snapshot_id": "018f0000-0000-7001-8000-000000000072",
    "project_activity_position": "44"
  },
  "source_schema_compatibility": {
    "minimum_reader": "storyos.schema.1",
    "maximum_reader": "storyos.schema.1"
  },
  "schema_catalog": "storyos.schema-catalog.v1",
  "table_family_counts": {
    "projects": "1"
  },
  "serialization_profile": "storyos.project-archive-json.jcs.v1",
  "digest_profile": "storyos.project-archive-root.jcs.v1",
  "limit_profile_revision": "storyos.foundation.absolute.v1",
  "entries": [
    {
      "path": "canonical/project.json",
      "media_type": "application/json",
      "payload_schema": "storyos.project-record.v1",
      "byte_length": "90",
      "digest": {
        "algorithm": "sha256",
        "profile": "storyos.archive-entry.raw-bytes.v1",
        "value_hex_lowercase": "e2e04dc008f68b1c35338884373c8c3a1f46ee1cb591dbddc6de1a782c682ba2"
      }
    }
  ],
  "provenance_closure": {
    "status": "complete",
    "edge_count": "0"
  },
  "known_purged_gaps": [],
  "root_digest": {
    "algorithm": "sha256",
    "profile": "storyos.project-archive-root.jcs.v1",
    "value_hex_lowercase": "608f98a9fd3f450199b32f62d88c008b35e945554c206c217dfafcf017cd4861"
  },
  "integrity_protection": {
    "kind": "signature",
    "profile": "storyos.project-archive-integrity.ed25519.v1",
    "trusted_anchor_id": "storyos.fixture.archive-signing-key.v1",
    "protected_input_profile": "storyos.project-archive-protected-input.jcs.v1",
    "protected_input_digest": {
      "algorithm": "sha256",
      "profile": "storyos.project-archive-protected-input.jcs.v1",
      "value_hex_lowercase": "623d622b68d28bc52607a5fa382a2587aac53b833cd13db69ee06f1d45aa27fe"
    },
    "signature_base64url": "T-fvnq2xuXoLbCLUgKERaOrXVQnedl3NnXK1AYvdMC0MV-6Fe9SGnbU-T5b0R5is5GM4IiDRw4_wX9UA1VxfAw"
  },
  "created_at": "2026-07-22T09:00:00.000Z"
}
```

The root digest excludes `root_digest` and `integrity_protection` exactly as
specified in section 13.7. The protected-input digest covers the closed object
`{ archive_profile, export_id, project_scope, root_digest }`; the signature is
verified against the independently trusted fixture anchor, not a key supplied
by the archive. The out-of-archive fixture anchor catalog maps the named anchor
to Ed25519 public key `xSMHMLALUXpg9LlfOQL8DeaYJMyYIa2szZuBOlUk3og`
(base64url, SHA-256
`b0ff220aba9ce5c1739acf3217d3193ef80af2e7ca619a82df98c16152c6fb4f`).

## 18. Downstream ownership and migration impact

| Downstream owner | Receives from this specification | Still owns |
| --- | --- | --- |
| [Define the Modular-Monolith and Repository Governance Boundaries](https://github.com/FrankQDWang/StoryOS/issues/59) | public/Core/internal/external surface separation and contracts-crate generation boundary | crate/module layout, dependency rules, repository governance |
| [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64) | Project Activity envelope, Snapshot/cursor identity, replay generation, Application Wire Record class, handoff failures | retention durations, compaction/archive mechanics, Mailbox and internal Run Event detail |
| [Define Foundation Evidence for the Standalone Eval Surface](https://github.com/FrankQDWang/StoryOS/issues/61) | inspectable typed protocol evidence and profile identities | Eval claims, datasets, scoring, evidence acceptance |
| [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) | required drift/adversarial families, fences, Attempts, commit and OutcomeUnknown invariants | executable tests, crash schedules, CI gates, fake destinations |
| [Lock the First Production Vertical Slice and Handoff Criteria](https://github.com/FrankQDWang/StoryOS/issues/62) | complete contract menu and absolute ceilings | which coherent subset ships first, effective defaults, acceptance gate |

The PostgreSQL storage contract continues to own physical tables, RLS,
migrations, roles, payload placement, and deployment upgrade sequencing. The
contracts-crate decision continues to own generator implementation. No ticket
above may silently weaken exact Project Scope, server-derived identity,
manifest-before-egress, immutable history, rebuildable projections,
Proposal/Acceptance, Capability/Approval, Attempt/OutcomeUnknown, or fenced
recovery.

This is a pre-implementation Foundation decision, so it requires no production
data migration now. Its first implementation is nevertheless a new public and
persisted contract and must establish API release 1, schema catalog, wire
corpus, and limit profile atomically. Later changes apply the classifications
in section 14 and the database N/N-1 expand/migrate/contract discipline.

No new ADR is required: this specification is the designated owner of the
wire protocol detail, while the durable architectural premises are already
accepted in existing ADRs. A future choice that reverses a hard premise across
multiple contracts must receive its own ADR; normal schema additions do not.

## 19. Normative invariants

An implementation conforms only if all of these remain true:

1. Every project-bearing operation and record binds exact
   `{ owner_user_id, project_id }`; public owner, Project, role, or Capability
   claims never authorize it.
2. Client Session, Host, Origin, anti-forgery, Project Scope, object Scope, and
   operation authority are separate fail-closed checks; a nonce is consumed
   only against its exact idempotency key, record, and command digest.
3. Commands use exact idempotency digest binding and explicit concurrency
   preconditions; duplicates cannot create another logical effect, and every
   admitted domain settlement returns its stable typed Receipt path.
4. `Committed` means the complete owning transaction committed; `Accepted`
   means only durable asynchronous admission. Committed refusal, conflict,
   invalidity, and no-effect are Receipt-bearing settlements, not lost Problems.
5. Canonical Queries identify a committed Snapshot; Projection Queries expose
   source boundary, generation, watermark, completeness, and lag.
6. Artifact identity, Revision identity, digest, payload bytes, and rebuildable
   projections remain distinct.
7. One exact Project Scope has one canonical Project Activity Stream. SSE is a
   replayable delivery view, not authority or the source of truth.
8. Cursor reconnect reauthorizes and resumes exclusively after the cursor;
   client Event-ID dedupe handles at-least-once overlap, cursor-too-old never
   skips silently, and compaction never guesses a position.
9. Public failures do not reveal another User or Project object's existence;
   uncertainty remains explicitly `outcome_unknown`.
10. Every physical retry, resend, fallback, or repair has a new Attempt; stale
    fences cannot settle or publish late work.
11. Tool, MCP, App, Provider, research, import, and external contents remain
    untrusted. Every Tool/MCP use crosses all seven gates, results repeat them
    before later context use, final admission and manifests commit before
    egress, and only Host/Core authority can change authoritative state.
12. Commands and control inputs are closed; public outputs evolve additively
    only where unknown meaning is presentation-safe.
13. Public N/N-1 support uses generated projections; external contracts use
    exact pinning, Project Scope-bound use and Credential bindings,
    compatibility decisions, drift quarantine, and fresh authorization for
    widening. Global registrations contain no Project data or credentials.
14. Historical Application Wire Records and domain history are immutable;
    upgrades append projections and migration evidence.
15. Every operation freezes named effective bounds no wider than its immutable
    Protocol Limit Profile; numeric changes create a new revision, dynamic
    pressure is temporary rate/concurrency admission, and token limits have an
    exact counting profile and Provider mapping, without author settings.
16. Rust contracts are the sole editable external source; OpenAPI, JSON Schema,
    TypeScript, catalogs, fixtures, and wire corpus cannot drift independently.
17. Project archive root coverage excludes only its root and integrity proof,
    every digest recomputes, and import requires independently trusted
    integrity protection before visibility.

## 20. Accepted inputs and evidence

This specification composes, rather than reopens:

- [Use HTTP Commands with Replayable SSE Events](https://github.com/FrankQDWang/StoryOS/issues/24);
- [Own External Contracts in a Rust Contracts Crate](https://github.com/FrankQDWang/StoryOS/issues/25);
- the accepted Authoritative State, Artifact, Proposal, AgentRun/Subrun/Mailbox,
  Tool/MCP/Skill, Model Gateway, Memory/Research, Context/Disclosure,
  Transcript/App, PostgreSQL storage, and threat-model decisions recorded in
  [CONTEXT.md](../../CONTEXT.md) and the Foundation issue history;
- [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md);
- [Artifact Domain Model](artifact-domain-model.md);
- [Fiction Memory and Research Provenance Semantics](fiction-memory-and-research-provenance-semantics.md);
- [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md);
- [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md); and
- [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md).

Official protocol facts and the limits of what those standards define are
recorded in the linked primary-source research note. StoryOS-specific replay,
scope, authority, idempotency, compatibility, limits, and evidence rules are
application contracts fixed here, not claims that HTTP, SSE, JSON Schema,
OpenAPI, JSON-RPC, MCP, or MCP Apps supply them automatically.
