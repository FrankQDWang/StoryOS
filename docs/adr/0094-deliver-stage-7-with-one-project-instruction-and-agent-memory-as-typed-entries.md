---
status: accepted
---

# Deliver Stage 7 with One Project Instruction and Agent Memory as Typed Entries

The author decided this on 2026-11-16 in conversation, at the start of Stage 7 ([#365](https://github.com/FrankQDWang/StoryOS/issues/365)). This ADR records the scope of Stage 7 and the form of Agent Memory. It amends the mechanism of [ADR 0035](0035-use-background-generated-project-memory.md) and the sections of the [Memory contract](../foundation/fiction-memory-and-research-provenance-semantics.md) that give that mechanism. The direction of ADR 0035 stays. [ADR 0095](0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md) records the manuscript tools of the same Stage.

The evidence is the study [Memory and manuscript retrieval for Stage 7](../research/stage7-memory-and-manuscript-retrieval-2026-11-16.md).

## Context

The Stage 7 parent and its 15 child tickets date from the planning of 2026-09-15. Stage 4 to Stage 6, the cleanup of the repository, and ADR 0036 to ADR 0093 came after that planning.

At `main` `ae03e16b`, the repository has these facts:

- The Project Agent, the Project Conversation, and the Conversation Memory Settings have records. `createAgentRun` writes the default settings. One control of the writing workspace starts a new Project Conversation.
- Each Run has a Project Instruction Binding, and its value is always "none configured". No Project Instruction record and no command to write one exist.
- No fiction record exists: no Fiction Assertion, no Character, no relationship, no timeline record, and no Proposal kind for them.
- No Memory record exists. The only background work is the settlement of a Run and the Project export.
- A Run is a tool loop with Host Tools ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md)). Each Tool result returns to the model through Context Assembly ([ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md)).
- The Web client has two pages: the Project list and the writing workspace. It has no settings page. After the author starts a new Project Conversation, no control opens the earlier one again.

The study read the source of three coding assistants and the public material of other products. Its results:

- The three assistants keep memory as short, readable entries. A small index is in each context, and the assistant reads an entry when it needs it. A background pass adds entries and also removes them. None of the three uses embeddings for memory.
- One of them gets typed observations from a model request with no tools, and the host validates and stores them. Its consolidation returns typed operations. This form fits StoryOS, which has no shell agent and validates each model output.
- No fiction product in the study extracts story facts automatically. The author keeps them by hand.

The author described the content of Memory. It is not a copy of the novel. It holds the writing habits of the author, and the facts that a search or a read of a long manuscript cannot give: a deliberate foreshadowing, a hidden plot line, the personality of a character, a relationship, and a major event.

## Decision

### 1. The scope of Stage 7

Stage 7 delivers the Project Instruction, Agent Memory, the manuscript tools of ADR 0095, one settings page of a Project, and a list of the Project Conversations.

These contracts stay deferred. Their glossary entries stay, and no released Stage implements them:

- The structured fiction records: the Fiction Assertion with its Proposition, Story Scope, and Epistemic Scope as typed records, and the Character, world fact, relationship, and timeline records with their Proposal kinds. The author found them too complex and decided no form to show them. A character, a relationship, and a major event are Memory Entries (section 3).
- The Author Preference and the Inferred Preference as separate records. The Project Instruction is the one lasting instruction that the author writes. A habit that the author states in a Conversation is a Memory Entry.
- A control for the Conversation Memory Settings. The records and the Host defaults stay: use and contribution are on.
- An edit of a Memory Entry on the settings page.
- The removal of superseded Memory payload bytes under a Retention Profile, and the fence of a covered-copy restriction. No Retention Profile and no author command for a copy restriction exist.

### 2. The Project Instruction

- A Project has at most one Project Instruction: one text that the author writes on the settings page. Each save makes one immutable Revision. An empty text means that no Project Instruction is configured.
- `createAgentRun` binds the current Revision, and the bound text is in the context of the Run with the rank of the Context contract: below the current instruction of the author and above a Skill instruction. A Run in progress keeps its Revision. A change applies to the next Run.
- The text obeys the Context item limit ([ADR 0052](0052-set-the-context-item-limit-from-the-measured-model-input.md)). A save above the limit is refused, and the text is never shortened.
- The names: the table `project_instruction_revisions` keeps each Revision with its identity `instruction_revision_id` and its revision counter `revision`. The command `updateProjectInstruction` (`PUT /api/v1/projects/{project_id}/instruction`) saves the text, and its Activity kind is `project_instruction_updated`. The query `getProjectInstruction` (`GET` on the same path) reads the current Revision. A save above the limit gets the problem code `instruction_over_item_limit`.

### 3. The Memory Entry

Agent Memory is a set of Memory Entries of one Project. A Memory Entry is a short Markdown text. It has a title, a one-line description, a type, an origin, and its source references. It records the times of its creation, its last change, and its last read.

- **Types:** writing preference, foreshadowing, character, relationship, major event, and other. The type "other" holds, for example, a decision about the story that the manuscript does not have yet.
- **Origins:** the request of the author, a Project Conversation, and a Chapter.
- **Content:** an entry holds a synthesis or an intent. It does not copy a passage. The manuscript stays the source of the exact text, and the Agent reads the manuscript when a task depends on an exact fact.
- **Authority:** an entry is Data-only Context. It is not Authoritative State, it has no Instruction Authority, and it grants no permission. This is the rule of ADR 0035.
- **Classification and storage:** a Memory Entry is an Artifact in the meaning of [ADR 0001](0001-separate-authoritative-state-artifacts-and-operational-records.md): a durable item that is not Authoritative State. Its records are their own tables in PostgreSQL, in the Project Scope, as the records of the Research Artifact are. They are not a part of the Tool Artifact family that the first form of the Memory contract named. Each change keeps the earlier version, and the versions stay until the Project is deleted.

### 4. The Memory Index and the read

- The Memory Index is one bounded Context item of each Run: the type, the title, and the description of each current entry, in groups by type. It does not hold the text of an entry.
- The Agent reads one entry and searches the entries by keyword with Host Tools. The read records the time of the last read.
- Memory uses no embedding and no vector index.
- The Index and the read tools are in a Run only when the Conversation Memory Settings permit use.

### 5. Three writers

1. **The request of the author.** When the author asks for a lasting change of Memory in a Conversation, the Agent creates, changes, or deletes an entry with a Host Tool. The change applies at once, and the origin of the entry is the request of the author. The Host does not classify the words of the author; the description of the Tool gives the rule to the model.
2. **A Project Conversation.** When a Conversation is quiet and has settled messages that no capture read, background work starts one capture with those messages. This runs only when the Conversation Memory Settings permit contribution.
3. **A Chapter.** When the authoritative text of a Chapter changed and the Chapter is quiet, background work starts one capture with the text of that Chapter. It looks for characters, relationships, and major events. For a manuscript that exists before this function, background work captures each Chapter one time, in Chapter order, at a limited rate, with no action of the author.

A capture is one bounded tool loop. The model gets its input, the Memory Index, and read-only Host Tools: the manuscript keyword search and the manuscript read of ADR 0095, and the Memory read. Thus the capture of a Chapter can read or search an earlier Chapter when the Chapter alone does not give sufficient information. The loop ends with a typed list of Memory Observations, which can be empty. The Host validates and stores them. A capture cannot write an entry, start a search on the web, or change the manuscript.

A Memory Observation is not a Memory Entry. It is an Operational Record of background work, and it stays as the record of its capture until the Project is deleted. The Agent does not read it, and the Index does not list it. Ordinary writing does not wait for this work.

### 6. Consolidation and pruning

- When a Project has observations that wait, a consolidation pass sends one model request with no tools. Its input is the current entries and the observations. Its reply is a list of typed operations: create, update, delete, merge, and the conflict flag of section 7.
- The Host validates each operation and applies the list in one transaction. A pass whose input is older than the current entries applies nothing and runs again.
- Pruning is a named duty of the pass. It removes an entry that later material contradicts, joins duplicates, removes an entry that no Run read for a long time, and keeps the Index in its bound.
- The Host refuses each background operation on an entry whose origin is the request of the author. Such an entry changes only on a new request of the author in a Conversation, or by a delete on the settings page.
- A delete by the author removes the entry from the Index, the tools, and the page. It leaves a mark with the title and the description, and each later pass gets the marks so that it does not create that entry again. The record of the versions stays until the Project is deleted, so that the record of an earlier Run stays complete.

### 7. A conflict with an entry of the author

The rule of section 6 must not leave each stale entry of the author to the author alone.

- **In a Run.** When the Agent reads an entry of the author and finds that it conflicts with the manuscript or with the present words of the author, it does not change the entry. It tells the author the conflict and asks which is correct, in the text of its reply. It changes or deletes the entry only after the answer of the author. This is a rule in the instructions of the model, not a check of the Host.
- **In background work.** A capture or a consolidation that finds such a conflict returns one typed conflict flag: the entry, a short reason, and the source Chapter or Conversation. The Host records the flag and does not change the text of the entry. The settings page and the Memory Index show the flag, and the Agent asks the author in a later Run. A new request of the author on that entry, or its delete, removes the flag.

### 8. The bounds of the capture

A model records too many small details when only its instructions limit it. The instructions say what an entry is for: a fact that a later Chapter needs and that a search cannot find. These controls do not depend on the instructions:

- **Host limits.** The Host limits the number of observations of one capture, the length of one entry, and the size of the Memory Index. It refuses an observation or an operation above a limit.
- **One entry for each subject.** A Project has one entry for each character and one entry for each pair of characters. A new detail goes into that entry and competes for its length, and it does not become a new entry.
- **Two steps.** An observation is not an entry. The consolidation pass sees all entries together, and it can drop an observation.
- **Use.** The pass removes first the background entries that no Run read for a long time.
- **Delete marks.** The marks of the entries that the author deleted go to each later capture and pass as examples of content that the author does not want.
- **Calibration.** Before the release, one recorded live run on a sample manuscript measures the number of entries, and the ticket sets the limits from that record.

### 9. The settings page

- Each Project has one settings page. The control at the bottom left of the writing workspace opens it. It has two sections: the Project Instruction and Memory.
- The Memory section lists the current entries in groups by type. Each entry shows its title, its text, its origin with a link to the source Conversation or Chapter, the time of its last change, and its conflict flag when it has one. Each entry has a delete control.
- The page has no review queue and no approval step for a background entry. The control of the author is to see each entry, to delete it, and to correct it in a Conversation.

### 10. The Conversation list

- The writing assistant panel has a list of the Project Conversations of the Project. The author opens an earlier Conversation again, reads it, and continues it. Each Conversation keeps its own Provider continuation.
- The origin of a Memory Entry on the settings page opens its source Conversation.
- The list has no rename control and no delete control.

### 11. What changes in ADR 0035 and the Memory contract

These parts stay: background generation, readable text, a bounded summary in the context, a read on demand, fallible reference material with no authority, the PostgreSQL authority, and the Project Isolation.

These parts change:

- Memory Entries and typed operations replace the set of Memory Documents that one consolidation Agent publishes as a whole.
- A write at once on the request of the author replaces the Memory Note that waits for a consolidation.
- A Chapter is also an input of the capture.
- Each entry has a type and an origin, and background work cannot change an entry of the author.
- A capture is a bounded tool loop with read-only tools, and Host limits bound its output.

## Considered options

- The Codex form was rejected: an agent with a shell that edits Markdown files in a sandbox and publishes a complete set. StoryOS has no such agent, and it validates each model output as typed data.
- The structured fiction records were rejected for Stage 7. See section 1.
- A capture only from Conversations was rejected. Text that the author writes in the editor with no Conversation would never reach Memory.
- An approval of each background entry by the author was rejected. ADR 0035 forbids a review queue as a condition of writing.
- A change of an author entry by background work was rejected. A foreshadowing that the author dictated must not change with no action of the author.
- A capture of a Chapter with no tools was rejected by the author. The capture of a later Chapter can need the text of an earlier Chapter.
- A question box with buttons for a conflict was not added. StoryOS has no such control, and a question in the reply text needs no new mechanism.
- Embeddings for Memory were rejected. A Project has some hundreds of entries, and the Index and a keyword search are sufficient.
- A direct edit of an entry on the page was deferred. The author corrects an entry in a Conversation.

## Consequences

- Each new Context source class needs a new scope revision of the standing grant ([ADR 0051](0051-record-the-standing-grant-scope-and-refuse-a-disclosure-outside-it.md)). Each change of a request form needs a new mapping revision and a new Registration ([ADR 0093](0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md) section 3).
- The capture and the consolidation are the first model work outside a Run of the author. The ticket that adds them owns their admission, their bounds, their records, and their recovery.
- A generated entry can be wrong, and no product in the study proves automatic capture for fiction. The controls are the origin on each entry, the delete, the correction in a Conversation, and the rule that the manuscript is the source of an exact fact.
- A later view of characters, relationships, or a timeline reads the Memory Entries of those types. [S8-06](https://github.com/FrankQDWang/StoryOS/issues/433) of Stage 8 names fiction records that Stage 7 does not make; the Stage 8 refresh changes it.
- The glossary has the Memory Entry, the Memory Index, and the Memory Observation. The Memory contract and the Stage 7 rows of the delivery documents change with this ADR. Each other document that names a Memory Document or a Memory Note keeps its text until the ticket that builds the related part changes it.
- The refresh of the Stage 7 parent names each child ticket that stays, changes, or closes.
