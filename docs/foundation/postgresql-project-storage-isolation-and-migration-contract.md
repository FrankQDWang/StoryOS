# PostgreSQL Project Storage, Isolation, and Migration Contract

- Status: accepted
- Contract revision: `release1-storage-contract-2026-07-31`
- Wayfinder resolution: [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56)
- Canonical glossary: [`CONTEXT.md`](../../CONTEXT.md)
- Deployment decision: [ADR 0004: Adopt a PostgreSQL Service and Project Isolation Boundary](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)
- Research input: [PostgreSQL Project Storage, Isolation, and Migration Source Audit](../research/postgresql-project-storage-isolation-and-migration-source-audit.md)
- Parent semantic contracts: [Artifact domain model](artifact-domain-model.md), [Manuscript state machine](manuscript-revision-proposal-state-machine.md), [Fiction memory and research provenance](fiction-memory-and-research-provenance-semantics.md), and [Context assembly and disclosure](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Operational lifecycle contract: [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md)
- Release 1 persistence catalog: [`postgresql-release-1-persistence-catalog.json`](postgresql-release-1-persistence-catalog.json)
- Review-time catalog verifier: [`verify-postgresql-release-1-persistence-catalog.py`](verify-postgresql-release-1-persistence-catalog.py)

## 0. Release 1 closure and compatibility identity

Sections 1–12 preserve the accepted Foundation decisions. This Release 1
revision closes the physical gaps exposed by the completed Core, Author Command
Admission, Web Editor Session, and public protocol contracts. The JSON catalog
is part of this contract: it is not production DDL and it does not create a
second semantic owner. A table family, schema identity, migration edge, or
projection not represented there is not an admitted Release 1 persistence
surface.

The active storage identity is the exact tuple below:

```text
StorageCompatibilityIdentity {
  database_schema_identity: storyos.postgresql.schema.v1
  active_schema_version: storyos.persistence.release-1.v1
  persisted_format_catalog_id: storyos.persistence.catalog.release-1.v1
  migration_chain_id: storyos.persistence.migrations.release-0-to-release-1.v1
  migration_chain_digest: sha256 over the catalogued migration chain
  public_release: storyos.public.release.1
  route_catalog_id: storyos.public.route-catalog.release-1.v1
  route_catalog_contract_revision: release1-wire-catalog-2026-07-31
  route_catalog_sha256: sha256 over the LF-normalized Release 1 route catalog
  compatibility_profile: storyos.public.same-release.v1
  release_identity_schema_id: storyos.compatibility.release-identity.v1
}
```

The public protocol's `Release1CompatibilityIdentity` remains owned by
[Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md).
Storage contributes the complete persistence tuple through the protocol's
contract-graph/release identity; it does not invent a parallel public release
or route. The Server, Worker, protected Web Client, generated contracts, route
catalog, Event catalog, migration ledger, and active schema must compare the
same values. A mismatch returns `upgrade_required` before a domain attempt,
Project Activity cursor advancement, external effect, or reopening live
traffic. A matching semantic version alone is never sufficient.

The supported stored predecessor is exactly
`storyos.persistence.release-0.v1`; a new installation enters the same active
Release 1 schema without a predecessor. The catalog proves one acyclic path for
the supported predecessor and records every transactional and nontransactional
phase. Historical rows retain their own schema and digest identities; a reader
projects them into the active release only after the migration and restore gates
below pass.

## 1. Scope and authority

This specification defines the Foundation physical persistence contract for all
accepted StoryOS durable semantics. PostgreSQL is the authoritative physical
database in both the local Foundation Validation Deployment and a later
controlled cloud deployment. This document owns database topology, schema and
aggregate placement, keys and constraints, transaction and concurrency rules,
payload placement, disposable projections, secret references, migration,
backup, restore, and Project portability.

It does not redefine creative authority, AgentRun or Subrun semantics, context
or disclosure gates, Model routing, Tool execution, collaboration, or
retention durations. It maps their already accepted facts into a fail-closed
store. It specifies no production Rust, SQL migration file, deployment, or
cloud vendor.

The following remain prohibited:

- SQLite or a database, file, directory, or schema per Project;
- Neo4j, a separate vector database, a message broker, microservices, or
  whole-system Event Sourcing;
- a process-global User, ProjectId-only ownership, or caller-side filtering as
  the storage isolation boundary;
- Provider-specific schema, including Bailian-specific tables or migrations;
- an external object store, PostgreSQL Large Object, or application-compressed
  canonical payload in the Foundation;
- secret material in ordinary PostgreSQL rows, logs, backups, or exports.

## 2. Database topology, ownership, and roles

### 2.1 One database and one application schema

One StoryOS service deployment uses one PostgreSQL database and one
application schema named `storyos`. Tables are separated by aggregate and
durability role, not by User or Project. A later multi-User service adds rows,
not databases or schemas. Globally reusable definitions may omit Project Scope
only when they contain no project-derived data, authority, Credential
Reference, or runtime state.

The schema uses explicit qualified names. The runtime connection does not rely
on a mutable `search_path`. Extensions, when later admitted, are installed in
a separately controlled schema and cannot own canonical StoryOS semantics.

### 2.2 Database roles

The minimum role separation is:

| Role | Contract |
|---|---|
| `storyos_owner` | `NOLOGIN`; owns schema, tables, functions, constraints, and RLS policies; never handles a request |
| `storyos_runtime` | non-owner, non-superuser, `NOBYPASSRLS`; receives only explicit DML and function grants needed by the Server |
| `storyos_migrator` | separate controlled login that may assume owner authority only during a migration; absent from the request pool |
| `storyos_backup` | separate controlled maintenance role for whole-database backup; absent from the request pool and unable to resolve credential values |
| `storyos_restore` | isolated restore or migration environment role; never shares runtime credentials |

No application deployment connects as the database owner, superuser, or a role
with `BYPASSRLS`. Backup, restore, schema inspection, and migration paths may
legitimately see more than one Project, but they are explicit maintenance
boundaries with separately audited credentials and cannot be reached through a
StoryOS request.

### 2.3 Trusted transaction-local Project Scope

Every project request begins a database transaction before any project-bearing
query. Trusted Host authorization resolves:

```text
ProjectScope {
  owner_user_id: UserId
  project_id: ProjectId
}
```

The database adapter sets both members as transaction-local settings, for
example through `SET LOCAL` or transaction-local `set_config`. Every project
table has both RLS `USING` and `WITH CHECK` expressions matching those settings.
RLS is enabled and forced. A missing, malformed, partially set, or mismatched
scope matches no row and admits no write. Connection-pool return cannot retain
scope because transaction-local settings end at commit or rollback.

Only trusted Host state may select the scope. A client body, model output,
Tool argument, globally unique object ID, or prior connection setting cannot.
RLS is mandatory defense in depth; the Server must still authorize every
command and include exact scope predicates in SQL.

User-scoped rows such as `users` use the trusted current User identity and
forced User-level RLS; the runtime cannot enumerate other Users. Bootstrap and
identity provisioning are separate controlled paths. Globally reusable,
project-data-free definitions use explicit grants rather than a fabricated
Project Scope.

## 3. Physical aggregate and table ownership

### 3.1 Common row shapes

Every project-bearing relation contains non-null `owner_user_id` and
`project_id` as its first ownership columns. Canonical identities are typed
UUIDv7 values, but UUID order and embedded time have no semantic meaning.
Immutable rows carry trusted audit time and their owning schema/profile version.
Mutable normalized rows carry an explicit monotonic version used as an expected
write precondition.

The common shapes are:

```text
ScopedIdentityRow  = owner_user_id, project_id, typed_id, ...
ImmutableFactRow   = owner_user_id, project_id, typed_id, schema_version,
                     created_at, ...
VersionedHeadRow   = owner_user_id, project_id, logical_id,
                     current_revision_id, head_version, ...
PayloadRow         = owner_user_id, project_id, payload_id, payload_family,
                     payload_schema_version, digest_profile, digest,
                     canonical_bytes, byte_length, created_at
```

Audit timestamps are evidence only. Freshness, ordering, locking, idempotency,
and authorization never depend on wall time.

### 3.2 Normative table catalog

The following catalog fixes physical ownership boundaries. An implementation
may add narrow subtype tables or split a family for size and access patterns,
but it cannot merge disjoint durable spaces, replace typed relations with a
generic EAV/event bucket, or remove Project Scope from a project-bearing row.

| Aggregate owner | Canonical `storyos` table families | Physical rule |
|---|---|---|
| Identity and Project | `users`, `projects` | `users` is User-scoped; `projects` owns the exact Project Scope and immutable Project Author |
| Project policy | `project_instruction_revisions`, `project_instruction_heads`, `author_preference_revisions`, `project_policy_revisions`, `project_destination_grants`, `project_tool_enablements`, `context_controls`, `proactive_trigger_revisions`, `trigger_occurrences`, `trigger_admission_decisions` | immutable revisions and decisions plus narrow current Heads; no mutable JSON settings bag |
| Credential binding | `credential_references`, `project_credential_bindings` | non-secret resolver, availability, and scoped binding evidence only; identity establishment may consume it, but neither the Reference nor binding grants destination use |
| Processing destination identity | `processing_destination_identities`, `processing_destination_identity_evidence_revisions` | canonical immutable Project Scope-bound, non-authorizing identity of the actual processor, endpoint, account, control class, intake/disclosure boundary, and append-only versioned establishment or re-verification evidence; evidence may reference project-free service-surface identity and optional scoped Credential-binding inputs, never a Grant, use binding, compatibility Decision, or execution authority |
| Authoritative State | `authoritative_objects`, `authoritative_revisions`, `authoritative_payloads`, `authoritative_heads`, `authoritative_commits`, `authoritative_commit_members` | immutable revisions and commits; normalized Heads are the current authority pointer |
| Manuscript subtype | `manuscript_objects`, `manuscript_blocks`, `manuscript_revision_members`, `revision_anchors` | subtype rows reference Authoritative identities and exact revisions in the same Scope |
| Artifact | `artifacts`, `artifact_revisions`, `artifact_payloads`, `artifact_heads`, `artifact_provenance_edges` | never shares a Head or payload identity with Authoritative State |
| Proposal | `proposals`, `proposal_revisions`, `proposal_heads`, `proposal_operations`, `proposal_operation_resolutions`, `proposal_generations`, `proposal_stream_events` | Proposal state axes remain immutable evidence; current Head is normalized separately; Receipt and fence rows are owned by Operational Records |
| Author command admission | `anti_forgery_challenges`, `author_command_admissions`, `author_command_admission_settlements` | immutable User, exact existing or Server-allocated prospective Project Scope, trusted-client and Editor Session, writer generation, action class, command digest, target, one-use nonce/idempotency, lifetime, and one typed terminal settlement |
| Web editor coordination | `editor_sessions`, `project_writer_generations`, `editor_input_fences`, `proposal_pause_fences` | one current Project writer generation; stale generations are fenced; the IndexedDB Local Edit Journal remains non-authoritative client continuity data |
| Domain command evidence | `domain_receipts`, `validation_receipts`, `acceptance_receipts`, `undo_receipts`, `undo_acceptance_receipts`, `author_undo_receipts`, `receipt_settlements`, `domain_events`, `command_idempotency`, `author_action_entries`, `scope_counters`, `aggregate_counters` | one committed outcome per command key, one independent Author Action frontier, and transactional committed-domain order |
| Agent execution | `agent_runs`, `subruns`, `subrun_joins`, `run_grant_revisions`, `subrun_capability_grants`, `run_plans`, `run_steps`, `run_lanes`, `run_events`, `run_mailbox_messages`, `run_mailbox_deliveries`, `run_waits`, `wait_resolutions`, `run_holds`, `run_wakeups`, `run_leases`, `run_execution_attempts`, `run_budget_accounts`, `run_budget_reservations`, `usage_records` | normalized live state is backed by immutable events and receipts; mailbox delivery is durable and leases and budget claims are fenced |
| Transcript and approval | `transcript_items`, `approval_waits`, `approval_decisions`, `steering_inputs`, `run_checkpoints` | Transcript items and decisions are canonical Operational Records; checkpoints are disposable projections |
| Tool, MCP, and Skill | `tool_specs`, `tool_registration_revisions`, `tool_registration_heads`, `tool_registration_status_events`, `tool_calls`, `tool_attempts`, `mcp_server_registration_revisions`, `mcp_server_registration_heads`, `mcp_app_artifacts`, `skill_package_revisions`, `skill_package_heads`, `skill_activations` | reusable non-project definitions are unscoped only when content-free of project data; calls, activations, Apps, and status evidence are scoped |
| Model Gateway | `model_registration_revisions`, `model_registration_heads`, `model_registration_status_events`, `model_capability_profiles`, `model_operational_snapshots`, `model_routing_policy_revisions`, `model_route_requests`, `model_route_decisions`, `model_invocations`, `model_attempts`, `model_usage_settlements` | Registration/capability contracts are unscoped only while free of Project data, Credential References, actual endpoint/account or disclosure destinations, and authority; the model surface uses the shared scoped external-use-binding shape to pin Project Scope, use/Credential authorization, one already-established Processing Destination Identity and exact evidence revision, and bounds, while a separate later compatibility Decision evaluates that binding; every snapshot, route decision, invocation, attempt, and fallback pins both records; Provider-neutral columns and versioned Adapter payload add no Provider-specific authority table |
| Memory and research | `memory_candidates`, `memory_admissions`, `memory_suppressions`, `research_claims`, `research_evidence_edges`, `source_snapshots` | source Revisions and provenance remain canonical; retrieval projections remain separate |
| Context and disclosure | `operation_requirements`, `context_candidates`, `context_eligibility_decisions`, `context_selection_decisions`, `context_projections`, `context_assembly_manifests`, `context_manifest_members`, `project_external_use_binding_revisions`, `external_contract_compatibility_decisions`, `destination_attempts`, `wire_payload_projections`, `outbound_disclosure_events`, `destination_attempt_settlements` | Model, Tool, MCP, Provider, and research surfaces share one scoped external-use-binding shape that pins an already-existing Processing Destination Identity, exact evidence revision, and owning authorization; a compatibility Decision follows and references one already-existing binding; the Manifest and exact non-secret wire evidence commit before external dispatch claim |
| Durable work delivery | `outbox_entries`, `worker_fences` | outbox intent commits with its owning transition; a worker generation fences all settlements |
| Disposable retrieval | `retrieval_documents`, `retrieval_fragments`, `retrieval_terms`, `embedding_projections`, `projection_dependencies`, `projection_invalidations`, `projection_generations`, `context_cache_entries`, `context_cache_dependencies` | all rows are scoped, dependency-complete, immediately disqualifiable, and rebuildable |
| Disposable read model | `read_model_checkpoints` | current-only materialization with canonical and Project Activity dependencies; its generation, watermark, invalidation, rebuild, and visibility rules are catalogued separately from historical Snapshot/replay evidence |
| Storage administration | `schema_migrations`, `migration_phases`, `migration_phase_checksums`, `recovery_copies`, `backup_wal_evidence`, `restore_proofs`, `recovery_visibility_proofs`, `project_export_manifests`, `project_export_entries`, `project_restore_staging`, `project_restore_validation` | maintenance-only metadata; staging is never visible as a live Project |

Tables whose logical facts already belong to an Artifact or Operational Record
may use a typed subtype relation referencing that owner rather than duplicating
payload. The one-to-one relationship and same-space ownership must be enforced
by a scoped foreign key. Globally reusable ToolSpecs, schemas, mapping profiles,
and adapter definitions use content-addressed or versioned global identities;
their project enablement, use, evidence, and cached effects remain scoped.

### 3.3 Release 1 persistence-family ledger

The catalog is the exhaustive Release 1 ledger. Its families are deliberately
coarse enough to keep physical ownership reviewable and narrow enough that no
one family crosses an authority or durability boundary:

| Catalog family | Durable space | Physical responsibility | Public or internal boundary |
|---|---|---|---|
| `identity-user` | User identity | User rows and forced User-level access | Project creation/listing input; never a Project export payload |
| `project-canonical` | Authoritative State | Project policy, manuscript objects, immutable Revisions/Payloads, Heads, Commits, and scoped counters | Core and Release 1 manuscript/project routes |
| `artifact-proposal-draft` | Artifacts | Artifact, Proposal, Draft, provenance, memory, research, and source Revision histories | Proposal/Artifact routes; never a Canonical Head |
| `operational-receipts-actions` | Operational Records | All typed Receipt kinds, Author Actions, idempotency, and domain evidence | Settlement queries and Receipt-backed Activity |
| `operational-admission-editor` | Operational Records | Admission, nonce/challenge, Editor Session, writer-generation, Input Fence, and Pause Fence records | Admission and editor-session routes |
| `operational-run-mailbox` | Operational Records | Run, Subrun, Step, Mailbox, Transcript, Approval, budget, lease, fence, and outbox evidence | Agent/run routes and downstream retention owner |
| `operational-context-disclosure` | Operational Records | Processing Destination Identity, requirement/selection, Manifest, binding, Decision, Attempt, wire projection, and disclosure evidence | Context, model, Tool, and MCP boundaries |
| `operational-wire-history` | Historical Operational Records | Immutable Application Wire Records and first materialized public Event representations | Public commands/Events are represented; query-response bytes and transport framing are not |
| `operational-project-activity` | Historical Operational Records | Project Activity Events, positions, replay generations, replay floors, and handoff evidence | One canonical Project Activity stream |
| `operational-snapshot-replay` | Historical Operational Records | Authorized Snapshot reading boundaries, cursor evidence, and generation handoffs | Snapshot/resync/query-history routes |
| `operational-lifecycle` | Operational Records | Archive, Tombstone, Suppression, retention, and deletion decisions | Lifecycle owner; physical cleanup remains downstream policy |
| `projection-generation-control` | Disposable projections | Dependency closure, invalidations, generation pointers, and source watermarks | Internal rebuild control; never an authority source |
| `projection-retrieval` | Disposable projections | Retrieval documents, fragments, and lexical terms | Search/retrieval read path; rebuildable |
| `projection-embedding` | Disposable projections | Scope-bound embedding observations and their exact external-use dependencies | Retrieval acceleration; unavailable is explicit |
| `projection-context-cache` | Disposable projections | Context projections, cache entries, and dependency rows | Context continuity; rebuilt from canonical inputs |
| `projection-read-model` | Disposable projections | Read-model checkpoints and other current-only materializations | Current view only; never history or authority |
| `global-definitions` | Global definitions | Project-free versioned Tool, MCP, Skill, Model, schema, and capability definitions | Explicit global grants; no project-derived bytes |
| `credential-references` | Scoped Operational Records | Opaque resolver references, binding generations, and availability evidence | Reference-only in backups/exports; never a secret store |
| `admin-migration` | Maintenance records | Schema ledger, phase checkpoints, and checksums | Migrator only; no request-path grant |
| `admin-recovery-copy` | Maintenance records | Recovery Copy, base-backup/WAL evidence, restore proof, and visibility proof | Backup/restore roles only |
| `admin-project-portability` | Maintenance records | Project Export manifests/entries and isolated restore staging/validation | Exact-scope archive/restore; staging never live |

`Authoritative State`, `Artifacts`, and `Operational Records` therefore remain
three disjoint durable spaces even when one Core Transition writes all three.
`Receipt`, `Author Action`, `Author Command Admission`, `Application Wire
Record`, and `Project Activity` are not additional authority levels. A
projection may reference any of them through exact scoped dependencies, but it
cannot become their source, and a projection family cannot be included in a
Project Export as a substitute for canonical or historical evidence. The
catalog's table-family uniqueness check rejects accidental merging.

## 4. Keys, constraints, and fail-closed references

### 4.1 Primary and alternate keys

`users` has primary key `(user_id)`. `projects` has primary key
`(owner_user_id, project_id)` and a foreign key from `owner_user_id` to
`users(user_id)`. Project ownership is immutable.

Every project-bearing identity table has primary key:

```text
(owner_user_id, project_id, typed_id)
```

When a surrogate physical key is justified for storage locality, the same
three columns still have a non-partial `UNIQUE NOT NULL` constraint and every
project reference uses that key. Global uniqueness of a UUID never replaces
the composite constraint. A project-bearing relation whose natural identity
has more components places Project Scope first.

### 4.2 Composite foreign keys

Every reference between project-bearing rows repeats both scope members:

```text
FOREIGN KEY (owner_user_id, project_id, target_id)
REFERENCES storyos.target(owner_user_id, project_id, target_id)
MATCH FULL
```

This rule applies to canonical rows, subtype rows, payloads, Heads, provenance
edges, idempotency outcomes, outbox work, manifests, caches, embeddings, and
read models. There are no unscoped project-object foreign keys and no trigger
that silently repairs a mismatched owner or Project. Missing targets or any
scope mismatch fail before commit.

Foreign keys are non-deferrable by default. A cyclic aggregate or staged
Project Restore may use `DEFERRABLE INITIALLY IMMEDIATE` only when the
transaction explicitly defers it and validates the full graph before atomic
visibility. Production runtime commands cannot disable constraints or RLS.

Destination references are deliberately acyclic. A
`processing_destination_identities` row owns its initial boundary evidence;
later `processing_destination_identity_evidence_revisions` rows reference only that
same-Scope Identity plus project-free service-surface and optional same-Scope
`project_credential_bindings` inputs, but no Grant, use binding, compatibility
Decision, Snapshot, route, or Attempt. A
`project_destination_grants` row references an already-existing same-Scope
Identity; a `project_external_use_binding_revisions` row references that
Identity, its exact current evidence revision, its Grant or other owning
authorization, Registration, and optional Credential binding; and an
`external_contract_compatibility_decisions` row
references the already-existing binding. Snapshot, route, invocation, and
Attempt rows then reference both binding and Decision. Every project-bearing
join uses the full composite Scope, so no reverse or unscoped foreign key can
turn Identity evidence into authority.

### 4.3 Required uniqueness and checks

At minimum, constraints enforce:

- one Head per scoped logical identity and Head kind;
- one current Project row per Project Scope;
- one immutable Revision identity and one payload binding per Revision;
- one committed domain sequence value per Project Scope and sequence kind;
- one command outcome per `(scope, command_kind, idempotency_key)`;
- one Author Command Admission per author-owned idempotency record and one
  terminal settlement per admission;
- one-use anti-forgery nonce binding per Client Session generation, exact
  existing or prospective Scope, route, command kind, digest, and idempotency
  record;
- one current writer generation per Project Scope and exact generation binding
  on every editor-bound admission;
- one Editor Input Fence per scoped Editor Session, writer generation, local
  intent range, and active Proposal Generation, with at most one associated
  Proposal Pause Fence;
- one immutable attempt sequence per Run lane, ToolCall, Model Invocation, and
  Destination operation as required by their owning semantics;
- one active lease generation per fenced resource;
- non-negative reservations and usage, with committed settlement bounded by
  the owning budget contract;
- payload byte length, digest profile, and family limit consistency;
- complete paired nullability for optional composite references;
- supported schema, coordinate, digest, Adapter, and projection profiles.

Enums that change compatibility semantics are represented by versioned checked
text or lookup identities, not PostgreSQL enum types that make staged removal
or renaming unsafe. Database checks reject impossible local shapes; aggregate
logic and transition receipts reject semantic conflicts that cannot be stated
as row constraints.

### 4.4 Index discipline

Every foreign key has a matching source index beginning with Project Scope.
Every runtime lookup and queue path begins with its isolation, lifecycle, and
eligibility keys before rank or time. No global project-derived index, partial
index, materialized view, or vector namespace may omit `owner_user_id` and
`project_id`. A globally reusable-definition index contains no project-derived
content.

The Foundation starts without per-Project partitions. Later measured
partitioning may use scope hash, lifecycle, or time only when every partition
preserves the parent constraints and RLS and runtime roles have no direct
partition grants. Partitioning never changes aggregate or Project identity.

### 4.5 Release 1 atomic write sets and physical record mapping

The Core Transition is the physical boundary for a successful author-owned
command. In one scoped transaction it resolves the idempotency arbiter, locks
the exact Heads and counters, appends any immutable Canonical or Artifact
Revision, writes the selected typed Receipt, writes the Author Action when the
result allocates one, advances normalized Heads, appends the required Project
Activity position/Event, records projection invalidations, and inserts the
outbox or wakeup intent. For an admitted command it also settles the
`AuthorCommandAdmission` with `ReceiptSettled` in that same write set. No row
from that set is published as committed before the transaction commits.

The physical mapping is closed as follows:

| Logical record | Catalog family | Required physical rule |
|---|---|---|
| Authoritative State, Authoritative Revision, Authoritative Commit, Head, and canonical payload | `project-canonical` | Immutable Revision/Payload and Commit rows plus one scoped current Head; Commit and counter sequence are gapless on commit |
| Artifact, Proposal, Draft, provenance, lifecycle source, and source Snapshot | `artifact-proposal-draft` | Immutable Artifact/Proposal/Draft revisions and typed lifecycle/provenance edges; no shared Head or payload identity with Canonical State |
| Domain, Validation, Acceptance, Undo Acceptance, and Author Undo Receipt | `operational-receipts-actions` | One typed immutable Receipt identity per first attempt; exact retry returns the same row and every allocation is explained by its result variant |
| Author Action and Author Undo Frontier evidence | `operational-receipts-actions` | One independent scoped Author Action sequence; successful author Proposal edits receive a Forward action, while Admission/Fence/refusal/conflict/no-effect evidence does not |
| Author Command Admission, terminal settlement, anti-forgery challenge, and idempotency | `operational-admission-editor` and `operational-receipts-actions` | Exact User, Project Scope, client/session/writer generation, action class, digest, target, nonce, lifetime, and one terminal settlement; `outcome_unknown` is nonterminal |
| Editor Session, current writer generation, Input Fence, and Proposal Pause Fence | `operational-admission-editor` | Scope-bound operational evidence; stale generations are fenced and no browser Local Edit Journal row becomes PostgreSQL authority |
| Application Wire Record | `operational-wire-history` | Store exact accepted schema-valid message-content bytes once with route, method, release, schema, content type, digest profile, idempotency/Command reference, and resulting identity; never store cookies, headers, nonces, secrets, malformed bodies, or query-response archives |
| Public Event wire representation | `operational-wire-history` | Store the first compact JSON representation with Event identity, Activity profile, schema, redaction profile, and representation digest; duplicate delivery is not another Wire Record |
| Project Activity Event and position | `operational-project-activity` | Append one immutable Event and scoped `project_activity_position`; bind replay generation, Event schema, typed Receipt reference, cause, resulting Heads, and redaction profile |
| Snapshot, cursor, replay floor, generation, and handoff evidence | `operational-snapshot-replay` and `operational-project-activity` | Snapshot is an authorized Server reading boundary; cursor is bound to Scope/requester/filter/profile/generation; old generation either has exact verified handoff or returns `activity_cursor_too_old` |
| Run Event, Mailbox, Transcript, Approval, Attempt, budget, lease, and outbox evidence | `operational-run-mailbox` and `operational-context-disclosure` | Immutable events and delivery evidence plus fenced live state; retention/compaction semantics remain owned by [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md) |
| Context Assembly Manifest, external-use binding, compatibility Decision, Destination Attempt, disclosure, and external wire projection | `operational-context-disclosure` | Manifest and exact non-secret wire projection commit before dispatch claim; OutcomeUnknown disclosure is durable before possible I/O; binding and Decision are separate records |
| Retrieval, embedding, context cache, and read-model rows | `projection-retrieval`, `projection-embedding`, `projection-context-cache`, `projection-read-model` | Disposable, scoped, dependency-complete, generation-bound, invalidatable, and rebuildable; none is canonical, historical Wire evidence, or sole content copy |
| Projection generation, invalidation, and watermark | `projection-generation-control` | Canonical transition appends invalidation atomically; a staged generation becomes visible only after dependency closure and watermark validation |
| Credential Reference and Project-use binding | `credential-references` | Opaque locator/reference, generation, status, and availability evidence only; a restore marks unresolved bindings `Unbound` and never carries secret material |
| Migration ledger, Recovery Copy, restore proof, Project Export manifest, and restore staging | `admin-migration`, `admin-recovery-copy`, and `admin-project-portability` | Maintenance-only roles and isolated staging; staging is never a live Project and proof is required before activation or visibility |

Every project-bearing row in these mappings repeats the exact Scope in its
primary/unique key and every project-bearing reference repeats Scope in a
`MATCH FULL` composite foreign key. A physical implementation may split one
catalog family into narrower tables, but it must preserve the family identity,
transaction group, immutable/head/projection property, and all catalogued
dependencies.

An external model, Tool, MCP server, embedding service, Keychain, filesystem,
or network is never part of the Core transaction. Before external I/O the
Context Assembly Manifest, non-secret Application Wire Record/Projection,
pending Destination Attempt, fenced dispatch claim, and OutcomeUnknown
Disclosure Event must be committed in the order already specified in section
5.6. A lost acknowledgement is reconciled from the typed Receipt, idempotency,
Activity, and Attempt records; it is never interpreted as a negative outcome.

## 5. Transactions, concurrency, and recovery cuts

### 5.1 Transaction boundary

Every domain command, Run transition, admission decision, budget settlement,
and egress preparation has one named transaction owner. The transaction writes
all canonical facts, current normalized state, sequence movement, immutable
Receipt, projection invalidation, and required outbox or wakeup intent, then
commits once. Success is published only after commit.

An external model, embedding, Tool, MCP, network, filesystem, Keychain, or
Provider call never occurs inside that transaction and never decides whether
it committed. A pre-commit crash exposes none of its rows. A post-commit lost
acknowledgement is resolved through the idempotency outcome and Receipt.

### 5.2 Isolation and lock order

Normal single-aggregate work uses `READ COMMITTED`, exact expected Heads,
unique constraints, and row locks. It does not depend on an earlier unlocked
read. A command locks only the guard rows needed for its invariant, in this
canonical order:

1. Project guard row;
2. idempotency arbiter row;
3. logical Head or Run-lane rows sorted by typed identity bytes;
4. scope and aggregate counter rows sorted by counter kind;
5. budget and grant rows sorted by typed identity;
6. outbox, wakeup, or worker-fence rows.

No correctness rule depends on transaction-local advisory locks. Advisory
locks may serialize the one migration runner, but cannot guard domain state.
`SKIP LOCKED` is permitted only for competing delivery workers after canonical
work exists; it is forbidden for author commands, eligibility, or authority.

When a cross-row predicate cannot be reduced to a unique/check/exclusion
constraint or an explicit locked guard row, that named transaction uses
`SERIALIZABLE`. Serialization failures and deadlocks retry the entire
transaction from trusted inputs with bounded backoff; a partial retry is
forbidden.

### 5.3 Gapless committed sequences

UUIDv7, audit time, and PostgreSQL sequence objects never supply domain order.
Each gapless-on-commit order uses a scoped `scope_counters` or
`aggregate_counters` row locked in the owning transaction. The row stores the
last committed value. The next value is checked for overflow and written only
in the transaction that writes the ordered fact. Rollback therefore consumes
no domain value. Independent orders use independent counter rows and never
imply causality across kinds.

### 5.4 Idempotency

`command_idempotency` contains scope, command kind, caller-visible
`idempotency_key`, canonical command digest and digest profile, outcome kind,
exact Receipt or result reference, and committed time. Its unique composite key
arbitrates concurrent first attempts.

- same key, kind, scope, and digest returns the immutable original outcome;
- same scope and kind with a different digest is a typed misuse and changes
  nothing;
- the same opaque key value in another Scope or command kind addresses an
  independent composite namespace and neither locates nor conflicts with this
  record;
- a crash before the admission transaction commits leaves no admission, nonce
  consumption, domain effect, or outcome;
- a crash after terminal Core settlement is recovered from the outcome without
  re-execution;
- idempotency never suppresses a new physical external retry, which receives a
  new attempt identity and disclosure evidence.

For an author-owned command, the admission transaction atomically claims the
pre-domain idempotency record, consumes its one-use nonce, and inserts the
immutable `author_command_admission`. The Core transaction atomically writes
the typed Receipt, command outcome, Project Activity intent, and the admission's
terminal Receipt settlement.

A crash after admission but before Core settlement leaves one inspectable
pending admission. Recovery first queries the idempotency and Receipt facts.
If no Receipt exists, only an unexpired `direct_editor_action` may resume the
exact admitted command automatically. An expired admission,
`explicit_editor_command`, or
`explicit_project_command` receives a terminal `requires_reconfirmation`
settlement; a later author confirmation uses a new idempotency record, nonce,
and admission.

### 5.5 Leases and fencing

Leases are durable scheduling permission, not execution truth. Each protected
Run lane, outbox item, wakeup, or long operation has a monotonically increasing
fence generation and an unguessable claim token. Claim or renewal uses database
time and a row lock. Every settlement, checkpoint, budget charge, and successor
claim supplies the exact current generation and token; stale workers can append
no result even if their process continued after expiry.

### 5.6 Outbox, egress, and crash cuts

Canonical transitions insert their outbox or wakeup intent atomically. Delivery
is at least once; semantic effects are deduplicated by the receiving command or
represented as distinct attempts, never inferred from queue deletion.

External egress obeys this order:

1. commit the exact Context Assembly Manifest and required semantic request;
2. prepare and commit the exact non-secret Wire Payload Projection and pending
   Destination Attempt;
3. in a short dispatch transaction, revalidate current scope, lifecycle,
   suppression, grant, budget, destination, and credential-reference
   availability, then acquire a fenced claim;
4. atomically append the OutcomeUnknown Outbound Disclosure Event and bind it
   to that claim;
5. commit the dispatch claim;
6. only then inject the ephemeral credential and permit external I/O;
7. settle confirmed or uncertain evidence in a new fenced transaction.

A crash before step 5 causes no external I/O and no Disclosure Event. A crash
after step 5 remains OutcomeUnknown even when bytes may not have left. A resend
is a new Destination Attempt and a new disclosure decision; it never rewrites
the predecessor. Queue claims, responses, sockets, and Provider logs are not
transaction authority.

## 6. Canonical payload placement

### 6.1 Envelope and payload separation

Hot identity, Head, lifecycle, routing, and state columns remain in narrow
aggregate tables. Canonical prose, Artifact content, Transcript content,
source snapshots, Run event bodies, manifests, and wire projections use
immutable payload tables in the same PostgreSQL transaction. Each owner stores
a scoped payload identity plus payload family, serialization schema, digest
profile, digest, byte length, and exact canonical bytes.

Payload foreign keys always include Project Scope and logical owner identity.
There is no cross-Project payload deduplication. Same-scope physical
deduplication may be added only when an immutable content-addressed owner and
reference accounting preserve deletion, export, and provenance semantics; it
is not a Foundation requirement.

### 6.2 Representation, compression, and size

Digest-exact serialized content is stored as `bytea`; value-semantic prose may
use `text` when its digest profile explicitly canonicalizes the text value.
Typed searchable envelope fields stay in ordinary columns. Opaque evolving
facts may use versioned `jsonb`, but a JSON object is never an unversioned
catch-all or the only place where scope, identity, lifecycle, ordering,
eligibility, or foreign references exist.

PostgreSQL TOAST owns initial compression and out-of-line storage. The
Foundation uses neither PostgreSQL Large Objects nor application-level payload
compression nor an external object store. Every payload family has one
versioned hard byte-limit profile enforced before and at insertion. Oversize
content returns a typed refusal and produces no partial row or silent external
storage path. Changing a limit or serialization/digest profile is a protocol
and migration change.

### 6.3 Immutability and erasure

Canonical payload rows are insert-only until a separately authorized retention
purge is due. Correction appends a new Revision or fact. Purge never mutates a
historical digest into a digest of replacement text; it leaves the required
Tombstone, lifecycle decision, provenance gap, and historical evidence defined
by the [operational retention contract](run-event-mailbox-snapshot-retention-and-archival-semantics.md).

## 7. Retrieval, cache, read-model, and embedding projections

### 7.1 Physical layout and authority

All project-derived retrieval and cache data remains in PostgreSQL under the
same Project Scope constraints and forced RLS. `retrieval_documents` and
`retrieval_fragments` bind exact source identity, source Revision, source
digest, fragment profile, qualification dependencies, and projection
generation. Lexical term rows and any physical indexes are disposable.

`embedding_projections` stores scope, exact source fragment, source digest,
embedding input profile and digest, exact Model Registration and capability
profile, one already-established Processing Destination Identity, the exact
Project Scope-bound model-use/Credential binding that pins it, and the separate
subsequent compatibility Decision, vector dimension,
observed vector values, generation, and status. A
Foundation implementation may use a PostgreSQL-native array and exact scan;
an optional later in-database vector extension and ANN index remain disposable
projection choices and require their own pinned migration and restore proof.
No Provider name or embedding model becomes canonical schema identity.

Requesting an external embedding is a destination operation that crosses the
full Context Assembly, Project Destination Grant, manifest, disclosure, and
attempt boundary before its observed vector can be stored. Projection rebuild
or cache maintenance grants no special disclosure authority.

Context caches and read models have explicit dependency rows. A cache key alone
is insufficient. No retrieval row, score, vector, cache, or read model is
authoritative, proves eligibility, or becomes the sole copy of content.

### 7.2 Qualification before ranking

Every retrieval use first reapplies current Project Scope and owning-domain
qualification: source Revision and digest, lifecycle and retention, Memory
Admission and applicable Memory Suppression, permission, Purpose, destination,
grant, Adapter, and policy. An unavailable or unverifiable dependency excludes
the candidate before similarity or lexical rank. Rank can choose only among
eligible rows and uses a stable tie-break over typed source and fragment IDs.

An RLS-safe index hit is still only discovery. The canonical source is joined
or batch-rechecked in the same scoped operation. A cache entry is reusable only
when every dependency is exact and currently qualified.

### 7.3 Freshness and invalidation

Every canonical transition that changes a projection dependency atomically
increments the applicable scoped projection epoch or appends a
`projection_invalidations` row. The invalidation becomes effective at the
canonical commit even when physical cleanup is delayed. Readers compare the
stored dependency closure and generation against current canonical facts; stale
or unknown rows are invisible.

Tombstone, Archive, Memory Suppression, Context Exclude, grant revocation,
retention expiry, source correction, and destination change propagate only to
the uses each semantic control governs. They are not collapsed into one delete
bit. Historical manifests and prior disclosure evidence remain unchanged while
current inspection may report that their dependencies are now invalid.

### 7.4 Rebuild boundary

Projection tables can be dropped and rebuilt from canonical records, exact
source versions, and versioned projection jobs. Rebuild creates a new
generation in staging, validates scope and dependency closure, and switches a
scoped generation pointer atomically. Partial generations never serve reads.

Externally produced embeddings are observations and may not reproduce the same
floating-point bytes later. Rebuild determinism therefore means deterministic
source enumeration, qualification, input projection, job manifest, and stable
ranking within one fixed generation; it does not fabricate byte equality from
a Provider. If the exact embedding route is unavailable, the generation stays
Unavailable and retrieval falls back only to an independently admitted mode.

### 7.5 Release 1 generation, watermark, and recovery visibility contract

Every disposable or historical projection family carries the following
identity, whether the physical implementation stores the fields in one row or
in a typed side relation:

```text
ProjectionIdentity {
  owner_user_id, project_id
  projection_family
  projection_profile_revision
  projection_generation
  source_watermark_kind
  source_watermark
  dependency_closure_digest
  state: building | ready | unavailable | invalidated
}
```

The dependency closure includes every source identity and exact Revision or
payload digest, Project Scope, lifecycle/retention and suppression facts,
authorization/grant and destination facts when applicable, external
Registration/Adapter/Model profiles when applicable, and the canonical input
projection. A cache key, Event position, or source ID without this closure is
not a valid projection row. `projection_generation` is scoped and monotonic;
the `source_watermark` is the exact canonical Commit or Project Activity
position that the generation has processed. It is not a wall-clock timestamp,
UUID order, or browser local sequence.

The owning canonical transaction appends an invalidation or advances the
scoped generation epoch before commit. Readers compare the full closure,
generation, watermark, and current lifecycle eligibility. An invalidated,
unknown, stale, cross-scope, or partially populated row is invisible even if
its index returns a hit. Historical Application Wire Records and public Event
representations are immutable evidence and are never silently invalidated or
rewritten; a current view may report that their source is no longer eligible.

A rebuild follows one positive sequence:

1. fence the old generation for writes and mark the new generation `building`;
2. enumerate canonical and historical sources in a deterministic Scope-bound
   order, reapply current qualification, and record the job manifest;
3. write rows only into the staged generation with complete dependency closure;
4. validate every scope, foreign reference, digest, invalidation, projection
   profile, index, and watermark postcondition;
5. mark the generation `ready` and atomically advance the scoped generation
   pointer; and
6. drop or quarantine the old generation and invalid indexes only after the
   new pointer and recovery evidence are durable.

An interrupted build is resumable from its last verified batch or discarded
without changing canonical state. An invalid index is dropped and rebuilt; it
is never treated as a valid empty result. A missing external embedding route
marks that generation `unavailable`, and the read path uses only an explicitly
admitted fallback. No partial generation serves a Snapshot or query.

After a whole-service or Project Restore, traffic remains fenced until all of
the following are true: canonical facts, typed Receipts, Heads, sequences,
idempotency, Activity positions, and lifecycle records validate; each required
projection is `ready` at or beyond the required watermark or has a typed
`unavailable` fallback; a fresh authorized Snapshot/resync boundary is
available; the replay floor and cursor generation are coherent; and the
Recovery Visibility Proof demonstrates that later Redaction, Tombstone,
Archive, Suppression, retention, and availability facts have been applied. A
database that is physically restorable but fails any lifecycle or projection
proof remains in `recovery_hold` or `read_only`; it is not exposed as current.

## 8. Credentials, lifecycle, and retention handoff

### 8.1 Credential Reference persistence

`credential_references` and project-use bindings, including model-use bindings,
store only opaque reference
identity, backend kind and namespace, non-secret locator, generation, status,
availability evidence, and exact authorized Project Scope. They never store a
secret value, value digest, decrypting key, authorization header, or
credential-bearing transport bytes.

The Foundation-local resolver uses macOS Keychain. Environment variables are
development/test inputs only. A later controlled-cloud resolver uses a managed
secret service through the same backend-neutral contract without changing
Registration, attempt, or Project semantics. Secret resolution occurs only at
the narrow execution boundary after durable admission and is redacted from
errors, tracing, panic reports, support bundles, backups, and exports.

A Project Restore preserves reference identity and non-secret metadata but
marks any unresolved binding Unbound. Only an authorized explicit rebind may
make it available; matching a locator string never silently binds a secret.

### 8.2 Archive, Tombstone, Suppression, and purge

Archive changes ordinary visibility while retaining canonical content and
history. Tombstone records that a source is no longer live and controls future
use according to its owning domain. Memory Suppression prevents memory-derived
or ordinary-recall use without deleting or rewriting the raw source. Context
Exclude applies to its exact operation requirement. These states have distinct
tables or typed lifecycle events and distinct qualification predicates.

The owning transition writes the lifecycle fact and projection invalidation in
one transaction. Physical cache and index deletion may lag; eligibility may
not. Downstream retention policy supplies durations and purge eligibility. A
purge worker uses a fenced, idempotent decision, deletes only the payloads the
policy authorizes, and preserves required Tombstones, immutable decisions,
disclosure history, and explicit provenance gaps. A later purge cannot rewrite
what an earlier Run considered, selected, prepared, or may have disclosed.

Project Export includes active Archive, Tombstone, Suppression, lifecycle,
retention, and provenance facts needed to reproduce current eligibility. It
does not resurrect purged payload or pretend a known gap is complete.

## 9. Schema migration and compatibility

### 9.1 Migration ledger and runner

`schema_migrations` records a monotonic schema version, immutable migration ID,
checksum, required StoryOS release identity, runner version, start
and finish evidence, and final status. `migration_phases` records each
transactional or nontransactional phase and its verified postcondition. The
migrator acquires one database-level advisory lock only to exclude another
migrator, verifies the entire applied checksum chain, and refuses drift.

Transactional DDL commits atomically. Operations such as concurrent index
builds run as explicit resumable phases, detect and remove invalid remnants,
and never mark the migration complete until postconditions pass. Every
backfill is scoped, bounded, idempotent, restartable, and followed by constraint
validation. Migration code never disables Project constraints to make runtime
writes succeed.

### 9.2 Prepare, migrate, activate, verify

Every storage release uses an exclusive controlled activation:

1. stop new author and Worker writes and settle or fence in-flight work;
2. create and verify the required Recovery Copy;
3. apply additive structures and bounded, resumable, Scope-checked backfills;
4. validate constraints, indexes, digests, canonical equivalence, and restore
   preconditions;
5. activate the one matching Server, Worker, Web Client, generated contract,
   and schema release;
6. run post-activation contract and author-journey verification; and
7. contract obsolete structures only after the recovery point and verification
   evidence are durable.

Runtime startup and write admission require the exact active schema release.
Recovery uses the verified Recovery Copy, PITR, or a forward repair. Migration
history, historical payload schema identities, Receipts, Events, and archive
profiles remain interpretable without running an older application binary.

### 9.3 Release 1 migration chain and phase contract

The Release 1 chain is catalogued as
`storyos.persistence.migrations.release-0-to-release-1.v1`. It has one
supported stored predecessor, `storyos.persistence.release-0.v1`, and one
active terminal, `storyos.persistence.release-1.v1`. A fresh database uses the
same ordered phases after schema initialization. No predecessor may skip the
chain by presenting a matching public semantic version.

The migration runner acquires the one database-level advisory lock only for
migrator exclusion, records the lock owner and runner revision, and verifies
the complete immutable checksum chain before reading or writing schema state.
`storyos_runtime` is never granted migration authority. Every phase records
its phase class, transaction boundary, input schema, batch/cursor checkpoint,
input and output checksums, postcondition, attempt number, and terminal
disposition in `migration_phases`.

| Phase | Boundary | Positive postcondition | Restart and failure rule |
|---|---|---|---|
| `preflight` | Read-only control transaction | Supported schema, exact catalog/release identity, complete prior checksum chain, role/RLS posture, and no conflicting runner | Idempotent; mismatch enters `failed_unavailable` or refuses writes without touching domain rows |
| `recovery_copy` | Nontransactional backup/restore boundary | Recovery Copy manifest, chain/gap check, and isolated restore/read proof are durable | Retry the same copy identity; missing or unverifiable chain leaves service `read_only` |
| `expand_transaction` | Transactional DDL | Additive columns, tables, constraints, and indexes exist without weakening old constraints/RLS | Transaction rollback leaves a usable predecessor; a committed phase is replay-safe by checksum |
| `backfill_nontransactional` | Bounded batch transactions | Every Scope-bound row has the new representation and batch checksum; progress is resumable | Re-run only incomplete or checksum-matching batches; mismatch quarantines the batch and leaves `read_only` or restores the last usable copy |
| `validate_transaction` | Transactional validation | Composite references, payload digests/limits, Heads, counters, indexes, invalid-index scan, and historical readers pass | Failure never activates; invalid indexes are dropped/rebuilt or the service remains `read_only` |
| `activate` | Fenced controlled cutover | One exact storage/protocol identity is active and all write/visibility gates are true | Any identity, lifecycle, projection, or restore-proof mismatch enters `recovery_hold` or `failed_unavailable` |
| `contract_cleanup` | Transactional DDL after proof | Obsolete structures are removed only after the new recovery point and post-activation proof are durable | Idempotent retry; failure leaves the active contract usable but `read_only` until cleanup is verified |

Transactional phases either commit their complete DDL/postcondition or roll
back. Nontransactional backup, concurrent-index, and bounded-backfill phases
never pretend to be atomic: each checkpoint is independently checksummed and
the runner must choose a verified predecessor, verified intermediate checkpoint,
or isolated Recovery Copy before continuing. A phase with an invalid index,
missing batch, checksum drift, unsupported payload, or uncertain source is not
marked complete. Migration never disables forced RLS, composite foreign keys,
payload constraints, or scope predicates to make a backfill succeed.

### 9.4 Positive activation state machine

The only positive path is:

```text
NewInstallRequired or PredecessorPresent
  -> Preflight
  -> RecoveryCopyRequired
  -> RecoveryCopyVerified
  -> Migrating
  -> Verifying
  -> Activating
  -> Active
  -> ContractCleanupPending (optional deferred cleanup)
  -> Active
```

The runner may enter `Active` only after the exact gates in section 7.5 and the
catalog's `active_write_gates` pass. `Preflight` checks the server-declared
database identity, persisted-format catalog, migration-chain digest, route
catalog digest, same-release protocol identity, supported predecessor, role
posture, forced-RLS policy inventory, and current migration ledger. It also
proves that all in-flight author/Worker writes are settled or fenced before the
Recovery Copy boundary.

Failure is positive and inspectable, not an implicit rollback guess:

| Boundary | State | Allowed behavior |
|---|---|---|
| Before Recovery Copy verification | `failed_read_only` or `failed_unavailable` | No new author/Worker writes; retry only the same controlled preflight/copy or restore a verified predecessor |
| Transactional phase rollback | predecessor remains usable | Keep the predecessor active only as a read-only controlled recovery surface; do not advertise the new public release |
| Nontransactional phase interruption | checkpoint is complete and verified | Resume the exact idempotent batch; otherwise restore the last complete checkpoint/Recovery Copy |
| Constraint, digest, backfill, or invalid-index validation failure | `failed_read_only` | Drop/rebuild the invalid index or repair through the named migration; no guessed repair or partial activation |
| Restore chain/lifecycle range gap | `recovery_hold` | Keep Project unavailable or read-only until Recovery Visibility Proof is complete |
| Same-release identity mismatch | `failed_unavailable` | Return `upgrade_required` before domain attempt, cursor movement, external effect, or live traffic |
| Post-activation cleanup failure | `failed_read_only` | Keep the active verified schema but defer cleanup; never delete the only rollback/recovery path |

`storyos_runtime` starts or admits writes only from `Active` with a current
Recovery Visibility Proof. A runtime process cannot infer readiness from a
matching schema version, a responsive database, a complete HTTP health check,
or a projection row. The activation record binds the exact Server, Worker,
protected Web Client, generated public contract, route/Event catalog, storage
catalog, and migration ledger identities.

## 10. Backup, restore, export, and deployment migration

### 10.1 Foundation Recovery Service Profile

PostgreSQL acknowledges an author-visible successful commit only with
synchronous durability enabled. Ordinary process or power crash recovery has
zero acknowledged-data loss. Loss of the Foundation database host or disk has
an RPO of at most fifteen minutes and an RTO of at most two hours.

The Foundation keeps `fsync`, `full_page_writes`, and synchronous commit
durability enabled. It takes a daily physical base backup and archives WAL into
a failure domain independent of the database host with a tested emission,
transfer, and gap-detection interval that keeps the latest recoverable point no
more than fifteen minutes behind. The chain includes
the non-secret PostgreSQL configuration and role/grant manifest that physical
WAL backup does not supply, while deployment credentials and Keychain material
remain separate. Every release candidate must restore an isolated instance,
replay to a chosen point, run schema and invariant validation, and record a
successful `restore_proof`. A backup file without a completed restore proof is
not recovery evidence.

Retention duration belongs to the [operational retention contract](run-event-mailbox-snapshot-retention-and-archival-semantics.md). Whatever
window it declares must keep at least one complete base-backup/WAL chain and
must be continuously checked for gaps. The Foundation does not require a
synchronous replica, automatic failover, or high-availability cluster. A later
cloud deployment may tighten, but never weaken without declaring, its own
service profile.

### 10.2 Whole-service restore and major upgrades

Physical backup plus WAL/PITR restores one service failure domain. Logical
`pg_dump`/`pg_restore` is a separately tested path for whole-service
portability and supported major-version upgrades; `pg_upgrade` may be admitted
only with a successful compatibility check and rollback recovery point. A
selected-table dump is never a Project export.

Restore occurs in isolation with runtime traffic disabled. It restores schema,
constraints, canonical data, operational evidence, and projection metadata;
validates migration checksums, scoped referential closure, payload digests,
Heads, sequences, idempotency, manifests, and outbox fences; rebuilds or drops
disposable projections as required; rotates runtime and maintenance
credentials; then enables traffic only after the operational contract's
Recovery Visibility Proof succeeds. OutcomeUnknown work remains uncertain and
is reconciled, never silently replayed.

### 10.3 Project Export Archive and Project Restore

A Project Export Archive is produced from one `REPEATABLE READ`, read-only
transaction for one exact Project Scope. Its signed or integrity-protected
manifest records
archive format, source schema compatibility, Project Scope, table-family
counts, serialization and digest profiles, every included object and payload
digest, provenance closure, and known purged gaps. It includes all non-secret
canonical Authoritative State, Artifacts, Operational Records, histories,
lifecycle and retention facts, Credential References, and required payloads.
It excludes secret values, global reusable definitions that can be referenced
by stable version and supplied separately, and all disposable projections,
caches, embeddings, and read models.

Project Restore is exact-scope restoration, not copy. The target must authorize
the same durable User identity and contain no row for the Project Scope. The
archive loads into maintenance-only staging, validates format and schema
compatibility, integrity, complete scoped referential closure, payload and
manifest digests, global-definition availability, and absence of conflict,
then becomes visible through one atomic promotion. Any failure removes or
quarantines staging and changes no live Project. Projections rebuild afterward;
Credential References that cannot resolve become Unbound.

Restore never merges, overwrites, remaps IDs, changes owner, or creates a new
Project. Copy, fork, ownership transfer, and collaboration require later
domain commands.

### 10.4 Local-to-cloud migration

Local and cloud deployments use the same schema, Project Scope, RLS,
transaction, and migration contract. Migration chooses either a whole-service
restore or exact Project Restore. Cutover stops local writes, captures and
verifies the final recovery boundary, restores into an isolated target,
rebinds deployment credentials, validates every Project and projection gate,
then enables the target. The source remains read-only until acceptance and is
retired only under the retention contract. There is no dual write, implicit
ownership transfer, Provider migration dependency, or cross-deployment secret
copy.

### 10.5 Recovery evidence, lifecycle proof, and measurement boundaries

The Foundation Recovery Service Profile is retained exactly: synchronous
PostgreSQL durability gives zero acknowledged-data loss for an ordinary process
or power crash; host/disk loss has declared RPO at most fifteen minutes and RTO
at most two hours; the profile uses a physical base backup plus WAL archive,
independent failure-domain storage, and an isolated restore proof. Those values
are storage contract requirements, not claims made by a logical dump test.

The [Representative Writing-Path Performance and Storage-Growth Envelope](../research/representative-writing-path-performance-and-storage-growth-envelope.md)
is calibration evidence only. Its sampled logical dump/restore, WAL deltas,
relation sizes, compaction timings, and modelled annual bytes do not prove
physical base-backup/WAL continuity, host-loss recovery, lifecycle proof,
RPO/RTO, or a deployment capacity limit. They cannot set a payload hard limit,
retention duration, checkpoint cadence, compaction policy, or public timeout.
Any future capacity recommendation must replace the synthetic corpus with the
exact Release 1 schema/DTO cardinality and explicitly include payload tables,
indexes, WAL, backups, bloat, archives, projections, and recovery-chain
headroom; it remains non-normative until a named owner accepts it.

`RecoveryVisibilityProof` is a durable maintenance record, not a health-check
boolean. It binds the Recovery Copy/restore point, migration and catalog
identity, verified lifecycle/redaction range, canonical fact and History
digests, Project Activity/replay generation, projection generation/watermark,
credential-reference availability, and the exact checks performed. A missing
WAL segment, lifecycle range, invalidated projection, or unresolved integrity
check keeps the affected Project in `recovery_hold` and the service
`read_only`/`unavailable`; it never exposes an older recovered view as current.

Physical backup/WAL restores preserve all non-secret database facts and
metadata required by the profile. Logical whole-service migration preserves
identities, schema histories, Receipts, Events, Application Wire Records,
Heads, sequences, idempotency, lifecycle, manifests, and `OutcomeUnknown`
uncertainty, then rebuilds disposable projections. Project Export remains the
exact-scope portable archive described in 10.3, not a selected-table dump.
Credential References and non-secret binding metadata may be restored, but
secret values, value digests, decrypting keys, authorization headers, and
credential-bearing transport bytes are excluded from rows, backups, exports,
logs, tracing, support bundles, and diagnostic material. An unresolved
reference is explicitly `Unbound` until an authorized rebind.

## 11. Required proof gates

Downstream implementation is not Foundation-ready until automated evidence
covers at least:

1. direct SQL, application, join, foreign-key, cache, retrieval, embedding,
   idempotency, outbox, and restore attempts across either scope member all
   fail closed;
2. runtime role posture proves non-owner, non-superuser, `NOBYPASSRLS`, forced
   RLS, transaction-local scope, and no pooled-scope leakage;
3. every Core and Run transition crash cut exposes either the complete Receipt,
   sequence, Heads, events, invalidations, and outbox intent or none of them;
4. concurrent identical retries in one composite namespace produce one
   immutable outcome; a changed digest in that namespace and an unauthorized
   cross-Scope substitution are refused, while an independently authorized
   command in another Scope uses its own namespace;
5. rolled-back and conflicting transitions consume no committed-domain order,
   and counter overflow fails closed;
6. stale lease generations cannot settle work, charge budget, append output,
   or suppress a current worker;
7. failure to commit a Context Assembly Manifest or dispatch claim produces
   zero egress, while every post-claim crash preserves an OutcomeUnknown
   Disclosure Event before possible I/O;
8. external retries create new attempts and evidence without rewriting or
   double-settling the predecessor;
9. payload-family limits, canonical digests, immutable payload constraints,
   TOAST-backed round trips, and oversize atomic refusal pass;
10. dumps, backups, Project exports, ordinary logs, tracing, support bundles,
    wire projections, and failure messages contain no credential value or
    value digest;
11. Tombstone, Archive, Suppression, Exclude, grant revocation, source change,
    and retention expiry immediately invalidate exactly their governed future
    uses while preserving historical manifests and disclosure evidence;
12. deleting every disposable projection and rebuilding produces complete
    scoped dependency closure, stable source enumeration and fixed-generation
    ranking, without treating refreshed embedding bytes as canonical;
13. a migration survives a crash in every transactional and nontransactional
    phase, detects checksum drift and invalid indexes, validates backfills, and
    enforces exact active StoryOS release identity;
14. physical base backup plus WAL restores selected crash points with zero
    acknowledged loss for ordinary crashes and proves the declared disaster
    RPO and RTO in an isolated environment;
15. logical whole-service migration and a Project Export/Restore round trip
    preserve exact identities, histories, payload digests, sequences,
    idempotency outcomes, lifecycle, provenance, manifests, and uncertainty;
16. Project Restore refuses an existing Scope, wrong User, missing object,
    digest mismatch, unsupported schema, secret-bearing archive, and partial
    graph without exposing staging;
17. local-to-cloud cutover preserves Project Isolation, leaves unresolved
    credentials Unbound, rebuilds projections, and performs no dual write;
18. database corruption, failed invariant validation, missing recovery chain,
    or unsupported contract keeps the service read-only or unavailable rather
    than guessing or repairing history.
19. the Release 1 persistence catalog parses as UTF-8/LF JSON, has one identity
    per family/table/schema/migration edge, proves one predecessor path, and
    resolves every declared Markdown owner and anchor;
20. every project-bearing catalog family has the complete Project Scope,
    forced-RLS, runtime-grant, and `MATCH FULL` composite-reference profile,
    while global and maintenance families are explicitly unscoped or isolated;
21. Canonical, Artifact, Operational, projection, and maintenance families
    have disjoint physical table ownership; typed Receipt kinds, Author Action,
    Admission, Application Wire Record, Project Activity, Snapshot/cursor,
    replay, and disposable projection rows are not silently merged;
22. every Release 1 route with a persistence-bearing owner is mechanically
    covered by a catalog family, every settlement query is the exact named GET
    operation, every success/no-effect Activity Event is catalogued, every
    public Event has an Activity owner, and wire-history coverage is explicit;
23. every disposable projection names canonical dependencies, a generation,
    source watermark, invalidation set, rebuild rule, and recovery visibility
    gate; deletion and rebuild cannot expose a partial generation;
24. backup, whole-service restore, Project Export, Project Restore, projection
    rebuild, and secret-exclusion classifications are mutually consistent;
    a negative self-test proves that a duplicate identity or broken owner
    anchor is rejected;
25. the catalog's protocol route-catalog digest and migration-chain digest
    agree with the checked-in Release 1 inputs, so a same-release activation
    cannot skip storage migration by reusing a public protocol identity.

## 12. Normative invariants and handoff

1. PostgreSQL is the sole authoritative physical database from Foundation
   Validation onward.
2. Every project-bearing canonical or disposable row and every project-bearing
   reference contains the exact Project Scope.
3. Composite constraints and forced RLS independently reject cross-scope data;
   caller filtering and UUID uniqueness are insufficient.
4. Authoritative State, Artifacts, and Operational Records remain disjoint
   durable spaces even when one transaction spans them.
5. A successful transition, its Receipt, committed sequence, invalidation, and
   outbox intent are one atomic fact.
6. External I/O never occurs before the required durable manifest and dispatch
   evidence, and never decides transaction success.
7. Committed domain order is transactionally gapless and never derived from a
   native sequence, UUID, or clock.
8. Canonical payloads remain in PostgreSQL under hard limits; a projection,
   Provider, cache, or object store is never their sole copy.
9. Retrieval eligibility precedes ranking, and every disposable row remains
   scoped, dependency-complete, invalidatable, and rebuildable.
10. Secret material never enters ordinary domain persistence or portable data;
    Credential References fail closed and rebind explicitly.
11. Lifecycle changes invalidate future use without rewriting historical
    evidence; retention purge preserves required Tombstones and provenance
    gaps.
12. Migration release identity is declared and verified; schema drift and
    partial phase completion block startup or writes.
13. Backup success means a verified restore, and Project import means exact
    restoration without merge, remap, overwrite, or ownership transfer.
14. Local and cloud deployments preserve the same Project Isolation and
    persistence semantics.
15. Release 1 activation binds the exact database schema, persisted-format
    catalog, migration chain/digest, public route catalog, and same-release
    identity; matching semantic versions cannot bypass migration or restore
    proof.
16. New installation and every supported predecessor use one positive,
    checksummed migration path; transactional and nontransactional phase
    failure leaves a verified predecessor, Recovery Copy, read-only state, or
    unavailable state rather than guessed history.
17. Typed Receipts, Author Actions, Admissions, Application Wire Records,
    Project Activity positions, Snapshot/replay evidence, and lifecycle facts
    are immutable or explicitly versioned Operational Records with their own
    identities; no browser projection, Event arrival, or query response is a
    substitute for one of them.
18. A disposable projection is visible only with complete Scope-bound
    dependency closure, current generation, source watermark, and valid
    invalidation state; a restored Project requires a fresh Snapshot/resync and
    Recovery Visibility Proof before live traffic.
19. The machine-readable persistence catalog and its verifier are review-time
    contract proof, not production DDL or executable recovery. Their derived
    statistics never become hard limits, retention defaults, or RPO/RTO claims.

The versioned command/query/event protocol owns exact wire DTOs and byte-limit
values while preserving every key, scope, digest, and compatibility field in
this contract. The deterministic verification ticket owns executable crash,
concurrency, isolation, migration, export, and recovery harnesses. The
retention ticket owns durations and final compaction policy without weakening
historical evidence or immediate invalidation. The first editor-first
implementation stage begins only after these proof gates are represented in
its issue.

### 12.1 Explicit owner boundary

This contract owns physical persistence and activation prerequisites only. The
public protocol owns route, DTO, Event, wire-schema, and public compatibility
meaning; Core and Artifact contracts own creative authority, Proposal, Draft,
Receipt allocation, and Author Action meaning; the Editor Session contract owns
browser journal/projection continuity; [Run Event, Mailbox, Snapshot,
Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md)
owns retention duration, checkpoint policy, compaction, archival, tombstone
and deletion settlement; and [Define Deterministic Verification and
Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) owns
executable fault schedules and acceptance gates. The performance report remains
research input only. None of those owners may weaken the storage identity,
Scope/RLS, immutable history, Recovery Visibility Proof, or secret exclusion
defined here, and this contract does not decide their unowned semantics.

### 12.2 Requirement traceability

| Requirement | Contract closure | Catalog/verifier proof |
|---|---|---|
| PGS-REL-001 | Sections 0, 9.3–9.4, invariant 15, and the activation gates | `schema_identity`, protocol binding, route/migration digest checks |
| PGS-MIG-002 | Sections 9.1–9.4 and failure-state table | `migration_chain`, `state_machine`, unique predecessor-path check |
| PGS-MIG-003 | Sections 9.1 and 9.3 | phase class, transaction, restart/idempotency/postcondition checks |
| PGS-ISO-004 | Sections 2 and 4 plus the catalog role/scope profiles | role profiles, scope profiles, forced-RLS, grant, and composite-reference checks |
| PGS-OWN-005 | Sections 3.2–3.3 and 4.5 | unique table-family ownership and public logical-record mapping |
| PGS-PROJ-006 | Sections 7.3–7.5 and 10.5 | generation/dependency/watermark/invalidation/rebuild/visibility checks |
| PGS-REC-007 | Sections 8–10.5 | portability and secret-exclusion classification checks |
| PGS-BOUND-008 | Sections 10.1, 10.5, and 12.1 | owner anchors plus explicit non-normative measurement/retention boundaries |
| PGS-VERIFY-009 | Sections 0, 11.19–11.25, and 12.2 | positive verifier, route/Event cross-check, Markdown links, and negative self-test |
