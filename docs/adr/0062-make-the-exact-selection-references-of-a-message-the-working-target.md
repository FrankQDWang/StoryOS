---
status: accepted
---

# Make the Exact Selection References of a Message the Working Target

On 2026-10-14 and 2026-10-15, the author decided how the author points the assistant at text. The decisions are on [S4-08: Add a Selection to the Assistant Conversation](https://github.com/FrankQDWang/StoryOS/issues/1165). This ADR replaces the caret-based candidate target of the web client.

## Context

Before this change, a request worked on the Current Chapter. When the author clicked in a pending Proposal candidate, the panel kept that candidate as the Working Target (`proposal_candidate`). The author did not ask for this caret-based target, and it caused defects, for example a stale target after a resolve. The author wants the interaction of Zed: select text, add it to the conversation, and write the instruction.

## Decision

- **Selection Reference.** The author adds an exact text range to one assistant message. A reference is one of two kinds:
  - `chapter_text`: a Chapter, its base Authoritative Revision, a start block and offset, and an end block and offset.
  - `proposal_candidate_text`: a pending Proposal candidate (Chapter, Proposal, Operation, and Proposal Revision), and a start offset and an end offset in its candidate text.
- **Offsets.** All offsets use the `storyos.editor.utf16-code-unit.v1` profile of the editor. A reference keeps the exact range, not only paragraphs.
- **Message.** One message holds 1 to 8 references. A reference has no mode. The model reads the author instruction and decides if it changes the referenced text or only uses it. A message with no reference works on the Current Chapter as before.
- **Working Target.** A message with references sends the Working Target kind `selection_references`. Its changeable targets are:
  - Each block of the Current Chapter that no pending Proposal reserves.
  - Each block that a `chapter_text` reference covers and that no pending Proposal reserves, with the reference base.
  - The block of each referenced pending candidate, with its candidate text.

  The subset rule of ADR 0061 applies. Text outside these targets is not changeable. A referenced block that a pending Proposal reserves is readable, but not changeable.
- **Exact text.** The client sends no text. The Host reads the selected text from the base Revision or from the candidate revision. A reference is unavailable when:
  - Its base is not the current head.
  - Its candidate is not pending at the revision of the reference.
  - Its range is reversed or empty, passes the end of the text, or splits a surrogate pair.

  An unavailable reference blocks the Context Assembly with `working_target_revision_unavailable`. StoryOS then sends nothing.
- **Model input.** The request sends each reference with its exact range and its exact text. The Host instructions tell the model the rule. A change replaces all of a block. The text outside the references stays unchanged unless the author request tells otherwise. StoryOS adds no mechanical check of the text outside a reference. The author examines the Proposal and decides.
- **One Decision.** The reply contract does not change. One reply gives one Agent Decision. The Decision is prose changes of Chapter blocks, or a revision of exactly one referenced candidate, or an answer. A reply that changes a candidate and also another target is not valid, and the current validation refuses it. StoryOS adds no interim behavior and no sentence for a request that asks to change both kinds.
- **Candidate revision.** A change of a referenced candidate goes through the current candidate revision path, and its location outcome is `revised`. When steering continues the same Run, the reference moves to the new revision and covers all of the new candidate text.
- **Steering.** A steering input can also add 1 to 8 references, in the optional field `selection_references` of `steer-agent-run.request.v1`. The field is additive, so a request with no references keeps its schema and its digest.
  - The new references join the references of the Run in their order. A reference that the Run already has does not join again.
  - One Run holds at most 8 references. A steering input that makes the Run hold more is refused before Admission with the problem `steering_reference_limit`, and it writes no row.
  - The next decision of the Run uses all of its references, and its Working Target is then `selection_references`. A Run with a stored candidate target keeps that target.
  - After each send, the web composer clears all labels, and the conversation history keeps the referenced text. While a Run is active, the composer shows the add control only when the Run has fewer than 8 references.
- **Context Inspect.** Context Inspect shows each reference, the blocks that it covers, and its current availability.
- **Web client.** The web client removes the caret-based target and the public `proposal_candidate` kind. The web client is the only client, so the request schema stays `create-agent-run.request.v2`. A stored record with a candidate target still decodes and shows in Context Inspect.
- **Composer behavior.** The web composer shows each reference as a label in paragraphs, for example 「第 3 章 渡口（第 2–4 段）」. The label of a candidate reference is 「第 3 章 渡口（第 2 段的修改建议）」. A partly selected paragraph counts in the label. Before the send, the client moves each range through the author edits and uses the newest base or candidate revision. When all referenced text is deleted, or the candidate is rejected or withdrawn, the label disappears with no sentence. When the author accepts a referenced candidate, the reference becomes a Chapter text reference.
- **Caret awareness.** The model does not get the caret position. This needs a second contract field, a drift binding, and its own instructions, so it is not easy.

## Considered options

- A reference mode, "modify" or "reference". Rejected by the author: the instruction tells the intent.
- Paragraph references with no character offsets. Rejected by the author: the model must get the exact selected text and range.
- One Decision that changes Chapter blocks and a candidate together. Rejected for Stage 4: it needs an Agent tool loop, which is outside the scope of Stage 4.
- The client sends the selected text. Rejected: the Host reads exact text only from a versioned source.

## Consequences

- The Host instructions changed, so the wire digest of an Agent Plan request changed. A Model Registration does not bind the instruction text, so no Registration drift occurs.
- The Operation Requirement payload records the admitted references with their exact text. The Context Inspect response has the new optional field `selection_references`.
- Six pull requests deliver the change. The first adds the Chapter text references. The second adds the candidate references and removes the public `proposal_candidate` kind. The third adds the references of a steering input. The fourth, the fifth, and the sixth change the web client.
- The activity payload of a retained steering input can have the key `selection_references` (migration 0093).
