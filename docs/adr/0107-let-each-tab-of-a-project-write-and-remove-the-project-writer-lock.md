---
status: accepted
---

# Let Each Tab of a Project Write, and Remove the Project Writer Lock

The author decided this on 2026-11-27 in conversation, during the work of [Edit at Once and Wait Less](https://github.com/FrankQDWang/StoryOS/issues/1932). The coordinator explained the writer takeover. The author did not ask for it and rejected it: no read-only tab, no button, no takeover. The author also rejected real-time merge of the edits of two tabs as not necessary. The author selected option A: each tab writes, and a tab with an earlier base catches up. This decision replaces section 1 of [ADR 0106](0106-take-over-the-writer-when-a-project-opens-split-an-accepted-candidate-at-line-feeds-and-return-the-editor-session-state-in-command-responses.md).

## Context

- StoryOS has two protections against a tab that writes on old text. The first is the Project writer lock: one Editor Session of a Project is the writer, and each other Editor Session is read-only until the author clicks `接管写作`. The second is the Revision compare: each Author Edit names the Revision that the tab saw (`expected_authoritative_revision_id`), and the Server refuses the edit with `stale_authoritative_head` when the Chapter has a later Revision. Acceptance, rejection, and Author Undo have their own head compares.
- The Revision compare alone prevents an overwrite of later text. The writer lock adds a read-only state, a takeover command, a button, sentences, and a writer generation in each command. On the instance, the lock made a new tab read-only with no reason on the screen, and the author lost input.
- Google Docs has no lock: each tab and each User writes, and the Server orders and transforms the changes (operational transformation). StoryOS does not need real-time merge for one author.
- The writer lock is in the Server, the contracts, the generated client, the Web client, the migrations, and the [Web editor session semantics](../foundation/web-editor-session-synchronization-and-recovery-semantics.md).

## Decision

### 1. No writer lock

- Each Editor Session of the owner of a Project can send each command that a writer can send today. No Editor Session is read-only because another Editor Session exists.
- The command `TakeOverProjectWriter`, the button `接管写作`, the read-only reasons `secondary_session`, `superseded_by_takeover`, and `binding_invalid` of the writer, and the sentences of a read-only tab go.
- The writer generation leaves the command requests and the Editor Session read. Each changed request or response form gets a new version. The OpenAPI contract and the generated client change with them.
- Each other check stays: the Command Challenge and its rate classes, the idempotency rules, the Project Scope, the protected Client Session, the Revision compare of an Author Edit, and the head compares of Acceptance, rejection, and Author Undo.

### 2. A tab with an earlier base catches up

- When an Author Edit gets `stale_authoritative_head`, the tab reads the current Chapter, puts its input that the Server did not get on the current text, and sends it again with the new base. It shows no sentence and no button.
- The input of the tab goes on the current text block by block. A block that only the other tab changed keeps the text of the other tab. A block that the tab changed replaces the current text of that block: the later edit of the same block wins.
- No typed text is lost with no sign. When the current Chapter does not have a block that the tab changed, the text of the tab stays in the Chapter as a paragraph at the place of that block.
- When a tab comes to the front and the Chapter on its screen has a later Revision on the Server, the tab shows the later text. Its input that the Server did not get goes on that text by the same rule.
- The two tabs do not show each other's input character by character. A tab shows the edits of another tab when it comes to the front or when its save gets `stale_authoritative_head`.

### 3. Changes to ADR 0106

- Section 1 of ADR 0106 (takeover at open) is replaced by this decision.
- Section 3 of ADR 0106 stays for Apply Author Edit and Accept Proposal. The response state has the base Snapshot identity and the base Revision, and no writer. The command Take Over Project Writer goes.

## Consequences

- A new tab and a new load of the page accept input as soon as the editor shows the Chapter. No tab shows `需要处理` because of another tab.
- When the author types in two tabs on the same block, the later edit of that block wins. The text of the earlier edit stays in the Revision history and in Author Undo.
- The writer generation, the takeover command, and their tests, journeys, and contracts leave the code. The [Web editor session semantics](../foundation/web-editor-session-synchronization-and-recovery-semantics.md) change with them.
- [#1943](https://github.com/FrankQDWang/StoryOS/issues/1943) (a journal record of the takeover command) has no subject after this change.

## Considered options

- **Take over the writer when a Project opens (ADR 0106 section 1).** Not selected: the author did not ask for a writer lock, and an earlier tab still shows a read-only state and a button.
- **Take over at the first input or click in a tab.** Not selected: it keeps the lock and its states, and two open tabs take the writer from each other.
- **Real-time merge as in Google Docs (operational transformation or a CRDT).** Not selected: one author does not need it, and the save, Proposal, Acceptance, and Author Undo models replace text block by block with Revisions. Each of them must change for it.
