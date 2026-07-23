# Run Event, Mailbox, Snapshot, Retention, and Archival Semantics

- Status: accepted
- Wayfinder resolution: [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64)
- Canonical glossary: [CONTEXT.md](../../CONTEXT.md)
- Storage and isolation boundary: [PostgreSQL Project Storage, Isolation, and Migration Contract](postgresql-project-storage-isolation-and-migration-contract.md)
- Protocol boundary: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)
- Context and disclosure boundary: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Trust boundary: [StoryOS Service, Client, and External Trust Boundaries Threat Model](storyos-service-client-external-trust-boundaries-threat-model.md)
- Eval evidence boundary: [Foundation Evidence for the Standalone Eval Surface](eval-evidence-foundation.md)
- Measurement input: [Representative Writing-Path Performance and Storage-Growth Envelope](https://github.com/FrankQDWang/StoryOS/issues/76)
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
owns representative measurements only; this specification adopts their
accepted retention, checkpoint, compaction, and storage-growth consequences
through [PER-002](../../EXPERIMENTAL-TUNING-REGISTER.md).
Measurements alone never change a profile or authorize cleanup.

### 2.2 Retention Profile and non-retroactive decisions

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

### 2.3 Command idempotency, Attempt, and outbox floors

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

### 2.4 Event, Activity, and wire-evidence preservation

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

Exact quantitative values remain Retention Profile inputs calibrated through
[PER-002 and PER-006 through PER-008](../../EXPERIMENTAL-TUNING-REGISTER.md). They
must be supplied as versioned effective values before implementation, not
inferred from this semantic contract or exposed as routine author configuration.
No implementation may weaken the confirmed lifecycle, evidence, scope,
recovery, or disclosure boundaries while choosing those values.
