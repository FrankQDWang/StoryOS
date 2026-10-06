# Glossary: Authoritative State, manuscript, and revision

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Authoritative State**:
The author-approved current truth of a novel project, including prose, established fictional-world truth, characters, relationships, timeline, and manuscript structure. Authority is a binary boundary reached only through an explicit author-authorized domain action; lifecycle, confidence, and lock status do not form authority levels.
_Avoid_: Canon (too narrow), accepted artifact, Agent memory

**Manuscript Structure Transition**:
One successful author-owned change to live Volume or Chapter identity, title, parentage, Canonical Sibling Order, or removal. It is an Authoritative State change and advances Manuscript Tree Revision by one. A Current Chapter change is not this transition.
_Avoid_: Structural command as a second authority system, fake prose Revision, physical tree_order as public order, Current Chapter change

**Canonical Sibling Order**:
The 1-based rank of one live Volume among the Project's live Volumes, or of one live Chapter among the live Chapters of its parent Volume, in current Authoritative manuscript structure. A successful create acknowledgement, a new create Activity event, and exact replay of that acknowledgement report this same rank for that node. A historical create Activity that stored a storage key in the same field is not this rank.
_Avoid_: Storage key, physical row order, browser-owned order, tombstone-inclusive sequence

**Current Chapter**:
The optional Project-scoped pointer to the Chapter that is the author's present working location. Its absence on an Empty Project is lawful. It is Workspace Context, not manuscript structure and not Authoritative State.
_Avoid_: Authoritative Chapter, required chapter, current passage as authority

**Manuscript Tree Revision**:
The Project Scope-local generation of Authoritative manuscript structure. One Manuscript Structure Transition advances it by one. A Current Chapter change does not.
_Avoid_: Authoritative Revision, Canonical Sibling Order, Canonical Query Snapshot, physical tree_order

**Canonical Manuscript Tree**:
The current live Volume and Chapter hierarchy under one Project Scope, read only with the latest Canonical Query Snapshot that committed with those facts. A stale Snapshot is not a historical tree.
_Avoid_: Embedded Snapshot tree, versioned historical tree, live rows beside an older Snapshot

**Structural Authority Settlement**:
The complete Core Transition outcome for one Manuscript Structure Transition or Current Chapter change, visible only when its Receipt, Author Action, Activity, Snapshot, and any required Authoritative Commit appear together.
_Avoid_: Per-command persist, Author Edit settlement, browser settlement

**Fiction Assertion**:
An addressable fiction-domain statement composed of one Proposition, Story Scope, and Epistemic Scope. Its owning Authoritative Revision or Artifact determines authority; the Assertion shape itself does not, and only incompatible Propositions under genuinely overlapping scopes conflict.
_Avoid_: Flat entity property, universal canon fact, memory fact

**Proposition**:
The content asserted about exact project subjects, separate from where or when it applies, who holds or communicates it, and whether its owning source is authoritative.
_Avoid_: Unscoped fact, belief, database field

**Story Scope**:
The work, fictional world or continuity, branch, story time, scene, and narrative position within which a Fiction Assertion applies. Story-world validity and narrative revelation are distinct, while audit time is neither.
_Avoid_: Audit timestamp, chapter number as story time, global story fact

**Epistemic Scope**:
The holder or in-fiction source and epistemic relation that frames a Proposition as project fact or as something known, believed, suspected, claimed, remembered, or retold. Different holders and relations are not interchangeable, and project truth never automatically becomes character knowledge or permitted revelation.
_Avoid_: Belief owner only, narrator truth, automatic knowledge propagation

**Authoritative Revision**:
An immutable version of one object in Authoritative State other than manuscript structure, appended only by StoryOS Core through a Direct Author Action, Acceptance, or safe compensation and guarded by an expected prior revision.
_Avoid_: Artifact Revision, mutable row, Manuscript Tree Revision

**Authoritative Commit**:
The Project Scope-ordered atomic record of one author-authorized domain transaction, identifying its Project Scope, actor, cause, every prior and resulting Authoritative Revision, which may be empty, and any Manuscript Structure Transition by its prior and resulting Manuscript Tree Revision plus the affected Volume or Chapter identities. Its scope-local sequence begins at one and advances without gaps only when Authoritative State changes; a Current Chapter change, and refused, failed, or no-change attempts, allocate none.
_Avoid_: Project snapshot, Run Event, attempted-command sequence, wall-clock order, fake prose Revision, full tree image in the Commit

**Durable Identity**:
A stable, opaque, strongly typed identity assigned to one durable StoryOS entity or record. Identity types are not interchangeable, and an identity never conveys authority, causality, freshness, project order, or capability.
_Avoid_: Content Digest, sortable clock, shared string ID, capability token

**Revision Lineage**:
The single-parent linear history of immutable Revisions under one Durable Identity, with every append guarded by the exact expected current Revision. A stale append conflicts without branching, overwriting, or automatic rebasing; alternative work receives a new Proposal or Artifact identity linked through Provenance.
_Avoid_: Revision tree, last-write-wins, implicit rebase, mutable history

**Revision Envelope**:
The common immutable identity, lineage, schema, creator, cause, audit-time, payload, and integrity boundary carried by every Authoritative Revision and Proposal Revision. Its parent is the exact expected head matched by the successful command, while physical payload placement is outside the domain contract.
_Avoid_: Mutable metadata row, storage blob layout, authority marker

**Canonical Payload**:
The exact immutable content owned by an Authoritative Revision, Artifact Revision, or Operational Record and required, while retained, to interpret, verify, replay, export, or migrate it. An authorized lifecycle operation may make its bytes unavailable only while preserving the exact identity, digest, availability fact, and known gap; a cache, index, Provider copy, or external runtime is never its sole source.
_Avoid_: Blob cache, derived projection, Provider-held source of truth

**Core Transition**:
The single logical atomic boundary in which StoryOS Core validates one idempotent domain command and durably records its complete outcome. Revisions, Commits, resolutions, heads, Receipts, Author Actions, lifecycle events, the author command Admission's `ReceiptSettled` link when applicable, and required follow-up intent become visible together. A refusal or conflict records no partial domain effect and only the no-authority Receipt, Refused Edit Draft, or Proposal condition allocated by its exhaustive result.
_Avoid_: Partial commit, database rollback as undo, external effect as transaction truth

**Core Transition Outcome**:
The exhaustive Core classification of one admitted command as Applied, NoEffect, Conflicted, or Refused, with one stable reason code for each non-applied reason. Core owns which outcome kind a Receipt records, and only Applied changes the command's target. The declared settlement profile of the command fixes which authority records an Applied outcome allocates. An Applied outcome allocates an Authoritative Commit only when Authoritative State changes. A zero-authority outcome can record Operational Records that its command declares, such as a writer takeover, but no Authoritative Commit or Author Action. A pre-Admission refusal is not an Outcome.
_Avoid_: Success boolean, HTTP status, Receipt result text, adapter-chosen outcome

**Block Split Identity**:
When a stable top-level manuscript block is split, the fragment containing the original block start retains its Block ID and the new right fragment receives a new Block ID. Identity inheritance never depends on cursor direction or editor-generated IDs.
_Avoid_: Shared fragment identity, cursor-dependent inheritance, regenerated original ID

**Block Join Identity**:
When two adjacent stable top-level manuscript blocks join, the left block retains its Block ID and the right block leaves current Authoritative State. The right identity is never reassigned, while its historical Authoritative Revisions and Commit references remain addressable regardless of edit direction.
_Avoid_: Direction-dependent survivor, reused removed ID, erased block history

**Block Transfer Identity**:
An atomic, verifiable move preserves the Block ID while changing its location or parent structure; copying, duplication, ordinary paste, external import, or a non-atomic cut and paste creates a new Block ID with source provenance. Equal content alone never proves identity continuity.
_Avoid_: Content-derived identity, copied Block ID, inferred move

**Block Retype Identity**:
A one-to-one change of a stable manuscript block's type preserves its Block ID and appends an Authoritative Revision. A transformation that changes block cardinality or hierarchy follows the split, join, and transfer identity rules instead.
_Avoid_: Type-derived identity, one ID for multiple blocks, regenerated retyped block

**Block Identity Restoration**:
Undo and redo of one unchanged reversible structural action restore its exact historical Block IDs; an identity that left current state may return only as that same object's restoration. State drift makes redo unavailable, and a newly executed structural action receives newly generated identities.
_Avoid_: Fresh ID on exact redo, reassigned historical ID, redo across drift
