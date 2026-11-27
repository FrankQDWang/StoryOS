---
status: accepted
---

# Let the Author Type in a Selected Chapter Before the Chapter Switch Settles, and Hold That Input in the Tab

The author decided this on 2026-11-26 in conversation, after the measurement of Speed Batch 2. The words of the author: "切换章节，我理解是说马上就要可以编辑的" (after a Chapter switch the author can edit at once), "如果是网络问题，那就不是我们的关系" (the network time of a user is not a matter of the product), and "按方案一做" (the first of two plans, with its loss bound). [Type at Once After a Chapter Selection: Arrival Input in the Tab](https://github.com/FrankQDWang/StoryOS/issues/1915) is the parent of the work. This decision applies to the Protected Web Client only. The Server, the public protocol, the schema, and the Local Edit Journal format do not change.

## Context

At `main` `ed0d6ffc`, the editor of a selected Chapter is ready after four steps in series: the Chapter read, the Command Challenge, the Set Current Chapter command, and the reads of the reopen. In the browser of the author one request needs about 0.5 s, and the editor is ready after 2.4 s to 3.0 s. On a fast path the steps still need some tenths of a second. Until then, the tab accepts no input for the selected Chapter.

The input cannot go to the Server or to the Local Edit Journal before the switch settles:

- The Server keeps one base row with one Chapter for an Editor Session ([ADR 0060](0060-use-one-snapshot-identity-for-the-editor-session-base-and-its-canonical-query-snapshot.md)). An applied Author Edit moves that row. An Author Edit of a Chapter that is not the Chapter of the base gets `idempotency_binding_conflict` and leaves an unsettled Admission.
- The Local Edit Journal has one active base for a partition, and the key of a partition has no Chapter. A record of another Chapter that no group covers makes the Journal read fail, and the editor opens as read-only recovery. Each record stores the Snapshot identity and the base position, and only the Server gives them.
- The tab keeps no text of a Chapter that is not the Current Chapter.

## Decision

### Terms

- **Arriving Chapter:** the Chapter that the author selected in a tab that holds the writer, from the selection until the tab has the editor of that Chapter on its Editor Session base, or until the switch ends with another result.
- **Arrival Input:** the author input in an Arriving Chapter. It is only in the memory of the tab. It is not in the Local Edit Journal and not in the Pending Edit Projection.

### 1. Copies of Chapter text in the tab

- For each Chapter of the open Project, the tab keeps in memory a copy of the title, the Revision identity, and the blocks. Three events write the copy: the Chapter read of a selection in the tab, the read at the pointer, and a switch, which writes the settled base of the Chapter that the tab leaves. Another Chapter read can write the copy, but this is not a requirement. A read response writes the copy only when no other write of that copy started after the read started.
- A copy can also hold the fact "this Chapter has no open suggestion". The fact comes from a read of the open suggestions of the Chapter, or from the settled display of the Chapter that the tab leaves. The tab removes the fact from each copy when it learns of a new or changed Proposal of the Project.
- A copy is not authority. Its only use is the display of an Arriving Chapter. The copies end when the tab leaves the Project or loads again.
- When the pointer is on a Chapter of the contents that has no copy, the tab reads that Chapter and its open suggestions one time. These reads send no command.

### 2. The selection

This section applies when the author selects another Chapter in the contents and the switch of today is permitted: the tab holds the writer and its editor is ready. It does not apply while an Author Undo of the tab is in flight or an IME composition is open in the editor: the selection then keeps the path of today.

- The tab shows the Arriving Chapter at once from its copy. With no copy, the tab shows it when the Chapter read answers.
- The switch of today runs with no change: the quiet step of the earlier Chapter, the Chapter read, the Command Challenge, the Set Current Chapter command, and the reopen. The Server receives the same requests in the same order.
- From the selection, the editor of the earlier Chapter accepts no input. The Editor Session Writing Controller of the earlier Chapter completes its journaled input as today.

### 3. Arrival Input

- The Arriving Chapter accepts input when its copy has the fact "this Chapter has no open suggestion", no end of sections 4 to 7 stopped the input, and no wait for a Challenge rate limit is active. In each other case the Arriving Chapter is read-only until the editor of today is ready. While a notice of section 4 is open, an Arriving Chapter accepts no input.
- The permitted input is text, a deletion, a new paragraph, an IME composition, and a paste of plain text. Author Undo, a block move, a block retype, a selection reference, each Proposal control, and a message to the assistant are not available in an Arriving Chapter.
- While Arrival Input exists, the state line shows the saving state, and the browser asks the author before the tab unloads.
- Arrival Input has the bound of the in-memory queue of [ADR 0038](0038-separate-author-edit-challenge-admission-from-shared-command-admission.md): 240 operations. At the bound, the Arriving Chapter accepts no more input until the hand-over.

### 4. The hand-over

When the switch ends with the editor of the Chapter ready:

- The tab compares the Revision of the Editor Session base with the Revision of the copy that the author typed on.
- When the two are equal and the editor shows no suggestion in the Chapter, the tab gives the Arrival Input to the Editor Session Writing Controller through its normal capture entry, in the order of the input. From there it is normal input: the Local Edit Journal, the Author Edit, and the save state. The caret and the scroll position stay. The Server receives only command shapes that normal input gives.
- The hand-over does not start during an IME composition. It starts when the composition ends.
- In each other case, no part of the Arrival Input goes to the controller. The tab shows the editor of the Chapter as the Server has it. The tab shows the text of the Arriving Chapter with the Arrival Input in a notice that says that this text is not saved. The notice has a copy action, and it stays until the author closes it.

When the Chapter read of the switch answers with a Revision that is not the Revision of the copy: with no Arrival Input, the tab replaces the shown text and the copy. With Arrival Input, the Arriving Chapter accepts no more input, and the last rule above applies when the switch ends.

### 5. A switch that does not reach the editor

When the quiet step refuses, or the switch ends with a refusal, a conflict, or a failed read, the tab shows what the switch of today shows: the earlier Chapter and the sentence of today. When the Set Current Chapter command ends with no answer of the Server, its result is not known. The tab then accepts no input, reads the Project again, and shows the Chapter and the editor that the Server has. Section 4 applies to Arrival Input with that editor. When that read fails, the tab shows the sentence of today, and the editor accepts no input until a later selection reads the Project again. When the reopen finds that the tab lost the writer, the tab shows what the switch of today shows: the selected Chapter, read-only. In the other cases of this section, Arrival Input goes to the notice of section 4.

### 6. A second selection

- With no Arrival Input, a selection of another Chapter replaces the Arriving Chapter on the screen at once. A Set Current Chapter command in flight completes. Then the tab sends the switch from the result of that command to the newest selection. The tab mounts no editor for a Chapter that is not the newest selection.
- With Arrival Input, a selection of another Chapter waits until the hand-over ended. Then it runs as a new selection, and its quiet step waits for the save of that input as today. A later selection replaces a selection that waits.

### 7. The Challenge rate limit

While the switch waits for the `Retry-After` of a Challenge rate limit, the Arriving Chapter shows the sentence of today for that wait and accepts no new input. Arrival Input from before the wait stays until the switch ends.

### What does not change

- A tab that does not hold the writer, the switch of a Proposal navigation, and the Author Undo of a switch keep the path of today. Two exceptions: when the command of a Proposal navigation ends with no answer of the Server, the editor accepts no input until a selection reads the Project again, as in section 5. A command of a selection waits for the end of a command of a Proposal navigation in flight, as in section 6.
- The Current Chapter of [ADR 0026](0026-keep-current-chapter-outside-authoritative-state.md) and each check of the Server stay.
- The Editor Session Writing Controller stays the one owner of the Pending Edit Projection ([ADR 0046](0046-give-the-pending-edit-projection-one-owner-in-the-editor-session-writing-controller.md)). Arrival Input is outside the projection until the hand-over. A current-chapter change still needs no unsettled input of the earlier Chapter.
- Input in a Chapter that has a base enters the Local Edit Journal at once ([ADR 0065](0065-keep-challenge-wait-input-in-the-local-edit-journal-as-records-that-follow-an-unsettled-group.md)). Arrival Input has no base, so it cannot be a Journal record.

### Loss bound

A crash, a new load, or a closed tab loses the Arrival Input. The loss is at most the input from the selection to the hand-over. This is the same class of loss as the held input of ADR 0046. The author accepted this bound on 2026-11-26.

## Considered options

- **The Server keeps a base for each Chapter of an Editor Session, and the pointer moves in the background.** Rejected for now: it changes the schema, the Local Edit Journal version, and the rules of Author Undo. It has the size of a Stage. It is the way to remove the loss bound later.
- **Journal the Arrival Input with no base.** Rejected: it changes the record format and the recovery rules of the Journal.
- **Change the Current Chapter only at the first input.** Rejected for now: it changes what the Current Chapter means for the reopen, for the target of the assistant, and for the statistics.
- **Show the text at once, and accept input when the switch settled.** Rejected by the author on 2026-11-25.
- **One request for the switch and its data.** Not selected: it has fewer steps, but the author still waits for the Server before the first key.

## Consequences

- The first ticket proves the hand-over with a prototype before a product change, also around an IME composition. When the proof fails, the work stops and the author decides again.
- Each end of sections 4 to 7 needs a test that shows that no Arrival Input goes away with no notice.
- The measured result on the personal instance is the time from the selection to the first accepted key, with a copy and with no copy.
