# Fiction Memory and Research Provenance Semantics

- Status: accepted
- Wayfinder resolution: [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51)
- Canonical glossary: [`CONTEXT.md`](../../CONTEXT.md)
- Parent domain model: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Context and disclosure owner: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Authority decision: [ADR 0001](../adr/0001-separate-authoritative-state-artifacts-and-operational-records.md)
- Ownership decision: [ADR 0004](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)
- Memory decision: [ADR 0035](../adr/0035-use-background-generated-project-memory.md)

## 1. Scope and authority

Agent Memory supports continuity across Project Conversations and AgentRuns.
StoryOS uses the Codex CLI mechanism: extract useful context from eligible prior
conversation records in background work, consolidate readable memory documents,
supply a bounded navigation summary, and let the Agent search and read details
when relevant. Grok Build is a supporting reference, not the deciding contract.

Memory is separate from active model-context compaction, durable conversation
history, and Provider continuation. It is an aid to recall, not a second source
of fictional truth or a rule system for interpreting every author statement.
General model prompts guide extraction, consolidation, and retrieval. StoryOS
does not classify ordinary creative requests into a special exclusion registry,
per-claim admission lifecycle, or deterministic semantic removal graph.

All memory inputs, documents, notes, jobs, indexes, tool reads, and disclosures
bind one trusted `ProjectScope { owner_user_id, project_id }`. Background work
cannot pool content across Projects or Users. A content hash, similar wording,
or shared Provider account grants no access. The PostgreSQL and isolation
contract remains in force; a Markdown representation is not an unmanaged local
store or permission to copy the Codex runtime.

This is a semantic contract. It does not implement Memory, choose physical
paths or database tables, calibrate budgets, or release a product stage.

## 2. Durable information spaces and generated documents

StoryOS retains three durable information spaces:

| Space | Role in Memory |
|---|---|
| Authoritative State | current author-owned prose, fiction facts, and explicit constraints that the Agent can read through their domain tools |
| Artifacts | conversation Messages, inspectable source material, generated Memory Documents, and author-requested Memory Notes |
| Operational Records | exact maintenance inputs, execution outcomes, publication and usage observations, and historical context evidence |

A `Memory Document` is generated, non-authoritative text with a stable document
identity and revision. The set includes per-conversation summaries, consolidated
notes, and a short navigation summary. Markdown supports bounded search and
read; no vector database, graph, or embedding call is required for ordinary
Memory. Storage owns durable revisions and the current published document set.
Memory Documents and Memory Notes use the existing Tool Artifact family, with
their content role defined by this contract. They add no top-level Artifact
family or Creator: model work binds an AgentRunStep, and a memory tool write
binds its ToolCall. Background Workers schedule that existing bounded work.

The consolidation Agent can replace a document's current content and remove
obsolete generated documents. Publication retains the revision identity needed
by already recorded uses, subject to retention. It does not overwrite an older
Run's input evidence or alter the source conversation. Document versioning is
not a separate immutable domain object and decision for each inferred sentence.

Generated Memory may record preferences, working context, useful outcomes,
failed approaches, and unresolved uncertainty. It does not turn an assistant
suggestion, tentative fiction, or remembered statement into Authoritative State.
Existing source objects remain independently readable and inspectable.

## 3. Background extraction

Memory generation and use can be enabled or disabled separately. A Project
Conversation can be excluded from future generation without deleting it or
rewriting the active model context. These are explicit settings on identified
resources; the Agent need not prove the meaning of every sentence to enforce
them. Setting defaults and exact routes belong to release and protocol owners.

An eligible input is a durable snapshot of a prior, sufficiently idle Project
Conversation that permits generation and whose records the job may read. Skip
active conversations and provisional streams. A conversation need not be closed
forever: a later settled snapshot can replace its earlier extraction input.
Memory does not read an unsettled editor buffer, promote pending work, or use a
previous memory summary as independent proof of what happened.

Phase 1 supplies bounded conversation records to the model and requests a
conversation summary, reusable memory notes, and source references. It may
return no useful memory. General instructions ask it to retain useful context,
keep uncertainty, and distinguish author decisions from proposals and guesses;
there is no fixed taxonomy or minimum incident count for every memory claim.
Secret handling applies before model submission and before publication.

The job records the exact input version, model and prompt versions, output,
and outcome. Bounded selection, leases, idempotency, concurrency limits, and
retry rules protect execution. Repeated delivery does not create duplicate
current inputs. These controls prove which work completed, not that extracted
statements are true. Ordinary writing does not wait for this work or receive
per-item memory confirmation requests.

## 4. Consolidation and selective recall

Phase 2 consolidates a bounded set of extraction outputs and pending Memory
Notes within one Project Scope. It updates per-conversation summaries,
consolidated memory, source links, and the navigation summary. Only one
publication for that Project may advance the current set at a time; a stale or
failed job cannot publish over a newer set or expose a partial document set.
Unchanged input may produce a no-op. Failed work can resume under the existing
Worker and execution contracts without blocking the editor.

The consolidation Agent can merge, rewrite, or remove generated notes using
general instructions and available sources. New author feedback, changed
inputs, age, and observed use can guide this work. Retention and input budgets
bound the set; an index and a successful model output do not certify semantic
correctness. Any use counter records an observed read or reported citation,
not proof that the model attended to the material.

At the start of a relevant conversation, Context Assembly may supply a small
memory navigation summary and general read guidance. It does not inject the
entire memory collection into every model call. The Agent decides whether to
search the notes, read a relevant document, or consult current domain sources.
Tools enforce Project Scope, access, actual lifecycle restrictions, and bounds
before returning content. Source text stays reference material and gains no
instruction authority merely by entering a memory document.

Memory is fallible context. The Agent follows current author instructions and
checks current sources when the task depends on their present value. Ranking
measures relevance, not truth, authority, or permission. Ordinary recall does
not require a Memory Candidate, Memory Admission, or Admitted Memory Entry.

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
author-authorized domain path. Memory may retrieve it but cannot create or
widen it.

An `Inferred Preference` is a non-binding interpretation of prior author action
or feedback. An `Operational Lesson` is a non-binding account of a potentially
useful execution outcome. Either can appear in generated notes with relevant
sources and uncertainty; neither requires a separate Candidate or Admission
state. Repetition, confidence, silence, or retrieval grants no authority.

Memory cannot install or alter a SkillPackage, ToolSpec, Capability, project
policy, or executable permission. A generated procedure can advise the Agent;
changing governed executable behavior still uses its owning process. Fiction
facts and Research Claims retain their own semantics rather than becoming
entries in a universal memory taxonomy.

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

## 8. Correction, forgetting, and recorded history

Ordinary corrections and changes of mind enter the conversation as author
Messages. The Agent interprets them with the available context. Earlier material
may remain recorded or already supplied to the Provider while later instructions
express the author's current intent. Do not interrupt ordinary assistance to
compile that intent into a formal memory or context control.

When the author requests a lasting Memory change, the Agent can append a
`Memory Note`: plain-language guidance for the next consolidation pass, with
its author-message source. The Agent interprets the request through its normal
loop and tool use; no deterministic natural-language classifier is required.
Background extraction can learn automatically from eligible conversations,
while a direct memory-update tool requires an author request. A note is neither
an Author Preference nor a source-access prohibition.

A note can ask to add, correct, or forget remembered information. Consolidation
uses it to revise generated documents. Report a recorded note separately from
a published update. Do not promise immediate recall removal, prevention of all
equivalent future inferences, precise semantic erasure, or erasure of content
already sent to a model. General context compaction may summarize current task
state; a memory note does not require a continuation-chain reset.

An ordinary source edit or deletion changes later source reads. It does not
rewrite conversation records or automatically erase every generated paraphrase.
The Agent can read current sources, and later consolidation can revise old notes.
Known source unavailability and document age remain inspectable. History records
which document revisions StoryOS supplied, not what the model actually used.

## 9. Actual access, archive, and deletion controls

Project Isolation, permissions, Archive, Redaction, Tombstone, and Project
Deletion retain their owning business contracts. A memory tool cannot return a
resource that those controls forbid. An explicit setting that excludes a
conversation from generation stops new extraction from that input; rebuilding
from the permitted set remains background work rather than a precise semantic
forgetting guarantee.

A real deletion or access restriction must cover generated copies that the
owning policy includes. If the affected content cannot be isolated safely, the
storage and retention owners must define a conservative document-set boundary,
unavailability, and rebuild from allowed inputs. They must not infer a complete
semantic dependency graph from model-generated citations. Memory Notes alone
do not constitute physical deletion or satisfy that policy.

Provider retention and already disclosed data remain separate external facts.
No generated memory revision rewrites an immutable disclosure record. The
Context, Model, and retention owners decide any required future-use fence at
the boundary they actually control.

## 10. Source references and rebuildable retrieval

Memory Documents keep references to their extraction inputs and to relevant
source records. These help inspection and fresh lookup. A missing reference is
reported as unavailable; a link alone does not prove support for every sentence
or provide a complete transitive influence closure.

Text search is sufficient for the initial Memory mechanism. Optional full-text,
vector, or graph indexes are disposable access projections over published
Memory Document revisions and other permitted source objects. Their IDs are
not durable domain identities. Reads recheck actual Project Scope, current
access and lifecycle, and publication identity before returning content.
An external embedding call remains an authorized disclosure operation.

Index loss does not lose the published memory documents, Memory Notes, or
conversation records. A new index can be built from available documents.
Regenerating memory from retained inputs is a separate model operation and need
not produce identical wording. Unavailable retained inputs limit what can be
regenerated; do not invent their contents or silently treat them as evidence.

## 11. Author interaction

The author can inspect the current memory summary and documents, their update
status, and available source links. Ordinary conversation can express a memory
correction. Explicit settings control reading existing memory and contributing
to future memory; source editing and real deletion use their domain paths.

No memory review queue, Candidate confirmation card, scope form, or separate
maintenance conversation is a prerequisite to writing or Agent assistance.
Generated memory can be corrected without treating the document as fictional
truth. Current prose, author-confirmed settings, and source evidence remain
independently inspectable.

## 12. Ownership handoff

| Owner | Required alignment |
|---|---|
| [Context](context-assembly-retrieval-and-outbound-disclosure-semantics.md) | bounded summary and tool-read intake, ordinary user steering, known source references, historical context, and truthful disclosure evidence |
| [Model](../adr/0033-use-volcengine-responses-for-the-first-real-model-path.md) | general context compaction and continuation without ordinary-correction chain invalidation |
| [Protocol](versioned-command-query-artifact-event-protocol.md) | memory document inspection, plain-language notes, separate use/generation settings, publication status, and source references |
| [Storage](postgresql-project-storage-isolation-and-migration-contract.md) | document revisions and publication, job recovery, exact Project Scope, current reads, index rebuild, export/restore, and physical families |
| [Retention](run-event-mailbox-snapshot-retention-and-archival-semantics.md) | document, note, and job retention, real copy deletion, unavailable evidence, and operational-history compaction |
| [Release](ai-independent-editor-first-release-baseline-and-handoff-criteria.md) and [proof](deterministic-verification-and-failure-recovery-gates.md) | Stage 7 background memory and observable verification; no semantic correctness or exact-forgetting oracle |

These original owners remain responsible for their pending revisions. This
Memory decision does not claim that the older consumer documents, persistence
catalog, or published Stage tickets already implement this contract. Actual
runtime and schema changes remain with the later approved implementation work.

## 13. Normative invariants

1. Generated Memory is inspectable, Project-scoped, and non-authoritative.
2. Background extraction, consolidation, active context compaction, and Provider
   continuation are distinct operations.
3. Ordinary author corrections use the general Agent loop; no semantic
   exclusion or per-claim admission mechanism is required.
4. Memory Notes request document changes; settings and business commands own
   access, generation eligibility, and real deletion.
5. Failed or stale maintenance cannot publish a partial or superseded set.
6. A summary navigates to memory; it does not load the whole collection.
7. Current source facts and author authority are independent from remembered
   interpretations. Research evidence retains its exact-source contract.
8. Source references and reported usage do not prove model influence or truth.
9. Document updates preserve historical input identities subject to retention.
10. Memory maintenance does not interrupt active writing or bypass a real
    permission, disclosure, Proposal, or Acceptance boundary.
