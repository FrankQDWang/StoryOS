---
status: accepted
---

# Accept a Proposal on an Earlier Base by Replacing Its Target Text

The specification [Accept a Proposal on an Earlier Base by Replacing Its Target Text](https://github.com/FrankQDWang/StoryOS/issues/1213) owns this ADR. The author decision of 2026-10-17 on [Keep a Path to Replan for a Conflicted Proposal Revision Without a Conflict Condition](https://github.com/FrankQDWang/StoryOS/issues/1211) gives the product rule. This ADR records the Acceptance rule, the target resolution order, the records, the readers, and the removal of Replan from the editor.

## Context

Before this change, Acceptance needed the base of the Proposal Revision to be the Chapter Head. When the author edited the Chapter after the Proposal opened, Acceptance gave the conflict `changed_head` and recorded a Proposal Conflict condition. Only an explicit Replan made the Proposal acceptable again. Some paths gave a Revision with the validation `conflicted` and no condition. The author could not accept or replan that Revision, so the candidate work was lost. Also, the author does not want a separate Replan step. For the author, a Proposal has three actions: accept, reject, and edit the candidate text.

## Decision

- **Rule.** Acceptance of a Proposal on an earlier base applies. The candidate text replaces the current text of its target. Thus author text that changed in that target after the Proposal base is replaced. Author Undo of the Acceptance restores that author text.
- **Eligibility.** Acceptance does not need the base to be the Chapter Head. It still needs the Project Scope, the Admission, the current Proposal Revision, retention, generation `ready`, closure `open`, pending selected Operations, and the dependency and Bundle rules. The validation `invalid` and an invalid Acceptance condition still block Acceptance. The validation `conflicted` and a Proposal Conflict condition do not block it.
- **Recovery conflict.** A Proposal Recovery Conflict condition still blocks Acceptance, because StoryOS does not apply a Proposal whose state is not known. The author can reject or withdraw the Proposal.
- **Validation Receipt.** The named Validation Receipt must identify the Proposal Revision and have its base. Its result must be `valid` or `conflicted`. The altered-candidate check does not change.
- **Target resolution.** Acceptance resolves the target of each selected Operation on the current Chapter Head, in this order:
  1. `exact_base`: the Revision base is the Chapter Head. Acceptance applies as before.
  2. `block_replacement`: the Proposal is a Block Proposal, and its target Block is on the Chapter Head. The candidate text replaces all text of that Block.
  3. `proven_range`: the Proposal is an Inline Proposal, and the exact Anchor proof of ADR 0063 maps the Anchor range to the Chapter Head. The candidate replaces only that range. Reservations of other Proposals do not stop the proof at Acceptance.
  4. `base_block_replacement`: the Proposal is an Inline Proposal without that proof, and its target Block is on the Chapter Head. The Block text becomes the Anchor base Block text with the candidate at the stored range.
  5. Missing target: a target Block is not on the Chapter Head. Acceptance is refused with the reason `missing_target`, and nothing changes. The Proposal stays open, and the author can reject it.
- **Other Proposals.** Other open Proposals on the same Block stay open. Their bases are then behind the Chapter Head, and the same rule applies to their Acceptance.
- **Concurrency.** The Chapter Head compare-and-set of the Authoritative Revision profile does not change. When the expected Authoritative Revision of the request is not the Chapter Head, Acceptance gives the existing binding conflict.
- **Records.** Migration 0095 adds two columns to `storyos.acceptance_receipts`. `applied_authoritative_revision_id` is the Chapter Head that an applied Acceptance applied to. `target_resolution` has one of the four resolution values. A CHECK constraint requires both facts on each applied row and on no other row. The migration gives each earlier applied row the prior head of its Domain Receipt and `exact_base`, because the earlier rule applied only on the exact base.
- **Response.** The applied effect of `acceptProposal` gives `target_resolution`. An exact retry reads the value from the Acceptance Receipt row, so it gives the same effect. The refusal reasons add `missing_target`.
- **Readers.** The validation history reader accepts an applied Acceptance as valid history when all of these are true:
  - The prior head of its Domain Receipt is its recorded Chapter Head.
  - Its resolution is `exact_base` exactly when that Chapter Head is the Revision base.
  - Its Validation Receipt has the Revision base.
  getProposal, the Acceptance facts, the Project archive export, and the pinned export sources use this reader. The Project archive exports the two facts with each Acceptance Receipt row. Earlier Proposal Revisions, Validation Receipts, and Receipts do not change.
- **Damaged history.** When the validation history of the current Proposal Revision is not valid, Acceptance gives `503 project_store_unavailable`, as getProposal does.
- **Candidate edits.** A candidate edit validates the new Revision against its own base. A base behind the Chapter Head does not make the Revision `conflicted`. No new writer writes `conflicted` for a changed Chapter Head. Existing `conflicted` Revisions and Proposal Conflict conditions stay readable and acceptable.
- **Editor.** The editor does not show Replan. A Proposal on an earlier base shows Accept, Reject, and candidate editing. It also shows a short note: Acceptance replaces the current text of its target. When the open Proposal list is truncated or its read failed, the decision controls stay unavailable (ADR 0063).
- **Server.** The `replanProposal` command, its route, its contracts, and the Author Undo of a Replan stay. No editor path calls the command. The Author Undo Disposition of Acceptance does not change (ADR 0044).

## Delivery boundary

The tickets of the specification deliver this decision in parts:

- [Accept a Block Proposal on an Earlier Base by Replacing Its Block Text](https://github.com/FrankQDWang/StoryOS/issues/1218) delivers the rule for a Block Proposal: steps 1, 2, and 5, the records, the readers, and the response.
- Until the Inline ticket of the specification is delivered, an Inline Proposal keeps the earlier rule. Its Acceptance on an earlier base gives the conflict `changed_head` and records a Proposal Conflict condition. The validation `conflicted` and a Proposal Conflict condition block its Acceptance. The `Conflicted` variant of the Acceptance contract and the Inline tests of that rule stay until that ticket removes them.
- The record type and migration 0095 already permit `proven_range` and `base_block_replacement`, so the Inline ticket adds no migration.
- The candidate edit writers and the editor change come in their own tickets of the specification.

## Replaced rules

This decision replaces these earlier rules:

- ADR 0003 said that Acceptance stays unavailable until a new current Proposal Revision validates, and that target drift needs an explicit Replan. Acceptance now replaces the target text on the current Chapter Head. It does not map or revalidate the old Revision.
- ADR 0063 said that a changed head stays a conflict. This stays true for a Replan. A changed head does not make Acceptance a conflict.
- ADR 0044 said that the `conflicted` outcome of Acceptance writes its validation condition row. Only an Inline Proposal gives that outcome until the Inline ticket is delivered. After that ticket, Acceptance has no `conflicted` outcome.

## Considered options

- Keep Replan as a separate author step. Rejected by the author on 2026-10-17: a Proposal has only three author actions.
- Merge the changed author text and the candidate. Rejected: no exact merge rule exists for prose, and the author then gets text that the author did not see. A full replacement gives the candidate that the author saw, and Author Undo restores the replaced text.
- Map the old Revision to the Chapter Head and revalidate it. Rejected: this applies an inspected Proposal to a different base without a record of what it replaced.
- Ask for a confirmation at Acceptance. Rejected by the author: Acceptance stays one action, and the short note tells what it replaces.
- Write the candidate Block again when the author deleted it. Rejected: StoryOS never writes a paragraph that the author removed.
- Apply a Proposal with a Proposal Recovery Conflict condition. Rejected: the state of such a Proposal is not known.

## Consequences

- Author Undo of a replacing Acceptance restores the text of the Chapter Head that the Acceptance applied to.
- A partial Acceptance of a multi-Operation Proposal does not block the remaining Operations. Each selected Operation replaces its own target.
- The recovery drill and the packaged journeys keep fewer Proposal Conflict conditions, because an Acceptance on an earlier base applies.
