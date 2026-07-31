# Run Event, Mailbox, Snapshot, Retention, and Archival Semantics

- Status: current
- Contract revision: `release1-retention-contract-2026-07-31`
- Canonical baseline: `main@47a644056ca951c7c21f7c4ce81089fd66e2b08c` (tree `22f7ae2133f18f2800c384528388b19550552aca`)
- Wayfinder resolution: [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Storage and isolation boundary: [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md)
- Persistence family source of truth: [Release 1 persistence catalog](postgresql-release-1-persistence-catalog.json)
- Protocol boundary: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Public route/Event source of truth: [Release 1 route catalog](versioned-protocol-release-1-route-catalog.json)
- Context and disclosure boundary: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)
- Eval evidence boundary: [Foundation Evidence for the Standalone Eval Surface](eval-evidence-foundation.md)
- Measurement input: [Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76)
- Measurement report: [Representative Writing-Path Performance and Storage-Growth Envelope](../research/representative-writing-path-performance-and-storage-growth-envelope.md)
- Measurement provenance: [Issue 76 evidence bundle](../research/evidence/issue-76/README.md)
- Deterministic proof boundary: [Deterministic Verification and Failure-Recovery Gates](deterministic-verification-and-failure-recovery-gates.md)
- Release handoff boundary: [AI-Independent Editor-First Release Baseline and Handoff Criteria](ai-independent-editor-first-release-baseline-and-handoff-criteria.md)
- Decisions: [ADR 0008](../adr/0008-allow-policy-governed-post-seal-operational-compaction.md), [ADR 0009](../adr/0009-require-snapshot-resync-at-replay-generation-boundaries.md), [ADR 0010](../adr/0010-require-lifecycle-proof-before-recovery-visibility.md), and [ADR 0011](../adr/0011-require-explicit-project-deletion-settlement.md)

## 1. Purpose and authority

This specification defines the project-local lifecycle that bounds Run and
Subrun operational storage without rewriting historical facts. It composes the
accepted Event, Activity Stream, Snapshot, Attempt, outbox, Mailbox, Seal,
Artifact, Context Assembly Manifest, Outbound Disclosure Manifest, PostgreSQL,
and Eval contracts. It does not authorize a schema, Rust, client, deployment,
or storage implementation.

Every decision remains bound to one exact Project Scope. No archive, cache,
compaction product, export, restore, cursor, mailbox fact, or deletion result
may bridge Users or Projects or make an external Provider, Tool, MCP server,
embedding service, or client the source of truth.

For every Run fact and payload, StoryOS keeps four independent answers; none
implies another:

| Fact | Required distinction |
| --- | --- |
| historical occurrence | Whether an Event, Manifest, Attempt, Receipt, or lifecycle decision happened remains attributable evidence. |
| current eligibility | Whether an authorized reader, Context Assembly, cache, Export, or destination may use it is decided at the current operation. |
| payload availability | Whether the original bytes are retained, archived, compacted, redacted, tombstoned, or pending physical cleanup is separately recorded. |
| service availability | Whether a cursor can replay, an archive can be inspected, a payload can be exported, or a Scope can be restored has its own bounded protocol result. |

For example, a compacted Provider stream may remain an indisputable historical
Attempt while its bytes are unavailable and its cursor is outside the replay
floor. A redacted source may remain a historical Manifest reference while it
is immediately ineligible for inspection or future disclosure and its physical
cleanup is still pending.

### 1.1 Author journey and acceptance surface

The retention contract is complete only when an authorized author can follow
one consistent history surface:

1. Open a Project Activity or Run view and distinguish historical occurrence,
   current payload eligibility, and service availability.
2. Resume strictly after a valid cursor in its Replay Generation, or receive
   the existing `activity_cursor_too_old` result and a fresh Snapshot when the
   cursor is below the floor. The client never receives a guessed mapping or a
   silent skip.
3. Inspect a terminal, sealed Run/Subrun after compaction and see immutable
   Events, Receipt/Attempt/Manifest/Outcome evidence, Mailbox Seal/Fence,
   lifecycle Decision, digest, and an explicit unavailable-payload gap.
4. See an exact duplicate or late sealed Mailbox Message rejected without
   scheduling work, reopening a terminal Subrun, or reapplying an effect.
5. Export a consistent Project archive, verify manifest/digest/provenance/gaps,
   and restore only the same absent Scope after lifecycle and Recovery
   Visibility Proof; projections rebuild and a new Snapshot/resync is used.
6. Request Project deletion, observe new work/disclosure/export/restore fenced,
   see settled or `OutcomeUnknown` in-flight evidence, and never see the
   deleted Scope become readable through Project Restore.

An inspection surface reports the truth available under current authorization;
it does not dispatch a Provider, Tool, MCP server, embedding service, or
support request to reconstruct a missing payload.

## 2. Confirmed default: post-seal operational compaction

The ordinary author experience must not require manual storage administration.
StoryOS therefore uses an automatic, policy-versioned lifecycle for eligible
high-volume non-authoritative operational payloads after a Run or Subrun is
terminal and the root Subrun Mailbox Seal is committed. The lifecycle may make
those raw payload bytes unavailable, but it must retain an inspectable compacted
history with all of the following facts:

1. immutable Run Events and their causal identities and sequences;
2. relevant Attempts, committed Context Assembly and Outbound Disclosure
   Manifests, Receipts, final Result and Outcome, and external-effect evidence;
3. the Seal, directional high-watermarks, deduplication proof, and any required
   tombstone needed to reject a replayed Message ID;
4. exact digest, compaction policy and generation, source closure, lifecycle
   decision, time, actor or policy authority, and the current payload
   availability; and
5. a usable recovery checkpoint or Snapshot where this contract requires
   one, without representing it as live process state or a substitute for a
   known missing payload.

The resulting gap is a present availability fact, not a rewrite of what the
Run selected, prepared, dispatched, received, or concluded. Queries, exports,
replay, recovery, and Eval must expose that gap and never report byte-for-byte
replay, complete export, or full evidence availability when the raw payload is
no longer retained.

### 2.1 Retention classes, not a Run-wide TTL

Retention is assigned to an exact Operational Record fact or payload role, not
to an entire Run as one indistinguishable blob. The current policy must classify
each role as follows:

| Class | Meaning | Compaction rule |
| --- | --- | --- |
| Operational Evidence Floor | The durable fact envelope needed to inspect what happened: Event identity and sequence, relevant Attempt, Manifest, Receipt, terminal Result and Outcome, Seal and deduplication proof, lifecycle decision, digest, and every known availability gap. | It is never removed merely because a high-volume payload in the same Run becomes unavailable. Its later archive or project-deletion treatment remains explicit. |
| Compactable Operational Payload | High-volume, non-authoritative bytes such as eligible stream fragments or redundant diagnostics. | It may become unavailable only through the confirmed post-terminal, post-root-Seal compaction path. Its fact envelope and gap remain in the Evidence Floor. |
| Disposable Projection | Rebuildable cache, index, read model, or transient acceleration product that owns no historical fact. | It may be invalidated or removed under its owning contract, but must never be misrepresented as retained evidence or a compacted canonical payload. |

An unknown or unclassified record role fails closed to the Operational Evidence
Floor. One exact record may carry both a retained fact envelope and a separately
classified payload; retaining the former does not falsely claim that the latter
is still byte-available. Exact windows and capacities remain versioned policy
values. [Measure the Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76)
owns representative measurements only; this specification consumes that
evidence without adopting a retention value. PER-001, PER-002, PER-006,
PER-007, and PER-008 remain uncalibrated. No retention duration, checkpoint
cadence, hot/archive/compaction window, Fence capacity, Snapshot lifetime,
Recovery Copy window, deletion bound, or diagnostic window is a Release 1 hard
default until a named Retention Profile revision adopts it with workload,
environment, headroom, and owner. Measurements alone never change a profile or
authorize cleanup.

### 2.2 Logical record classes and the PostgreSQL owner boundary

The retention decision is made against a logical record class, never against a
hand-maintained list of tables. The Release 1 persistence catalog remains the
single physical source of truth. The family identifiers below are a review
crosswalk to that catalog; they are not a second table ledger, and their
derived family/table counts must continue to come from the existing catalog
verifier. For each named sub-class, its existing contract remains the sole
owner of meaning; #64 is the sole owner of the Retention Profile, Decision,
and availability treatment, while #56 remains the physical-storage owner.
Artifact Tombstone remains the explicit Artifact-contract exception.

| Logical record class | #56 catalog family | Why the record exists and minimum retain floor | Compact or rebuild rule | Archive, tombstone, and delete rule | Meaning owner, unchanged per named sub-class |
| --- | --- | --- | --- | --- | --- |
| Project identity, instructions, policy, authoritative heads, revisions, and commits | `project-canonical` | It is the current Project-authoritative state. Retain while the Scope exists and through every accepted author settlement, export, restore, and deletion settlement. | Never compact or rebuild the source. A projection may be rebuilt from it. | Never archive as a substitute for current authority. Project deletion follows section 10 and retains deletion evidence after payload cleanup. | Project/Core contract for canonical state; #56 owns physical storage. |
| Artifact, Proposal, Draft, source, research, and Artifact lifecycle records | `artifact-proposal-draft` | They preserve author-owned creative state, inspectable proposals, drafts, provenance, and their lifecycle. Retain through the owning contract's settlement and any operational reference or author inspection that names them. | Never infer authority from a rebuildable view. Operational summaries may be rebuilt only from the retained canonical record. | Artifact Tombstone and creative deletion remain with the Artifact contract. #64 preserves the operational reference, reason, digest, and gap but cannot tombstone or delete an Artifact. | Artifact/domain contract for Artifacts; Core/Proposal contracts for their named Proposal/Draft records. |
| Typed Receipts, validation/acceptance/undo settlement, Author Actions, command idempotency, and receipt settlements | `operational-receipts-actions` | They prove exact author admission, effect identity, acknowledgement, retry, undo, and settlement. Retain until the command/effect is terminal, every exact retry and inspection obligation is settled, and Project Deletion Settlement permits the reduced fence form. | Payload detail may be reduced only to an exact Receipt/Command Idempotency Fence; identity, digest, result, settlement, and provenance are not rebuildable substitutes. | Archive only with an inspectable manifest. Tombstone/delete only under settled project deletion with the minimum settlement, fence, digest, and gap evidence retained. | Core owns Receipts/Author Actions; Author Command Admission owns admission/idempotency binding. |
| Admission, anti-forgery, editor session, writer generation, input fence, and proposal pause fence | `operational-admission-editor` | They prove which command or editor generation was admitted, whether takeover is safe, and whether a recovery retry is exact. Retain through terminal settlement, convergence or takeover proof, and all pending/unknown outcomes. | Never compact a pending or unknown binding. After settlement, only a verifiable fence/terminal descriptor may replace large payload detail. | Archive only after the editor/session owner’s recovery floor is satisfied. Deletion is fenced by Project Deletion Settlement and leaves the safe lifecycle evidence needed to reject reuse. | Author Command Admission and Web Editor Session owners; #64 governs retention treatment. |
| Agent Run, Subrun, plan, step, grant, lease, execution attempt, result, recovery, usage, transcript, approval, steering, and worker fence | `operational-run-mailbox` | They explain execution state and recovery. Retain until Run/Subrun terminality, finalization, all relevant Attempts/effects/OutcomeUnknown states, and the root Mailbox Seal are durably settled. | Only settled high-volume payloads may be compacted. Run/Subrun identity, terminal transition, cause, result, recovery decision, and fences remain in the Evidence Floor; disposable projections may rebuild. | Archive settled bytes with a manifest or compact them with an explicit gap. Tombstone/delete only through the project lifecycle; never delete a record that could still decide a retry or effect. | AgentRun/Subrun contract for Run meaning; execution owners for their named records. |
| Immutable Run Events and causal event payloads | `operational-run-mailbox` | They are the authoritative execution history for what the Run did and why. Retain identity, Scope, sequence, cause, schema, digest, and lifecycle meaning for the Evidence Floor through archive and restore proof. | Segment compression is lossless only. Raw associated payload may compact after all settlement and seal conditions; an Event identity or sequence is never rebuilt from a summary. | Archive may move the bytes while preserving digest and order. Tombstone/delete requires a safe historical gap and deletion settlement; it may not claim the event never happened. | Run/Event owner for meaning; #64 owns availability and replay-retention boundary. |
| Mailbox messages, deliveries, acknowledgements, consumption, Seal, directional high-watermarks, and deduplication Fence | `operational-run-mailbox` | They prevent duplicate consumption and late work. Retain every unsettled delivery; after root Seal retain the Seal digest, direction, sender generation, high-watermarks, and Fence needed to reject late/replayed IDs. | Per-message evidence may be reduced only atomically with a valid Seal Fence. A late sealed message never reopens work. | Include Seal/Fence in archive, export, restore, and gap evidence while the root is retained. Project deletion may remove payloads only after settlement and retains rejection/lifecycle proof. | Mailbox/Run owner for meaning; #64 owns compaction and archival condition. |
| Context Assembly, retrieval, Tool/MCP, model, Processing Identity, disclosure Manifest, destination Attempt, wire payload, and destination settlement | `operational-context-disclosure` | They prove minimum context, manifest-before-egress, destination identity, authorization, and external-effect uncertainty. Retain manifests, Attempt identity, selection/provenance closure, disclosure, and OutcomeUnknown until settlement and inspection obligations close. | Never compact a pending Attempt or unresolved OutcomeUnknown. Settled payloads may compact, but the manifest and destination/effect evidence remain. | Archive only under current source eligibility. Redaction immediately blocks reuse/export/egress; physical deletion retains safe provenance and known external disclosure truth. | Context/Disclosure owns manifests; Tool/MCP and destination contracts own their named attempts/effects. |
| Application Wire Records, wire payloads, and event representations | `operational-wire-history` | They bind a public or internal exchange to exact request/response/event identity, schema, digest, and cause. Retain the envelope through protocol settlement, replay, and author inspection. | Lossless compression or rebuild of a read projection is allowed; semantic wire identity and digest are not reconstructed from a cache. | Archive bytes with manifest/digest; redaction or project deletion exposes only permitted safe identity and gap, never a false successful or erased exchange. | Protocol/wire owner for meaning; #64 owns the retention class. |
| Project Activity Events, payloads, positions, replay generations, floors, cursors, and handoffs | `operational-project-activity` | They provide the one public Project Activity chronology and exact cursor handoff. Retain immutable identity, sequence, cause, generation, floor, Snapshot handoff, and known gaps. | A generation boundary may compact old payloads only after publishing its closing position, new floor, Snapshot, and Decision. No cross-generation cursor mapping is rebuilt or guessed. | Archive/export/restore carries generation and gap evidence. Deletion retains the safe gap and handoff needed to return the protocol’s typed result, not a silent empty stream. | #58 owns wire/error meaning; #64 owns generation-retention transition. |
| Run Checkpoints, Canonical Query Snapshots, Snapshot members, and generation handoffs | `operational-run-mailbox` + `operational-snapshot-replay` | A Run Checkpoint accelerates durable Run recovery; a Query Snapshot is an authorized read boundary; neither is source authority. Retain each until its declared validity, dependent generation handoff, or recovery proof is complete. | Rebuild Checkpoints/Snapshots from canonical facts when valid. Expiry discards the projection only after its typed resync path is available; it never deletes source history. | They are not backups or archives. A missing/expired Snapshot returns the existing typed resync result; a restored Scope publishes a new Snapshot rather than reusing an invalid token. | #64 owns policy/eligibility; #58 owns public Snapshot/cursor semantics; #56 owns physical persistence. |
| Lifecycle, Retention Decision, archival decision, project Tombstone, redaction/suppression, and deletion records | `operational-lifecycle` | They explain why availability changed and are themselves evidence. Retain the Decision, Profile revision, eligibility proof, actor/policy, digest, due condition, gap, and settlement through the resulting state and any restore/deletion visibility proof. | Never rebuild or infer a lifecycle decision from current bytes. A later decision appends a new fact and cannot rewrite an earlier decision. | These records are the Tombstone/manifest/gap/provenance floor. Physical cleanup may delete named payload copies only after its Decision and proof; project deletion retains the minimum settlement and safe evidence. | #64. Artifact Tombstone semantics remain with the Artifact owner. |
| Historical projections, retrieval/embedding indexes, context caches, read models, and generation-control projections | `projection-generation-control`, `projection-retrieval`, `projection-embedding`, `projection-context-cache`, `projection-read-model` | They accelerate authorized reads and are never the historical source. Retain only for their projection validity and rebuild dependencies. | Rebuild or invalidate from canonical facts and current lifecycle. A disposable projection may be deleted without creating a historical gap, but its source cannot be deleted early merely because it was rebuildable. | Never include as canonical Project Export/Restore content. Redaction/tombstone invalidates source-derived fields; cleanup is the projection owner’s responsibility. | Each projection owner for its own family; #64 supplies current eligibility and suppression facts. |
| PostgreSQL base backup/WAL Recovery Copy and Project Export/Restore staging | `admin-recovery-copy` + `admin-project-portability` | Recovery Copies preserve the physical recovery chain; portability staging proves an exact-scope archive. Retain until the named recovery, export, import, lifecycle, and visibility proofs settle. | Never treat a logical export as a physical backup or a booted database as visible. Staging and derived indexes may rebuild after integrity proof. | Recovery Copy rotation and staging deletion follow their own adopted windows, but must retain lifecycle range, manifest/root digest, known gaps, and deletion evidence until proof allows cleanup. | #56 owns physical backup/restore/portability execution; #64 owns semantic lifecycle inclusion and visibility conditions. |

The remaining catalog families (`identity-user`, `global-definitions`,
`credential-references`, and `admin-migration`) retain their existing owners;
this contract neither invents a second policy for them nor makes credential
values, global definitions, or migration history project-local payloads. A
family identifier missing from this crosswalk is therefore a review failure,
not permission to classify it as disposable.

### 2.3 Action vocabulary, proof order, and safe cleanup

The lifecycle verbs have one positive meaning. They are not interchangeable
labels for deleting bytes:

| Action | Positive meaning | Minimum proof before the action | Evidence left after the action |
| --- | --- | --- | --- |
| Retain | Keep canonical facts or currently eligible bytes available to authorized operations. | The current Profile and Scope permit availability; all source and disclosure checks still apply. | The record/payload plus its identity, digest, provenance, and lifecycle state. |
| Compact | Make only an eligible payload unavailable while preserving historical truth. | Exact class; terminal/finalized Run/Subrun; root Seal and directional high-watermarks; settled Receipt/Admission/Attempt/Manifest/idempotency and no unresolved OutcomeUnknown dependency; current Decision; generation/floor/Snapshot handoff where relevant; no unresolved recovery, redaction, deletion, or other owning-contract hold. | Evidence Floor, source digest, Decision, availability gap, and any Seal/Fence or replay handoff. |
| Rebuild | Recreate a projection or checkpoint from retained canonical facts. | Source closure is complete and current lifecycle eligibility is rechecked. | A new projection identity and source revision/generation; no claim that the projection was the source. |
| Archive | Move retained bytes to an explicit cold state without making them ordinary retrieval or replay input. | Immutable archive manifest, Scope, source Snapshot, schema/profile, entry count/digests, provenance closure, known gaps, root digest, integrity proof, and authorized lifecycle Decision. | Manifest, digest, archive state, and visibility result; bytes remain retained until a later deletion proof. |
| Redact or Tombstone | Make a named payload or Scope immediately ineligible before physical cleanup. | Authoritative Decision commits, all new reads/egress/export/reuse are fenced, and safe identity/reason/provenance can be recorded. | Tombstone/Decision, safe digest or identity, provenance, known gap, historical disclosure fact, and cleanup state. |
| Delete | Physically remove the explicitly named copy after logical ineligibility and all required windows/proofs settle. | Exact deletion scope, settlement, no pending/unknown effect that depends on the bytes, archive/Recovery Copy/lifecycle visibility proof, and idempotent deletion completion. | Minimum Tombstone/manifest/gap/provenance and deletion-settlement/completion evidence; never an invented empty history. |

Cleanup is a two-phase semantic operation: first commit the Decision and its
reader-visible boundary, then remove only the bytes named by that Decision.
A crash before cleanup leaves retained bytes and a pending Decision; a crash
after cleanup leaves the Decision, digest, gap, generation/Fence handoff, and
idempotent cleanup result. No reader may observe a missing source with no
corresponding lifecycle evidence. If any prerequisite is unknown, the
operation fails closed and retains the Evidence Floor.

### 2.4 Quantitative evidence and profile adoption

Numbers are evidence-qualified inputs, not self-justifying defaults. A value
becomes effective only when a new immutable Retention Profile revision records
its workload, environment, headroom, owner, and proof reference. The following
crosswalk is the complete quantitative adoption boundary for this contract:

| Input or value | Evidence class and workload/environment | Headroom and interpretation | Owner and adoption rule |
| --- | --- | --- | --- |
| Foundation Recovery Profile: zero acknowledged-data loss for ordinary process/power crash; host/disk-loss RPO at most 15 minutes; RTO at most 2 hours | Accepted hard recovery contract in the PostgreSQL foundation profile; physical base backup/WAL chain, not the Issue 76 logical synthetic path. | The recovery chain and drill must provide the declared margin; these values do not define hot retention, compaction, or deletion duration. | #56 owns the physical profile and #60 later proves it. #64 may require lifecycle application before visibility but cannot narrow or reinterpret it. |
| Versioned Protocol absolute ceilings, including 1 MiB public JSON body, 64 MiB referenced payload, 8 MiB upload chunk, 4 MiB query page, 1,000 Events or 8 MiB per SSE reconnect replay, and 4 MiB queued SSE bytes | Accepted hard wire-validity ceilings from the Release 1 protocol profile; public route/event workload and same-release compatibility boundary. | They are safety ceilings, not throughput, retention, replay-SLA, or capacity targets; effective product values may be lower only under the protocol owner’s revision. | #58 owns the ceilings and error meaning. #64 must not derive retention windows or deletion deadlines from them. |
| Issue 76 observations: 50,000-event scan versus 5,000-event tail after checkpoint; 90% synthetic deletion/rewrite observation; local 4-vCPU/4-GiB and 2-vCPU/2-GiB profiles | Controlled synthetic PostgreSQL workload and local bounded environment; not production/cloud evidence and not a full persistence-family workload. | No production headroom, WAL/lock/scratch margin, or recovery proof was established. These values are candidate experiment points only. | #76 owns the measurements. #64 may use them to request calibration, never as an effective profile value. |
| Issue 76 model: 20,000 and 60,000 annual commands projected to 92.81 MiB and 278.44 MiB across four small families | Modelled projection with excluded major families and local schema assumptions; not a deployment capacity or retention forecast. | No observed project cardinality, recovery-chain footprint, growth margin, or multi-tenant headroom. | #76 owns the model; #56 owns capacity adoption if later accepted. It creates no deletion or archive deadline here. |
| Persistence catalog derives 21 families, 169 unique table families, 16 Project-scoped families, and 5 projection families | Mechanically derived repository-baseline coverage from the #56 JSON catalog and its existing verifier; not a workload measurement or policy value. | Headroom is not applicable; these counts change only when the catalog changes and must never be copied into a second ledger. | #56 catalog/verifier owns the derivation; #64 only crosswalks logical treatment. |
| Protocol catalog derives 58 operations and 53 public Event schemas | Mechanically derived repository-baseline coverage from the #58 route catalog and its existing verifier; not a retention or throughput target. | Headroom is not applicable; the route catalog remains the sole route/Event truth and no count authorizes cleanup. | #58 catalog/verifier owns the derivation; #64 only consumes settlement and Activity/Snapshot/cursor meaning. |
| `PER-001`, `PER-002`, `PER-006`, `PER-007`, and `PER-008` | Unresolved tuning questions covering checkpoint cadence, Run Event hot/archive windows, compactable payload/Fence/Snapshot validity, Recovery Copy deletion, and diagnostics; no accepted workload/environment result. | No headroom and no effective value. Any proposed band must remain pending until its named owner accepts it in a Profile revision. | #64 owns retention-policy adoption for the relevant rows; #56/#60 own physical recovery or executable proof dependencies. |

No duration, byte limit, count, percentage, CPU/memory profile, RPO/RTO,
checkpoint cadence, Snapshot lifetime, Fence capacity, or deletion deadline
outside this table is an effective Release 1 default. A later profile must
state what evidence changed and what safety margin remains; a measurement
cannot mutate a prior Decision retroactively.

### 2.5 Retention Profile and non-retroactive decisions

A Retention Profile is a versioned policy contract selected for one exact
Project Scope. It supplies the effective hot replay, checkpoint, archive,
compaction, capacity, and retention values for each Operational Retention Class.
The default profile is a product policy, not routine author configuration, but
its identity and effective values are inspectable to the author.

Every checkpoint, archive, compaction, purge, or explicit refusal appends one
Retention Decision. That immutable decision binds the Project Scope, exact
record or payload role, retention class, Profile Revision, frozen values,
eligibility and settlement evidence, due condition, actor or policy authority,
time, and resulting availability fact. A Profile Revision by itself does not
change an existing record. Applying changed values to an existing record
requires a later Retention Decision that makes that migration visible before it
changes availability.

No profile change may silently shorten a previously recorded payload lifetime,
rewrite historical Manifests or Attempts, make a compacted payload available
again, or turn a known export or replay gap into complete evidence. Experimental
calibration may create a new Profile Revision; it does not become retroactive
cleanup authority.

### 2.6 Command idempotency, Attempt, and outbox floors

Public command idempotency remains exact for its Project Scope, command kind,
key, and digest. While a command, Receipt, effect, Attempt uncertainty, or
replay reference remains meaningful, its idempotency evidence cannot be
removed. Once larger execution payloads no longer require hot storage, a
Retention Decision may reduce the arbiter to a Command Idempotency Fence that
retains the exact namespace, digest, Command ID, immutable acknowledgement or
replayable acknowledgement reference, final Receipt or operation reference,
and retention provenance.

A matching retry replays the same logical acknowledgement through current
authorized redaction without re-executing; a differing digest remains an
idempotency conflict and changes nothing. A known old key never becomes a new
command because its full record was archived or compacted. If the Profile's
Fence capacity is exhausted, admission fails explicitly until a later
authorized Profile Revision expands capacity; it never forgets a key in order
to accept a new command.

An outbox intent, wakeup, external dispatch claim, or Destination Attempt may
not be compacted while it is pending, leased, unsettled, or OutcomeUnknown.
Settlement may reduce its large payload according to its Retention Class, but
the Evidence Floor preserves its intent, fence, Attempt identity, uncertainty,
and final outcome. A resend remains a new Attempt and never reuses an old
attempt, outbox claim, or idempotency Fence as proof of a new external effect.

### 2.7 Event, Activity, and wire-evidence preservation

Every immutable Run Event and Project Activity Event keeps its semantic event
identity, causal references, Scope, typed sequence, schema, event kind, digest,
and lifecycle meaning in the Operational Evidence Floor. A physical Event
segment may be losslessly compressed or moved to Operational Archive, but no
compaction may semantically delete, reorder, rewrite, or replace a committed
Event. The public replay floor limits service of old Events; it does not turn a
historical Event into a fact that never occurred.

Where an Event, Application Wire Record, Tool result, Provider stream, or other
execution record has a large associated byte payload, its descriptor, exact
digest, owning Attempt or Manifest, and availability state remain in the
Evidence Floor. The bytes themselves may be a Compactable Operational Payload
only after all applicable settlement, Seal, idempotency, disclosure, and
OutcomeUnknown boundaries are complete. This separation never makes a digest a
replacement for unavailable bytes or proof that a destination used them.

### 2.8 Compaction eligibility and decision transaction

The following predicate is deliberately stricter than “old enough”. A payload
is eligible only when every applicable clause is true:

1. its exact User, Project Scope, logical class, source closure, and current
   Profile revision are known;
2. the owning Run and Subrun are terminal and finalized, or the payload is an
   independently disposable diagnostic/projection record whose own contract
   permits cleanup without weakening an active Run’s recovery;
3. for a Run-associated payload, the root Mailbox Seal is committed, its
   directional high-watermarks are frozen, and a Seal Deduplication Fence can
   reject every delayed or repeated Message ID in the closed generation;
4. every dependency applicable to the payload, including Admission, Receipt,
   Author Action, idempotency Fence, outbox intent, Attempt, Context Assembly
   or Disclosure Manifest, Proposal or Draft lifecycle transition, and
   external effect, is settled or retained as a named dependency. An
   `OutcomeUnknown`, unresolved lease/effect, or missing manifest-before-egress
   proof blocks the associated payload;
5. the Run Event and Activity source identities, sequence/cause closure,
   disclosure provenance, digest, and current availability are inspectable;
6. no Redaction Decision, Project Deletion Settlement, recovery hold, archive
   integrity failure, or other lifecycle fence requires the source bytes; and
7. the Retention Decision records the exact due condition, evidence, actor or
   policy authority, and reader-visible handoff required by the class.

The commit order is also part of the contract:

1. freeze the old source position and capture the source digest and replay
   generation;
2. commit the Retention Decision and, where applicable, the new replay
   generation, floor, Snapshot handoff, Archive manifest, or explicit purged
   gap;
3. publish the new reader boundary and idempotent cleanup operation; and
4. remove only the named payload copies after the boundary is durable.

If a fault occurs before step 4, the bytes remain usable and the Decision is
pending. If it occurs after step 4, the Evidence Floor, Decision, digest, gap,
Fence, and handoff describe the unavailable payload. Repeating cleanup with
the same operation identity is a no-op; a new Decision cannot reinterpret a
previous gap as a retained payload. This is the semantic boundary that later
deterministic verification must exercise; it is not an implementation here.

## 3. Bounded replay generations and Snapshot resync

Every Project Activity cursor belongs to exactly one Replay Generation. A
compaction or archival boundary records the old generation's final Activity
position, then publishes a new generation, its replay floor, and a freshly
authorized canonical Snapshot. The boundary does not fork the canonical
Project Activity chronology or relabel old events.

StoryOS deliberately chooses no cross-generation cursor mapping. A cursor
below the current replay floor returns the protocol's
`activity_cursor_too_old` outcome. The only recovery is Activity Stream Resync:
the Server reauthorizes a fresh Snapshot for the exact Project Scope, filter,
redaction, schema, and current lifecycle; the client resumes strictly after
that Snapshot's Activity position. It must show the generation boundary rather
than present this recovery as continuous byte-for-byte replay.

The old generation's closing position, the new replay floor, Snapshot identity,
compaction evidence, and known availability gaps remain inspectable historical
facts. Retention may remove raw payload and cursor replay service only through
later rules in this specification; it may never silently translate, advance,
or reinterpret an old cursor.

## 4. Mailbox settlement and sealed deduplication

Before a root's Subrun Mailbox Seal commits, every Message ID's durable
delivery, acknowledgement, consumption, and idempotency evidence remains live.
No age, capacity, archive, compaction, or cache rule may remove the evidence
needed to decide a delayed or repeated delivery without changing its original
meaning. Message payload availability remains independently classed, but an
unsettled delivery blocks compacting the record that proves its idempotency.

After the root Seal, a Retention Decision may replace per-Message deduplication
records only by atomically creating a Seal Deduplication Fence. The Fence binds
the exact root and Project Scope, Seal identity and digest, mailbox direction,
sender generation, and its recorded directional high-watermark. There can be
no interval in which both the individual evidence and a valid Fence are absent.

For a sealed sender generation, a late message whose sequence is at or below
the Fence's high-watermark is rejected as a replay or invalid late delivery;
one above it is rejected because that generation is closed. Neither outcome may
consume a payload, schedule a RunStep or Run Wakeup, mutate a parent, retry an
effect, or reopen a terminal Subrun. The Fence is retained with at least the
root's Operational Evidence Floor and is included in archival, export, restore,
and availability-gap evidence while the root itself remains retained.

## 5. Durable checkpoints and expiring query Snapshots

A Run Checkpoint is a durable, Project Scope-bound PostgreSQL projection at an
exact committed Run sequence. It stores no Worker memory, live model session,
lease ownership, reusable authority, or uncommitted output. It may accelerate
recovery of an active Run, but recovery always revalidates the durable records,
current fence, lifecycle, and policy before executing more work. The Retention
Profile controls when a checkpoint is materialized or replaced; discarding it
never discards the Run's source facts.

A Canonical Query Snapshot is instead an authorized, time-bounded read boundary
over durable facts. Its token or materialization may expire under the Retention
Profile and then returns the established `snapshot_expired` resync outcome. A
replay-generation boundary publishes a new Snapshot for Activity Stream Resync,
but neither its full query result nor every historical Snapshot becomes a
permanent archive.

Terminal, sealed Runs retain their Operational Evidence Floor and any required
compaction boundary, not a mandatory permanent Checkpoint or Snapshot copy.
Neither kind of projection may overwrite source facts, serve as a backup, or
claim raw-payload completeness where a known gap exists.

## 6. Archive, compaction, and deletion are different facts

Operational Archive is a reversible, Project Scope-bound cold-retention state:
its payload bytes remain retained and may be inspected or restored only through
an authorized current read or lifecycle operation. Archive excludes the payload
from ordinary retrieval, model context, replay service, cache reuse, and
outbound disclosure. It does not reactivate past permission, grant, destination,
or eligibility and cannot bypass Context Assembly or destination disclosure.

Operational History Compaction instead makes an eligible Compactable Operational
Payload unavailable. Its Evidence Floor, digest, Retention Decision, and known
gap remain; an archive, cache, Provider continuity handle, export, or restore
cannot silently recreate its raw bytes. Compaction is therefore not an archive
state with a different label.

### 6.1 Archive integrity and recovery visibility

Every Operational Archive is one exact Project Scope and one immutable archive
root. Its manifest records the archive profile and revision, source Snapshot
and replay generation, export or archive operation identity, schema/catalog
revision, logical entry classes, serialization and path rules, per-entry
identity and digest, provenance closure, known purged gaps, source lifecycle
state, root digest, integrity proof, and creation/verification times. Counts
are derived from the entries; a count without entry identity and digest is not
an archive proof. Archive inspection validates the manifest and root before
reading any payload and rechecks current User/Project authorization and
redaction/tombstone state.

The following surfaces are intentionally separate:

| Surface | What it preserves and who owns it | What it cannot impersonate |
| --- | --- | --- |
| Operational Archive | Reversibly retained cold bytes and their exact Project-scoped manifest. #64 owns the semantic state and eligibility; physical storage remains #56’s boundary. | It is not ordinary retrieval, Activity replay, a live Run, a canonical Snapshot, or a whole-service backup. |
| Project Export Archive | A transactionally consistent, portable archive of one exact User/Project Scope, usable for a same-User/Project Scope only when that Scope is absent. #56 owns staging; #58 owns the public export route/DTO. | It is not a PostgreSQL base backup/WAL chain and cannot restore a deleted Scope, external Provider state, credentials, or omitted projections. |
| Whole-service Recovery Copy | Physical PostgreSQL base backup/WAL and service recovery chain. #56 owns execution, RPO/RTO, and Recovery Visibility Proof. | It is not a Project Export, an author-readable archive, or proof that a recovered database may expose every Project. Lifecycle ranges and visibility gates still apply. |
| Project Restore | A validated import operation that materializes only the same absent Scope from a valid Project Export Archive, then rebuilds disposable projections and publishes a fresh Snapshot. | It is not a merge, overwrite, identity remap, whole-service restore, archive inspection shortcut, or resurrection after Project Deletion Settlement. |

An archive root is not visible merely because its bytes are present. Before
inspection, export, or restore, the service verifies Scope, manifest/schema
compatibility, all entry digests, provenance closure, known gaps, lifecycle
range, redaction/tombstone state, and the applicable Recovery Visibility Proof.
Any missing range, digest mismatch, cross-Scope entry, invalid credential
binding, or unresolved deletion/redaction decision produces a recovery hold or
typed failure before visibility. Archive verification can establish byte
integrity; it cannot establish that a payload is currently eligible for
context, disclosure, replay, or author use.

Artifact Tombstone remains the author-owned final deletion state defined by the
Artifact contract. It removes an Artifact's payload while preserving the
minimum tombstone and provenance relationship. This specification never turns
an Operational Archive or compaction decision into an Artifact Tombstone, and
never lets an Operational lifecycle operation delete Authoritative State,
Manuscript Revisions, Proposals, or author-owned Artifact payloads. Only the
separate author-owned Project Deletion Settlement in section 10 may begin
whole-Scope deletion.

## 7. Redaction commits before physical cleanup

A Redaction Decision is an immutable, Project Scope-bound lifecycle fact. Its
owning transaction makes the named payload, fragment, or read-view scope
immediately ineligible for current inspection, ordinary retrieval, cache reuse,
Context Assembly, Project Export, and every future outbound disclosure. Archive
status, a Provider continuity handle, a projection, or a delayed cleanup worker
cannot override that ineligibility. A pending unsubmitted operation is cancelled
or reassembled under current policy; a prior committed Manifest, Attempt,
Outbound Disclosure Event, Receipt, or Run Event remains historical evidence.

Redaction Execution is a separate fenced, idempotent physical cleanup process.
It may remove only copies authorized by the committed Decision, including local
payload copies and disposable projections. Its delay never permits use of the
redacted content, and its completion never rewrites history or claims that a
prior destination submission was retracted. Provider-internal retention or use
after a historical disclosure remains unknown rather than being silently
represented as erased.

A Project Export after redaction includes the retained non-secret current
records and the Redaction Decision, lifecycle, digest, provenance, and safe
availability-gap evidence; it excludes the redacted payload. Project Restore
preserves that gap and cannot recreate it from archive, cache, export, backup,
or external destination state.

## 8. Recovery Copy retention and deletion completion

PostgreSQL base backups and WAL segments are bounded Recovery Copies governed by
the Retention Profile. They preserve the Foundation Recovery Service Profile's
complete verifiable recovery chain, but are neither Project Export Archives nor
ordinary read sources. Backup retention and physical cleanup are separate from
the immediate logical effects of Redaction, Tombstone, Archive, Compaction, and
current eligibility.

Before any restored Project Scope becomes readable or may execute new work, a
Recovery Visibility Proof must establish that every recoverable later lifecycle
decision relevant to the selected recovery target has been applied. This
includes Redaction, Tombstone, Retention Decision, availability gap, and
applicable Archive state. A missing or unverifiable lifecycle range, including
an RPO recovery-chain gap, fails closed to a recovery hold; a successfully
booted database is not sufficient proof that an old Project view is safe to
serve.

Physical Deletion Completion is recorded only after the authorized online,
archive, and Recovery Copy windows for the erased payload have expired or been
verifiably cleaned. Until then the author-facing state remains immediately
inaccessible, while its availability may accurately state that recovery-copy
rotation is pending. StoryOS cannot assert deletion from a Project Export the
author already received or from a previously disclosed external destination.

## 9. Diagnostic projections, support, and telemetry

Local logs, tracing, crash diagnostics, and support correlation are Diagnostic
Projections: bounded, non-authoritative, Project Scope-bound records of safe
correlation IDs, categories, reason codes, times, counters, and availability
facts. They contain no default manuscript prose, prompt, research content,
raw Provider/Tool/MCP payload, Project Instruction, Credential, credential
value digest, or hidden reasoning. A support workflow follows these references
back to retained canonical evidence through current authorization; it does not
copy that evidence into a support archive.

Diagnostic Projections have their own short Retention Profile, are excluded
from Project Export and Project Restore, and immediately lose read eligibility
for any source-derived field when the relevant source is Redacted, Tombstoned,
or otherwise currently hidden. A Compactable-unavailable source may leave only
its safe correlation and availability-gap facts visible; it never lets a
Diagnostic Projection reconstruct the missing payload. Removing a Diagnostic
Projection never removes or rewrites the canonical Event, Attempt, Manifest,
Receipt, or lifecycle fact that it may reference.

Any telemetry, crash reporting, or support data sent beyond the StoryOS
Controlled Processing Boundary is a separate Telemetry Disclosure. It remains
subject to current source eligibility, minimum disclosure, destination identity,
manifest-before-egress, Attempt, and Redaction checks; diagnostic purpose never
creates ambient payload access or an export/restore exception.

## 10. Explicit Project deletion lifecycle

Retention never implicitly deletes a whole Project Scope. Only the Project
Author may submit a Project Deletion Request. Its atomic admission immediately
fences new AgentRuns, outbox dispatch, Context Assembly, Project Export,
Project Restore, and outbound disclosure for that exact Scope. Existing work
enters controlled cancellation or recovery; no deletion path treats a missing
result as success or silently repeats an external effect.

Project Deletion Settlement may commit only after every known in-flight
operation has a durable settled result or an explicit OutcomeUnknown record.
It fences future workers and makes the Scope logically unreadable,
unexecutable, unexportable, and unrestorable. It retains only the minimum
deletion decision, availability gap, lifecycle provenance, and known
external-effect evidence needed to state what happened without reconstructing
the deleted Project.

Physical cleanup of online payloads, archives, and Recovery Copies follows the
Retention Profile and ends with Physical Deletion Completion. Project Export
and Project Restore never recreate a deleted Scope; a disaster-recovery path
remains subject to Recovery Visibility Proof and must reapply its deletion
lifecycle before any visibility.

### 10.1 Deletion settlement, Tombstone, and purged-gap evidence

Deletion has one monotonic semantic sequence, even if physical workers retry:

| Phase | Required transition | Evidence that remains authoritative |
| --- | --- | --- |
| Request admitted | The Project Author’s exact command is admitted for the exact Scope; new Run, dispatch, disclosure, export, import, and restore work is fenced. | Admission, command idempotency, request identity, current writer/worker fences, and the list of in-flight operations. |
| Settlement committed | Every in-flight operation has a settled result or explicit `OutcomeUnknown`; the Scope becomes logically unreadable, unexecutable, unexportable, and unrestorable. | `project_tombstones` and `project_deletion_records` retain Scope/User identity, request and settlement identity, decision/profile revision, settlement time, lifecycle provenance, high-watermark/generation boundary, known purged gaps, and known external-effect evidence. |
| Physical cleanup pending | Authorized online, archive, and Recovery Copy workers remove only the copies named by the settled Decision. A retry uses the same cleanup identity and cannot reopen the Scope. | Tombstone, deletion Decision, per-copy manifest/digest and cleanup status, pending recovery-copy rotation, and every gap needed to explain unavailable bytes. |
| Physical Deletion Completion | All named local copies have a verified completion result or an explicit retained recovery boundary; this does not assert deletion at an external destination or from an archive already delivered to the author. | Completion identity, scope of completed cleanup, remaining Tombstone/manifest/gap/provenance floor, and the settled external-effect uncertainty. |

`project_tombstones` is a logical visibility fence, not proof that every byte
has already been physically removed. A purged-gap record names the logical
identity, source generation/position or manifest entry, digest when safe, the
Decision and reason category, and whether the unavailable copy was compacted,
redacted, tombstoned, or physically deleted. It never fabricates replacement
content. The contract does not invent a legal or compliance retention policy;
any such policy must be an explicitly named owner and Profile input. Until a
named Profile and proof permit removal of the minimum deletion evidence, the
Tombstone and gap remain queryable only through the safe lifecycle surface.

## 11. Author inspection and history availability

An authorized author inspection Query must distinguish the historical fact from
the current payload state. For every material Run, Subrun, Event, Attempt,
Manifest, Result, Mailbox Fence, or source closure reference, it reports the
applicable current state without inventing completeness:

| Current state | Author-facing meaning |
| --- | --- |
| retained | The authorized payload remains inspectable under current policy. |
| archived | The payload remains retained but requires an explicit authorized archive inspection or restoration; it is not ordinary model context or replay service. |
| compacted | The historical fact and compacted evidence remain, but the original payload bytes are unavailable. |
| redacted or tombstoned | Current policy prevents payload inspection; only the safe identity, reason category, and availability gap are shown where permitted. |
| recovery hold or Project deletion settlement | The Scope is not safely readable or executable; the view shows only the safe lifecycle state permitted by non-oracle policy. |

Inspection never dispatches a Provider, Tool, MCP, embedding, telemetry, or
support request merely to reconstruct history. It is a current authorized Query
over StoryOS-held evidence; an unavailable payload is not fetched from a cache,
external destination, old client, or provider session. Eval consumes the same
availability facts and cannot convert a limitation into a complete evidence
claim.

## 12. Project Export and Restore

Project Export is an explicit read-only operation at one transactionally
consistent Project Scope boundary. It includes every currently exportable
non-secret canonical record and payload, including authorized archived content,
along with the exact Retention Profile and Decisions, Replay Generation
boundaries, Mailbox Seals and Fences, idempotency outcomes, lifecycle,
provenance, and known availability gaps required to interpret the Project. It
excludes caches, indexes, embeddings, Diagnostic Projections, Query/Snapshot
results, Credential values and value digests, and Provider-held state.

Compacted, redacted, tombstoned, or physically deleted payloads are represented
only by their permitted identity, digest, lifecycle/provenance, and explicit
gap. The export must not invent placeholder bytes, silently omit the gap, or
claim byte-for-byte replay. Export neither broadens a destination grant nor
creates a new disclosure.

Project Restore validates and stages this archive only as the same Project Scope
for the same User where that Scope is absent. It preserves all included
identities, known gaps, lifecycle states, profiles, Seals, Fences, and
idempotency outcomes; it rebuilds disposable projections and leaves unresolved
Credential References Unbound. It never merges, overwrites, remaps identities,
revives unavailable bytes, enables archived content as ordinary context, or
restores a Project after Project Deletion Settlement.

## 13. Boundaries preserved by compaction

Operational History Compaction is not an Artifact Tombstone and does not act on
Authoritative State, Manuscript Revisions, Proposals, author-owned Artifacts,
or their author-initiated deletion path. It does not edit prior Run Events,
Manifests, Attempts, disclosure evidence, source closures, or the public
Project Activity chronology.

It also does not waive current eligibility. A cache, Provider continuity handle,
archived payload, summary, or later reconstruction remains subject to the
current lifecycle, permission, retention, suppression, Context Assembly, and
destination-disclosure checks. A compacted or unavailable payload cannot be
silently redisclosed or resurrected through a cache, Provider, export, or
restore.

## 14. Required invariants and completion constraints

### 14.1 Stable requirement IDs and acceptance ownership

These IDs are the review vocabulary for this contract. They identify semantic
obligations without creating a second physical-table, route, or Event ledger.

| ID | Required result | Acceptance owner |
| --- | --- | --- |
| RET-001 | Every fact and payload is bound to one exact User/Project Scope; an unknown role fails closed to the Evidence Floor. | #64 semantics with #56 scope/RLS proof. |
| RET-002 | Authoritative State, Artifacts, Operational Evidence, Historical Projections, Disposable Projections, and Maintenance/Recovery Copy remain disjoint; rebuildable never means canonical. | #64 classification with the #56 physical catalog. |
| RET-003 | Receipt, Admission, Author Action, Attempt, Manifest, disclosure, idempotency, OutcomeUnknown, and lifecycle evidence survive until their named settlement/visibility proof. | #64 semantics and the later deterministic gate. |
| RET-004 | Run/Subrun terminality, finalization, root Seal, directional high-watermarks, and deduplication Fence precede eligible operational compaction. | #64 semantics and the later Run/Mailbox gate. |
| RET-005 | Run Events, Application Wire Records, Activity identity/sequence/cause, replay generations, floors, and handoffs remain truthful under compression or archive. | #58 wire/replay meaning and #64 lifecycle. |
| RET-006 | Checkpoint, Canonical Query Snapshot, and Activity replay generation stay distinct; expiry and old cursors use the existing typed resync behavior. | #58 protocol meaning and #64 policy. |
| RET-007 | Every Retention Profile/Decision exposes values, eligibility evidence, actor/policy, digest, and availability transition and is non-retroactive. | #64. |
| RET-008 | Operational Archive preserves bytes but excludes ordinary retrieval/context/replay/disclosure; compaction makes eligible payload unavailable and cannot recreate it. | #64. |
| RET-009 | Redaction/Tombstone makes current use ineligible before physical cleanup while preserving safe identity, provenance, reason, digest/gap, and prior disclosure truth. | Artifact owner for Artifact Tombstone; #64 for project/operational lifecycle. |
| RET-010 | Project Deletion is author-requested, settles every in-flight operation or records `OutcomeUnknown`, fences the Scope, and retains minimum deletion evidence. | #64 with #58 delete settlement and #56 visibility. |
| RET-011 | Archive/export manifest, root digest, provenance closure, known gaps, and integrity proof stay distinct from Recovery Copy and Project Restore; restore validates lifecycle before visibility. | #56 physical restore, #58 archive wire, #64 lifecycle inclusion. |
| RET-012 | Author inspection reports retained/archived/compacted/redacted/tombstoned/recovery-hold/deletion state without inventing completeness or dispatch. | #64 and the editor-first author journey. |
| RET-013 | Every numeric input is classified as accepted hard contract, observation, controlled synthetic result, modelled projection, or candidate band with workload/environment/headroom/owner; unmeasured values remain unresolved. | #64 adoption rule; #76 evidence only. |
| RET-014 | Existing PostgreSQL and protocol catalogs/verifiers provide physical-family, route/settlement, public Event, Activity/Snapshot/cursor, and owner-boundary consistency without a second ledger. | Existing verifiers; #60 later supplies executable proof. |

### 14.2 Required invariants and completion constraints

Later deterministic verification must demonstrate at least that:

1. a compacted payload leaves its Event, Attempt, Manifest, Receipt, digest,
   availability gap, and where applicable Seal/Fence intact;
2. no Event identity, sequence, causal relation, or historical disclosure fact
   is semantically deleted by Event segment compression or archive;
3. a cursor below its Replay Generation floor returns the established resync
   outcome and a fresh Snapshot, never a guessed mapping or silent gap;
4. an active or unsealed root cannot lose unsettled mailbox or idempotency
   evidence, while a sealed late message cannot schedule work or reapply an
   effect;
5. a matching public command retry stays idempotent after its large payload is
   compacted, and a differing digest or known old key cannot create a new
   command;
6. redaction blocks cache reuse, Export, Context Assembly, and future egress
   before asynchronous cleanup, while historic Manifest/Attempt facts remain
   truthful;
7. archive, compaction, Tombstone, Recovery Copy, Project Export, Project
   Restore, Diagnostic Projection, and provider continuity cannot resurrect an
   unavailable payload or grant past authority;
8. a restored Scope with a missing lifecycle range remains in recovery hold;
   a deleted Scope never becomes readable through Project Restore; and
9. every state, fence, lifecycle decision, export, restore, Query, cache, and
   cleanup action fails closed across User or Project Scope.

### 14.3 Deterministic proof and downstream handoff

The [deterministic verification and failure-recovery owner](https://github.com/FrankQDWang/StoryOS/issues/60)
later supplies executable fixtures, fault schedules, oracles, and evidence
bundles for this semantic boundary. It must place faults before and after Run
terminal settlement, root Mailbox Seal, Retention Decision, replay-generation
and floor/Snapshot publication, raw payload cleanup, Redaction Decision,
Project Deletion Settlement, archive-root proof, and Recovery Visibility Proof.
Positive traces cover normal replay, compaction, archive inspection, restore,
deletion, and exact idempotent retry. Negative traces cover unsettled
authority, unknown effects, stale cursors, wrong Scope, missing lifecycle
range, corrupt archive proof, and post-deletion access. This contract names
those proof obligations but does not implement the harness or choose release
stages; [the release-baseline owner](https://github.com/FrankQDWang/StoryOS/issues/62)
consumes the finalized semantics, and the terminal audit remains downstream.

### 14.4 Catalog and public-protocol consistency check

The PostgreSQL family crosswalk above is checked against the single #56 JSON
catalog; its verifier derives the family and table coverage. The public
protocol remains checked against the single #58 route/Event catalog; its
verifier derives operation and Event coverage. Retention semantics do not add
or rename a route, DTO, Event, or physical family.

The #58 Accepted settlement query identity remains unchanged for these
asynchronous operations: `deleteProject` uses `getCommand`,
`createReplacementProposal` uses `getProposal`, `createAgentRun` uses
`getAgentRun`, `importProjectArchive` uses `getImportOperation`,
`exportProjectArchive` uses `getExportOperation`, and
`exportHumanReadableManuscript` uses
`getHumanReadableManuscriptExport`. The Activity stream continues to use the
existing generation/floor/Snapshot and `activity_cursor_too_old` semantics;
this contract only supplies the lifecycle evidence and availability transition
that those public meanings inspect.

Exact quantitative values remain Retention Profile inputs calibrated through
[PER-001, PER-002, and PER-006 through PER-008](../../EXPERIMENTAL-TUNING-REGISTER.md). They
must be supplied as versioned effective values before implementation, not
inferred from this semantic contract or exposed as routine author configuration.
No implementation may weaken the confirmed lifecycle, evidence, scope,
recovery, or disclosure boundaries while choosing those values.
