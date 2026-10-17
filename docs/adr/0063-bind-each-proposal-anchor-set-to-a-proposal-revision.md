---
status: accepted
---

# Bind Each Proposal Anchor Set to a Proposal Revision

The specification [Keep an Inline Proposal Editable After Replan and List the Open Proposals of a Chapter](https://github.com/FrankQDWang/StoryOS/issues/1198) owns this ADR. The architecture finding [Bind Inline Anchors to Proposal Revisions](https://github.com/FrankQDWang/StoryOS/issues/882) gives the cause. This ADR records the Anchor set binding, the reader rule, the Replan proof rule, and the source of the open Proposal list.

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
  - An applied Replan of an Inline Proposal writes the proven set of the new Revision on the expected Authoritative Revision.
- **Inline creation checks.** Inline creation reads no Anchor row. Its reservation check uses `storyos.proposal_operations`, so the reader rule does not apply to it.
- **Replan proof.** An explicit Replan of an Inline Proposal proves each Anchor of the Anchor set of the conflicted Revision on the expected Authoritative Revision. Each condition is necessary:
  - The Block has the same identity and the paragraph kind on the base and on the current Revision.
  - The Anchor has the current schema version, coordinate profile, and boundary profile.
  - The base slice matches the stored base-slice digest.
  - An exact comparison of the base and current Block text puts the complete range inside one unchanged run.
  - The slice text at the new range is equal to the base slice.
  - No other unresolved reservation is on the Block.
- **Comparison for the proof.** The comparison takes the common prefix and the common suffix as unchanged runs, and Myers compares only the middle. Thus a long insertion at an edge stays exact. An insertion inside the range slides out to the nearest edge when the slide moves it only over equal characters. Thus a space typed next to a space at an edge keeps the proof. When the comparison reaches its trace limit, it has no exact runs, and the proof fails.
- **Structural premise.** The proof does not examine the neighbor Blocks. While the reservation exists, Author Edit accepts in the reserved Block only a collapsed insertion at an Anchor edge. Thus a split, a join, a move, or a deletion of the Block cannot occur before the Replan.
- **Replan result.** When each Anchor has a proof, one transaction writes the new Revision, its Anchor set on the expected Authoritative Revision, and the Receipt. The Operation keeps its identity. When one proof fails, the Replan is refused with `unavailable_proof`. It writes no Revision, and the conflict stays. The proof comes after the head check, so a changed head stays a conflict.
- **Archive.** The Project archive export writes each row of `canonical/proposal_anchors.json` with its `proposal_revision_id`. Thus the archive keeps the Anchor set of each Proposal Revision.

## Open Proposal list

- **Source.** The Server list is the source of the open Proposal list. The `listChapterOpenProposals` query gives the open Proposals of one Chapter in one Project Scope.
- **Open.** An open Proposal has the closure `open` and at least one pending Operation.
- **Order.** The list order is the Proposal identity order. Each producer makes the Proposal identity with UUIDv7, so this order is the open order.
- **Length.** The list has a maximum of 64 entries. The author approved this number on 2026-10-17. A longer set gives the completeness value `truncated`.
- **Client.** The Protected Web Client reads this list when the workspace loads and at each Proposal change. Then it reads each Proposal with `getProposal`. A locator that the current AgentRun gives only causes an earlier read of the list. The client keeps no Proposal locator in browser storage.
- **Unknown heads.** When the list read fails or the list is truncated, the client does not know all Proposal heads. Then it keeps the Proposal decision controls unavailable.

## Considered options

- Keep the Anchor on the Operation and change its base at Replan. Rejected: an Anchor row is historical target evidence, and a change of it removes the evidence of the earlier Revision.
- Ignore the base difference in the web client. Rejected: this weakens the exact target authority.
- Prove the Replan Anchor also against the neighbor Blocks. Rejected: an author edit in a neighbor Block then refuses a Replan, and the author loses the edit path of the Inline candidate. The ownership rule already prevents the structural changes.
- Map the old offsets without an exact comparison. Rejected: an equal slice at a different place can hide a changed range.
- Keep the Proposal locators in the storage of each browser tab, or copy them to a shared browser store. Rejected by the author on 2026-10-17: a new tab or a different browser does not show the pending Proposals.
- Give the reader rule to each reader as its own query. Rejected: five SQL readers include Anchor rows in larger joins. One function gives the rule one owner.

## Consequences

- The Undo paths of the specification write their own Anchor sets. Until then, a read of an Inline Proposal fails closed after Author Undo of an Acceptance in Reopen mode. Before this change, the same read gave a stale Anchor. The aggregate pull request of the specification delivers these paths together.
- A Replan with no proof keeps the conflict. The author can then reject or withdraw the Proposal, or ask for a new Proposal.
- Author Undo of a Replan selects the set of the restored Revision through the parent chain. A later Anchor set on the replanned Revision needs a copy at that Undo.
