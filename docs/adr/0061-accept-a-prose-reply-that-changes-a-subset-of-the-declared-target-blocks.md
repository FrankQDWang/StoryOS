---
status: accepted
---

# Accept a Prose Reply That Changes a Subset of the Declared Target Blocks

On 2026-10-14, the author selected the coverage rule of a prose change reply. The decision was made on [Decide how to treat a prose reply that changes only some declared target blocks](https://github.com/FrankQDWang/StoryOS/issues/1161). This ADR amends the exact-coverage rule of the Stage 3 multi-location Proposal work (commit `bfd5dc68`, [PR #878](https://github.com/FrankQDWang/StoryOS/pull/878)).

## Context

The Stage 3 rule required one change for each declared target block, and no other change. In the S4-05 live journey ([#396](https://github.com/FrankQDWang/StoryOS/issues/396)), the Working Target declared the two paragraphs of a Chapter, and the author asked to expand only the second paragraph. The model returned one valid change for the second paragraph. The validation refused the reply, no Proposal opened, and the author saw a sentence that did not tell the cause. The author paid for the request. An instruction that changes only a part of the target is a usual author request.

## Decision

- A valid prose change reply changes a subset of the declared target blocks. It has at least one change.
- Each change is for a declared target block, with the declared base revision of that block. Two changes for the same block are not valid.
- A change for an undeclared block, a duplicate change, an empty change list, or a change that is not valid for another reason refuses the whole reply. Then no Proposal opens.
- A declared target block that the reply does not change keeps its text. It has no Proposal Operation and no location in the Agent Decision.
- A Chapter of the Working Target that has no changed block gets no Proposal.
- The first Create, the Retrieve of a stored response, and the successor Create use the same rule.
- The Host instructions of the Agent Plan request tell the model to give one change for each target block that it changes, and to leave out each target block that it does not change.

## Considered options

- Keep the exact-coverage rule, and show a clear sentence for a reply that changes a part of the target. Rejected: the author loses a valid result of a usual request.
- Copy the current text of each unchanged block into the reply as a change. Rejected: the Proposal then shows Operations that change nothing.

## Consequences

- The location count of an Agent Decision is the count of changed blocks, not of declared targets.
- The Host instructions changed, so the wire digest of an Agent Plan request changed. A Model Registration does not bind the instruction text, so no Registration drift occurs. An unknown Create that StoryOS sent before this change, with a successor after this change, pauses with `request_changed` and sends nothing.
- After this change, S4-05 sends one live request that changes a part of the target, and records that the Proposal opens for that block only.
