# StoryOS

StoryOS is a novel-project workspace in which the author retains authority over creative truth while Agents, Tools, Skills, and MCP Apps produce inspectable assistance around it.

## Language

**Authoritative State**:
The author-approved current truth of a novel project, including prose, canon, characters, timeline, outline, structure, and author plans. Authority is a binary boundary reached only through an explicit author-authorized domain action; lifecycle, confidence, and lock status do not form authority levels.
_Avoid_: Canon (too narrow), accepted artifact, Agent memory

**Authoritative Revision**:
An immutable version of one authoritative domain object, created through a Direct Author Action, Acceptance, or safe compensation and guarded by an expected prior revision.
_Avoid_: Artifact Revision, mutable row

**Authoritative Commit**:
The project-ordered atomic record of one author-authorized domain transaction, identifying its actor, cause, and all prior and resulting Authoritative Revisions. It provides a global sequence without copying a full project snapshot.
_Avoid_: Project snapshot, Run Event

**Direct Author Action**:
A deterministic, immediately visible change caused through the author's own editor input path against one exact authoritative target under direct manipulation, including manual paste. Bulk, cross-location, not-fully-previsible, Agent-, Tool-, MCP-, or extension-produced changes remain Proposal-gated even when an author click initiates them.
_Avoid_: Author-triggered automation, silent bulk edit

**Operational Record**:
A durable record of execution, context, authorization, usage, validation, or a state transition, such as an AgentRun, RunStep, RunPlan, ContextManifest, ToolCall, Approval, Artifact Lifecycle Event, Domain Receipt, or Run Event. It can reference and produce Artifacts but does not inherit Artifact lifecycle or authority.
_Avoid_: Artifact, temporary log

**Artifact**:
A durable, typed output or evidence item produced during author, Agent, or Tool work. An Artifact may propose or support a change, but never becomes Authoritative State in place.
_Avoid_: Result, blob, authoritative artifact

**Artifact Identity**:
The stable identity whose revisions form one linear history guarded by an expected revision. Alternative or merged work creates a new derived Artifact, while the provenance graph across Artifact identities may form a DAG.
_Avoid_: Content hash, revision branch

**Artifact Revision**:
An immutable snapshot of an Artifact's content and provenance. Derivation and Acceptance always reference an exact Artifact Revision rather than only the evolving Artifact identity.
_Avoid_: Current blob, mutable version

**Content Digest**:
An integrity identifier for an immutable payload that may also support physical storage deduplication. It never replaces Artifact or Artifact Revision identity, so causally distinct outputs remain distinct even when their payloads match.
_Avoid_: Artifact ID, semantic identity

**Provenance**:
The structured lineage that identifies an Artifact Revision's creator, exact source revisions or snapshots, schema version, creation time, and integrity digest. Provenance belongs to the Artifact Revision itself and does not depend on reconstructing a Run log.
_Avoid_: Metadata blob, inferred history

**Provenance Edge**:
A typed relationship from an Artifact Revision to an exact source or cause. Its role distinguishes direct derivation, evidentiary support, context availability, and the Message or goal being answered.
_Avoid_: Source list, citation text

**Creator**:
The single actor or causal execution step that directly produced an Artifact Revision. Earlier authorship and contributions remain visible through Provenance Edges rather than a mutable contributors list.
_Avoid_: Contributors array, original creator only

**External Source Snapshot**:
A Research Artifact Revision containing an immutable captured version of externally retrieved evidence, including when and where it was obtained and an integrity digest of the captured content. Re-fetching creates a new Snapshot, while annotation or correction creates a derived Research Artifact; a live URL alone is not a Snapshot.
_Avoid_: Bookmark, source URL

**Imported Source Snapshot**:
A Research Artifact Revision containing an immutable capture of evidence supplied from a local file or explicit import. Re-importing creates a new Snapshot, while annotation or correction creates a derived Research Artifact.
_Avoid_: Attachment, untracked file

**Research Synthesis**:
A Research Artifact that combines or interprets evidence and binds its claims to exact Source Snapshot revisions through supported-by Provenance Edges.
_Avoid_: Source Snapshot, uncited summary

**Claim**:
A stable, addressable conclusion within a Research Synthesis, linked to the exact evidence that supports it. A Claim remains non-authoritative regardless of confidence or repetition.
_Avoid_: Canon fact, paragraph citation

**Finding**:
A stable, addressable conclusion within an Analysis Report, linked to its project targets and supporting evidence. A Finding may suggest Candidates or Proposals but cannot directly change Authoritative State.
_Avoid_: Decision, automatic fix

**Artifact Lifecycle Event**:
An auditable transition in an Artifact's workflow or retention state, tied to an exact Artifact Revision and attributed to an actor and reason. It changes the Artifact's current state projection without creating a content revision.
_Avoid_: Status edit, metadata revision

**Retention State**:
The common disposition of an Artifact independent of its type-specific workflow: retained, archived, or tombstoned. Archived content is excluded from normal retrieval, while tombstoned is a terminal state that removes content from use and retains only per-revision minimum identity and audit relationships.
_Avoid_: Workflow state, authority level

**Artifact Tombstone**:
The minimum non-content records left for an Artifact and each Revision after the author removes their owned payloads, indexes, and derived caches. They preserve artifact and revision IDs, parent link, kind, creation time, integrity digest, deletion provenance, and necessary relationships without deleting separately referenced Artifacts or shared payloads still referenced by another logical Artifact.
_Avoid_: Archived Artifact, soft-deleted payload

**Purged Source Reference**:
A read-time projection shown when an immutable Provenance Edge resolves to an Artifact Revision Tombstone. The original edge never changes; the projection exposes the removed revision identity and digest and makes lost verifiability explicit.
_Avoid_: Broken link, hidden deletion

**Workflow State**:
The type-specific progress of a Core Artifact through its own review or production process. Workflow State is independent of Retention State and never grants authority by itself.
_Avoid_: Authority level, retention status

**Artifact Closure**:
The reversible open or closed disposition used only by Candidates and Drafts, with the closed reason `dismissed`, `superseded`, or `abandoned`. Deriving a Proposal does not close its source Artifact.
_Avoid_: Proposal resolution, archive

**Supersession**:
A provenance relationship stating that a newer Artifact takes the place of an older one for a stated purpose. It preserves both Artifacts and does not rewrite their revision histories.
_Avoid_: Overwrite, implicit latest

**Core Artifact**:
An Artifact type whose semantics and lifecycle are owned by StoryOS. Any Artifact capable of proposing a change to Authoritative State must be a Core Artifact.
_Avoid_: Built-in output, privileged extension

**Extension Artifact**:
A namespaced and versioned Artifact type produced by a Tool or MCP extension for inspectable data or presentation. A known enabled schema may request a Core Proposal through the Host, while an unknown schema is preserve-and-read-only and cannot invoke Tools, source or produce Proposals, validate, or participate in Acceptance; compatible migration appends a revision, while semantic-identity change derives a new Artifact.
_Avoid_: Plugin-owned state, MCP-owned truth

**Proposal**:
A Core Artifact containing inspectable, core-validatable domain changes together with their targets, base versions, and preconditions. It is the only Artifact kind that can become eligible for Acceptance, but Proposal identity alone never grants eligibility.
_Avoid_: Suggestion, direct write, executable extension

**Proposal Operation**:
A stable, independently resolvable domain change within a Proposal; dynamically calculated diff hunks are never operations. Historical applied or rejected incarnations remain frozen, while reopening creates a new Proposal Revision and retains the operation ID only when target and semantic identity are unchanged.
_Avoid_: Diff hunk, visual change marker

**Proposal Bundle**:
A Proposal subtype whose stable Bundle-level Operations reference exact child Proposal Revisions, selected child Operation IDs, and dependencies without copying child payloads. It declares atomic or ordered-independent execution, and Bundles cannot be nested.
_Avoid_: Mixed-domain Proposal, nested workflow

**Acceptance Eligibility**:
The predicate requiring an exact Proposal Revision to be retained, ready, valid for current targets, open, and selected only over pending Operations. Proposal identity, creator confidence, or a historical Validation Receipt cannot grant eligibility alone.
_Avoid_: Acceptable type, trusted Proposal

**Ready Partial**:
A Proposal generation outcome preserved after production stops before its intended completion. It remains editable but is not eligible for Acceptance until the author explicitly completes the current content or generation finishes.
_Avoid_: Failed Proposal, accepted partial

**Proposal Conflict**:
The condition in which a Proposal's target, base version, or preconditions no longer hold. A conflicted Proposal cannot be accepted or silently rebased and must instead be replaced or explicitly replanned.
_Avoid_: Stale warning, automatic merge

**Proposal Rejection**:
An author's non-destructive decision not to apply selected pending Proposal Operations. Reopen creates a new pending Proposal Revision, which cannot regain Acceptance Eligibility until current targets and preconditions validate successfully.
_Avoid_: Withdrawal, deletion

**Proposal Withdrawal**:
A non-destructive removal of a Proposal from active review by its current producer or the author. Withdrawal is not represented as an author rejection.
_Avoid_: Rejection, deletion

**Undo Acceptance**:
An author-authorized action that safely creates compensating authoritative versions and, when retained source content and a safe linear head allow it, a new Proposal Revision containing the previously applied content against the compensated base. Otherwise it creates a new derived Proposal or Reversal Proposal and never overwrites later conflicting author changes.
_Avoid_: History deletion, editor-only undo

**Reversal Proposal**:
A Proposal that expresses the inverse of an earlier Acceptance against current Authoritative State when a direct Undo Acceptance would conflict with later changes. It requires ordinary inspection and Acceptance.
_Avoid_: Forced rollback, silent undo

**Candidate**:
A Core Artifact presenting one independently reviewable semantic fact or object without carrying an authoritative change command. It can serve as a source for a Proposal but cannot be accepted directly; independently selectable alternatives remain separate Candidates.
_Avoid_: Proposal, pending truth

**Draft**:
A Core Artifact containing editable work that has not been expressed as validated domain changes. It can serve as a source for a Proposal but cannot be accepted directly.
_Avoid_: Proposal, authoritative draft

**Message**:
A Core Artifact representing one visible contribution to a project transcript. It references exact Artifact Revisions for embedded results and views rather than copying their payloads or resolving mutable latest versions.
_Avoid_: Run Event, hidden reasoning

**Research Artifact**:
A Core Artifact that captures or synthesizes source-backed research for later inspection and use. It can support a Proposal but cannot directly change Authoritative State.
_Avoid_: Canon, unsourced note

**Analysis Report**:
A Core Artifact containing a derived evaluation or interpretation of project state, Artifacts, or evidence. It remains advisory even when produced by a trusted Skill.
_Avoid_: Decision, authoritative assessment

**Tool Artifact**:
A Core Artifact envelope for durable Tool or Service output that has no more specific Core Artifact kind, including permitted namespaced extension schemas. A domain-recognized result uses its specific kind with the ToolCall as Creator rather than adding a duplicate Tool Artifact wrapper.
_Avoid_: Tool result event, direct write

**App View Artifact**:
A Core Artifact that preserves a transcript-embedded MCP App as a reproducible View Descriptor: fixed UI resource digest, protocol version, exact input revisions, authorized host-context snapshot, optional schema-bound view state, and static fallback. It never stores a live iframe runtime or controls authoritative domain data.
_Avoid_: App-owned state, iframe snapshot

**Derivation**:
The creation of a new Artifact from exact source Artifact Revisions while preserving those sources and their lineage. Derivation never changes a source Artifact's kind in place.
_Avoid_: Conversion, type mutation

**Acceptance**:
An author-authorized action that applies selected operations from an exact eligible Proposal Revision through a StoryOS-owned domain handler. Domain Proposal selections are atomic, while Proposal Bundles obey their explicit atomic or ordered-independent policy.
_Avoid_: Promotion, status flip, overwrite

**Acceptance Receipt**:
The durable outcome of one Acceptance attempt, identifying its command digest and idempotency key, exact Proposal Revision and selected operations, prior and resulting Authoritative Revisions, zero or more Authoritative Commits, child Receipts, and result. Bundle progress may be derived across linked attempt Receipts.
_Avoid_: Success message, accepted Artifact

**Domain Receipt**:
An immutable StoryOS Core record of a validation or domain-command attempt, including success, refusal, redirection, and outcomes with no Authoritative State change. A Domain Receipt is neither an Artifact nor Authoritative State and has no revision, derivation, retention, or Acceptance lifecycle.
_Avoid_: Artifact, log text

**Undo Acceptance Receipt**:
The immutable, idempotent Domain Receipt produced by an Undo Acceptance attempt, identifying the original Acceptance Receipt, command digest, and one outcome: compensated with a Commit, reversal required with a Reversal Proposal, or unavailable with a reason.
_Avoid_: Compensation Receipt, undo message

**Validation Receipt**:
The immutable result of StoryOS Core validating an exact Proposal Revision against exact target versions, domain invariants, and preconditions. It remains true for those historical inputs but ceases to be current or applicable after any relevant Proposal or target change.
_Avoid_: Model confidence, creator-approved flag
