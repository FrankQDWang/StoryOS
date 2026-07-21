# PostgreSQL Project Storage, Isolation, and Migration Source Audit

- Status: accepted evidence for [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md); sections explicitly marked as confirmed decisions record the ticket's HITL outcomes, while technical observations remain research evidence
- Audited: 2026-07-21
- Repository baseline: `76248569176974a9822187e70ff755feea13ab51`
- Database documentation baseline: PostgreSQL 18 current documentation
- Canonical domain source: [`CONTEXT.md`](../../CONTEXT.md)
- Accepted architectural boundary: [ADR 0004](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)

## 1. Audit boundary

This note separates three kinds of statement:

- **Accepted StoryOS input** restates an existing repository contract and is not reopened here.
- **PostgreSQL fact** is supported by PostgreSQL's official documentation.
- **Recommendation or open decision** is design input for the HITL ticket and has no authority until the ticket resolves it.

The audit does not read or depend on `.reference/**`. It does not choose Rust types, write migrations, provision PostgreSQL, or implement a runtime.

## 2. Accepted StoryOS storage inputs

The repository already fixes these boundaries:

1. PostgreSQL is the authoritative physical database from the Foundation Validation Deployment onward. SQLite, per-project database/file identity, a standalone vector database, a graph database, microservices, a message broker, and whole-system Event Sourcing are outside the Foundation.
2. One stable User owns each Project. Every project-bearing command, canonical row, reference, index, cache, idempotency fact, recovery fact, and disclosure binds exact `ProjectScope { owner_user_id, project_id }`.
3. Authoritative State, Artifacts, and Operational Records are disjoint durable spaces. Derived indexes and caches never become a fourth truth space.
4. Authoritative and Proposal objects have immutable linear Revision histories and exact current Heads. Artifact identities also have immutable linear Revisions. Historical records are append-only unless an owning Tombstone contract explicitly removes payload while retaining minimum proof.
5. A Core Transition commits its Revisions, Heads, sequences, resolutions, Receipts, lifecycle events, and required outbox/wakeup intent as one logical atom.
6. A Run Transition commits normalized current records, Run Events, sequence movement, and outbox/wakeup intent atomically. Leases and execution attempts are durable and fenced.
7. Context Assembly Manifest commits before any destination work. For external egress, the exact non-secret Wire Payload Projection exists before a durable dispatch claim atomically creates an OutcomeUnknown Outbound Disclosure Event; only then may external I/O occur.
8. Retrieval Indexes, Agent Memory access projections, Run Checkpoints, editor projections, caches, embeddings, and read models are disposable and deterministically rebuildable from canonical exact-version sources and qualification facts.
9. Credential values and credential-value digests never enter project records, Tool arguments or results, transcript, persisted wire projections, ordinary logs, backups, or exports. Durable records may contain only opaque Credential References and non-secret availability/status evidence.
10. Archive, Tombstone, Memory Suppression, and Context Exclude are different controls. Historical context and disclosure evidence is not rewritten by later changes. Retention durations and Run/mailbox compaction remain owned by the downstream retention ticket.

## 3. Persisted-domain inventory

This is a logical ownership inventory, not a one-table-per-term prescription. A physical design may normalize several records into one family or split one payload from its envelope, but it must preserve every listed identity, scope, version, current-state, and history boundary.

### 3.1 Identity and project policy roots

- User and Project, including immutable Project ownership and exact Project Scope.
- Project Instruction identities, immutable Revisions, current Head, and per-AgentRun Project Instruction Binding or explicit absence.
- explicit Author Preferences and other author-owned project settings.
- Memory Suppressions, Context Pins and Excludes, Project Destination Grants, Project Tool Enablements, Proactive Triggers and their immutable Revisions, and project policy versions.
- opaque Credential References and their authorized project-use bindings; never credential material.

### 3.2 Authoritative and manuscript state

- authoritative domain object identities, Authoritative Revisions, exact Heads, payload digests, schema/coordinate/digest profiles, and Provenance.
- Authoritative Commits and the gapless Project Scope-local Authoritative Commit sequence.
- Author Intents, author-owned action references, the gapless Project Scope-local Author Action sequence, compensation bindings, and the derived Author Undo Frontier.
- manuscript block structural identities and typed split, join, transfer, retype, and restoration evidence.
- Domain, Validation, Acceptance, Undo Acceptance, and Author Undo Receipts.
- command idempotency bindings between exact Project Scope, command kind, idempotency key, command digest, and immutable Receipt/outcome.

### 3.3 Artifacts, evidence, and Proposals

- Artifact identities, immutable Artifact Revisions, exact Heads, kinds/schemas, payload digests, Creators, and typed Provenance Edges.
- Artifact lifecycle events and current Retention/Workflow/Closure projections.
- Proposal identities, Revisions, Operations and their per-incarnation resolution, four state axes, Anchors, Validation Receipts, Generations, stream sequences, reservations, Pause Fences, conflicts, and Proposal Bundles.
- Drafts, Messages and replacement relations, Research Artifacts, Source Snapshots, Research Syntheses and Claims, Evidence Locators and Relations, Analysis Reports and Findings, Tool Artifacts, Candidates, Memory Candidates, Admission Decisions, Admitted Memory Entry materializations when used, and Memory lifecycle relations.
- content-addressed immutable payload records and reference counts or equivalent reachability proof if physical deduplication is used; content digest never replaces logical identity.
- Artifact Tombstones, per-Revision minimum deletion proof, Purged Source projections, and deletion provenance.

### 3.4 AgentRun, Subrun, scheduling, and budgets

- AgentRuns, root/child Run Lanes, Run lifecycle and outcome, Run sequences, RunSteps, Step Snapshots, Agent Decisions, Execution Attempts, and Recovery Decisions.
- Run Plans and immutable Plan Revisions, PlanSteps, Checkpoints as replaceable projections, Waits and resolutions, Holds, Wakeups, Steering Inputs, pause/cancellation/finalization intent and settlement.
- Leases and monotonically increasing fencing tokens, execution-capacity reservations, budget envelopes/ceilings/targets, reservations, borrowing, finalization reserves, usage settlement, and guardrail counters.
- Proactive Trigger Occurrences, Batches, Admission Decisions, misfire handling, and idempotent occurrence-to-root-Run bindings.
- Subrun Requests, Subruns, Context Bundles, attenuated Capability Grant Revisions, parent/child identity, Joins, lifecycle/outcome/finalization, Results and Result Dispositions.
- Subrun Mailbox Messages, per-direction sequences, delivery/acknowledgement/consumption facts, capacity/backpressure, Progress Report supersession, Undeliverable records, terminal delivery intent, Mailbox Seal, sender generations, and deduplication high-watermarks.
- immutable Run Events and durable outbox/wakeup intent. Checkpoints and author UI read models remain projections.

### 3.5 Skills, Tools, models, and MCP Apps

- Skill Sources, installation records/scopes, SkillPackage References, content-addressed project-owned SkillPackage Snapshots, revocations, Drafts, optional StoryOS extensions, parameter sets, selection sets/decisions, outcome obligations, dependency resolutions, load requests, and script ToolCalls.
- StoryOS ToolSpecs and versions, Tool Discovery and Registration Revisions/status, Tool contract drift evidence, Tool Exposures as disposable step projections, Capability Grants, Approvals and Policy Decisions.
- ToolCalls, effect Requests/Outcomes, execution Attempts, destination intake contracts, exact targets, idempotency, and produced record references.
- Model Provider Adapter definitions, Model Registration Revisions/status, capability profiles, operational snapshots, routing policies, route requests/decisions/overrides, invocations, Attempts, Attempt Requests/cancellation/repair, stream events, failures, outcomes, usage settlements, and sanitized telemetry projections.
- App UI Resource identities/Revisions, retained immutable resource payloads, execution-eligibility generations, derived-data cache versions, execution admissions, App View Artifact Revisions/stages/fallbacks, Prepared Receipts, Instances, negotiation/replay decisions, deliveries, semantic Action Requests/routing/settlement, response deliveries, and terminal reasons.

### 3.6 Context, retrieval, and disclosure

- Operation Input Snapshots, Operation Requirements, Context Candidates and exact Source Versions, eligibility/trust decisions, ranking inputs/results, sufficiency decisions, and bounded Projections with complete loss/source closure.
- immutable Context Assembly, Destination Context, and Outbound Disclosure Manifests.
- Project Destination Grants, Destination Disclosure Approvals, Destination Attempts, final Admission Decisions, exact non-secret Wire Payload Projections, Outbound Disclosure Events, confirmation/reconciliation evidence, and processing-destination identities.
- Context Includes/Pins/Excludes and immutable historical inspection evidence.
- Retrieval/embedding documents, Agent Memory entries, Context Cache Entries, read models, checkpoints, and editor projections only as scoped, versioned, disposable projections with deterministic rebuild and current eligibility recheck.

## 4. PostgreSQL facts that constrain the design

### 4.1 Composite scope constraints are native

PostgreSQL primary keys and unique constraints may span multiple columns, and a foreign key may reference a matching group of columns. A referenced group must be a primary key, a unique constraint, or a suitable non-partial unique index. `MATCH FULL` prevents a partially-null composite reference, while `NOT NULL` removes the all-null escape. PostgreSQL recommends `UNIQUE`, `EXCLUDE`, or `FOREIGN KEY` instead of cross-row `CHECK` expressions, because cross-row `CHECK` assumptions are not continuously enforced and can break dump/restore.

Evidence: [PostgreSQL 18 constraints](https://www.postgresql.org/docs/current/ddl-constraints.html) and [CREATE TABLE foreign-key match types](https://www.postgresql.org/docs/current/sql-createtable.html).

**Implication:** a project-bearing logical reference can physically include `(owner_user_id, project_id, target_id)` and reference a same-scope unique key. Global uniqueness of `target_id` is not enough to prove scope.

### 4.2 Row security is a second gate with role caveats

When Row-Level Security is enabled, normal row access must pass a policy and the absence of a policy defaults to deny. Superusers and roles with `BYPASSRLS` always bypass it; table owners normally bypass it unless the table uses `FORCE ROW LEVEL SECURITY`. RLS does not govern whole-table operations such as `TRUNCATE` or the referential-integrity checks behind `REFERENCES`.

Evidence: [PostgreSQL 18 row security policies](https://www.postgresql.org/docs/current/ddl-rowsecurity.html).

`SET LOCAL` applies only to the current transaction, and `set_config(..., true)` has the same transaction-local behavior. This permits a pool-safe transaction scope value if every runtime transaction establishes it before project-bearing SQL and no query is allowed outside that boundary.

Evidence: [PostgreSQL 18 SET](https://www.postgresql.org/docs/current/sql-set.html) and [system administration functions](https://www.postgresql.org/docs/current/functions-admin.html).

**Confirmed HITL decision (2026-07-21):** StoryOS requires same-scope composite constraints plus forced RLS as a mandatory second boundary. Project-bearing runtime access uses a non-owner, non-superuser, non-`BYPASSRLS` role with trusted transaction-local Project Scope; migration, backup, and controlled maintenance roles and paths remain separate.

### 4.3 Transaction isolation and row locking are separate tools

`READ COMMITTED` is PostgreSQL's default and each statement sees a new committed snapshot. `REPEATABLE READ` keeps one transaction snapshot. `SERIALIZABLE` additionally detects dependency patterns that cannot correspond to a serial execution and aborts one transaction with `serialization_failure`; callers must be prepared to retry the whole transaction.

Evidence: [PostgreSQL 18 transaction isolation](https://www.postgresql.org/docs/current/transaction-iso.html) and [SET TRANSACTION](https://www.postgresql.org/docs/current/sql-set-transaction.html).

`SELECT ... FOR UPDATE` and related clauses lock selected rows until transaction end. `NOWAIT` can refuse contention. `SKIP LOCKED` creates an inconsistent view and is unsuitable for general-purpose reads, but PostgreSQL explicitly identifies queue-like multi-consumer access as a use case. Locks should be acquired in a stable order to reduce deadlocks.

Evidence: [PostgreSQL 18 SELECT locking clause](https://www.postgresql.org/docs/current/sql-select.html) and [explicit locking](https://www.postgresql.org/docs/current/explicit-locking.html).

**Implication:** exact Head rows, per-project counters, Run lanes, leases, and outbox claims can use narrowly scoped row locks and expected versions. Blanket `SERIALIZABLE` is not automatically required for every transaction, but multi-row predicates whose correctness is not reducible to locked rows/constraints may need it.

### 4.4 Native sequences are not gapless domain order

`nextval` is atomic across sessions, but its value is not reclaimed after transaction rollback, `ON CONFLICT`, or some crashes. PostgreSQL states that sequence objects cannot provide gapless sequences.

Evidence: [PostgreSQL 18 sequence manipulation functions](https://www.postgresql.org/docs/current/functions-sequence.html).

**Implication:** the repository's gapless-on-commit Authoritative Commit, Author Action, Proposal stream, Run, and mailbox orders cannot be implemented with ordinary PostgreSQL sequence objects. They need transactionally locked scope/aggregate counter rows or an equivalent committed-row allocation scheme.

### 4.5 Unique arbitration supports idempotency

`INSERT ... ON CONFLICT DO UPDATE` guarantees one atomic insert-or-update outcome under concurrency when no independent error occurs. A unique constraint or non-partial unique index can arbitrate the conflict.

Evidence: [PostgreSQL 18 INSERT](https://www.postgresql.org/docs/current/sql-insert.html).

**Implication:** a unique key over `(owner_user_id, project_id, command_kind, idempotency_key)` can prevent duplicate first attempts. The stored command digest and immutable outcome must still be compared explicitly; `ON CONFLICT` alone does not define StoryOS idempotency semantics.

### 4.6 Large values already have transactional row-external storage

PostgreSQL TOAST transparently compresses and/or moves large `text`, `bytea`, `jsonb`, and other variable-length values out of the main heap tuple into a table-owned TOAST relation. Per-column storage and compression strategy is configurable. TOAST values remain part of the owning PostgreSQL transaction and ordinary backup/restore.

Evidence: [PostgreSQL 18 TOAST](https://www.postgresql.org/docs/current/storage-toast.html) and [database physical storage](https://www.postgresql.org/docs/current/storage.html).

**Implication:** StoryOS does not need an external object store merely to keep moderately large immutable prose, manifests, transcript items, source snapshots, or App resources off hot envelope rows. A logical payload table can separate access patterns while PostgreSQL preserves atomicity. Very large media or future scale may justify a later external blob contract, but that would add a distributed crash, backup, deletion, and migration boundary.

**Confirmed HITL decision (2026-07-21):** every current canonical payload remains transactionally inside PostgreSQL. Hot envelopes and state may reference logically separate immutable payload tables, while TOAST owns initial physical compression and out-of-line storage. The Foundation adds no external object store, PostgreSQL Large Object API, or application-level compression; every payload family has a hard size limit, and an oversize value fails explicitly until a later contract admits another storage boundary.

### 4.7 Backup, export, and upgrade solve different problems

`pg_dump` creates a transactionally consistent logical export without blocking ordinary readers or writers. Custom and directory formats support selective and parallel restore and per-segment compression. PostgreSQL expects dump output to load into newer server versions, while `pg_dump` itself refuses to dump a server newer than its own major version. A selected-table dump does not automatically include all dependencies needed for a standalone restore.

Evidence: [PostgreSQL 18 pg_dump](https://www.postgresql.org/docs/current/app-pgdump.html) and [pg_restore](https://www.postgresql.org/docs/current/app-pgrestore.html).

Physical base backups plus a continuous WAL archive support whole-cluster crash recovery and point-in-time recovery. They are not project-selective logical exports, and WAL does not back up manually edited PostgreSQL configuration files.

Evidence: [PostgreSQL 18 continuous archiving and PITR](https://www.postgresql.org/docs/current/continuous-archiving.html) and [pg_basebackup](https://www.postgresql.org/docs/current/app-pgbasebackup.html).

Major-version migration can use logical dump/restore, `pg_upgrade`, or logical replication depending on deployment constraints; `pg_upgrade --check` can validate an in-place path before changing data.

Evidence: [PostgreSQL 18 pg_upgrade](https://www.postgresql.org/docs/current/pgupgrade.html).

**Implication:** StoryOS needs distinct contracts for database disaster recovery, whole-service migration, and one-Project product export/import. A table-filtered `pg_dump` is not by itself a safe Project export.

**Confirmed HITL decision (2026-07-21):** Foundation Project export/import is exact-scope restore, not copying. A Project Export Archive preserves the original `owner_user_id`, `project_id`, object identities, immutable history, and provenance. Project Restore requires an authorized same-User target where that Project Scope is absent, validates and stages the complete archive before atomic visibility, and fails on any conflict rather than merging, overwriting, or remapping identities. Disposable projections are rebuilt and unavailable Credential References remain Unbound. Copy, fork, new-Project import, and ownership transfer require a separate future contract.

**Confirmed HITL decision (2026-07-21):** an author-visible successful commit has zero acknowledged-data loss after an ordinary process or power crash. Loss of the Foundation database host or disk has an RPO of at most fifteen minutes and an RTO of at most two hours. The Foundation uses synchronous PostgreSQL commit durability, a daily physical base backup plus continuous WAL archival in a failure domain independent of the database host, and a successful automated restore proof for every release candidate. Retention duration remains downstream, but every claimed window needs a complete recovery chain. Synchronous replicas, automatic failover, and a high-availability cluster are out of Foundation scope; a later cloud deployment may tighten the profile.

### 4.8 Online-safe schema changes are staged operations

Most `ALTER TABLE` forms take strong locks. Foreign keys and check constraints may be added `NOT VALID` so new writes are enforced immediately, then validated over existing rows with a weaker lock. `CREATE INDEX CONCURRENTLY` avoids blocking writes but runs outside a transaction block, does more work, and can leave an invalid index that needs explicit recovery.

Evidence: [PostgreSQL 18 ALTER TABLE](https://www.postgresql.org/docs/current/sql-altertable.html) and [CREATE INDEX](https://www.postgresql.org/docs/current/sql-createindex.html).

**Implication:** migrations need a durable phase/state contract and postcondition checks. A migration runner cannot assume every release change is one all-or-nothing transaction, and application compatibility must cover expand/backfill/validate/switch/contract phases when a change cannot be instantaneous.

### 4.9 Credential references need a separate secret boundary

Apple describes Keychain Services as encrypted storage for small user secrets and explicitly supports generic-password items. Its API lets an application store encrypted secret data under non-secret lookup attributes rather than implementing its own encryption.

Evidence: [Apple Keychain Services](https://developer.apple.com/documentation/security/keychain-services/), [adding a password to the keychain](https://developer.apple.com/documentation/security/adding-a-password-to-the-keychain), and [`kSecClassGenericPassword`](https://developer.apple.com/documentation/security/ksecclassgenericpassword).

PostgreSQL's `pgcrypto` can encrypt values, but its own security notes say the data and passwords passed to those functions move between the client and database server in clear text and therefore require trusting both the system and database administrators. Database-side encryption also leaves StoryOS responsible for a separate encryption-key lifecycle and does not by itself keep secret material out of database backups or Project exports.

Evidence: [PostgreSQL 18 `pgcrypto` security limitations](https://www.postgresql.org/docs/current/pgcrypto.html#PGCRYPTO-NOTES-SECURITY).

**Implication:** a Foundation-local Keychain implementation is a simpler secret-material boundary than storing Provider credentials, Tool tokens, or their decrypting keys in ordinary PostgreSQL records. PostgreSQL can still own a scope-bound opaque locator, backend kind, non-secret generation metadata, availability state, and audit evidence. A backend-neutral resolver contract is required for later controlled-cloud deployment because a local Keychain locator cannot silently become a cloud secret locator. Project export/import must carry the reference semantics without carrying the secret and must require explicit rebinding when the destination cannot resolve it.

**Confirmed HITL decision (2026-07-21):** PostgreSQL stores only Project Scope-bound Credential References and non-secret resolver metadata; credential values and value digests remain in a deployment-specific secret backend. The Foundation-local implementation uses macOS Keychain, environment variables are development/test inputs only, and a later controlled-cloud deployment uses a managed secret service through the same backend-neutral resolver contract without selecting a vendor now. Ordinary database backups, logs, support material, and Project exports exclude secret material. Import preserves the reference semantics but leaves an unavailable destination binding explicitly Unbound until an authorized rebind.

## 5. Evidence-backed recommendation set and confirmed decisions

Items 2, 3, 6, 7, 10, and 11 form confirmed HITL decisions above. The other items are evidence-backed technical requirements for the Foundation specification:

1. Use one PostgreSQL database and one StoryOS-owned schema family, organized by aggregate ownership rather than one schema/database per Project.
2. Put `(owner_user_id, project_id)` on every project-bearing canonical, reference, idempotency, outbox, projection, and cache row. Use same-scope composite unique keys and composite foreign keys for every project-bearing reference.
3. Add forced RLS as defense in depth for all project-bearing runtime tables, using a non-owner, non-superuser, non-`BYPASSRLS` runtime role and transaction-local trusted scope. Keep migrations, backup, and controlled maintenance on separate roles and paths.
4. Use ordinary `READ COMMITTED` plus exact expected Heads, unique constraints, and row locks for normal single-aggregate transitions; acquire locks in canonical order. Use `SERIALIZABLE` only where a cross-row predicate cannot be reduced to explicit constraints or locked guard rows, and retry the whole transaction on serialization failure.
5. Use transactionally updated per-Project/per-aggregate counter rows for gapless committed domain sequences. Never use UUIDv7, wall time, or native sequences as domain order.
6. Store immutable payloads in PostgreSQL payload tables keyed by Project Scope, logical payload identity, digest profile, and digest. Let TOAST handle physical compression/out-of-line storage initially; do not add an object store or PostgreSQL large-object API in the Foundation.
7. Store only opaque credential locators and backend identity in PostgreSQL. Secret material stays in a deployment-specific secret backend and is excluded from ordinary database backup, logs, Project export, and domain payloads.
8. Treat canonical writes plus outbox/wakeup/egress intent as one transaction. Claim queue rows through fenced worker records; every external retry is a new attempt with its own durable evidence.
9. Maintain a schema-version ledger plus compatibility metadata. Verify every migration postcondition, separate nontransactional phases, and block startup when the binary is outside the declared database compatibility window.
10. Use a StoryOS-defined Project Export Archive for one-Project round trips. It contains canonical records and required immutable payloads, omits disposable projections, caches, and secret material, preserves original identities and provenance, and restores through a validating staged transaction rather than direct table copying.
11. Use physical backup plus WAL/PITR for service disaster recovery and a separately tested logical dump/restore path for major upgrades and whole-service portability.

## 6. Open decisions that change the long-term contract

No unresolved product-owner decision remains in this research scope. Transaction shape, lock ordering, migration staging, projection rebuild, and proof fixtures are implementation-contract details constrained by the accepted domain semantics and the confirmed decisions above.

Retention durations, mailbox/event compaction, and final archival policy are deliberately not decided here because the existing downstream retention ticket owns them. This storage ticket must define the handoff and enforce deletion/invalidation propagation without preempting those later durations.
