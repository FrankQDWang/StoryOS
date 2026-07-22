# PostgreSQL Project Storage, Isolation, and Migration Contract

- Status: accepted
- Wayfinder resolution: [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56)
- Canonical glossary: [`CONTEXT.md`](../../CONTEXT.md)
- Deployment decision: [ADR 0004: Adopt a PostgreSQL Service and Project Isolation Boundary](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)
- Research input: [PostgreSQL Project Storage, Isolation, and Migration Source Audit](../research/postgresql-project-storage-isolation-and-migration-source-audit.md)
- Parent semantic contracts: [Artifact domain model](artifact-domain-model.md), [Manuscript state machine](manuscript-revision-proposal-state-machine.md), [Fiction memory and research provenance](fiction-memory-and-research-provenance-semantics.md), and [Context assembly and disclosure](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Operational lifecycle contract: [Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](run-event-mailbox-snapshot-retention-and-archival-semantics.md)

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
| Authoritative State | `authoritative_objects`, `authoritative_revisions`, `authoritative_payloads`, `authoritative_heads`, `authoritative_commits`, `authoritative_commit_members`, `author_action_entries` | immutable revisions and commits; normalized Heads are the current authority pointer |
| Manuscript subtype | `manuscript_objects`, `manuscript_blocks`, `manuscript_revision_members`, `revision_anchors` | subtype rows reference Authoritative identities and exact revisions in the same Scope |
| Artifact | `artifacts`, `artifact_revisions`, `artifact_payloads`, `artifact_heads`, `artifact_lifecycle_events`, `artifact_provenance_edges` | never shares a Head or payload identity with Authoritative State |
| Proposal | `proposals`, `proposal_revisions`, `proposal_heads`, `proposal_operations`, `proposal_operation_resolutions`, `proposal_generations`, `proposal_stream_events`, `proposal_pause_fences`, `validation_receipts`, `acceptance_receipts`, `undo_receipts` | Proposal state axes and receipts remain immutable evidence; current Head is normalized separately |
| Domain command evidence | `domain_receipts`, `domain_events`, `command_idempotency`, `scope_counters`, `aggregate_counters` | one committed outcome per command key and transactional committed-domain order |
| Agent execution | `agent_runs`, `subruns`, `subrun_joins`, `run_grant_revisions`, `subrun_capability_grants`, `run_plans`, `run_steps`, `run_lanes`, `run_events`, `run_mailbox_messages`, `run_mailbox_deliveries`, `run_waits`, `wait_resolutions`, `run_holds`, `run_wakeups`, `run_leases`, `run_execution_attempts`, `run_budget_accounts`, `run_budget_reservations`, `usage_records` | normalized live state is backed by immutable events and receipts; mailbox delivery is durable and leases and budget claims are fenced |
| Transcript and approval | `transcript_items`, `approval_waits`, `approval_decisions`, `steering_inputs`, `run_checkpoints` | Transcript items and decisions are canonical Operational Records; checkpoints are disposable projections |
| Tool, MCP, and Skill | `tool_specs`, `tool_registration_revisions`, `tool_registration_heads`, `tool_registration_status_events`, `tool_calls`, `tool_attempts`, `mcp_server_registration_revisions`, `mcp_server_registration_heads`, `mcp_app_artifacts`, `skill_package_revisions`, `skill_package_heads`, `skill_activations` | reusable non-project definitions are unscoped only when content-free of project data; calls, activations, Apps, and status evidence are scoped |
| Model Gateway | `model_registration_revisions`, `model_registration_heads`, `model_registration_status_events`, `model_capability_profiles`, `model_operational_snapshots`, `model_routing_policy_revisions`, `model_route_requests`, `model_route_decisions`, `model_invocations`, `model_attempts`, `model_usage_settlements` | Registration/capability contracts are unscoped only while free of Project data, Credential References, actual endpoint/account or disclosure destinations, and authority; the model surface uses the shared scoped external-use-binding shape to pin Project Scope, use/Credential authorization, one already-established Processing Destination Identity and exact evidence revision, and bounds, while a separate later compatibility Decision evaluates that binding; every snapshot, route decision, invocation, attempt, and fallback pins both records; Provider-neutral columns and versioned Adapter payload add no Provider-specific authority table |
| Memory and research | `memory_candidates`, `memory_admissions`, `memory_suppressions`, `research_claims`, `research_evidence_edges`, `source_snapshots` | source Revisions and provenance remain canonical; retrieval projections remain separate |
| Context and disclosure | `operation_requirements`, `context_candidates`, `context_eligibility_decisions`, `context_selection_decisions`, `context_projections`, `context_assembly_manifests`, `context_manifest_members`, `project_external_use_binding_revisions`, `external_contract_compatibility_decisions`, `destination_attempts`, `wire_payload_projections`, `outbound_disclosure_events`, `destination_attempt_settlements` | Model, Tool, MCP, Provider, and research surfaces share one scoped external-use-binding shape that pins an already-existing Processing Destination Identity, exact evidence revision, and owning authorization; a compatibility Decision follows and references one already-existing binding; the Manifest and exact non-secret wire evidence commit before external dispatch claim |
| Durable work delivery | `outbox_entries`, `worker_fences` | outbox intent commits with its owning transition; a worker generation fences all settlements |
| Disposable retrieval | `retrieval_documents`, `retrieval_fragments`, `retrieval_terms`, `embedding_projections`, `projection_dependencies`, `projection_invalidations`, `projection_generations`, `context_cache_entries`, `context_cache_dependencies`, `read_model_checkpoints` | all rows are scoped, dependency-complete, immediately disqualifiable, and rebuildable |
| Storage administration | `schema_migrations`, `migration_phases`, `restore_proofs`, `project_export_manifests`, `project_restore_staging` | maintenance-only metadata; staging is never visible as a live Project |

Tables whose logical facts already belong to an Artifact or Operational Record
may use a typed subtype relation referencing that owner rather than duplicating
payload. The one-to-one relationship and same-space ownership must be enforced
by a scoped foreign key. Globally reusable ToolSpecs, schemas, mapping profiles,
and adapter definitions use content-addressed or versioned global identities;
their project enablement, use, evidence, and cached effects remain scoped.

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
- same key with a different digest, kind, or scope is a typed misuse and
  changes nothing;
- a crash before commit leaves neither domain effect nor outcome;
- a crash after commit is recovered from the outcome without re-execution;
- idempotency never suppresses a new physical external retry, which receives a
  new attempt identity and disclosure evidence.

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
checksum, required application compatibility interval, runner version, start
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

### 9.2 Expand, migrate, switch, contract

Breaking storage changes use these phases:

1. expand with additive nullable structures or `NOT VALID` constraints while
   enforcing the new rule for new writes where PostgreSQL permits;
2. backfill in bounded resumable batches with scope and digest checks;
3. validate constraints, indexes, and canonical equivalence;
4. switch readers and writers only after the declared binary and schema
   compatibility gates pass;
5. contract obsolete structures in a later release after no supported binary
   reads or writes them and a recovery point exists.

The declared rolling compatibility window is current release `N` and immediate
predecessor `N-1` only, and only during an upgrade. A binary declares minimum
read schema, minimum write schema, and maximum understood schema. Startup and
write admission fail when the database lies outside that interval. The local
Foundation deployment may use an exclusive maintenance window and run only
`N`, but it follows the same phased schema contract so cloud deployment does
not require a different data model.

Rollback within the window means returning to a compatible binary and schema
phase. Once new incompatible writes or contract DDL occur, recovery uses a
verified backup/PITR path or a forward repair; destructive down-migrations are
not assumed safe.

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
4. concurrent identical command retries produce one immutable outcome, while a
   changed digest or Scope is refused;
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
    enforces the `N`/`N-1` window;
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
12. Migration compatibility is declared and verified; schema drift and partial
    phase completion block startup or writes.
13. Backup success means a verified restore, and Project import means exact
    restoration without merge, remap, overwrite, or ownership transfer.
14. Local and cloud deployments preserve the same Project Isolation and
    persistence semantics.

The versioned command/query/event protocol owns exact wire DTOs and byte-limit
values while preserving every key, scope, digest, and compatibility field in
this contract. The deterministic verification ticket owns executable crash,
concurrency, isolation, migration, export, and recovery harnesses. The
retention ticket owns durations and final compaction policy without weakening
historical evidence or immediate invalidation. The first production slice owns
implementation only after these proof gates are represented in its plan.
