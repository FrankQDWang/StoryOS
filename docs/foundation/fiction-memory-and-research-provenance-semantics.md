# Fiction Memory and Research Provenance Semantics

- Status: accepted
- Wayfinder resolution: [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51)
- Canonical glossary: [`GLOSSARY.md`](../../GLOSSARY.md)
- Parent domain model: [Artifact and Authoritative-State Domain Model](artifact-domain-model.md)
- Context and disclosure owner: [Context Assembly, Retrieval, and Outbound Disclosure Semantics](context-assembly-retrieval-and-outbound-disclosure-semantics.md)
- Authority decision: [ADR 0001](../adr/0001-separate-authoritative-state-artifacts-and-operational-records.md)
- Ownership decision: [ADR 0004](../adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md)
- Memory decision: [ADR 0035](../adr/0035-use-background-generated-project-memory.md)
- Memory form decision: [ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)

## 1. Scope and authority

Agent Memory supports continuity across Project Conversations and AgentRuns.
StoryOS keeps Memory as short, readable Memory Entries ([ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)).
Background work captures typed observations from settled Project Conversations
and from Chapters. A consolidation pass makes and prunes the entries. A bounded
Memory Index is in the context of a Run, and the Agent searches and reads the
entries when relevant. Codex CLI, Claude Code, and Grok Build are design
references, and none of them is the deciding contract.

Memory is separate from active model-context compaction, durable conversation
history, and Provider continuation. It is an aid to recall, not a second source
of fictional truth or a rule system for interpreting every author statement.
General model prompts guide extraction, consolidation, and retrieval. StoryOS
does not classify ordinary creative requests into a special exclusion registry,
per-claim admission lifecycle, or deterministic semantic removal graph.

All memory inputs, entries, observations, jobs, indexes, tool reads, and disclosures
bind one trusted `ProjectScope { owner_user_id, project_id }`. Background work
cannot pool content across Projects or Users. A content hash, similar wording,
or shared Provider account grants no access. The PostgreSQL and isolation
contract remains in force; a Markdown representation is not an unmanaged local
store or permission to copy the Codex runtime.

This is a semantic contract. It does not implement Memory, choose physical
paths or database tables, calibrate budgets, or release a product stage.

## 2. Durable information spaces and Memory Entries

StoryOS retains three durable information spaces:

| Space | Role in Memory |
|---|---|
| Authoritative State | current author-owned prose and explicit constraints that the Agent can read through their domain tools |
| Artifacts | conversation Messages, inspectable source material, and the Memory Entries with their versions |
| Operational Records | exact maintenance inputs, Memory Observations, execution outcomes, usage observations, and historical context evidence |

A `Memory Entry` is non-authoritative text with a stable identity and versions.
It has a title, a one-line description, a type, an origin, and source
references. The types are writing preference, foreshadowing, character,
relationship, major event, and other. The origins are the request of the
author, a Project Conversation, and a Chapter. An entry holds a synthesis or an
intent and does not copy a passage. Markdown supports bounded search and read;
Memory uses no vector database, graph, or embedding call. A Memory Entry is an
Artifact. Its records are their own PostgreSQL tables in the Project Scope, as
the records of the Research Artifact are, and they are not a part of the Tool
Artifact family. A `Memory Observation` is the typed output of one capture. It
is an Operational Record, it waits for the consolidation pass, and it is not
an entry.

A change of an entry keeps the earlier version until the Project is deleted,
so that the record of an earlier Run stays complete. It does not overwrite an older
Run's input evidence or alter the source conversation. An entry version is not
a separate immutable domain object and decision for each inferred sentence.

Memory may record the writing habits of the author, a foreshadowing or a hidden
plot line, the personality of a character, a relationship, a major event, a
decision about the story that the manuscript does not hold yet, and unresolved
uncertainty. It does not turn an assistant
suggestion, tentative fiction, or remembered statement into Authoritative State.
Existing source objects remain independently readable and inspectable.

## 3. Background capture

Conversation Memory Settings independently control Agent use and contribution
for each exact Project Conversation. They change only when all associated
foreground root Runs are Terminal or absent; independent background Memory
work does not block adjustment. The [protocol owner](versioned-command-query-artifact-event-protocol.md#1310-conversation-memory-protocol)
defines the atomic idle gate and exact routes. Changing a setting neither
deletes Memory nor rewrites recorded context. The release owner fixes defaults.

A capture has one of two inputs. The first is the settled messages of a quiet
Project Conversation that permits contribution and that no earlier capture
read. The second is the authoritative text of a Chapter that changed and is
quiet. For a manuscript that exists before Memory, background work captures
each Chapter one time, in Chapter order, at a limited rate. Skip active
conversations and provisional streams. Memory does not read an unsettled editor
buffer, promote pending work, or use an earlier entry as independent proof of
what happened.

A capture is one bounded tool loop. The model gets its input, the Memory Index,
and read-only Host Tools: the manuscript keyword search, the manuscript read,
and the Memory read. Thus the capture of a Chapter can read an earlier Chapter.
The loop ends with a typed list of Memory Observations and may return none.
General instructions ask it to keep only what a later Chapter needs and a
search cannot find, to keep uncertainty, and to distinguish author decisions
from proposals and guesses. The Host limits the number of observations of one
capture and refuses an observation above a limit. Secret handling applies
before model submission and before storage.

The job records the exact input version, model and prompt versions, output,
and outcome. Bounded selection, leases, idempotency, concurrency limits, and
retry rules protect execution. Repeated delivery does not create duplicate
current inputs. These controls prove which work completed, not that extracted
statements are true. Ordinary writing does not wait for this work or receive
per-item memory confirmation requests.

## 4. Consolidation and selective recall

When a Project has observations that wait, a consolidation pass sends one model
request with no tools. Its input is the current entries, the observations, and
the marks of the entries that the author deleted. Its reply is a list of typed
operations: create, update, delete, merge, and a conflict flag. The Host
validates each operation and applies the list in one transaction. A pass whose
input is older than the current entries applies nothing, and a failed pass
exposes no partial result. Unchanged input may produce a no-op. Failed work can
resume under the existing Worker and execution contracts without blocking the
editor.

Pruning is a named duty of the pass. It removes an entry that later material
contradicts, joins duplicates, removes first the background entries that no Run
read for a long time, and keeps the Memory Index in its bound. A Project has
one entry for each character and one entry for each pair of characters.
Input budgets and the Host limits bound the set; a successful model output
does not certify semantic correctness. Any use counter records an observed read, not
proof that the model attended to the material.

Background work cannot change or delete an entry whose origin is the request of
the author. When a capture or a pass finds that such an entry conflicts with
the manuscript or a Conversation, the Host records a conflict flag on the entry
and keeps its text. The Agent asks the author about a flagged entry in a later
Run.

Context Assembly supplies the Memory Index and general read guidance to a Run
when the Conversation Memory Settings permit use. It does not inject the entire
memory collection into every model call. The Agent decides whether to search
the entries, read an entry, or consult current domain sources.
Tools enforce Project Scope, access, actual lifecycle restrictions, and bounds
before returning content. Source text stays reference material and gains no
instruction authority merely by entering a Memory Entry.

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

In Release 1, a claim of a Research Synthesis follows
[ADR 0087](../adr/0087-record-a-research-synthesis-as-one-immutable-research-artifact.md).
It cites one or more exact External Source Snapshots or Source Gaps of its
AgentRun, and a claim that cites only Source Gaps is valid. A citation has no
Evidence Locator and no support, opposition, or qualification class. The other
rules of this section give the Evidence Relation model, which Release 1 does
not record.

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

When the author requests a lasting Memory change, the Agent creates, changes,
or deletes a Memory Entry with a Host Tool, and the origin of that entry is the
request of the author. The change applies at once. The Agent interprets the
request through its normal loop and tool use; no deterministic natural-language
classifier is required. Background capture can learn automatically from
eligible conversations and Chapters, while the direct write requires an author
request. An entry is neither an Author Preference nor a source-access
prohibition.

When the Agent finds in a Run that an entry of the author conflicts with the
manuscript or with the present words of the author, it does not change the
entry. It asks the author in its reply, and it changes the entry only after the
answer. A delete by the author on the settings page removes the entry from the
Memory Index, the tools, and the page. It leaves a mark, so that background
work does not create that entry again. Do not promise prevention of all
equivalent future inferences, precise semantic erasure, or erasure of content
already sent to a model. General context compaction may summarize current task
state; a Memory change does not require a continuation-chain reset.

An ordinary source edit or deletion changes later source reads. It does not
rewrite conversation records or automatically erase every generated paraphrase.
The Agent can read current sources, and later consolidation can revise old
entries. Known source unavailability and entry age remain inspectable. History
records which entry versions StoryOS supplied, not what the model actually used.

## 9. Actual access, archive, and deletion controls

Project Isolation, permissions, Archive, Redaction, Tombstone, and Project
Deletion retain their owning business contracts. A memory tool cannot return a
resource that those controls forbid. An explicit setting that excludes a
conversation from generation stops new extraction from that input; rebuilding
from the permitted set remains background work rather than a precise semantic
forgetting guarantee.

A real deletion or access restriction must cover generated copies that the
owning policy includes. If the affected content cannot be isolated safely, the
storage and retention owners must define a conservative entry-set boundary,
unavailability, and rebuild from allowed inputs. They must not infer a complete
semantic dependency graph from model-generated citations. A delete of a
Memory Entry by the author alone does not constitute physical deletion or
satisfy that policy.

Provider retention and already disclosed data remain separate external facts.
No generated memory revision rewrites an immutable disclosure record. The
Context, Model, and retention owners decide any required future-use fence at
the boundary they actually control.

## 10. Source references and rebuildable retrieval

Memory Entries keep references to their capture inputs and to relevant source
records. These help inspection and fresh lookup. A missing reference is
reported as unavailable; a link alone does not prove support for every sentence
or provide a complete transitive influence closure.

Text search is sufficient for Memory, which has no vector index ([ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)).
The manuscript has a vector index for the hybrid search of [ADR 0095](../adr/0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md).
Each full-text, vector, or graph index is a disposable access projection over
current entries, Chapters, and other permitted source objects. Their IDs are
not durable domain identities. Reads recheck actual Project Scope, current
access and lifecycle, and the current version before returning content.
An external embedding call remains an authorized disclosure operation.

Index loss does not lose the Memory Entries, the Chapters, or conversation
records. A new index can be built from them. Regenerating memory from retained
inputs is a separate model operation and need not produce identical wording.
Unavailable retained inputs limit what can be regenerated; do not invent their
contents or silently treat them as evidence.

## 11. Author interaction

The author can inspect the current Memory Entries on the settings page of the
Project: the text, the type, the origin, the source link, and the conflict flag
of each entry. The author can delete an entry there. Ordinary conversation can
express a memory correction. The Conversation Memory Settings keep their Host
defaults and have no control ([ADR 0094](../adr/0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md)); source editing and real deletion
use their domain paths.

No memory review queue, Candidate confirmation card, scope form, or separate
maintenance conversation is a prerequisite to writing or Agent assistance.
Generated memory can be corrected without treating the entry as fictional
truth. Current prose, author-confirmed settings, and source evidence remain
independently inspectable.

## 12. Ownership handoff

| Owner | Required alignment |
|---|---|
| [Context](context-assembly-retrieval-and-outbound-disclosure-semantics.md) | bounded summary and tool-read intake, ordinary user steering, known source references, historical context, and truthful disclosure evidence |
| [Model](../adr/0033-use-volcengine-responses-for-the-first-real-model-path.md) | general context compaction and continuation without ordinary-correction chain invalidation |
| [Protocol](versioned-command-query-artifact-event-protocol.md) | Memory Entry inspection and delete, separate use/generation settings, and source references |
| [Storage](postgresql-project-storage-isolation-and-migration-contract.md) | entry versions, job recovery, exact Project Scope, current reads, index rebuild, export/restore, and physical families |
| [Retention](run-event-mailbox-snapshot-retention-and-archival-semantics.md) | entry, observation, and job retention, real copy deletion, unavailable evidence, and operational-history compaction |
| [Release](ai-independent-editor-first-release-baseline-and-handoff-criteria.md) and [proof](deterministic-verification-and-failure-recovery-gates.md) | Stage 7 background memory and observable verification; no semantic correctness or exact-forgetting oracle |

These original owners remain responsible for their pending revisions. This
Memory decision does not claim that the older consumer documents, persistence
catalog, or published Stage tickets already implement this contract. Actual
runtime and schema changes remain with the later approved implementation work.

## 13. Normative invariants

1. Generated Memory is inspectable, Project-scoped, and non-authoritative.
2. Background capture, consolidation, active context compaction, and Provider
   continuation are distinct operations.
3. Ordinary author corrections use the general Agent loop; no semantic
   exclusion or per-claim admission mechanism is required.
4. A request of the author changes a Memory Entry at once, and background work
   cannot change an entry of the author. Settings and business commands own
   access, capture eligibility, and real deletion.
5. A failed or stale consolidation pass applies no operation.
6. The Memory Index navigates to memory; it does not load the whole collection.
7. Current source facts and author authority are independent from remembered
   interpretations. Research evidence retains its exact-source contract.
8. Source references and reported usage do not prove model influence or truth.
9. Entry changes keep each earlier version until the Project is deleted.
10. Memory maintenance does not interrupt active writing or bypass a real
    permission, disclosure, Proposal, or Acceptance boundary.
