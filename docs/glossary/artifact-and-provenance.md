# Glossary: Artifact, provenance, and Artifact kinds

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

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
An integrity identifier for an immutable payload computed under one exact Digest Profile that may also support physical storage deduplication. It never replaces Artifact or Artifact Revision identity, so causally distinct outputs remain distinct even when their payloads match.
_Avoid_: Artifact ID, semantic identity, unprofiled hash

**Digest Profile**:
The versioned, purpose-specific contract that defines the exact typed fields and canonical bytes covered by a Content Digest or command digest. A Profile is reproducible across StoryOS runtimes, never silently normalizes authoritative prose, and cannot be reused as a different integrity claim merely because the bytes match.
_Avoid_: Runtime serialization, implicit Unicode normalization, universal hash meaning

**Provenance**:
The structured lineage that identifies an Artifact Revision's creator, exact source revisions or snapshots, schema version, creation time, and integrity digest. Provenance belongs to the Artifact Revision itself and does not depend on reconstructing a Run log.
_Avoid_: Metadata blob, inferred history

**Provenance Edge**:
A typed relationship from an Artifact Revision to an exact source or cause. Its role distinguishes direct derivation, evidentiary support, context availability, and the Message or goal being answered.
_Avoid_: Source list, citation text

**Creator**:
The single closed producer identity that directly created an Artifact Revision: exact admitted Author, AgentRunStep, ToolCall, Import Operational Record, Core Transition Receipt, or Editor Recovery Creator. MCP and extension work resolves through ToolCall, model work through AgentRunStep, and raw browser-local state is never a Creator; earlier contributions remain visible through Provenance Edges.
_Avoid_: Generic System, browser event, model, MCP App, contributors array

**External Source Snapshot**:
A Research Artifact Revision containing an immutable captured version of externally retrieved evidence, including when and where it was obtained and an integrity digest of the captured content. Re-fetching creates a new Snapshot, while annotation or correction creates a derived Research Artifact; a live URL alone is not a Snapshot.
_Avoid_: Bookmark, source URL

**Imported Source Snapshot**:
A Research Artifact Revision containing an immutable capture of evidence supplied from a local file or explicit import. Re-importing creates a new Snapshot, while annotation or correction creates a derived Research Artifact.
_Avoid_: Attachment, untracked file

**Evidence Locator**:
A snapshot-relative, media-specific coordinate that lets an inspector re-find exact content inside one Source Snapshot Revision. It may identify structured web content, a PDF page and region, a media time range with transcript Revision, or a dataset version and replayable record or result; a live location alone is not evidence.
_Avoid_: Live URL fragment, current page position, citation text without a Snapshot

**Evidence Relation**:
A Claim-scoped relationship to an exact Source Snapshot Revision and Evidence Locator that distinguishes supporting, opposing, or qualifying evidence and names the exact Claim or subclaim addressed. `available_as_context` remains independent and proves neither use nor support, while derived conclusions retain their intermediate analysis rather than claiming that a source stated them directly.
_Avoid_: Source list, whole-document citation, context availability as support

**Research Synthesis**:
A Research Artifact that combines or interprets evidence and binds its claims to exact Source Snapshot revisions through supported-by Provenance Edges.
_Avoid_: Source Snapshot, uncited summary

**Research Claim**:
A stable, addressable, independently evaluable conclusion within a Research Synthesis, linked to exact evidence through Evidence Relations. Evidence that addresses only part of a compound conclusion must target an explicit subclaim or cause the Claim to split; a Claim remains non-authoritative regardless of evidence, confidence, or repetition.
_Avoid_: Claim (too generic), Canon fact, paragraph citation

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
The reversible open or closed disposition used only by Candidates and Drafts, with the closed reason `dismissed`, `superseded`, or `abandoned`. Derivation alone does not close a source Artifact, but an owning typed source-consuming transition may atomically close the exact source as `superseded` while preserving its immutable history.
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

**Candidate**:
A Core Artifact presenting one independently reviewable semantic fact or object without carrying an authoritative change command. It can serve as a source for a Proposal but cannot be accepted directly; independently selectable alternatives remain separate Candidates.
_Avoid_: Proposal, pending truth

**Memory Document**:
A versioned, non-authoritative Artifact containing a generated conversation summary, consolidated memory notes, or memory navigation summary under one Project Scope. Its source references aid inspection and fresh lookup without proving every statement or a complete semantic influence history.
_Avoid_: Authoritative fact, per-claim Candidate, Admission Decision, hidden model state

**Memory Note**:
An inspectable, author-requested Artifact containing plain-language guidance to add, correct, or forget generated Memory, linked to the requesting author Message. Recording the note requests later consolidation; it does not prove publication, physical deletion, an access prohibition, or a change to Authoritative State.
_Avoid_: Author Preference, semantic suppression rule, deletion receipt, context exclusion

**Memory Publication**:
The durable, Project Scope-bound identity of one complete published set of exact Memory Document revisions and its navigation summary, linked to the maintenance outcome that produced it. Current publication and current read availability are separate: a newer set does not rewrite earlier inputs or make a restricted older payload readable.
_Avoid_: Latest Artifact alias, search index, authoritative fact, semantic correctness proof

**Inferred Preference**:
A non-binding interpretation of prior author action or feedback that can appear in Agent Memory with its sources and uncertainty. Repetition, confidence, author silence, or prior use never turns it into an authoritative constraint or lets it override a current author instruction.
_Avoid_: Hidden policy, binding preference, implicit instruction, procedural memory

**Operational Lesson**:
A non-binding account of a potentially useful execution outcome, with relevant sources, scope, and uncertainty. It may advise the Agent but cannot change a SkillPackage, ToolSpec, Capability, policy, or execution permission.
_Avoid_: Executable instruction, promoted Skill, verified general rule

**Draft**:
A non-authoritative Core Artifact containing editable work that has not been expressed as validated domain changes, including Plan, Refused Edit, and Recovery Drafts. It follows Artifact revisions, common retention, and reversible Draft closure, may source a Proposal, and cannot be accepted directly.
_Avoid_: Proposal, authoritative draft

**Message**:
A Core Artifact representing one visible contribution to a project transcript. It references exact Artifact Revisions for embedded results and views rather than copying their payloads or resolving mutable latest versions.
_Avoid_: Run Event, hidden reasoning

**Project Conversation**:
The durable, author-visible grouping of Messages under one stable identity and exact Project Scope, which can contain contributions from multiple AgentRuns. A new Project Conversation has a separate identity and Provider continuation boundary without changing Project Agent identity; no AgentRun, browser session, or Provider reference defines that identity.
_Avoid_: AgentRun, Provider session, Project Agent identity, browser tab

**Research Artifact**:
A Core Artifact that captures or synthesizes source-backed research for later inspection and use. It can support a Proposal but cannot directly change Authoritative State.
_Avoid_: Canon, unsourced note

**Analysis Report**:
A Core Artifact containing a derived evaluation or interpretation of project state, Artifacts, or evidence. It remains advisory even when produced by a trusted Skill.
_Avoid_: Decision, authoritative assessment

**Eval Surface**:
A future observation surface outside the MVP, intended as a separate Web page outside the main writing interface with separate APIs. Its intended scope is the Agent input and Text or Function Calling output at each step, Trace, token usage, and elapsed time; no design or implementation is authorized now.
_Avoid_: MVP feature, assessment platform, main writing interface

**Tool Artifact**:
A Core Artifact envelope for durable Tool or Service output that has no more specific Core Artifact kind, including permitted namespaced extension schemas. A domain-recognized result uses its specific kind with the ToolCall as Creator rather than adding a duplicate Tool Artifact wrapper.
_Avoid_: Tool result event, direct write

**Derivation**:
The creation of a new Artifact from exact source Artifact Revisions while preserving those sources and their lineage. Derivation never changes a source Artifact's kind in place.
_Avoid_: Conversion, type mutation
