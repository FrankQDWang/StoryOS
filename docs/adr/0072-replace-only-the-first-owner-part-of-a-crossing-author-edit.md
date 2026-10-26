---
status: accepted
---

# Replace Only the First Owner Part of a Crossing Author Edit

On 2026-10-22, the author decided the rule of [Replace only the first owner part when an author edit crosses a Proposal](https://github.com/FrankQDWang/StoryOS/issues/1260). The coordinator confirmed the interpretation below on 2026-10-23.

## Context

The author can select across the Chapter text and a pending Proposal candidate to give both parts to the Agent ([ADR 0062](0062-make-the-exact-selection-references-of-a-message-the-working-target.md), [ADR 0071](0071-let-the-agent-grow-a-block-proposal-over-adjacent-chapter-blocks.md)). Before this decision, an author edit on such a selection was one `mixed` Author Edit. The Server refused it and created a Refused Edit Draft. The author can expand that Draft into a Proposal with `candidate_blocks`. The Server then refused each edit of that candidate with `unsupported_intent_shape`.

## Decision

- **First owner.** The owner of a selected character is the Chapter text or one Proposal candidate. The first owner of a selection is the owner of its first selected character in document order. The direction of the selection (anchor and head) does not change the first owner.
- **First owner part.** The first owner part is the contiguous part of the selection from its start to the first text of another owner. For a selection over Chapter block A, a candidate P, and Chapter block B, the first owner part is the selected part of A only.
- **Edit.** Typing, paste, cut, delete, Enter, and a confirmed IME composition on a selection that crosses owners replace only the first owner part. The other part does not change, and the editor removes it from the selection. After the edit, the selection is a caret at the end of the replacement, in the first owner.
- **Drop.** A drop inserts the dropped text at the drop point and replaces no selected text. Thus it changes only the owner of the drop point. When the drop point is not in text, the drop replaces the selection, and the rule above applies.
- **Target block.** The editor refuses a Chapter edit that removes the target block of a pending block Proposal, with or without a crossing. A first owner part that joins Chapter blocks can remove such a block. Then the input makes no edit.
- **Two candidates.** The same rule applies to a selection that crosses two candidates.
- **Cut.** A cut copies only the first owner part, because only that part is removed.
- **IME.** A composition on a crossing selection shows in the browser until it ends. At the end, the editor restores the document from the start of the composition and replaces the first owner part with the confirmed text. A cancelled composition makes no edit.
- **Author Edit.** The edit is an ordinary single-owner Author Edit: a Chapter edit, or a candidate edit with its present `mixed` ownership partition. Its Author Undo is the ordinary Undo of that edit. The Web editor sends no Author Edit that changes both the Chapter text and a candidate, so it creates no Refused Edit Draft.
- **Guard.** The editor refuses each replacement of a crossing selection that an input did not narrow first. Such an input makes no edit.
- **Selection references.** A crossing selection still gives one reference for each owner part to the Agent. This rule does not change selection references.

## Considered options

- Option A of the issue: keep the Refused Edit Draft and show a reason for the `unsupported_intent_shape` refusal. Replaced by the author on 2026-10-22.
- The owner at the anchor of the selection. Rejected: the result then changes with the drag direction of the same text.
- Replace each part of the first owner, for example A and B around P. Rejected: one ordinary edit cannot replace two parts that a candidate separates.

## Consequences

- The Server path that creates a Refused Edit Draft stays, and the Web editor still shows existing Drafts from its Local Edit Journal. A Draft retry can still create a Draft. A later cleanup issue removes the Refused Edit Draft and expansion code.
- No Web input creates a new Refused Edit Draft. Thus the packaged exact-dist journeys and the recovery drill do not examine the Draft path through the Web editor any more. The exact-dist journey of response loss, reload, and Server restart now examines a first owner edit. Until the cleanup issue removes the Draft code, only the browser-source tests and the HTTP tests examine it. An example is `edit-inline-proposal-http.integration.test.ts` and its restored Drafts in `scripts/verify-recovery-hold.sh`.
- A present Server rule limits a Chapter-first edit next to an inline Proposal. While an inline Proposal is pending, the Server accepts a Chapter edit in that Chapter only as an insertion at an edge of the candidate. Thus such a Chapter-first edit conflicts with `proposal_head_present`, and the Chapter text and the candidate do not change. This rule is not new, and a Chapter edit that crosses no candidate gets the same result. A block Proposal has no such limit. [Let a Chapter edit outside a pending inline Proposal succeed, or remove inline Proposals](https://github.com/FrankQDWang/StoryOS/issues/1273) decides the rule.
- No contract, route, or persisted format changes.
