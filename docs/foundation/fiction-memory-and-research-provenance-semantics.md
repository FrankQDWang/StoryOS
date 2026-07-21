# Fiction Memory and Research Provenance Semantics

- Status: accepted
- Wayfinder resolution: [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51)
- Canonical glossary: [`CONTEXT.md`](../../CONTEXT.md)
- Parent domain model: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Authority decision: [ADR 0001](../adr/0001-separate-authoritative-state-artifacts-and-operational-records.md)
- Ownership and deployment decision: [ADR 0004](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)
- Research inputs: [Fiction memory and research provenance semantics](../research/fiction-memory-and-research-provenance-semantics.md) and [Durable, inspectable Agent memory architecture](../research/durable-inspectable-agent-memory-architecture.md)

## 1. Scope and authority

This specification defines the logical contract for Project Scope-bound Agent
continuity across threads and AgentRuns, typed fiction assertions, durable
memory candidates, admission and lifecycle, author preferences, operational
lessons, research claims and evidence, suppression, and rebuildable retrieval
projections.

It refines the existing separation among Authoritative State, Artifacts, and
Operational Records. It does not create another durable truth space and does
not authorize implementation work. Database layout, wire representations,
retrieval algorithms, user-interface layout, and retention periods remain with
their owning Wayfinder tickets.

Every Settled Source, Candidate, Admission Decision, Entry, Suppression,
Evidence Relation, retrieval row, embedding input, cache key, and historical
context reference binds one trusted `ProjectScope { owner_user_id, project_id }`.
Project Scope is checked before discovery and after index lookup; a global
namespace, opaque object ID, content digest, or caller-supplied owner cannot
cross either the User or Project boundary. Missing or mismatched scope fails
closed and cannot be softened by relevance, confidence, or rank.

StoryOS serves Discovery Writing. Long-term memory supports continuity around
the passage and creative choices currently placed before the Agent; memory
maintenance never interrupts active writing with an unsolicited confirmation
request.

## 2. Durable information spaces and the Agent Memory view

StoryOS preserves three kinds of durable information:

| Durable space | Owns | Does not prove |
|---|---|---|
| Authoritative State | author-approved current fictional truth, creative decisions, and effective author constraints | that every Agent inference or research claim is true |
| Artifacts | inspectable candidates, drafts, research, analysis, feedback, inferred preferences, operational lessons, and other produced content | authority merely because content was retained, cited, or admitted |
| Operational Records | what a Run, Tool, policy, approval, failure, retry, or lifecycle transition actually did | a lasting creative fact, preference, Tool rule, or Skill |

`Agent Memory` is a source-bearing use-case view over exact records in those
spaces. It may select, summarize, and index eligible content for a current
need, but it never owns an independently writable fact. Every durable inference
remains a typed Artifact, and every authoritative change still uses its owning
author-authorized domain command.

Existing authoritative objects, Research Claims, and Operational Records are
not copied into generic memory objects merely to make them searchable. Search
access to an existing source is a projection over that source. Newly inferred
or generalized durable content is a typed Memory Candidate and must pass Memory
Admission before ordinary long-term recall.

If a memory projection conflicts with its source, the source wins. The
projection immediately loses current eligibility and is invalidated or rebuilt.
Repeated retrieval, model synthesis, confidence, age, or author silence never
raises authority.

## 3. Working Context, settlement, and candidate extraction

### 3.1 Working Context is not long-term memory

The editor buffer, current model stream, current Tool progress, active retry,
and Run-local working material may enter the current Run's bounded Working
Context. They cannot directly source a long-term Memory Candidate.

Long-term extraction begins only from an exact `Settled Source Version`: a
durably committed source revision or typed terminal outcome with defined
settlement meaning. Eligible examples include a committed author edit, a
settled Proposal outcome, a terminal AgentRun or ToolCall outcome, and complete
author feedback that has been sent and persisted.

Settlement outcomes retain their owning distinctions. AgentRun outcomes must
not flatten success, exhausted failure, cancellation, supersession, and partial
completion. ToolCall outcomes must not flatten success, retryable failure,
terminal failure, cancellation, and timeout. A retry that later succeeds does
not become a lasting failure lesson, and author cancellation does not become
evidence about Tool capability.

### 3.2 Extraction is asynchronous, typed, and idempotent

Extraction is a post-settlement derivation. It records the exact source
identity and revision, settlement fact, extraction-policy version, candidate
kind, supported scope, creator, and derivation evidence. Reprocessing the same
semantic input under the same policy is idempotent. A later extraction-policy
version is a new, attributable derivation and may explicitly replace prior
results.

The protocol ticket owns the concrete idempotency representation, but it must
distinguish at least the extraction-policy version, source type and identity,
source revision, candidate kind, and normalized claim scope. Event replay,
Worker restart, and repeated queue delivery must not duplicate one candidate.

When a source revision changes, candidates derived from the former revision
are re-evaluated. A replacement is extracted from the new settled revision; it
never edits the old candidate in place.

### 3.3 Source meaning constrains extraction

- An accepted Proposal may create Authoritative State through the existing
  Acceptance contract. Memory indexes the resulting authoritative revision; it
  does not derive a second authoritative fact.
- A rejected Proposal is rejection evidence and may source a bounded Inferred
  Preference. Rejection alone does not establish a general preference.
- A withdrawn Proposal records withdrawal and ordinarily supports no preference
  inference.
- One terminal ToolCall proves that one execution had that outcome. A reusable
  Operational Lesson requires multiple comparable, settled, causally
  independent Operational Records plus opposing evidence where available.
- A final text difference does not by itself explain author intent. Extraction
  uses typed domain outcomes and feedback rather than freely assigning meaning
  to a content diff.
- An existing memory projection is never an independent original source.
  Evidence independence is evaluated at the underlying Authoritative Revision,
  Artifact Revision, or Operational Record.

## 4. Memory Candidate and Memory Admission

Candidate extraction and ordinary recall admission are independent stages.

A `Memory Candidate` says only that one typed, source-bearing inference may be
worth retaining. It is not necessarily true, sufficiently supported, broadly
applicable, permitted, or eligible for recall. Initial typed candidate kinds
are `FictionAssertionCandidate`, `InferredPreference`, and
`OperationalLesson`.

The Candidate is an Artifact. Each Admission Decision and appended eligibility
or lifecycle fact is an immutable Operational Record. An Admitted Memory Entry
is the non-authoritative projection of one exact Candidate Revision under one
exact passing Admission Decision; if materialized for audit or access, that
materialization is immutable and remains rebuildable from those canonical
records.

Memory Admission evaluates one exact candidate and exact source set under one
policy revision. It checks at least:

- the candidate kind is consistent with its source classes;
- every claim is actually supported by the cited source content;
- project, work, continuity, branch, character, scene, story-time, narrative,
  Agent, task, and environment scopes are no broader than the evidence;
- a local or single event has not been generalized into a lasting rule;
- the claim does not conflict with current applicable Authoritative State;
- every exact source revision still exists and remains applicable;
- no later revision, correction, invalidation, or supersession has displaced it;
- current suppression, privacy, permission, role, and project-isolation rules
  allow recall; and
- claimed independent support resolves to independent original sources rather
  than a cycle of derived memory objects.

A passing decision creates an `Admitted Memory Entry`, which remains a
non-authoritative projection of that candidate. Other decisions may leave the
candidate pending, quarantined, suppressed, rejected, invalidated, superseded,
or expired. Those histories remain inspectable but do not participate in
ordinary recall.

Retrieval ranking chooses only among currently eligible sources and Admitted
Memory Entries. Ranking never decides truth, evidence sufficiency, permission,
admission, or authority.

## 5. Fiction Assertion semantics

A `Fiction Assertion` has three orthogonal meanings:

1. `Proposition`: what is asserted about which project subjects;
2. `Story Scope`: the work, fictional world or continuity, branch, story time,
   scene, and narrative position where the Proposition applies; and
3. `Epistemic Scope`: whether it is project-level fictional truth or something
   a character, narrator, or in-fiction source knows, believes, suspects,
   claims, remembers, or retells.

Epistemic Scope separates the holder from the relation. A character claiming a
Proposition is not equivalent to believing it. A character belief cannot
change project-level truth, and project-level truth does not automatically
become character knowledge, dialogue content, or narratively permitted
revelation.

Story-world validity, narrative revelation, and StoryOS audit time are also
distinct. An event may have happened earlier in story time but remain unknown
to a viewpoint character or reader until a later narrative position. The time
when StoryOS stored an Assertion carries no story-world meaning.

Two Fiction Assertions conflict only when their Propositions cannot coexist
and their relevant work, world or continuity, branch, story time, and
comparable epistemic scopes overlap. Surface disagreement explained by a
different time, branch, holder, epistemic relation, or narrative position is
not a domain conflict.

## 6. Preferences, lessons, and executable behavior

An `Author Preference` is an explicit, future-facing, scope-bounded author
constraint in Authoritative State. It is created or changed only through its
author-authorized domain path. Memory may retrieve the source but cannot create
or widen the constraint.

An `Inferred Preference` is a non-binding Candidate derived from author action
or feedback. It remains local to the evidenced scope, can be contradicted by a
current instruction, and never becomes binding through repetition, confidence,
silence, admission, or retrieval.

An `Operational Lesson` is a non-binding Candidate generalized from multiple
comparable and causally independent settled Operational Records. It preserves
supporting and opposing records, task and environment scope, relevant component
versions, inference strength, and an expiry or re-evaluation boundary. Once
admitted, it may appear only as a weak advisory input.

Only a separately governed SkillPackage, StoryOS ToolSpec, Capability, or
project policy can decide executable behavior. Turning a lesson into one of
those objects requires its owning change and verification process; the lesson
is neither modified nor promoted.

StoryOS does not use `Episodic Memory`, `Semantic Memory`, or `Procedural
Memory` as product types, permission boundaries, lifecycle owners, or primary
context categories. Specific execution events remain Operational Records;
fictional truth and character attitudes remain Fiction Assertions; external
conclusions remain Research Claims; preferences and lessons use their exact
terms; executable behavior remains with Skills, Tools, Capabilities, and policy.

## 7. Research Claims and exact evidence

Every `Research Claim` binds through an Evidence Relation to one or more exact,
immutable Source Snapshot Revisions and to an Evidence Locator that can re-find
the relevant content inside each referenced revision.

Locator semantics depend on the captured medium:

- Web evidence identifies stable captured blocks, heading structure,
  paragraphs, or text ranges inside the snapshot.
- PDF evidence identifies pages and page-relative paragraphs, text blocks, or
  coordinate regions.
- Video and audio evidence identifies a time range and the exact transcript
  revision used.
- Dataset evidence identifies the data version and record range or replayable
  query, with a materialized result when reproducibility requires it.
- Imported-document evidence identifies the file snapshot revision and its
  page, section, or structured-content position.

A mutable URL, current page, title, or search-result link may aid discovery or
display but is never sufficient evidence. Fetching the same URL again creates
a new Source Snapshot Revision and cannot replace the evidence cited by an
older Claim.

Evidence Relations distinguish support, opposition, and qualification and name
the exact Claim or subclaim addressed. Partial evidence requires a split Claim
or an explicit subclaim reference. Computation and cross-source synthesis retain
their intermediate Research Synthesis, Analysis Report, computation, and
derivation; a source that supports an input is not represented as directly
stating the derived conclusion.

`available_as_context` is independent from evidence. It proves only that a
source was supplied to a Run or model context, not that it was used or that it
supports any result. The existence of supporting evidence also does not by
itself make a Claim verified; current assessment considers opposing and
qualifying evidence, source scope, and unresolved conflict.

## 8. Immutable history and current lifecycle

Memory Candidates, Admission Decisions, Admitted Memory Entries, and their
source references are immutable. Correction and eligibility changes append
lifecycle facts and relationships:

| Relation | Meaning |
|---|---|
| `corrects` | a successor fixes an erroneous statement under the same applicable scope and effective time |
| `supersedes` | a once-applicable item is replaced from an explicit version, decision, or valid-time boundary |
| `invalidates` | an item loses recall eligibility without requiring an equivalent replacement |

Scope refinement is preferred when apparent conflict is explained by a work,
continuity, branch, story time, scene, narrative position, epistemic holder, or
epistemic relation. Neither statement is marked wrong merely because a flat
projection made them appear contradictory.

Once an entry is corrected, invalidated, or superseded for the current scope,
it immediately exits ordinary Agent Memory. If a replacement has not passed
Admission, StoryOS tolerates a temporary recall gap rather than continuing to
serve known-wrong content.

Every Run records the exact Memory Entries, source revisions, and Admission
Decisions actually supplied through its Project Scope-bound Context Assembly
Manifest and Step Snapshot.
Later correction never rewrites that historical context, so StoryOS can still
explain an earlier Agent decision using the information eligible at that time.

Current Agent Memory is a projection computed from immutable content and
appended lifecycle facts. Immutable history does not imply permanent current
eligibility.

## 9. Archive, Tombstone, and Memory Suppression

These are orthogonal semantics:

| Control | Content retained | Ordinary recall | Reversible | Prevents equivalent re-extraction |
|---|---:|---:|---:|---:|
| Archive | yes | no | normally yes | no |
| Tombstone | only minimum non-content proof | no | normally no | according to Storage and Retention policy |
| Memory Suppression | source unchanged | no | normally yes | yes, within its exact scope |

Archive preserves content, provenance, and lifecycle history while removing an
object from normal use. It does not stop later analysis of a still-valid source
from producing a materially different Candidate.

Tombstone performs an audited physical purge or cryptographic destruction for
author deletion, safety, privacy, or retention obligations. Only a minimum
non-content deletion proof remains. The Storage and Run-retention tickets own
physical fan-out and what protected evidence may legally remain.

Memory Suppression is an authoritative author or policy control under one exact
Project Scope over a work, Agent, source set, candidate kind, object set, or
semantic scope.
It changes no source content, but it applies during extraction, admission,
current-view construction, index rebuild, and retrieval. Existing matching
entries immediately leave ordinary recall. Replay, Worker recovery, policy
upgrade, source reprocessing, and index rebuild cannot recreate the prohibited
inference.

The authoritative Suppression instruction belongs to project policy state and
its immutable lifecycle records, never to a Candidate, Admitted Memory Entry,
or retrieval index.

An ordinary request to stop remembering or recalling content creates Memory
Suppression unless the author explicitly requests physical deletion. A
Suppression target should use object identities, scopes, and non-reversible
fingerprints where possible rather than copying prohibited sensitive content.
Lifting Suppression restores only eligibility for current re-evaluation under
current sources and policy; it does not restore an old Admission Decision.

Suppression and Tombstone never rewrite a historical Run's recorded context.

## 10. Rebuildable retrieval projections

Full-text, vector, graph, and other semantic retrieval structures are
disposable access projections. They own no domain identity, source truth,
Admission Decision, lifecycle fact, Suppression, Evidence Relation,
Provenance, or permission.

Every physical or logical retrieval namespace and cache key includes the exact
Project Scope. Index lookup may produce candidates only inside that scope, and
each hit is revalidated against canonical ownership before any content is
returned. Embedding generation through an external API is an Outbound
Disclosure operation and cannot use a cross-project batch or global text cache.

Index-local document, vector, node, and edge identifiers are not durable object
identities and cannot be cited by Runs, Artifacts, Claims, or audit records.
Index records may copy qualification fields for filtering and performance, but
those fields are non-canonical and identify the exact Project Scope, domain
object, source revision, qualification revision, index policy, and content/build
version from which they were projected.

Before a retrieval result enters Context Assembly, StoryOS fails closed unless
current domain records or a version-bound eligibility projection revalidate:

- current Admission and lifecycle eligibility;
- applicable Memory Suppression;
- current source revision and replacement relations;
- retention and Tombstone state;
- project, work, continuity, branch, character, story-time, narrative, and
  epistemic scope;
- caller permission; and
- any required evidence availability.

Similarity, text match, graph distance, and ranking scores represent relevance
only. They never mean truth, authority, evidence sufficiency, admission, or
execution permission. A missing, corrupt, deleted, migrated, or re-sharded
index leaves the canonical Candidate, Admission, Lifecycle, Suppression,
Provenance, and Evidence records intact and rebuildable.

Suppression and Tombstone revoke current recall eligibility immediately even
when physical cache and index cleanup is still in progress. Cleanup is required
but does not itself constitute the domain control.

## 11. Author interaction and control

Active writing is not paused for memory housekeeping. Automatic extraction,
admission, invalidation, expiry, and index maintenance run after source
settlement or in background work under their ordinary Run and capability
contracts.

An author interaction with remembered content routes to one of the owning
semantics:

- revise the exact source through its domain path;
- inspect, confirm, reject, or replace a Candidate through its Artifact and
  Admission lifecycle;
- create or change an explicit Author Preference through its authoritative
  command; or
- create Memory Suppression when the intended effect is to stop recall.

The author never edits a search document, vector, graph node, or consolidated
memory blob as if that projection were a new source of truth. Deliberate memory
inspection and settings may exist away from the uninterrupted writing flow.

## 12. Ownership handoff

This specification fixes the semantic invariants consumed by downstream work:

| Ticket | Owns next | Must preserve from this specification |
|---|---|---|
| [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54) | mandatory and dynamic selection, ranking, context budgets, explanation, author controls, and outbound disclosure | qualification before ranking, no authority from relevance, exact source and Admission evidence, no interruptive memory confirmation |
| [Specify the PostgreSQL Project Storage, Isolation, and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56) | PostgreSQL records and transactions, separately stored payloads if any, indexes, projection rebuild, physical deletion, backup, and migration | canonical domain records survive index loss; every copy preserves Project Scope; Suppression and Tombstone fan out to every derived copy without becoming index-only state |
| [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | exact identities, DTOs, commands, events, relations, idempotency keys, compatibility, and errors | immutable candidates and decisions, exact source revisions, append-only lifecycle, deterministic replay safety |
| [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64) | Run, Message, Context Assembly Manifest, snapshot, mailbox, and recovery-evidence retention | historical Runs retain what they actually used; every record preserves Project Scope; Working Context is not silently converted into long-term memory |

No new follow-up ticket is required: the clarified work is already owned by
these existing tickets.

## 13. Normative invariants

1. Agent Memory is a typed, source-bearing, rebuildable use-case view, not a
   fourth durable truth space.
2. No Memory Candidate is extracted from an unsettled source or live working
   state.
3. Extraction, Admission, and retrieval ranking are independent decisions.
4. Admission grants ordinary recall eligibility, never authority.
5. Existing durable source objects are not duplicated into generic memory
   objects merely for search.
6. Every durable inference remains a typed Artifact with exact source lineage.
7. Authoritative State changes only through its owning author-authorized domain
   command.
8. Fiction Proposition, Story Scope, and Epistemic Scope are orthogonal.
9. Story time, narrative position, and audit time are distinct.
10. Evidence availability, support, opposition, qualification, derivation, and
    context availability are distinct relations.
11. A mutable location or current remote representation is never sufficient
    historical evidence.
12. Candidate, Admission, Entry, and historical Run context are never corrected
    by in-place rewrite.
13. Known-invalid content leaves ordinary recall even when no replacement is
    admitted.
14. Memory Suppression survives extraction replay and every projection rebuild.
15. Archive, Tombstone, and Memory Suppression remain independent controls.
16. No retrieval index owns canonical domain meaning or durable identity.
17. Every memory source, lifecycle record, index entry, cache key, retrieval
    result, and embedding operation validates one exact Project Scope before
    discovery, use, disclosure, or reuse.
18. Current qualification and permission are revalidated before context use.
19. Operational Lessons advise but never execute or self-promote.
20. Active writing is never interrupted solely to confirm memory maintenance.
21. Historical Runs preserve the exact memory and source revisions actually
    supplied at the time.
