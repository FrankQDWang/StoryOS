---
status: accepted
---

# Bind Each Proposal Anchor Set to a Proposal Revision

The specification [Keep an Inline Proposal Editable After Replan and List the Open Proposals of a Chapter](https://github.com/FrankQDWang/StoryOS/issues/1198) owns this ADR. The architecture finding [Bind Inline Anchors to Proposal Revisions](https://github.com/FrankQDWang/StoryOS/issues/882) gives the cause. This ADR records the Anchor set binding and the reader rule. The later tickets of the specification add the Replan proof rule and the source of the open Proposal list.

## Context

Before this change, a Proposal Anchor row belonged to a Proposal and its Operation. It did not belong to a Proposal Revision. A Proposal Revision on a new base Authoritative Revision, for example the result of a Replan, kept the old Anchor. Each reader then gave an Anchor whose base was not the base of the current Proposal Revision. The web client cannot show such an Anchor, so the author was not able to edit the Inline candidate.

## Decision

- **Binding.** Each row of `storyos.proposal_anchors` has the column `proposal_revision_id`. It identifies the Proposal Revision that wrote the row. The column is part of the primary key, and a foreign key binds it to `storyos.proposal_revisions`. The runtime role keeps only SELECT and INSERT on the table, so an Anchor row stays immutable.
- **Existing rows.** Migration 0094 binds each existing row to the first Proposal Revision of its Proposal, which is the Revision with no parent. Before this change, only that Revision wrote Anchor rows. Release 1 storage bootstrap runs all migrations on an empty database. Thus the binding is for the migration contract, and it is not an in-place upgrade path.
- **Anchor set.** The Anchor set of a Proposal Revision comes from the nearest Revision in its parent chain that wrote Anchor rows. This includes the Revision itself. A Revision on the same base as its parent, for example a candidate edit, writes no new set. It uses the set of its parent chain.
- **Reader rule.** The SQL function `storyos.proposal_anchor_set_revision` selects the Anchor set Revision. Each Anchor reader uses it:
  - getProposal.
  - The Acceptance facts.
  - The Author Edit inline ownership and the inline ranges.
  - The Refused Edit Draft source capture.
- **Fail closed.** The selected Anchor set can have a different base Authoritative Revision than the Proposal Revision. Then the function stops the read with SQLSTATE `XX001` (`data_corrupted`). The reader then reports the read as unavailable. It never gives a stale Anchor.
- **Writers.** Each writer records the identity of the Proposal Revision that it writes:
  - Inline creation writes the set of the opening Revision.
  - Refused Edit Draft expansion writes the set of the opening Revision.
  - The Derived mode of Author Undo of an Acceptance copies the set of the accepted Revision to the opening Revision of the derived Proposal. The reader rule selects that set.
- **Inline creation checks.** Inline creation reads no Anchor row. Its reservation check uses `storyos.proposal_operations`, so the reader rule does not apply to it.
- **Archive.** The Project archive export writes each row of `canonical/proposal_anchors.json` with its `proposal_revision_id`. Thus the archive keeps the Anchor set of each Proposal Revision.

## Considered options

- Keep the Anchor on the Operation and change its base at Replan. Rejected: an Anchor row is historical target evidence, and a change of it removes the evidence of the earlier Revision.
- Ignore the base difference in the web client. Rejected: this weakens the exact target authority.
- Give the reader rule to each reader as its own query. Rejected: five SQL readers include Anchor rows in larger joins. One function gives the rule one owner.

## Consequences

- The Replan proof and the Undo paths of the specification write their own Anchor sets. Until then, a read of an Inline Proposal fails closed after an Inline Replan. It also fails closed after Author Undo of an Acceptance in Reopen mode. Before this change, the same read gave a stale Anchor. The aggregate pull request of the specification delivers these paths together.
- Author Undo of a Replan selects the set of the restored Revision through the parent chain. A later Anchor set on the replanned Revision needs a copy at that Undo.
