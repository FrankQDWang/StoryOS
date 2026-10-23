---
status: accepted
---

# Let the Agent Grow a Block Proposal over Adjacent Chapter Blocks

On 2026-10-22, the author selected option B on [Decide how the Agent treats a crossing selection of a Proposal and Chapter text](https://github.com/FrankQDWang/StoryOS/issues/1264). The contract is on [Let the Agent grow a block Proposal over the adjacent Chapter blocks of a crossing selection](https://github.com/FrankQDWang/StoryOS/issues/1265). This ADR amends [ADR 0061](0061-accept-a-prose-reply-that-changes-a-subset-of-the-declared-target-blocks.md) and the "One Decision" rule of [ADR 0062](0062-make-the-exact-selection-references-of-a-message-the-working-target.md).

## Context

The author can select text that starts in a pending Proposal candidate and continues into the next Chapter blocks, or the reverse. Then the author asks the Agent to change all of it, for example "make these lines one tight scene". ADR 0062 lets one reply revise the referenced candidate or change Chapter blocks, but not both. A reply that changes the two kinds is not valid, so the author gets no result and must make two requests and examine two Proposals.

## Decision

- **Proposal Growth.** One reply can revise the referenced candidate and also change Joinable Blocks. Then StoryOS adds those blocks to the same Proposal. This is one Agent Decision on one Proposal, so it needs no Agent tool loop.
- **Scope.** Only a `block_edit` Proposal grows. The real model makes only `block_edit` Proposals. An `inline_edit` Proposal does not grow, and its Proposal Anchors do not change.
- **Joinable Block.** A Chapter block is joinable when all these conditions are true:
  - The Run references one or more pending candidates, and all of them are candidates of one Proposal.
  - A `chapter_text` reference of the Run covers the block, fully or in part.
  - No pending Proposal reserves the block.
  - The pending Operations of the Proposal of the candidate make one adjacent run of blocks. The joinable blocks continue that run up or down with no gap.
- **Working Target.** The request marks each joinable target block. The Host instructions tell the model that it can change the candidate together with joinable blocks. The model changes the joinable blocks next to the candidate first, with no gap.
- **Reply validation.** The reply contract keeps its shape. A reply is a growth when it changes the referenced candidate and one or more joinable blocks, and no other block. The changed joinable blocks above the Proposal, and the changed joinable blocks below it, each start next to the Proposal and have no gap. These replies are not valid, and no Proposal changes:
  - A change of the candidate and of a block that is not joinable.
  - A change of the candidate and of a joinable block that leaves a gap.
  - A change of two candidates.

  The other rules of ADR 0061 and ADR 0062 stay. A reply with no candidate change opens a new Proposal, also for joinable blocks. A reply that changes only the candidate revises it as before.
- **Persistence.** A growth appends one Proposal Revision to the same Proposal. The Revision revises the candidate of the referenced Operation. StoryOS adds one pending Operation for each joined block, with its reservation, and the bundle policy of the Proposal becomes `atomic`. A growth writes no Proposal Anchor and no Author Action.
- **Recheck.** Under the lock of the Proposal head, StoryOS checks each joined block again. It must be in the current head at the base of the candidate, with no reservation. When a check fails, StoryOS refuses all of the growth with `stale_candidate`, and it writes nothing.
- **Result.** Each change of a growth has the location outcome `revised`. A joined block names its new Operation. All changes name the same new Revision. The location contract does not change.
- **Exact retry.** An exact retry of the request returns the same Run, and the Run writes no second Revision.
- **Second growth.** A grown Proposal can grow again over the next adjacent Chapter block. The joinable block rule uses the run of all pending Operations of the Proposal. One growth revises only one candidate: a reply that changes two or more candidates of the Proposal is not valid, also together with joinable blocks.
- **Steering after growth.** When steering continues the same Run, each candidate reference of the Proposal moves to the new Revision, as in ADR 0062. Each Chapter reference that covers a joined block gives one candidate reference of the new Operation of that block, with all of its candidate text. The new reference takes the place of the Chapter reference. A Chapter reference that also covers a block outside the growth stays, and the new reference comes after it.

## Considered options

- One Decision that changes Chapter blocks and a candidate as two Proposals. Rejected by the author: the author examines one continuous passage as two changes.
- Growth over each block that the selection covers, also when it is not adjacent. Rejected by the author: a Proposal then has parts that are not adjacent.
- A merge of two pending Proposals. Rejected: it is outside the scope of the decision.

## Consequences

- The Host instructions changed, so the wire digest of an Agent Plan request changed. A Model Registration does not bind the instruction text, so no Registration drift occurs.
- After a growth, the Run can hold more than 8 references, because one Chapter reference can give one candidate reference for each joined block. This is a known limit: while the Run holds 8 or more references, a steering input that adds references is refused with `steering_reference_limit`. The contract does not change.
- Migration 0098 grants the runtime role `UPDATE` of the column `bundle_policy` of `storyos.proposals` only.
- A growth is an Agent result, not an Author Action, so Author Undo does not reverse it. The Proposal Head Lineage of [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md) does not explain the growth Revision. Thus the Undo of an earlier author candidate edit or decision on that Proposal gets a Barrier, as after an Agent candidate revision.
- [Accept, withdraw, and show a grown Proposal](https://github.com/FrankQDWang/StoryOS/issues/1267) delivers the decisions on a grown Proposal and their display:
  - An Acceptance or a Rejection of a grown Proposal selects all of its pending Operations, as the atomic Bundle rule requires. The Proposal read shows the bundle policy in the field `bundle_policy`.
  - The editor shows each Operation of a grown Proposal at its block. On each part of an `atomic` Proposal, the `接受` and `拒绝` controls select all parts. The labels do not change, and the editor adds no control for a set of parts. The author selected this on 2026-10-23.
  - A withdrawal closes all of the grown Proposal in one step. The reservations of its Operations follow the present rule for a withdrawn Proposal.
  - Context Inspect shows the Joinable Blocks that the dispatched request of the decision marked, in the optional field `joinable_block_ids`. The Model Attempt records them when StoryOS dispatches the request.
