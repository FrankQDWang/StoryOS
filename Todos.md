# StoryOS Todos

This file holds product ideas and open points that are not built now. The source of the first entries is the discussion with the author on 2026-11-16 about the position of the product.

An item in this file is not a contract and not a ticket. [ADR 0096](docs/adr/0096-record-the-product-position-and-put-the-author-voice-before-stage-8-and-stage-9.md) owns the decisions of that discussion. When an ADR or a ticket takes an item, remove the item from this file and put a link in its place.

## 1. The position in short

The sentence of the product, in the words that the author accepted:

> 给“边写边发现”的作者用的小说编辑器：你只管往下写，设定自己长出来，助手学的是你的笔调。

In English: a novel editor for an author who discovers the story while writing. The author writes on, the facts of the story collect themselves, and the assistant learns the voice of the author.

- **The user.** The first user is the author of this repository, who writes long fiction. The target user is an author with no professional training who writes in order, with no outline. The plot and the characters grow during the writing.
- **Not an automatic writing machine.** The author gives the instructions, and the quality of the instructions decides the quality of the novel. The product does not ask for a professional author. It asks for the ideas of the author.
- **Where a plot point comes from.** From a discussion of the author with the assistant, from the author alone, or from the judgement of the assistant about the characters, the events, and the history of the story. The author decides each one.
- **Why the editor comes first.** In software, the code is not the product, so a person can work without reading it. In fiction, the text is the product, so the author must see the text and own it. The editor must be good with no AI.
- **No setup before writing.** Other fiction tools ask for character cards, an outline, and beats before the first page. StoryOS asks for none of them.

## 2. Three problems, in the order of the author

1. **Machine voice.** Generated prose reads as machine-written. No part of the product works on this problem today.
2. **Memory in a long novel.** Stage 7 works on this problem.
3. **When to make the world larger.** The model stays inside the known facts, or it adds people and places without control. The author keeps the design of the plot. This problem comes after the first two.

## 3. Where a general agent cannot follow

A general coding agent, for example Claude Code or Codex, can read files and write prose. These three properties need an editor that owns the manuscript:

1. **The origin of the text.** StoryOS can find which text the author typed and which text came from a Proposal. The code today has no origin record for each Block. It records the command kind of each Chapter revision, it keeps the identity of a Block across revisions, and it keeps the candidate text of each Proposal, also of a rejected one. Thus the origin can be derived, and a feature of section 4.1 must build that derivation first.
2. **A place beside the text.** StoryOS sees each edit and each decision on a Proposal, and it can put a mark at the exact sentence.
3. **Work with no request.** StoryOS can do background work for the Project when the author asks nothing, as the capture of Stage 7 does.

## 4. Ideas that are not built now

Each idea must pass two questions: does it need setup before writing (then it is refused), and does it work on problem 1 or problem 2 (then it comes first).

### 4.1 Machine voice

- **Examples from the text of the author, and learning from the changes of the author.** [ADR 0100](docs/adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md) decided these two ideas on 2026-11-21: they are not built. StoryOS infers no preference of the author automatically. The author puts sample passages into the Project Instruction.
- **Marks on machine phrases in the text.** The editor marks a sentence with a known machine pattern, for example “不是……而是……”, “眼中闪过一丝”, or a summary sentence at the end of a paragraph. The list starts as a general list. This function works with no AI. It is not in Stage 10 (ADR 0100).
- **A second model that reviews a draft.** A second model examines a draft of the assistant and gives suggestions, and the assistant revises the draft. The author named this on 2026-11-21 as a possible later mechanism. It needs an experiment first, and ADR 0100 keeps experiments out of Stage 10.
- **Skills that the author makes.** The author later provides own SkillPackages for the demands of one book. ADR 0089 deferred each Skill that an author provides, and Stage 10 ships only SkillPackages of the StoryOS distribution.
- **The model.** The author rates the Claude series and the Gemini series as the best for Chinese prose, and the GPT series below them. The Volcengine route is in use because it is cheap and has the Responses form, and it lets the full loop work first. Production will use another model. No vendor is selected, and the selection needs its own ADR.

### 4.2 Memory in a long novel, after Stage 7

- **A conflict notice in the text.** When a new passage is in conflict with Agent Memory, for example a dead character who comes back or an age that does not fit, the editor marks the passage.
- **Sleeping threads.** A hint from many Chapters before that has no result, and a character who did not appear for a long time, are in one list for the author and for the assistant.
- **A dreaming function for Agent Memory.** A general agent has a dreaming function: in a quiet time, it reads all its memory, joins duplicates, corrects old facts, and removes entries of no use. The consolidation pass of Stage 7 already removes, merges, and compresses the entries (ADR 0094 section 6), but it starts only when an observation of a capture waits, and it cannot read the manuscript. The author wants a dreaming function after Stage 11, and its difference from that pass is not decided ([ADR 0101](docs/adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md)).

### 4.3 When to make the world larger

- **A plot that grows from the characters.** Decided: [ADR 0101](docs/adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md) sets the scope of Stage 11. Each character says what it wants to do next, from its own file, and the author decides.

- **Directions before prose.** Before the assistant writes, it gives three directions of one sentence each: one that uses only the known people and places, one that brings back a sleeping thread or character, and one that adds a new person or place. The author selects one, and then the assistant writes.
- **A small floating assistant.** A small assistant floats beside the text, as in a general coding agent. It reminds the author with no request, and the author can speak with it by voice. The author named it on 2026-11-22 as a possible later form and has not decided it. Stage 11 shows no intent in the writing workspace (ADR 0101).

### 4.4 Proof of the position

- The author writes the author's own novel in StoryOS, and not in a general agent.
- One trial writes the same scene with StoryOS and with a general agent, and the author compares the two results.

## 5. Open points

- **The scope of the author-voice stage.** Decided: [ADR 0100](docs/adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md).
- **The production model.** Which vendor, which account, and which cost. See section 4.1.
- **Stage 8 and Stage 9.** The author agreed to an order for the work after Stage 11: the base for more than one Provider, then a smaller Stage 9, and the withdrawal of Stage 8 in its present form ([ADR 0101](docs/adr/0101-deliver-stage-11-with-a-character-file-in-agent-memory-character-intents-as-context-and-a-character-section-of-the-settings-page.md)). A later ADR decides each item when its work starts.

## 6. Parked items from the Stage 7 work

- **A Conversation that is older than the retention of the Provider.** The model does not get the earlier turns when the author continues such a Conversation. The panel shows one notice. No ticket sends the earlier author messages again.
- **No vector index.** The hybrid search reads the vectors with an exact scan in the Project Scope, because an HNSW index below the Project Isolation returned no row in the probe. The measured time was 61 to 72 ms at 20,000 rows. Look at this again when a Project is much larger. Record: [the probe](docs/research/embedding-and-pgvector-probe-2026-11-16.md).
- **The hosted database.** The probe found that the hosted service offers PostgreSQL 15 and 17, and the repository uses PostgreSQL 16. This is an item of the deployment preparation of ADR 0022.
- **One more request before `finish`.** In one live run of Stage 7, the model wrote text before it called `finish`, and the Run sent one more request. The release evaluation of Stage 7 looks at this.
