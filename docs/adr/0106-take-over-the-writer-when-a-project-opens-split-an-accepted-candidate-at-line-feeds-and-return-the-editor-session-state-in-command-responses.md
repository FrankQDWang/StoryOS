---
status: accepted
---

# Take Over the Writer When a Project Opens, Split an Accepted Candidate at Line Feeds, and Return the Editor Session State in Command Responses

The author decided this on 2026-11-27 in conversation. The coordinator used the personal instance as an author does and listed the problems. The author said "先修 1–3 组" (fix groups 1 to 3 first: the editor that does not accept input, the paragraphs of an accepted suggestion, and the steps in series). Then the author selected the recommended option of each of three questions. [Edit at Once and Wait Less: Takeover at Open, Paragraphs of an Accepted Suggestion, Fewer Steps](https://github.com/FrankQDWang/StoryOS/issues/1932) is the parent of the work.

## Context

Measured on the personal instance at `main` `54525595` from the browser of the author, where one request needs about 0.3 s to 0.5 s:

- **A new tab is read-only.** Each new tab of a Project opens a new Editor Session. The Server gives it `writer: read_only` with the reason `secondary_session`, also when the earlier tab is closed. The editor accepts no input and shows no reason. The author must find the button `接管写作`. The takeover then needs 3.0 s and opens the Current Chapter, not the Chapter on the screen.
- **An accepted suggestion of more than one paragraph is one block.** The model tool `propose_edit` gives the complete new text of one block. To continue a Chapter, the model puts the old text and the new paragraphs into one candidate with a line feed between the paragraphs. The Acceptance writes the candidate as the text of that one block (`accept_proposal_facts.rs`, `RevisionMembers::CopyFrom`). A later Proposal of one sentence shows the whole block again, a selection reference names the whole block, and search positions are hard to read.
- **Steps in series after a command.** After an Author Edit, an Acceptance, and a takeover, the client reads the Editor Session or opens the Project again before the editor is ready on the new base. A save needs 2.3 s to 3.5 s, an Acceptance 3.0 s, and a takeover 3.0 s.

[ADR 0017](0017-adopt-tiptap-for-production-block-replacement.md) permits a line feed in a block: Shift-Enter adds one. [ADR 0064](0064-accept-a-proposal-on-an-earlier-base-by-replacing-its-target-text.md) says that the candidate text replaces the text of its target block.

## Decision

### 1. Take over the writer when a Project opens

- When the tab opens a Project and the Server gives the new Editor Session `writer: read_only` with the reason `secondary_session`, the tab sends `TakeOverProjectWriter` one time, with no action of the author. It is the same command, with the same journal, idempotency, and Challenge rules, as the takeover of the button.
- The tab sends this command only at the open of a Project, which includes a new load of the page. It never sends it for the reason `superseded_by_takeover` or `binding_invalid`, after a failed or uncertain takeover, or at a later event of the same open. Thus two open tabs do not take the writer from each other in a loop.
- The tab that lost the writer shows the sentence `已在另一个标签页继续写作。` and the button `接管写作`. Its input that the Server did not get stays in that tab, as today. No hidden channel sends it.
- When the takeover does not apply, the tab shows the read-only screen with a sentence that tells the author that another tab writes, and the button.
- The button `接管写作` of a tab that lost the writer opens the Chapter that was on the screen, not only the Current Chapter of the Server.
- This changes the trigger of the command only. The section "Writer takeover" of the [Web editor session semantics](../foundation/web-editor-session-synchronization-and-recovery-semantics.md) stays: an uncertain takeover is never sent again by recovery.

### 2. Split an accepted candidate at line feeds

- When the Server applies an Acceptance, each line feed of the candidate text starts a new block. The first part keeps the identity and the type of the target block. Each next part is a new paragraph block with a new Block identity, in order, after it. A line feed of the block text outside the candidate stays a line feed.
- An empty part, from two line feeds together or from a line feed at the start or at the end of the candidate, makes no block.
- The split is a part of the Acceptance: one command, one Revision, one Receipt. Author Undo of the Acceptance restores the earlier Revision and its members, and the Block identity restoration rule applies.
- The candidate on the screen before the decision does not change: it is one candidate of one target block.
- A draft reference to a later part becomes a Chapter text reference when the editor shows the accepted Revision. When the editor shows another Revision first (not the Revision before the acceptance), or the client cannot read the new Proposal state, the reference leaves with no sentence, as for deleted text (ADR 0062). The composer does not send a reference that waits for the new blocks.
- The text of an existing Chapter does not change. A block that already has a line feed keeps it.
- This replaces the sentence of ADR 0064 "The candidate text replaces all text of that Block" for a candidate with a line feed: the candidate text replaces the text of the target block, and the line feeds of the candidate split it. The other rules of ADR 0064 do not change.

### 3. Return the Editor Session state in command responses

- The responses of the commands Apply Author Edit, Accept Proposal, and Take Over Project Writer carry the state of the Editor Session after the command: the same fields that a read of the Editor Session gives the tab for the next input (the base Snapshot identity, the base Revision, and the writer). The client uses them and does not read the Editor Session again after the command.
- The response form of each of these commands gets a new version. The client reads the state from the response only when the response has it.
- Each check stays: the Command Challenge before each command, its rate classes, the idempotency rules, and the writer generation compare. A read that protects another tab or another User stays.

## Consequences

- A new tab of a Project accepts input after its open, about one second later than a tab that holds the writer. An earlier tab that is still open becomes read-only. Its input that the Server did not get stays in that tab only.
- A soft line break in a candidate of the model becomes a new paragraph at the Acceptance.
- A save, an Acceptance, and a takeover each need one step in series less. The wire forms of three commands change. The OpenAPI contract and the generated client change with them.

## Considered options

- **Keep the new tab read-only and only explain it.** Not selected: the author must click each time, also when the earlier tab is closed.
- **Take over at the first input or click.** Not selected: the typed input has no base and is lost, and two tabs take the writer from each other.
- **A new model tool form with several paragraphs or an insert change.** Not selected: it needs a new Operation kind, and changes of reservation, validation, acceptance, Author Undo, and display.
- **A split by the client after the Acceptance.** Not selected: it is a second Author Action, so the first Author Undo removes only the split, and a failure between the two leaves one block.
- **Remove the Command Challenge before a command.** Not selected: it is a security check.
